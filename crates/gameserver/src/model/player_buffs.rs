//! `Player::apply_buff` / `remove_buff` — landing and clearing one buff's stat
//! modifiers, and the recalc each triggers.

use crate::data::GameData;

use super::Player;
use super::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers};
use super::inventory::Inventory;
use super::skill::BuffSlot;
use super::skill::active_buff::ActiveBuff;
use super::stat_finalize::apply_modifier;
use super::stats::Stat;

impl Player {
    /// Land a buff, applying Java `EffectList.addActive`'s stacking and the
    /// `MaxBuffAmount`/`MaxDanceAmount` slot caps, then recompute. Returns
    /// whether the buff actually landed (`false` = refused because a same-type
    /// buff of equal/higher level is already active). Java
    /// `BuffInfo.initializeEffects` → `AbstractEffect.pump`.
    pub fn apply_buff(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &mut StatModifiers,
        inventory: &Inventory,
        buffs: &mut Buffs,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
        buff: ActiveBuff,
    ) -> bool {
        // Passive stat-pump markers aren't real buffs — they never stack-conflict
        // or count against the caps; fold and push as before.
        if buff.passive {
            for effect in &buff.effects {
                apply_modifier(mods, effect);
            }
            buffs.0.push(buff);
            self.recalculate_stats(data, base, mods, inventory, speeds, combat);
            return true;
        }

        // Java `EffectList.addActive` stacking: effects with no abnormal type
        // conflict only with the same skill id; typed effects conflict with any
        // buff of the same abnormal type.
        let none_type = buff.abnormal_type.is_empty() || buff.abnormal_type == "NONE";
        let conflict = buffs.0.iter().position(|e| {
            if none_type {
                e.skill_id == buff.skill_id
            } else {
                e.abnormal_type == buff.abnormal_type
            }
        });
        if let Some(idx) = conflict {
            // The higher (or equal) abnormal level wins; a lower one is refused.
            if buff.abnormal_level >= buffs.0[idx].abnormal_level {
                buffs.0.remove(idx);
            } else {
                return false;
            }
        }

        // Slot count cap: drop the oldest same-pool buff until this one fits
        // (Java removes the oldest in-use buff of the exceeding category).
        // `EnlargeAbnormalSlot` (Divine Inspiration 1405) raises the *good
        // buff* cap only — Java's `setMaxBuffCount`, which `EffectList` reads
        // for the buff pool and never for dances (G34 S4).
        let bonus_slots = mods.add.get(&Stat::MaxBuffSlots).copied().unwrap_or(0.0) as i32;
        let cap = match buff.slot {
            BuffSlot::Buff => Some(data.combat_caps.max_buff_count + bonus_slots),
            BuffSlot::Dance => Some(data.combat_caps.max_dance_count),
            BuffSlot::Uncapped => None,
        };
        if let Some(cap) = cap.filter(|c| *c > 0) {
            while buffs.0.iter().filter(|b| b.slot == buff.slot).count() as i32 >= cap {
                let Some(oldest) = buffs.0.iter().position(|b| b.slot == buff.slot) else {
                    break;
                };
                buffs.0.remove(oldest);
            }
        }

        buffs.0.push(buff);
        // A removal/override means the maps must be rebuilt from the survivors
        // (can't just fold the new one in) — same as `remove_buff`.
        mods.add.clear();
        mods.mul.clear();
        mods.by_move_type.clear();
        mods.by_position.clear();
        for b in &buffs.0 {
            for effect in &b.effects {
                apply_modifier(mods, effect);
            }
        }
        self.recalculate_stats(data, base, mods, inventory, speeds, combat);
        true
    }

    /// Remove an expired/replaced buff and recompute from scratch (Java just
    /// removes the `BuffInfo` and calls `resetStats()`, which rebuilds the
    /// maps from the remaining active buffs — do the same here rather than
    /// trying to subtract in place, which would drift under rounding).
    pub fn remove_buff(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &mut StatModifiers,
        inventory: &Inventory,
        buffs: &mut Buffs,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
        skill_id: i32,
    ) {
        buffs.0.retain(|b| b.skill_id != skill_id);
        mods.add.clear();
        mods.mul.clear();
        mods.by_move_type.clear();
        mods.by_position.clear();
        for buff in &buffs.0 {
            for effect in &buff.effects {
                apply_modifier(mods, effect);
            }
        }
        self.recalculate_stats(data, base, mods, inventory, speeds, combat);
    }
}
