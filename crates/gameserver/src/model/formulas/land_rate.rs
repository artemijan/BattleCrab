//! `calcEffectLandRate` — whether a debuff sticks — plus the attribute
//! bonus and the resurrect restore percent that feed the same clamps.

/// `Formulas.calculateSkillResurrectRestorePercent` — the reviver's WIT scales
/// how much of the lost XP their resurrection gives back.
///
/// ```java
/// if (base == 0 || base == 100) return base;
/// restore = base * WIT.calcBonus(caster);
/// if ((restore - base) > 20.0) restore += 20.0;
/// return min(max(restore, base), 90.0);
/// ```
///
/// Note the quirk on the third line: a bonus that already exceeds +20 gets a
/// *further* flat +20, so high-WIT revivers jump rather than scale smoothly.
/// Ported as written.
pub fn calc_resurrect_restore_percent(base: f64, wit_bonus: f64) -> f64 {
    if base == 0.0 || base == 100.0 {
        return base;
    }
    let mut restore = base * wit_bonus;
    if (restore - base) > 20.0 {
        restore += 20.0;
    }
    restore.max(base).min(90.0)
}

/// `Formulas.calcEffectSuccess` — a continuous skill's landing chance in percent.
///
/// ```java
/// if (activateRate == -1) return true;
/// int magicLevel = skill.getMagicLevel();
/// if (magicLevel <= -1) magicLevel = target.getLevel() + 3;
/// final double targetBasicProperty = getAbnormalResist(skill.getBasicProperty(), target);
/// final double baseMod = ((((((magicLevel - target.getLevel()) + 3) * skill.getLvlBonusRate()) + activateRate) + 30.0) - targetBasicProperty);
/// final double rate = baseMod * elementMod * traitMod * buffDebuffMod;
/// final double finalRate = traitMod > 0 ? CommonUtil.constrain(rate, skill.getMinChance(), skill.getMaxChance()) * basicPropertyResist : 0;
/// ```
///
/// Everything up to `+ 30.0` is **integer** arithmetic in Java and is kept so
/// here; the four mods multiply in Java's left-to-right order, which is not
/// interchangeable in floating point.
///
/// `skill.getMinChance()`/`getMaxChance()` default to `Config.MIN_/MAX_ABNORMAL_
/// STATE_SUCCESS_RATE` and are overridden by no skill this dist can reach — the
/// 15 that declare their own bounds are all id ≥ 11537 and none is learnable —
/// so [`LandRateBounds`] carries the config pair alone.
/// `Character.ini`'s `MinAbnormalStateSuccessRate` / `MaxAbnormalStateSuccessRate`
/// — the `constrain(rate, minChance, maxChance)` bounds in
/// `Formulas.calcEffectLandRate`. Carried as a struct for the same reason
/// [`crate::model::npc_stats::NpcStatMods`] is: the formula stays a pure function of its
/// inputs, and `World` (which owns the config) is not reachable from here.
///
/// `Default` is this dist's 10/90, so a test that is not about the clamp does
/// not have to name it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandRateBounds {
    pub min: f64,
    pub max: f64,
}
impl Default for LandRateBounds {
    fn default() -> Self {
        Self {
            min: 10.0,
            max: 90.0,
        }
    }
}
impl LandRateBounds {
    pub fn of(cfg: &crate::config::CharacterConfig) -> Self {
        Self {
            min: cfg.min_abnormal_state_success_rate,
            max: cfg.max_abnormal_state_success_rate,
        }
    }
}

pub fn calc_effect_land_rate(
    magic_level: i32,
    activate_rate: i32,
    lvl_bonus_rate: i32,
    target_level: i32,
    // The target's `Stat.RESIST_ABNORMAL_DEBUFF` (Java's `buffDebuffMod`):
    // 1.0 with no resistance, < 1 when resistant (Guts), > 1 when made
    // vulnerable (Touch of Death). Callers pass 1.0 for a non-debuff skill,
    // which is what Java's `skill.isDebuff() ? … : 1` does.
    debuff_resist_mod: f64,
    // `calcAttributeBonus` for the skill's element (Java's `elementMod`) —
    // an elemental debuff lands more easily on a target weak to its element.
    // 1.0 when neither side carries the element.
    element_mod: f64,
    // `calcGeneralTraitBonus` — 1.0 with no matching resistance, `< 1` when the
    // target resists this debuff's trait, `> 1` when the trait is a
    // *vulnerability*, and **0** when they are invulnerable to it.
    trait_mod: f64,
    // `getAbnormalResist(skill.getBasicProperty(), target)` — the target's
    // `ABNORMAL_RESIST_PHYSICAL`/`_MAGICAL` **stat**, *subtracted* inside
    // `baseMod`. 0 with no such stat.
    target_basic_property: f64,
    // `getBasicPropertyResistBonus(skill.getBasicProperty(), target)` — the
    // mesmerizing-debuff accrual chain (1.0 / 0.6 / 0.3 / 0), multiplied
    // **after** the clamp, which is why level 3 is a hard immunity rather than
    // a rate the 10 floor rescues. See `game_loop::stats::basic_property`.
    basic_property_resist: f64,
    // `Config.MIN_/MAX_ABNORMAL_STATE_SUCCESS_RATE`, the `constrain` bounds.
    bounds: LandRateBounds,
) -> f64 {
    if activate_rate == -1 {
        return 100.0;
    }
    let magic_level = if magic_level <= -1 {
        target_level + 3
    } else {
        magic_level
    };
    let base_mod = ((magic_level - target_level + 3) * lvl_bonus_rate + activate_rate + 30) as f64
        - target_basic_property;
    // Invulnerability is *not* clamped: Java's
    // `finalRate = traitMod > 0 ? constrain(rate, min, max) : 0` short-circuits
    // past the 10 floor, so an immune target refuses the debuff outright rather
    // than taking it one roll in ten.
    if trait_mod <= 0.0 {
        return 0.0;
    }
    // Otherwise Java multiplies the raw rate by the mods and clamps *after*
    // (`constrain(baseMod * elementMod * … * buffDebuffMod, minChance,
    // maxChance)`), so heavy resistance can pull an otherwise-capped debuff
    // below the 90 ceiling but never under the 10 floor — **except** through
    // `basicPropertyResist`, which Java multiplies in after the clamp and which
    // therefore *can* reach 0.
    // Java's order: `baseMod * elementMod * traitMod * buffDebuffMod`.
    (base_mod * element_mod * trait_mod * debuff_resist_mod).clamp(bounds.min, bounds.max)
        * basic_property_resist
}

