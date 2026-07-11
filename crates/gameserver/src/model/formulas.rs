//! Port of `model/stats/Formulas.java`, scoped to what the single-target cast
//! pipeline needs: magic damage, magic crit, cast timing, heal, and the
//! cast-break-on-hit roll. Every function documents the Java method it ports
//! and which terms are dropped. The dropped terms are all identity values for
//! an unarmed, shotless player with no trait/attribute stats — the only kind
//! of actor that exists so far: `SHOTS_BONUS`/spiritshots (1.0/absent),
//! trait/weakness/attribute mods (1.0), `SKILL_POWER_ADD` (0),
//! `RANDOM_DAMAGE` (weapon-supplied, unarmed = 0 → randomMod 1.0),
//! pvp/pve config multipliers (1.0 by default), `MAGICAL_SKILL_POWER` (1.0).

use crate::data::GameData;
use crate::model::skill::Skill;
use crate::model::stats::{BaseStat, Stat};
use crate::model::Player;

/// `Formulas.SKILL_LAUNCH_TIME` — the floor on the launch→finish phase.
const SKILL_LAUNCH_TIME_MS: f64 = 500.0;

/// `Formulas.calcMagicDam` (the `77 * power * sqrt(mAtk) / mDef` MDAM
/// formula). `mcrit` doubles the damage via `calcCritDamage`'s magic branch
/// (`2 * MAGIC_CRITICAL_DAMAGE(1) * DEFENCE_MAGIC_CRITICAL_DAMAGE(1)`). The
/// `ALT_GAME_MAGICFAILURES` resist branch is deferred (equivalent to running
/// with `MagicFailures = False`) — it needs `calcMagicSuccess`' magic-level
/// vs target-level table, which nothing else uses yet.
pub fn calc_magic_dam(m_atk: f64, m_def: f64, power: f64, mcrit: bool) -> f64 {
    (77.0 * power * m_atk.sqrt() / m_def.max(1.0)) * if mcrit { 2.0 } else { 1.0 }
}

/// `Formulas.calcCrit`'s magic branch for both-below-level-78 actors (base
/// classes cap at 40/76 here). `m_crit_rate` is the per-mille
/// `Player.m_crit_hit`; `roll` is `Rnd.get(1000)`. Good skills cap at 320‰,
/// bad skills at 200‰ (`DEFENCE_MAGIC_CRITICAL_RATE` defaults to the
/// attacker's rate and the balance multipliers to 1.0, so both drop out).
pub fn calc_magic_crit(m_crit_rate: f64, is_bad: bool, roll: i32) -> bool {
    let cap = if is_bad { 200.0 } else { 320.0 };
    m_crit_rate.min(cap) > roll as f64
}

/// `Formulas.calcAtkSpdMultiplier` (armorBonus = 1). The "weapon base" attack
/// speed for an unarmed player is the class template's `basePAtkSpd`.
pub fn calc_atk_spd_multiplier(p: &Player, data: &GameData) -> f64 {
    let t = data
        .player_templates
        .get(p.class_id)
        .or_else(|| data.player_templates.get(p.base_class_id))
        .cloned()
        .unwrap_or_default();
    let dex_bonus = data.stat_bonus.bonus(BaseStat::Dex, p.dex);
    let mul = p.stats_mul.get(&Stat::PhysicalAttackSpeed).copied().unwrap_or(1.0);
    let add = p.stats_add.get(&Stat::PhysicalAttackSpeed).copied().unwrap_or(0.0);
    dex_bonus * (t.base_p_atk_spd as f64 / 333.0) * mul + add / 333.0
}

/// `Formulas.calcMAtkSpdMultiplier` (armorBonus = 1).
pub fn calc_m_atk_spd_multiplier(p: &Player, data: &GameData) -> f64 {
    let wit_bonus = data.stat_bonus.bonus(BaseStat::Wit, p.wit);
    let mul = p.stats_mul.get(&Stat::MagicAttackSpeed).copied().unwrap_or(1.0);
    let add = p.stats_add.get(&Stat::MagicAttackSpeed).copied().unwrap_or(0.0);
    wit_bonus * mul + add / 333.0
}

/// `Formulas.calcSkillTimeFactor` — the divisor for hit/cancel time. No
/// channeling skills or NPCs exist; the spiritshot hit-time term is 0.
pub fn calc_skill_time_factor(p: &Player, data: &GameData, skill: &Skill) -> f64 {
    if skill.magic_type == 2 || skill.magic_type == 4 || skill.magic_type == 21 {
        return 1.0;
    }
    let factor = if skill.magic_type == 1 {
        calc_m_atk_spd_multiplier(p, data)
    } else {
        calc_atk_spd_multiplier(p, data)
    };
    factor.max(0.01)
}

/// `Formulas.calcSkillCancelTime` — the launch→finish phase length in ms.
pub fn calc_skill_cancel_time(p: &Player, data: &GameData, skill: &Skill) -> f64 {
    ((skill.hit_cancel_time * 1000.0) / calc_skill_time_factor(p, data, skill)).max(SKILL_LAUNCH_TIME_MS)
}

