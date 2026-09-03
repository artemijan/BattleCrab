//! The magic path: `calcMagicDam`, the magic-crit roll, and the
//! `calcMagicSuccess` land check for an offensive magic skill.

/// The outcome of `calcMagicDam`'s `ALT_GAME_MAGICFAILURES` block — how a
/// failed [`calc_magic_success`] roll reshapes the damage. Rolled by the
/// caller (it needs the RNG and sends the system messages) and handed to
/// [`calc_magic_dam`] so the adjustment lands at Java's point in the formula:
/// *before* the crit multiplier, which is why a resisted magic crit deals 2
/// damage rather than 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MagicFailure {
    /// The spell landed (or `MagicFailures = False`).
    #[default]
    None,
    /// First roll failed, second succeeded — `damage /= 2`.
    Half,
    /// Both rolls failed — `damage = 1`.
    Resisted,
}

/// `Formulas.calcMagicDam` (the `77 * power * sqrt(mAtk) / mDef` MDAM
/// formula). `mcrit` doubles the damage via `calcCritDamage`'s magic branch
/// (`2 * MAGIC_CRITICAL_DAMAGE(1) * DEFENCE_MAGIC_CRITICAL_DAMAGE(1)`).
/// `shots_bonus` is Java's `bss ? 4 : sps ? 2 : 1` (times the `SHOTS_BONUS`
/// stat, 1.0 here) applied to the base magic damage.
///
/// `failure` is the `ALT_GAME_MAGICFAILURES` verdict. Java mutates `damage`
/// inside the failure block and only then multiplies by `critMod` and the
/// trait/attribute/random/pvpPve mods, so the halving and the `damage = 1`
/// floor are applied here *ahead* of `mcrit`. (The trait and attribute mods are
/// real and applied by the caller; only random/pvpPve are still identities.)
#[allow(clippy::too_many_arguments)]
pub fn calc_magic_dam(
    m_atk: f64,
    m_def: f64,
    power: f64,
    mcrit: bool,
    crit_mul: f64,
    shots_bonus: f64,
    failure: MagicFailure,
    random_mul: f64,
) -> f64 {
    let mut damage = (77.0 * power * m_atk.sqrt() / m_def.max(1.0)) * shots_bonus;
    match failure {
        MagicFailure::None => {}
        MagicFailure::Half => damage /= 2.0,
        MagicFailure::Resisted => damage = 1.0,
    }
    // `damage * critMod * … * randomMod * …` — the spread is Java's, and it
    // applies to a nuke exactly as it does to a swing: a mage's own
    // `randomDamage` (10 on every class template) makes the same cast land
    // anywhere in ±10 %.
    damage * if mcrit { crit_mul } else { 1.0 } * random_mul
}

/// `Formulas.calcProbability` — the chance gate on `Confuse`/`RandomizeHate`.
///
/// ```java
/// return Rnd.get(100) < (((skill.getMagicLevel() + baseChance) - target.getLevel()
///        - getAbnormalResist(...)) * calcAttributeBonus(...) * calcGeneralTraitBonus(...));
/// ```
///
/// The level term is the heart of it: a target far above the skill's level
/// shrugs it off, and the threshold is **unclamped**, so it can go negative
/// (never lands) or above 100 (always lands) exactly as Java's comparison does.
///
/// `attribute_mod` and `trait_mod` are `calcAttributeBonus` and
/// `calcGeneralTraitBonus(…, ignoreResistance = false)`. Both used to be 1.0
/// for every actor this port modelled, and this function said so — that stopped
/// being true when the attribute and trait tables landed, so they are passed in
/// now rather than assumed.
///
/// Java's `Double.isNaN(baseChance)` branch is unreachable here — the parser
/// defaults a missing `<chance>` to 100.
pub fn calc_probability(
    magic_level: i32,
    base_chance: i32,
    target_level: i32,
    // `getAbnormalResist(skill.getBasicProperty(), target)` — the target's
    // `ABNORMAL_RESIST_PHYSICAL`/`_MAGICAL`, **subtracted inside the
    // parenthesis** before either multiplier. The same stat
    // `calc_effect_land_rate` already reads; this formula had been ignoring
    // it, so a target whose abnormal resistance holds off a stun took a
    // Confuse at full rate.
    abnormal_resist: f64,
    attribute_mod: f64,
    trait_mod: f64,
    roll: i32,
) -> bool {
    let threshold = ((magic_level + base_chance - target_level) as f64 - abnormal_resist)
        * attribute_mod
        * trait_mod;
    (roll as f64) < threshold
}