/// `Formulas.calcAttributeBonus`'s arithmetic tail (PLAN_G19_ATTRIBUTES.md):
/// `diff = attack − defence`; a positive gap scales up as
/// `min(1.025 + √(diff³/2)·0.0001, 1.25)`, a negative one down as
/// `max(0.975 − √(−diff³/2)·0.0001, 0.75)`, zero is exactly 1. Note the
/// discontinuity at ±1 (1.0 jumps past 1.025) — Java's, kept.
pub fn calc_attribute_bonus(attack: f64, defence: f64) -> f64 {
    let diff = attack - defence;
    if diff > 0.0 {
        (1.025 + ((diff.powi(3) / 2.0).sqrt() * 0.0001)).min(1.25)
    } else if diff < 0.0 {
        (0.975 - (((-diff).powi(3) / 2.0).sqrt() * 0.0001)).max(0.75)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decrease Speed 1 (magicLevel 35, activateRate 80, lvlBonusRate 30): the
    /// steep lvlBonusRate caps the chance at 90 vs a low-level mob and floors it
    /// at 10 vs a much higher-level one. `activateRate == -1` always lands (100),
    /// and a skill with no magic level uses `targetLevel + 3`.
    #[test]
    fn effect_land_rate_clamps_and_special_cases() {
        // (35 - 5 + 3)·30 + 80 + 30 = 1100 → clamp to 90.
        assert!(
            (calc_effect_land_rate(35, 80, 30, 5, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 90.0)
                .abs()
                < 1e-9
        );
        // (35 - 80 + 3)·30 + 80 + 30 = -1150 → clamp to 10.
        assert!(
            (calc_effect_land_rate(35, 80, 30, 80, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 10.0)
                .abs()
                < 1e-9
        );
        // activateRate -1 → guaranteed.
        assert!(
            (calc_effect_land_rate(35, -1, 30, 5, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 100.0)
                .abs()
                < 1e-9
        );
        // magicLevel <= -1 falls back to targetLevel + 3, so the level term is
        // (23 - 20 + 3) = 6: 6·5 + 10 + 30 = 70.
        assert!(
            (calc_effect_land_rate(-1, 10, 5, 20, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 70.0)
                .abs()
                < 1e-9
        );
    }

    /// The trait multiplier is folded in **before** the clamp, alongside the
    /// element and debuff-resist mods, so Stun Resistance 3 (30 %) takes a
    /// mid-range stun from 50 to 35. **Invulnerability is the exception**: it
    /// skips the clamp entirely (`traitMod > 0 ? constrain(…) : 0`), so an
    /// immune target refuses the debuff outright instead of taking it at the
    /// 10 % floor. And a *negative* defence trait is a vulnerability, which
    /// pushes the rate up into the ceiling.
    #[test]
    fn effect_land_rate_folds_the_trait_bonus_in_before_clamping() {
        // (20 - 20 + 3)·5 + 5 + 30 = 50 unresisted.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default())
                - 50.0)
                .abs()
                < 1e-9
        );
        // 30 % trait resistance → 0.70 → 35.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 0.70, 0.0, 1.0, Default::default())
                - 35.0)
                .abs()
                < 1e-9
        );
        // Invulnerable → 0, not the 10 floor.
        assert_eq!(
            calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 0.0, 0.0, 1.0, Default::default()),
            0.0
        );
        // A vulnerability (defence -15 → 1.15) raises it: 50 · 1.15 = 57.5.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 1.0, 1.0, 1.15, 0.0, 1.0, Default::default())
                - 57.5)
                .abs()
                < 1e-9
        );
        // It composes with the other two mods rather than replacing them.
        assert!(
            (calc_effect_land_rate(20, 5, 5, 20, 0.8, 1.25, 0.70, 0.0, 1.0, Default::default())
                - 35.0)
                .abs()
                < 1e-9
        );
        // The always-lands escape hatch is checked first, so even immunity
        // cannot stop an `activateRate == -1` debuff (Java returns true before
        // computing any mod).
        assert_eq!(
            calc_effect_land_rate(20, -1, 5, 20, 1.0, 1.0, 0.0, 0.0, 1.0, Default::default()),
            100.0
        );
    }
}
