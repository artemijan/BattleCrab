//! Turning accumulated `StatModifiers` into a final stat value — Java's
//! `Stat.getValue` and the finalizers that deviate from it.

use crate::data::GameData;

use crate::game_loop;

use super::Player;
use super::components::{self, BaseStats, StatModifiers};
use super::inventory::Inventory;
use super::skill::effects::StatModifierEffect;
use super::stats::{Stat, StatModifierType};

/// `Stat.defaultValue`: `base * mul + add` from the accumulated modifier
/// maps (1.0/0.0 when nothing has touched this stat). `pub(crate)`: also used
/// by `game_loop::combat::shield_stats`, which finalizes `ShieldDefence`/
/// `ShieldDefenceRate` over the equipped shield's own `sDef`/`rShld` outside
/// the `recalculate_stats` pass (shield block stats aren't cached on
/// `CombatStats`, so they're finalized fresh at combat-lookup time instead).
///
/// **This is the one place the order lives.** Java's `getValue(stat, base)` is
/// `(mul × base) + add`, and the alternative reading — folding `add` inside the
/// multiply — agrees on every stat carrying only one kind of modifier, so a
/// respelling of this formula elsewhere can be wrong for years without a test
/// noticing. `water::breath_ms` was, until 2026-08-18. Call this; do not
/// rewrite it.
///
/// **One term is deliberately missing.** Java adds
/// `getMoveTypeValue(stat, creature.getMoveType())`, which this cannot: the
/// move type is not a property of `StatModifiers`. That is safe only because
/// of what the datapack contains — the sole stats any `StatByMoveType` effect
/// on this dist targets are `REGENERATE_*` (64 entries) and `EVASION` (1), and
/// both are finalized at their own call sites, which do add the term
/// (`game_loop::stats::regen`, `game_loop::combat`). A stat that acquires a
/// `by_move_type` entry **and** comes through here would silently lose it, so
/// check that before routing a new stat to this function.
pub(crate) fn finalize(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
}

/// `P/MDefenseFinalizer.defaultValue`: `mul` floors at 0.5, and the result is
/// floored at `base × 0.2` (the class template's naked defense × 0.2) so a
/// heavy defense debuff can't drop below a fifth of the naked value.
pub(super) fn finalize_def(mods: &StatModifiers, stat: Stat, base: f64, floor: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0).max(0.5);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    (base * mul + add).max(floor)
}

/// `P/MAttackSpeedFinalizer.defaultValue`: same shape, but `mul` floors at
/// 0.7 instead of applying whatever's in the map directly (so an absent or
/// tiny buff doesn't produce a slower-than-0.7x cast/attack speed).
pub(super) fn finalize_speed(mods: &StatModifiers, stat: Stat, base: f64) -> f64 {
    if let Some(&fixed) = mods.fixed.get(&stat) {
        return fixed;
    }
    let mul = mods.mul.get(&stat).copied().unwrap_or(1.0).max(0.7);
    let add = mods.add.get(&stat).copied().unwrap_or(0.0);
    base * mul + add
}