/// `Formulas.calcManaDam` — the MP-drain damage formula, which is *not* the
/// HP one: `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef`.
///
/// Note the target's **max MP** is a direct multiplier, so the same nuke drains
/// far more from a high-MP mage than from a fighter. `shots_bonus` is Java's
/// `bss ? 4 : sps ? 2 : 1` applied to `mAtk` *before* the square root (unlike
/// [`calc_magic_dam`], where it scales the finished damage).
///
/// A magic crit triples the result and then **clamps to `crit_limit`** — a
/// per-skill cap (1600 on Aura Sink / Seal of Gloom, 7000 on Mana Burn / Mana
/// Storm) with no equivalent anywhere in the HP formulas.
///
/// Dropped terms, all identity here: `calcGeneralTraitBonus`,
/// `calculatePvpPveBonus`, and the sapphire-jewel bonus (a later chronicle's
/// item, absent from this dist).
#[allow(clippy::too_many_arguments)]
pub fn calc_mana_dam(
    m_atk: f64,
    m_def: f64,
    target_max_mp: f64,
    power: f64,
    shots_bonus: f64,
    failure: MagicFailure,
    mcrit: bool,
    crit_limit: f64,
    trait_bonus: f64,
    pvp_pve_bonus: f64,
) -> f64 {
    let m_atk = m_atk * shots_bonus;
    let mut damage = (m_atk.sqrt() * power * (target_max_mp / 97.0)) / m_def.max(1.0);
    // Java applies both of these **here**, ahead of the failure halving and
    // the crit clamp — and the clamp is the reason the order is not free:
    // `min(damage · 3, criticalLimit)` after the multipliers is not the same
    // as multiplying a clamped value afterwards. The dist ships real limits
    // (1450, 1600, 7000), so a crit that binds one would have been let past
    // it. The trait bonus is read with `ignoreResistance = false` here, unlike
    // every other damage formula.
    damage *= trait_bonus;
    damage *= pvp_pve_bonus;
    // The `ALT_GAME_MAGICFAILURES` block, applied at Java's point in the
    // formula — before the crit, like `calc_magic_dam`. Java has no
    // `damage = 1` floor on the full-resist branch here, only the halving,
    // so `Resisted` and `Half` do the same thing: ported as written.
    match failure {
        MagicFailure::None => {}
        MagicFailure::Half | MagicFailure::Resisted => damage /= 2.0,
    }
    if mcrit {
        damage = (damage * 3.0).min(crit_limit);
    }
    damage
}

/// `Formulas.calcMagicAffected` — the drain's own landing roll, separate from
/// the `calcMagicSuccess` resist check.
///
/// ```java
/// double defence = (skill.isActive() && skill.isBad()) ? target.getMDef() : 0;
/// double attack  = 2 * actor.getMAtk() * traitBonus;      // traitBonus 1 here
/// double d = (attack - defence) / (attack + defence) + 0.5 * Rnd.nextGaussian();
/// return d > 0;
/// ```
///
/// So it is a *noisy* mAtk-vs-mDef comparison: with equal attack and defence
/// the deterministic term is 0 and the coin flip is even; a large mAtk edge
/// pushes it toward certainty without ever quite reaching it.
///
/// `gaussian` is supplied by the caller (`World::roll_gaussian`) to keep this
/// function pure. `defence` is passed as 0 for a skill that is not both active
/// and bad, matching the Java branch.
pub fn calc_magic_affected(m_atk: f64, defence: f64, gaussian: f64) -> bool {
    let attack = 2.0 * m_atk;
    let sum = attack + defence;
    // Java would divide by zero here for a 0-mAtk actor and get NaN, which
    // compares false; guard explicitly rather than relying on that.
    if sum <= 0.0 {
        return false;
    }
    let d = ((attack - defence) / sum) + (0.5 * gaussian);
    d > 0.0
}

