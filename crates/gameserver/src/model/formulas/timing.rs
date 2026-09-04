//! Cast and attack timing: the attack-speed multipliers, `calcCastTimes`,
//! and the hit/reuse intervals derived from them.

use crate::data::GameData;
use crate::model::Player;
use crate::model::skill::Skill;
use crate::model::stats::{BaseStat, Stat};

/// `Formulas.SKILL_LAUNCH_TIME` — the floor on the launch→finish phase.
const SKILL_LAUNCH_TIME_MS: f64 = 500.0;

/// `Formulas.calcAtkSpdMultiplier` (armorBonus = 1). The "weapon base" attack
/// speed for an unarmed player is the class template's `basePAtkSpd`.
pub fn calc_atk_spd_multiplier(
    p: &Player,
    base: &crate::model::components::stats::BaseStats,
    mods: &crate::model::components::stats::StatModifiers,
    data: &GameData,
    // `Stat.weaponBaseValue(creature, PHYSICAL_ATTACK_SPEED)` — the **equipped
    // weapon's** declared attack speed, which replaces the class base for a
    // player holding one (`IStatFunction.calcWeaponBaseValue`, the same rule
    // `PAttackSpeedFinalizer` runs on). `None` bare-handed.
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    let t = data
        .player_templates
        .get_or_base(p.class_id, p.base_class_id)
        .cloned()
        .unwrap_or_default();
    let dex_bonus = data.stat_bonus.bonus(BaseStat::Dex, base.dex);
    let mul = mods
        .mul
        .get(&Stat::PhysicalAttackSpeed)
        .copied()
        .unwrap_or(1.0);
    let add = mods
        .add
        .get(&Stat::PhysicalAttackSpeed)
        .copied()
        .unwrap_or(0.0);
    let weapon_attack_speed = weapon_p_atk_spd.unwrap_or(t.base_p_atk_spd as f64);
    dex_bonus * (weapon_attack_speed / 333.0) * mul + add / 333.0
}

/// `Formulas.calcMAtkSpdMultiplier` (armorBonus = 1).
pub fn calc_m_atk_spd_multiplier(
    base: &crate::model::components::stats::BaseStats,
    mods: &crate::model::components::stats::StatModifiers,
    data: &GameData,
) -> f64 {
    let wit_bonus = data.stat_bonus.bonus(BaseStat::Wit, base.wit);
    let mul = mods
        .mul
        .get(&Stat::MagicAttackSpeed)
        .copied()
        .unwrap_or(1.0);
    let add = mods
        .add
        .get(&Stat::MagicAttackSpeed)
        .copied()
        .unwrap_or(0.0);
    wit_bonus * mul + add / 333.0
}

/// `Formulas.calcSkillTimeFactor` — the divisor for hit/cancel time (the
/// channeling branch in `calc_cast_times` bypasses it for the hit phase);
/// the spiritshot hit-time term is 0.
pub fn calc_skill_time_factor(
    p: &Player,
    base: &crate::model::components::stats::BaseStats,
    mods: &crate::model::components::stats::StatModifiers,
    data: &GameData,
    skill: &Skill,
    // `creature.isChargedShot(SPIRITSHOTS) || isChargedShot(BLESSED_SPIRITSHOTS)`
    // — Java's `spiritshotHitTime` of **0.4**, i.e. a charged mage casts at
    // `matkSpdMul · 1.4`. Ignored for anything but a magic skill, as in Java.
    spiritshot_charged: bool,
    // The equipped weapon's attack speed, for the physical arm — see
    // [`calc_atk_spd_multiplier`].
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    // `skill.getOperateType().isChanneling()` heads the same early return as
    // the three static magic types: a channeled skill's timing is fixed, so
    // its factor is 1 and its cancel time is **not** divided by cast speed.
    if skill.operate_type == crate::model::skill::target::OperateType::Channeling
        || skill.magic_type == 2
        || skill.magic_type == 4
        || skill.magic_type == 21
    {
        return 1.0;
    }
    let factor = if skill.magic_type == 1 {
        let m = calc_m_atk_spd_multiplier(base, mods, data);
        // `factor = matkspdmul + (matkspdmul * spiritshotHitTime)`.
        m + (m * if spiritshot_charged { 0.4 } else { 0.0 })
    } else {
        calc_atk_spd_multiplier(p, base, mods, data, weapon_p_atk_spd)
    };
    factor.max(0.01)
}

