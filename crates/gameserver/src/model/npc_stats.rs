//! NPC-side stat composition — the config multipliers (`NpcStatMods`) and the
//! finalized combat stats an NPC template plus its buffs produce.

use crate::data::GameData;

use crate::game_loop;

use super::components::{Buffs, CombatStats, Speeds, StatModifiers, Vitals};
use super::stat_finalize::{apply_modifier, finalize, finalize_def, finalize_speed};
use super::stats::Stat;

/// The permanent stat modifiers an NPC's *passive* template skills contribute.
/// Java's `Creature` constructor copies every `template.getSkills()` onto the
/// mob (`for (Skill s : template.getSkills().values()) addSkill(s)`); the
/// passive ones (operateType `P`) pump stats through the same add/mul maps as
/// buffs — this is where a retail mob's real HP/atk/def come from (skills 4408
/// HP Increase, 4410 P.Atk, 4412 P.Def, …), on top of the raw `<vitals>`/
/// `<attack>` base. The NPC counterpart of the player's `conditioned_passive_buffs`.
///
/// Weapon-conditioned effects (4415 "One-handed Sword" mastery, …) evaluate
/// against the template's `<equipment>` right hand — the NPC counterpart of
/// the player's paperdoll check. Armor-conditioned ones stay skipped: an NPC
/// wears no armor pieces, and Java's `<using kind="Heavy">` evaluates false
/// there too.
fn npc_passive_mods(data: &GameData, t: &crate::data::npc_data::NpcTemplate) -> StatModifiers {
    use crate::model::skill::effects::SkillEffect;
    use crate::model::skill::target::OperateType;
    let mut mods = StatModifiers::default();
    for &(skill_id, level) in &t.skill_list {
        let Some(skill) = data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        for effect in &skill.effects {
            if let SkillEffect::StatModifier(m) = effect
                && m.armor_condition == 0
                && (m.weapon_condition == 0
                    || (t.rhand != 0
                        && m.weapon_condition & data.item_data.weapon_type(t.rhand).mask_bit()
                            != 0))
            {
                apply_modifier(&mut mods, m);
            }
        }
    }
    mods
}

/// The champion multipliers that reach the NPC stat pipeline, resolved from
/// `Custom/ChampionMonsters.ini` for one NPC. Neutral (all ×1) for an ordinary
/// mob and whenever `ChampionEnable` is off, so a caller that has no champion
/// state to offer can pass `Default` and change nothing.
///
/// This exists so the finalizers stay pure functions of (template, buffs,
/// mods) — the config itself lives on `World`, which the stat layer has no
/// access to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NpcStatMods {
    /// `ChampionAtk` — P.Atk and M.Atk.
    pub atk: f64,
    /// `ChampionSpdAtk` — P.Atk speed and M.Atk speed.
    pub spd_atk: f64,
    /// `RaidPAttackMultiplier` / `RaidMAttackMultiplier` — the raid-only pass
    /// in `P|MAttackFinalizer`, applied *after* the champion one.
    pub raid_p_atk: f64,
    pub raid_m_atk: f64,
    /// `RaidPDefenceMultiplier` / `RaidMDefenceMultiplier` — same, in
    /// `P|MDefenseFinalizer`. All four are 1.0 on this dist.
    pub raid_p_def: f64,
    pub raid_m_def: f64,
}

impl Default for NpcStatMods {
    fn default() -> Self {
        Self {
            atk: 1.0,
            spd_atk: 1.0,
            raid_p_atk: 1.0,
            raid_m_atk: 1.0,
            raid_p_def: 1.0,
            raid_m_def: 1.0,
        }
    }
}