/// `Formulas.calcAtkSpd` — the post-finish cool phase in ms (magic scales by
/// casting speed against the 333 base, physical by attack speed against 300).
pub fn calc_atk_spd(p: &Player, skill: &Skill, skill_time: f64) -> i32 {
    if skill.magic_type == 1 {
        (skill_time / p.m_atk_spd.max(1) as f64 * 333.0) as i32
    } else {
        (skill_time / p.p_atk_spd.max(1) as f64 * 300.0) as i32
    }
}

/// `SkillCaster.calcSkillTiming` + the `startCasting` cool-time override →
/// `(hit_ms, cancel_ms, cool_ms)`. `calcSkillTiming` computes `_coolTime =
/// coolTime / timeFactor`, but `startCasting` immediately overwrites it with
/// `Formulas.calcAtkSpd(caster, skill, coolTime)` before it's ever used, so
/// only the override is ported. Client-displayed cast time (`MagicSkillUse` /
/// `SetupGauge`) is `hit + cancel`.
pub fn calc_cast_times(p: &Player, data: &GameData, skill: &Skill) -> (i32, i32, i32) {
    let factor = calc_skill_time_factor(p, data, skill);
    let cancel = calc_skill_cancel_time(p, data, skill);
    let hit = (skill.hit_time as f64 / factor - cancel).max(0.0) as i32;
    let cool = calc_atk_spd(p, skill, skill.cool_time as f64);
    (hit, cancel as i32, cool)
}

/// `handlers/effecthandlers/Heal.java` `instant()`, unarmed/shotless
/// narrowing: `staticShotBonus = 0`, `mAtkMul = 1 + 1 = 2` (no-grade weapon +
/// shot dynamic bonus), `HEAL_EFFECT`/`HEAL_EFFECT_ADD` stats absent (×1/+0),
/// healing-skill config multiplier 1.0. Magic crit triples the heal. The
/// overheal clamp is the caller's job (it needs the target's HP).
pub fn calc_heal(power: f64, m_atk: f64, mcrit: bool) -> f64 {
    (power + (2.0 * m_atk).sqrt()) * if mcrit { 3.0 } else { 1.0 }
}

/// `Formulas.calcAtkBreak`, `ALT_GAME_CANCEL_CAST` branch (default config
/// `AltGameCancelByHit = cast`): the caller must already have checked that
/// the target is casting and still abortable (pre-launch) — that check is
/// what sets `init = 15`. `men_bonus` is the target's `BaseStat.MEN` bonus;
/// `roll` is `Rnd.get(100)`. The `ATTACK_CANCEL` stat override and the
/// raid/HP-blocked/channeling guards don't apply to players yet.
pub fn calc_atk_break(dmg: f64, men_bonus: f64, roll: i32) -> bool {
    let init = 15.0 + (13.0 * dmg).sqrt() - (men_bonus * 100.0 - 100.0);
    let rate = init.clamp(1.0, 99.0);
    (roll as f64) < rate
}

#[cfg(test)]
mod tests {
    use super::*;

    /// power 12 (Wind Strike 1), mAtk 100, mDef 60: 77·12·√100/60 = 154.0;
    /// magic crit doubles it.
    #[test]
    fn magic_dam_matches_java_formula() {
        let dmg = calc_magic_dam(100.0, 60.0, 12.0, false);
        assert!((dmg - 154.0).abs() < 1e-9);
        let crit = calc_magic_dam(100.0, 60.0, 12.0, true);
        assert!((crit - 308.0).abs() < 1e-9);
    }

    /// mDef is floored at 1 so a zero-defence target can't divide by zero.
    #[test]
    fn magic_dam_survives_zero_mdef() {
        assert!(calc_magic_dam(100.0, 0.0, 12.0, false).is_finite());
    }

    /// Good skills cap the per-mille rate at 320, bad skills at 200; the
    /// comparison is strict (`rate > roll`).
    #[test]
    fn magic_crit_caps_and_thresholds() {
        assert!(calc_magic_crit(1000.0, false, 319));
        assert!(!calc_magic_crit(1000.0, false, 320));
        assert!(calc_magic_crit(1000.0, true, 199));
        assert!(!calc_magic_crit(1000.0, true, 200));
        assert!(!calc_magic_crit(0.0, false, 0));
    }

    /// Heal: power 83, mAtk 50 → 83 + √100 = 93; crit triples.
    #[test]
    fn heal_matches_java_formula() {
        assert!((calc_heal(83.0, 50.0, false) - 93.0).abs() < 1e-9);
        assert!((calc_heal(83.0, 50.0, true) - 279.0).abs() < 1e-9);
    }

    /// Neutral MEN (bonus 1.0): rate = 15 + √(13·dmg), clamped to [1, 99].
    #[test]
    fn atk_break_rate_and_clamps() {
        // dmg 100 → 15 + √1300 ≈ 51.06: roll 51 breaks, 52 doesn't.
        assert!(calc_atk_break(100.0, 1.0, 51));
        assert!(!calc_atk_break(100.0, 1.0, 52));
        // Huge MEN bonus can't push the rate below 1%.
        assert!(calc_atk_break(0.0, 2.0, 0));
        assert!(!calc_atk_break(0.0, 2.0, 1));
        // Massive damage caps at 99% — roll 99 still survives.
        assert!(!calc_atk_break(1e9, 1.0, 99));
    }
}