/// `Formulas.calcCrit`'s magic branch:
///
/// ```java
/// rate = creature.getStat().getValue(Stat.MAGIC_CRITICAL_RATE);
/// if ((target == null) || !skill.isBad()) return Math.min(rate, 320) > Rnd.get(1000);
/// double finalRate = target.getStat().getValue(Stat.DEFENCE_MAGIC_CRITICAL_RATE, rate) + target.getStat().getValue(Stat.DEFENCE_MAGIC_CRITICAL_RATE_ADD, 0);
/// if ((creature.getLevel() >= 78) && (target.getLevel() >= 78))
/// {
///     finalRate += Math.sqrt(creature.getLevel()) + ((creature.getLevel() - target.getLevel()) / 25);
///     return Math.min(finalRate, 320 * balanceMod) > Rnd.get(1000);
/// }
/// return (Math.min(finalRate, 200) * balanceMod) > Rnd.get(1000);
/// ```
///
/// `m_crit_rate` is the per-mille `Player.m_crit_hit`; `roll` is
/// `Rnd.get(1000)`. A good skill caps at 320‰ and never reaches the level
/// branch; a bad one caps at 200‰ until **both** sides are 78 or over, where
/// the cap lifts to 320‰ and a `sqrt(level)` bonus comes with it.
///
/// Two narrowings, carriers named: the balance multipliers are 1.0 (the dist
/// leaves `PVP_/PVE_MAGICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS` unpopulated),
/// and `DEFENCE_MAGIC_CRITICAL_RATE`/`_ADD` are declared only by skills in the
/// 10500+ id ranges, none of them learnable or on an NPC list here — so the
/// defender's term stays the identity `getValue(stat, rate) = rate`.
///
/// Java's own `(creature.getLevel() - target.getLevel()) / 25` is **integer**
/// division, so it contributes 0 for every level gap this chronicle can
/// produce; it is written out here rather than dropped, for the same reason
/// the sweep keeps identity terms.
pub fn calc_magic_crit(
    m_crit_rate: f64,
    is_bad: bool,
    caster_level: i32,
    target_level: i32,
    roll: i32,
) -> bool {
    if !is_bad {
        return m_crit_rate.min(320.0) > roll as f64;
    }
    if caster_level >= 78 && target_level >= 78 {
        let rate = m_crit_rate
            + f64::from(caster_level).sqrt()
            + f64::from((caster_level - target_level) / 25);
        return rate.min(320.0) > roll as f64;
    }
    m_crit_rate.min(200.0) > roll as f64
}

/// Inputs to [`calc_magic_success_rate`] — one field per term of Java's
/// `Formulas.calcMagicSuccess`, resolved from the world at the call site.
#[derive(Debug, Clone)]
pub struct MagicSuccess<'a> {
    /// Java `attacker.isAttackable() || target.isAttackable()` — true when
    /// either side is a monster/guard, i.e. any PvE cast. Selects the
    /// level-difference branch; false takes the magic-accuracy branch.
    pub pve: bool,
    pub target_level: i32,
    /// `skill.magicLevel` when `CalculateMagicSuccessBySkillMagicLevel` is on
    /// (dist default) and the skill has a positive magic level, else the
    /// caster's level.
    pub effective_level: i32,
    /// `attacker.getActingPlayer()`'s level — `None` when the caster is an NPC,
    /// which skips the NPC skill-chance penalty entirely (Java's null check).
    pub caster_player_level: Option<i32>,
    /// `target.isRaid() || target.isRaidMinion()` — raids are exempt from the
    /// level-78 skill-chance penalty.
    pub target_is_raid: bool,
    /// `MinNPCLevelForMagicPenalty` (78 on this dist).
    pub min_npc_level_for_magic_penalty: i32,
    /// `SkillChancePenaltyForLvLDifferences` (`2.5, 3.0, 3.25, 3.5`).
    pub skill_chance_penalty: &'a [f64],
    /// `attacker.getMagicAccuracy()` / `target.getMagicEvasionRate()` — read
    /// only on the non-PvE branch.
    pub magic_accuracy: i32,
    pub magic_evasion: i32,
    /// The target's `Stat.MAGIC_SUCCESS_RES` multiplier (Java's `resModifier`,
    /// `getMul(..., 1)`), applied to the whole failure term — so a value above
    /// 1 makes the attacker *less* likely to land the spell.
    pub res_modifier: f64,
}