/// The armor-conditioned passive buffs currently in effect for a player: for
/// every known passive skill carrying stat effects, the subset whose
/// `<armorType>` condition passes against the worn gear, as a hidden permanent
/// `ActiveBuff` (Java's `Player.addSkill` passive effects, re-evaluated at pump
/// time). Skills whose effects are all gated out contribute nothing. Shared by
/// `from_char` (enter-world) and `game_loop::stats::passive_skills` (equip changes).
/// `BaseStats` = the class template's six values **plus every flat bonus that
/// stacks onto them**: worn hennas (Java `recalcHennaStats`) and complete armor
/// sets (`BaseStatFinalizer`'s `getBaseStatValue`).
///
/// This exists because the composition had drifted into three hand-rolled
/// copies — the login build, the henna redraw, and (once sets landed) the
/// paperdoll change — and a term added to one is invisible to the others. Any
/// new flat base-stat source belongs here and nowhere else.
///
/// `None` when the object has no `Player`, i.e. nothing to compose for.
pub(crate) fn compose_base_stats(world: &crate::world::World, oid: i32) -> Option<BaseStats> {
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| (p.class_id, p.base_class_id))?;
    let t = world
        .data
        .player_templates
        .get_or_base(class_id, base_class_id)
        .cloned()
        .unwrap_or_default();
    let slots = world
        .objects
        .get_component::<components::HennaSlots>(&oid)
        .map(|h| h.0)
        .unwrap_or_default();
    let hs = world.data.hennas.stat_sums(&slots);
    let sets = game_loop::items::armor_sets::set_stat_sums(world, oid);
    // Java sums the set bonus as a double into the finalizer's base value and
    // the consumer truncates; every `<stat val>` on this dist is a whole
    // number, so the cast is exact rather than lossy.
    Some(BaseStats {
        str_: t.base_str + hs.str_ + sets.str_ as i32,
        dex: t.base_dex + hs.dex + sets.dex as i32,
        con: t.base_con + hs.con + sets.con as i32,
        int_: t.base_int + hs.int_ + sets.int_ as i32,
        wit: t.base_wit + hs.wit + sets.wit as i32,
        men: t.base_men + hs.men + sets.men as i32,
    })
}

/// Java `CreatureStat.mergeAdd`/`mergeMul`/`mergeMoveTypeValue`/
/// `mergePositionTypeValue` — accumulate one effect's contribution into the
/// modifier maps (multiple buffs on the same stat stack).
///
/// A *qualified* effect goes to its own map instead of `add`/`mul`, exactly as
/// Java routes it: it must not be folded into `add`/`mul`, or it would apply in
/// every state rather than the one it names. Each kind keeps Java's own merge
/// and identity — move type adds into 0.0, position multiplies into 1.0 — so
/// `mode` is not consulted on either path.
pub(crate) fn apply_modifier(mods: &mut StatModifiers, effect: &StatModifierEffect) {
    use crate::model::stats::StatQualifier;
    match effect.qualifier {
        Some(StatQualifier::MoveType(move_type)) => {
            *mods
                .by_move_type
                .entry((effect.stat, move_type))
                .or_insert(0.0) += effect.amount;
            return;
        }
        Some(StatQualifier::Position(position)) => {
            // `mergePositionTypeValue(stat, position, (amount/100)+1, MathUtil::mul)`
            // — the percentage is turned into a multiplier by the *handler*,
            // not the merge, and stacking positions multiply.
            *mods
                .by_position
                .entry((effect.stat, position))
                .or_insert(1.0) *= (effect.amount / 100.0) + 1.0;
            return;
        }
        None => {}
    }
    match effect.mode {
        StatModifierType::Diff => {
            *mods.add.entry(effect.stat).or_insert(0.0) += effect.amount;
        }
        StatModifierType::Per => {
            let entry = mods.mul.entry(effect.stat).or_insert(1.0);
            *entry *= (effect.amount / 100.0) + 1.0;
        }
    }
}

/// Java `Stat.weaponBaseValue` → `IStatFunction.calcWeaponBaseValue`: for a
/// player, the **right-hand weapon's** own declaration of a stat *replaces* the
/// class-template base (a two-handed weapon lives in RHand too). `None`
/// bare-handed, or when the weapon declares nothing for that stat, which is
/// the caller's cue to keep the template value.
///
/// The stat recompute inlines this rule for the five weapon-replace stats it
/// finalizes; this is the same read for callers outside that pass —
/// `calcAtkSpdMultiplier` needs the attack-speed one to scale a physical
/// skill's cast time, and used to take the class base instead.
pub(crate) fn weapon_base_stat(inventory: &Inventory, data: &GameData, stat: Stat) -> Option<f64> {
    let weapon = inventory.paperdoll_item(crate::model::inventory::PaperdollSlot::RHand)?;
    let stats = data.item_data.item_stats(weapon.item_id)?;
    stats
        .bonuses
        .iter()
        .find(|&&(s, _)| s == stat)
        .map(|&(_, v)| v)
}