/// `Formulas.calcSkillCancelTime` — the launch→finish phase length in ms.
pub fn calc_skill_cancel_time(
    p: &Player,
    base: &crate::model::components::stats::BaseStats,
    mods: &crate::model::components::stats::StatModifiers,
    data: &GameData,
    skill: &Skill,
    spiritshot_charged: bool,
    weapon_p_atk_spd: Option<f64>,
) -> f64 {
    ((skill.hit_cancel_time * 1000.0)
        / calc_skill_time_factor(
            p,
            base,
            mods,
            data,
            skill,
            spiritshot_charged,
            weapon_p_atk_spd,
        ))
    .max(SKILL_LAUNCH_TIME_MS)
}

/// `Formulas.calcAtkSpd` — the post-finish cool phase in ms (magic scales by
/// casting speed against the 333 base, physical by attack speed against 300).
pub fn calc_atk_spd(
    combat: &crate::model::components::stats::CombatStats,
    skill: &Skill,
    skill_time: f64,
) -> i32 {
    if skill.magic_type == 1 {
        (skill_time / combat.m_atk_spd.max(1) as f64 * 333.0) as i32
    } else {
        (skill_time / combat.p_atk_spd.max(1) as f64 * 300.0) as i32
    }
}

/// `SkillCaster.calcSkillTiming` + the `startCasting` cool-time override →
/// `(hit_ms, cancel_ms, cool_ms)`. `calcSkillTiming` computes `_coolTime =
/// coolTime / timeFactor`, but `startCasting` immediately overwrites it with
/// `Formulas.calcAtkSpd(caster, skill, coolTime)` before it's ever used, so
/// only the override is ported. Client-displayed cast time (`MagicSkillUse` /
/// `SetupGauge`) is `hit + cancel`.
pub fn calc_cast_times(
    p: &Player,
    base: &crate::model::components::stats::BaseStats,
    mods: &crate::model::components::stats::StatModifiers,
    combat: &crate::model::components::stats::CombatStats,
    data: &GameData,
    skill: &Skill,
    spiritshot_charged: bool,
    weapon_p_atk_spd: Option<f64>,
) -> (i32, i32, i32) {
    let factor = calc_skill_time_factor(
        p,
        base,
        mods,
        data,
        skill,
        spiritshot_charged,
        weapon_p_atk_spd,
    );
    let cancel = calc_skill_cancel_time(
        p,
        base,
        mods,
        data,
        skill,
        spiritshot_charged,
        weapon_p_atk_spd,
    );
    // Channeling (`CA1`) cast time is **static**: `_hitTime = max(hitTime −
    // cancelTime, 0)`, `_cancelTime = 2866` — no time-factor scaling, so
    // Volcano channels its full duration regardless of casting speed. The
    // cancel term does not scale either: `calcSkillTimeFactor` returns 1 for a
    // channeling skill, which this comment used to claim the opposite of.
    if skill.operate_type == crate::model::skill::target::OperateType::Channeling {
        let hit = (skill.hit_time as f64 - cancel).max(0.0) as i32;
        let cool = calc_atk_spd(combat, skill, skill.cool_time as f64);
        return (hit, 2866, cool);
    }
    let hit = (skill.hit_time as f64 / factor - cancel).max(0.0) as i32;
    let cool = calc_atk_spd(combat, skill, skill.cool_time as f64);
    (hit, cancel as i32, cool)
}

/// `Formulas.calculateTimeBetweenAttacks`: full swing period in ms.
pub fn calculate_time_between_attacks(p_atk_spd: i32) -> i32 {
    (500_000 / p_atk_spd.max(1)).max(50)
}

/// `Formulas.calculateTimeToHit` for the melee branches (bows/duals are out
/// of scope): when the damage lands within the swing.
pub fn calculate_time_to_hit(total_attack_time: i32, two_handed: bool) -> i32 {
    (total_attack_time as f64 * if two_handed { 0.735 } else { 0.644 }) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `500000 / atkSpd`, floored at 50 ms.
    #[test]
    fn time_between_attacks_matches_java() {
        assert_eq!(calculate_time_between_attacks(300), 1666);
        assert_eq!(calculate_time_between_attacks(1_000_000), 50);
    }

    /// Melee time-to-hit fractions (0.644 / 0.735 two-handed).
    #[test]
    fn time_to_hit_fractions() {
        assert_eq!(calculate_time_to_hit(1666, false), 1072);
        assert_eq!(calculate_time_to_hit(1666, true), 1224);
    }
}