/// `Formulas.calcMagicSuccess` — the percent chance (may fall below 0, meaning
/// "always fails") that a magic attack is not resisted.
///
/// PvE branch: `lvlModifier = 1.3^(targetLevel - effectiveLevel)`, so the
/// penalty compounds fast — a 9-level gap already costs ~10 points and an
/// 18-level gap drives the rate to 0. On top of that, targets at
/// `MinNPCLevelForMagicPenalty` or above that outlevel the *caster* by 3+
/// multiply the failure term by `SkillChancePenaltyForLvLDifferences`.
///
/// PvP branch: a step table on `magicAccuracy - magicEvasion`.
///
/// Java's `resModifier` is `getMul(MAGIC_SUCCESS_RES, 1)`, applied to the whole
/// failure term.
///
/// **Correction:** this was previously documented as fixed at 1.0 on this dist,
/// on the grounds that the only two items touching `magicSuccRes` (10207/10208,
/// the enhanced shirts) declare it in a `<stats>` block that Java parses into an
/// *additive* func `getMul` never sees. That is true of the items — but it
/// overlooked **skills**: Anti Magic 146 and M. Def. 147 grant the stat through
/// `ResistDDMagic`, an `AbstractStatPercentEffect`, which merges
/// *multiplicatively* and so is exactly what `getMul` reads.
pub fn calc_magic_success_rate(i: &MagicSuccess) -> i32 {
    let mut lvl_modifier = 1.0f64;
    let mut target_modifier = 1.0f64;
    let mut m_acc_modifier = 1i32;

    if i.pve {
        lvl_modifier = 1.3f64.powi(i.target_level - i.effective_level);

        if let Some(caster_level) = i.caster_player_level
            && !i.target_is_raid
            && i.target_level >= i.min_npc_level_for_magic_penalty
            && (i.target_level - caster_level) >= 3
            && !i.skill_chance_penalty.is_empty()
        {
            let level_diff = (i.target_level - caster_level - 2) as usize;
            target_modifier =
                i.skill_chance_penalty[level_diff.min(i.skill_chance_penalty.len() - 1)];
        }
    } else {
        let m_acc_diff = i.magic_accuracy - i.magic_evasion;
        m_acc_modifier = if m_acc_diff > -20 {
            2
        } else if m_acc_diff > -25 {
            30
        } else if m_acc_diff > -30 {
            60
        } else if m_acc_diff > -35 {
            90
        } else {
            100
        };
    }

    100 - java_round_float(m_acc_modifier as f64 * lvl_modifier * target_modifier * i.res_modifier)
}

/// `Rnd.get(100) < rate` — `roll` is 0-99.
pub fn calc_magic_success(i: &MagicSuccess, roll: i32) -> bool {
    roll < calc_magic_success_rate(i)
}