impl NpcStatMods {
    /// The two guards the finalizers repeat: champion multipliers need
    /// `Config.CHAMPION_ENABLE && creature.isChampion()`, raid multipliers need
    /// `creature.isRaid()`. They are independent — a champion raid minion takes
    /// both.
    ///
    /// `is_raid` is the caller's, not the template's, because Java's `_isRaid`
    /// is an *instance* flag: `Monster.onSpawn` calls
    /// `setIsRaidMinion(_master.isRaid())`, which sets the very same field, so
    /// a raid boss's escort scales like the boss. Only the spawn site knows
    /// whether it is building a minion.
    pub(crate) fn of(cfg: &crate::config::CombatConfig, champion: bool, is_raid: bool) -> Self {
        let mut m = Self::default();
        if cfg.champion.enable && champion {
            m.atk = cfg.champion.atk;
            m.spd_atk = cfg.champion.spd_atk;
        }
        if is_raid {
            m.raid_p_atk = cfg.npc.raid_p_atk_multiplier;
            m.raid_m_atk = cfg.npc.raid_m_atk_multiplier;
            m.raid_p_def = cfg.npc.raid_p_def_multiplier;
            m.raid_m_def = cfg.npc.raid_m_def_multiplier;
        }
        m
    }
}

/// Finalize an NPC's `CombatStats`, `Speeds`, and max HP/MP from its template
/// base → passive template skills → active buffs, through the same add/mul maps
/// and `finalize*` the player uses. Max HP/MP fold in the CON/MEN bonus exactly
/// like Java's `Max{Hp,Mp}Finalizer` (`base × statBonus`, then the passive/buff
/// `mul`/`add`) — NPCs are uncapped there (the HP_LIMIT branch is player-only).
/// Shared by spawn ([`crate::model::npc::spawn`]) and buff recompute.
pub(crate) fn npc_finalized_stats(
    data: &GameData,
    t: &crate::data::npc_data::NpcTemplate,
    buffs: &Buffs,
    mods_in: NpcStatMods,
) -> (CombatStats, Speeds, f64, f64) {
    let sb = &data.stat_bonus;
    let caps = &data.combat_caps;
    let mut base = game_loop::npc::npc_combat_stats(t, sb);
    // `PAttackFinalizer`/`MAttackFinalizer`/`P|MAttackSpeedFinalizer`:
    // `baseValue *= CHAMPION_ATK | CHAMPION_SPD_ATK` **before** the STR/DEX
    // bonus and before the buff mul/add. Multiplication commutes with the
    // bonus `npc_combat_stats` has already folded in, so scaling the base here
    // lands on the same number Java's chain does, and the caps below still
    // clamp last exactly like `validateValue`.
    base.p_atk *= mods_in.atk * mods_in.raid_p_atk;
    base.m_atk *= mods_in.atk * mods_in.raid_m_atk;
    base.p_atk_spd = (base.p_atk_spd as f64 * mods_in.spd_atk) as i32;
    base.m_atk_spd = (base.m_atk_spd as f64 * mods_in.spd_atk) as i32;
    // `P|MDefenseFinalizer`'s raid pass. There is no champion equivalent —
    // a champion hits harder but is no tougher.
    base.p_def *= mods_in.raid_p_def;
    base.m_def *= mods_in.raid_m_def;
    // Template passive skills are the NPC's innate stat base; player-cast buffs
    // (buffs.0) stack on top through the same maps.
    let mut mods = npc_passive_mods(data, t);
    for buff in &buffs.0 {
        for effect in &buff.effects {
            apply_modifier(&mut mods, effect);
        }
    }
    let combat = CombatStats {
        p_atk: finalize(&mods, Stat::PhysicalAttack, base.p_atk).clamp(0.0, caps.max_p_atk),
        m_atk: finalize(&mods, Stat::MagicalAttack, base.m_atk).clamp(0.0, caps.max_m_atk),
        // NPCs carry no naked-base/gear split, so the defense floor is a fifth
        // of the template value (mirrors the player's `base × 0.2`).
        p_def: finalize_def(&mods, Stat::PhysicalDefence, base.p_def, base.p_def * 0.2),
        m_def: finalize_def(&mods, Stat::MagicalDefence, base.m_def, base.m_def * 0.2),
        p_atk_spd: finalize_speed(&mods, Stat::PhysicalAttackSpeed, base.p_atk_spd as f64)
            .clamp(1.0, caps.max_p_atk_speed) as i32,
        m_atk_spd: finalize_speed(&mods, Stat::MagicAttackSpeed, base.m_atk_spd as f64)
            .clamp(1.0, caps.max_m_atk_speed) as i32,
        crit_hit: finalize(&mods, Stat::CriticalRate, base.crit_hit)
            .clamp(0.0, caps.max_p_crit_rate),
        m_crit_hit: base.m_crit_hit,
        accuracy: finalize(&mods, Stat::AccuracyCombat, base.accuracy as f64) as i32,
        evasion: finalize(&mods, Stat::EvasionRate, base.evasion as f64)
            .clamp(0.0, caps.max_evasion) as i32,
        magic_evasion: base.magic_evasion,
        magic_accuracy: base.magic_accuracy,
        // Range / random-damage aren't buffable here — keep the template values.
        atk_range: base.atk_range,
        random_dmg: base.random_dmg,
        // Buffs cannot move it: no skill on this dist declares a `shotBonus`
        // modifier, so an NPC's stays at the template's flat 1.
        shots_bonus_add: base.shots_bonus_add,
    };
    let speeds = Speeds {
        // No `RUN_SPD_BOOST` for NPCs (that's a player-only base add).
        run_spd: finalize(&mods, Stat::RunSpeed, t.base_run_spd),
        walk_spd: finalize(&mods, Stat::WalkSpeed, t.base_walk_spd),
        swim_run_spd: 0.0,
        swim_walk_spd: 0.0,
        move_multiplier: 1.0,
        base_run_spd: t.base_run_spd,
        base_walk_spd: t.base_walk_spd,
        // NPC templates on this dist declare no `<speed><…swim=…>`, so the
        // swim bases are 0 — `client_move_multiplier` falls back to 1.0 for
        // them, and nothing flips `swimming` on an NPC anyway (zone
        // revalidation is player-only in the port).
        base_swim_run_spd: 0.0,
        base_swim_walk_spd: 0.0,
        running: false,
        swimming: false,
        swamp_multiplier: 1.0,
    };
    // `Max{Hp,Mp}Finalizer`: `mul × (baseMax × {CON,MEN} bonus) + add`; the
    // bonus is skipped when the stat is 0 (`getX() > 0 ? bonus : 1`).
    let con_bonus = if t.base_con > 0 {
        sb.con_bonus(t.base_con)
    } else {
        1.0
    };
    let men_bonus = if t.base_men > 0 {
        sb.men_bonus(t.base_men)
    } else {
        1.0
    };
    let hp_mul = mods.mul.get(&Stat::MaxHp).copied().unwrap_or(1.0);
    let hp_add = mods.add.get(&Stat::MaxHp).copied().unwrap_or(0.0);
    let mp_mul = mods.mul.get(&Stat::MaxMp).copied().unwrap_or(1.0);
    let mp_add = mods.add.get(&Stat::MaxMp).copied().unwrap_or(0.0);
    let max_hp = hp_mul * (t.base_hp_max * con_bonus) + hp_add;
    let max_mp = mp_mul * (t.base_mp_max * men_bonus) + mp_add;
    (combat, speeds, max_hp, max_mp)
}