/// Java `Math.round(float)`, which is `(int) floor(a + 0.5f)` — *not* Rust's
/// `f64::round` (half away from zero). The distinction only shows on exact
/// `.5` values, but the narrowing to `f32` first matters too: `1.3^n` grows
/// past `f32::MAX` around n = 330, where both languages saturate the cast to
/// `Integer.MAX_VALUE` / `i32::MAX`.
fn java_round_float(v: f64) -> i32 {
    ((v as f32) + 0.5f32).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// power 12 (Wind Strike 1), mAtk 100, mDef 60: 77·12·√100/60 = 154.0;
    /// magic crit doubles it.
    #[test]
    fn magic_dam_matches_java_formula() {
        let none = MagicFailure::None;
        let dmg = calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, none, 1.0);
        assert!((dmg - 154.0).abs() < 1e-9);
        let crit = calc_magic_dam(100.0, 60.0, 12.0, true, 2.0, 1.0, none, 1.0);
        assert!((crit - 308.0).abs() < 1e-9);
        // Spiritshot doubles, blessed spiritshot quadruples the base.
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 2.0, none, 1.0) - 308.0).abs() < 1e-9
        );
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 4.0, none, 1.0) - 616.0).abs() < 1e-9
        );
    }

    /// mDef is floored at 1 so a zero-defence target can't divide by zero.
    #[test]
    fn magic_dam_survives_zero_mdef() {
        assert!(
            calc_magic_dam(100.0, 0.0, 12.0, false, 2.0, 1.0, MagicFailure::None, 1.0).is_finite()
        );
    }

    /// Java applies the `MagicFailures` adjustment to the *base* damage and only
    /// then multiplies by `critMod` — so a resisted crit lands on 2, not 1, and
    /// a halved crit keeps the full ×2.
    #[test]
    fn magic_failure_applies_before_the_crit_multiplier() {
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, MagicFailure::Half, 1.0) - 77.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(100.0, 60.0, 12.0, true, 2.0, 1.0, MagicFailure::Half, 1.0) - 154.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(
                100.0,
                60.0,
                12.0,
                false,
                2.0,
                1.0,
                MagicFailure::Resisted,
                1.0
            ) - 1.0)
                .abs()
                < 1e-9
        );
        assert!(
            (calc_magic_dam(
                100.0,
                60.0,
                12.0,
                true,
                2.0,
                1.0,
                MagicFailure::Resisted,
                1.0
            ) - 2.0)
                .abs()
                < 1e-9
        );
    }

    /// A PvE cast with no level gap and no level-78 penalty: `1.3^0 = 1`, so
    /// `rate = 100 - 1 = 99`.
    fn pve_input(target_level: i32, effective_level: i32) -> MagicSuccess<'static> {
        MagicSuccess {
            pve: true,
            target_level,
            effective_level,
            caster_player_level: Some(effective_level),
            target_is_raid: false,
            min_npc_level_for_magic_penalty: 78,
            skill_chance_penalty: &[2.5, 3.0, 3.25, 3.5],
            magic_accuracy: 0,
            magic_evasion: 0,
            res_modifier: 1.0,
        }
    }

    /// `rate = 100 - round(1.3^levelDiff)`. The curve is the whole reason a nuke
    /// on a far-higher-level mob has to fail: by +18 levels the rate is negative,
    /// so `Rnd.get(100) < rate` can never be true.
    #[test]
    fn magic_success_rate_follows_the_1_3_power_curve() {
        assert_eq!(calc_magic_success_rate(&pve_input(40, 40)), 99);
        assert_eq!(calc_magic_success_rate(&pve_input(46, 40)), 95); // 1.3^6  = 4.83  → 5
        assert_eq!(calc_magic_success_rate(&pve_input(49, 40)), 89); // 1.3^9  = 10.6  → 11
        assert_eq!(calc_magic_success_rate(&pve_input(52, 40)), 77); // 1.3^12 = 23.3  → 23
        assert_eq!(calc_magic_success_rate(&pve_input(55, 40)), 49); // 1.3^15 = 51.2  → 51
        assert!(calc_magic_success_rate(&pve_input(58, 40)) < 0); // 1.3^18 = 112.5
    }

    /// Casting *up* the level curve is free: a high-level caster on a low mob
    /// saturates at 100 (`1.3^-n` rounds to 0).
    #[test]
    fn magic_success_rate_saturates_against_lower_targets() {
        assert_eq!(calc_magic_success_rate(&pve_input(20, 40)), 100);
    }

    /// The roll is `Rnd.get(100) < rate`, so a rate of 49 lands on rolls 0-48.
    #[test]
    fn magic_success_roll_is_exclusive() {
        let input = pve_input(55, 40);
        assert!(calc_magic_success(&input, 48));
        assert!(!calc_magic_success(&input, 49));
        // A negative rate can never be rolled under, even at roll 0.
        assert!(!calc_magic_success(&pve_input(58, 40), 0));
    }

    /// `MinNPCLevelForMagicPenalty` (78) gates the extra `targetModifier`. At 78+
    /// with the caster 3+ levels below, the failure term is multiplied by the
    /// penalty table entry at `targetLevel - casterLevel - 2`, clamped to the last.
    #[test]
    fn magic_success_applies_the_level_78_npc_penalty() {
        // Target 78, caster 75 → levelDiff index 1 → 3.0. 1.3^3 = 2.197.
        // 100 - round(2.197 * 3.0) = 100 - round(6.59) = 93.
        let mut input = pve_input(78, 75);
        assert_eq!(calc_magic_success_rate(&input), 93);
        // Same gap against a 77 mob: below the threshold, so no targetModifier.
        // 100 - round(2.197) = 98.
        let below = pve_input(77, 74);
        assert_eq!(calc_magic_success_rate(&below), 98);
        // Raids are exempt from the penalty even at 78+.
        input.target_is_raid = true;
        assert_eq!(calc_magic_success_rate(&input), 98);
    }

    /// An NPC caster (`getActingPlayer() == null`) never picks up the penalty —
    /// Java's null check comes before the level test.
    #[test]
    fn magic_success_npc_caster_skips_the_penalty() {
        let mut input = pve_input(78, 75);
        input.caster_player_level = None;
        assert_eq!(calc_magic_success_rate(&input), 98);
    }

    /// The index clamps to the last table entry rather than panicking on a gap
    /// wider than the table (Java's `levelDiff >= length` branch).
    #[test]
    fn magic_success_penalty_index_clamps() {
        // Target 99, caster 78: index 19, table has 4 entries → 3.5.
        // Both the clamped and the out-of-range case drive the rate negative.
        assert!(calc_magic_success_rate(&pve_input(99, 78)) < 0);
    }

    /// PvP (neither side an `Attackable`) takes the magic-accuracy step table
    /// instead; the level gap is irrelevant there.
    #[test]
    fn magic_success_pvp_uses_the_accuracy_table() {
        let mut input = pve_input(80, 40);
        input.pve = false;
        // mAccDiff 0 > -20 → mAccModifier 2 → rate 98, despite the 40-level gap.
        assert_eq!(calc_magic_success_rate(&input), 98);
        input.magic_evasion = 22; // diff -22 → 30
        assert_eq!(calc_magic_success_rate(&input), 70);
        input.magic_evasion = 27; // diff -27 → 60
        assert_eq!(calc_magic_success_rate(&input), 40);
        input.magic_evasion = 32; // diff -32 → 90
        assert_eq!(calc_magic_success_rate(&input), 10);
        input.magic_evasion = 40; // diff -40 → 100
        assert_eq!(calc_magic_success_rate(&input), 0);
    }

    /// Good skills cap the per-mille rate at 320, bad skills at 200; the
    /// comparison is strict (`rate > roll`).
    #[test]
    fn magic_crit_caps_and_thresholds() {
        assert!(calc_magic_crit(1000.0, false, 40, 40, 319));
        assert!(!calc_magic_crit(1000.0, false, 40, 40, 320));
        assert!(calc_magic_crit(1000.0, true, 40, 40, 199));
        assert!(!calc_magic_crit(1000.0, true, 40, 40, 200));
        assert!(!calc_magic_crit(0.0, false, 40, 40, 0));
    }

    /// The bad-skill cap lifts to 320 once **both** sides are 78+, and the
    /// `sqrt(level)` bonus rides in with it — a good skill never reaches that
    /// branch, since Java returns before it.
    #[test]
    fn magic_crit_lifts_its_cap_for_high_level_pairs() {
        assert!(calc_magic_crit(1000.0, true, 78, 78, 319));
        assert!(!calc_magic_crit(1000.0, true, 78, 78, 320));
        // One side below 78 keeps the 200 cap.
        assert!(!calc_magic_crit(1000.0, true, 78, 77, 200));
        // 100%o at level 81 → 100 + 9 = 109 (the level-gap term is Java's own
        // integer division, 0 for any gap under 25).
        assert!(calc_magic_crit(100.0, true, 81, 78, 108));
        assert!(!calc_magic_crit(100.0, true, 81, 78, 109));
    }
}