/// Rebuild an NPC's `CombatStats`/`Speeds`/max-HP·MP from its template (incl.
/// passive template skills) plus its active buffs — the NPC counterpart of
/// `Player::recalculate_stats` + `apply_buff`/`remove_buff`. Called on every
/// buff apply/expire, so it starts from a clean base each time and can't drift.
/// Current HP/MP are only clamped *down* to a new max (Java never heals on a
/// max increase).
pub(crate) fn recompute_npc_stats_from_buffs(
    data: &GameData,
    t: &crate::data::npc_data::NpcTemplate,
    buffs: &Buffs,
    mods_in: NpcStatMods,
    combat: &mut CombatStats,
    speeds: &mut Speeds,
    vitals: &mut Vitals,
) {
    let (new_combat, new_speeds, max_hp, max_mp) = npc_finalized_stats(data, t, buffs, mods_in);
    *combat = new_combat;
    // Preserve the live running/swimming state (a mid-chase mob is running);
    // only the speed magnitudes recompute.
    speeds.run_spd = new_speeds.run_spd;
    speeds.walk_spd = new_speeds.walk_spd;
    vitals.max_hp = max_hp as i32;
    vitals.max_mp = max_mp as i32;
    vitals.cur_hp = vitals.cur_hp.min(max_hp);
    vitals.cur_mp = vitals.cur_mp.min(max_mp);
}
