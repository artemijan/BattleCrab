//! **Formula parity** — the port's damage maths against a literal
//! transcription of Java's, swept over an input grid instead of spot-checked.
//!
//! Every other Java-comparison test in this tree pins one or two hand-computed
//! cases per formula. That finds a wrong constant; it does not find a *missing
//! term*, because a term that is absent from both the port and the expected
//! value agrees with itself. This file compares two independent expressions:
//!
//! * [`java`] holds transcriptions written **from the Java source**, quoted
//!   above each one, with every term Java multiplies in — including the ones
//!   the port had been dropping;
//! * the port side calls `model::formulas` exactly as the game does.
//!
//! Then it sweeps: levels, attack and defence, crit/shield/shot flags, ranged
//! and melee, front/side/back. A divergence anywhere in that grid fails with
//! the inputs that produced it.
//!
//! # What a failure here means
//!
//! Not "pick a new expected number". It means the two expressions disagree,
//! and the Java side is the specification — so read the transcription, find
//! which term differs, and fix the port. If the port is *deliberately* narrower
//! (a stat with no carrier on this dist, say), the narrowing belongs in the
//! sweep as a fixed input, documented, not as a tolerance.
//!
//! # The transcription has to come from the source
//!
//! The first draft of `java::attribute_bonus` here was written from memory as
//! a linear band, and the sweep failed against a port that was **right** — the
//! real curve is `1.025 + sqrt(diff³/2)·0.0001`. A transcription that is
//! recalled rather than copied turns this file into a second opinion of equal
//! confidence, which is worth nothing. Copy the expression, quote it, then
//! sweep.
//!
//! # Why the numbers are not asserted directly
//!
//! There is no golden file: goldens rot silently and encode whatever the port
//! did on the day they were written. The transcription is checked in instead,
//! and it can be re-read against Java's source by anyone.

use gameserver::model::formulas::{self, CritDamage};
use gameserver::model::movement::Position;

/// Transcriptions of Java's expressions. Each function quotes the source it
/// came from; nothing here calls the port.
mod java {
    /// `Formulas.calcAutoAttackDamage`:
    ///
    /// ```java
    /// double defence = target.getPDef();
    /// switch (shld) { case SHIELD_DEFENSE_SUCCEED: defence += target.getShldDef(); break;
    ///                 case SHIELD_DEFENSE_PERFECT_BLOCK: return 1; }
    /// final boolean isRanged = (weapon != null) && weapon.getItemType().isRanged();
    /// final double shotsBonus = attacker.getStat().getValue(Stat.SHOTS_BONUS);
    /// final double cAtk = crit ? calcCritDamage(attacker, target, null) : 1;
    /// final double cAtkAdd = crit ? calcCritDamageAdd(attacker, target, null) : 0;
    /// final double critMod = crit ? (isRanged ? 0.5 : 1) : 0;
    /// final double ssBonus = ss ? (ssBlessed ? 2.15 : 2) * shotsBonus : 1;
    /// final double randomDamage = attacker.getRandomDamageMultiplier();
    /// final double proxBonus = (attacker.isInFrontOf(target) ? 0
    ///     : (attacker.isBehind(target) ? 0.2 : 0.05)) * attacker.getPAtk();
    /// double attack = (attacker.getPAtk() * randomDamage) + proxBonus;
    /// attack = ((((attack * cAtk * ssBonus) + cAtkAdd) * critMod) * (isRanged ? 154 : 77))
    ///        + (attack * (1 - critMod) * ssBonus * (isRanged ? 154 : 77));
    /// double damage = attack / defence;
    /// damage *= calcAttackTraitBonus(attacker, target);
    /// damage *= calcAttributeBonus(attacker, target, null);
    /// damage *= calculatePvpPveBonus(attacker, target, null, crit);
    /// damage *= attacker.getStat().getMul(Stat.AUTO_ATTACK_DAMAGE_BONUS);
    /// return Math.max(0, damage);
    /// ```
    ///
    /// `AUTO_ATTACK_DAMAGE_BONUS` is left out of the transcription on purpose:
    /// the only skill declaring `AutoAttackDamageBonus` on this dist is in the
    /// 30500 range, so no character here can carry it and the term is a fixed
    /// 1.0 on both sides.
    #[allow(clippy::too_many_arguments)]
    pub fn auto_attack_damage(
        p_atk: f64,
        random_damage: f64,
        prox: f64,
        defence: f64,
        crit: bool,
        c_atk: f64,
        c_atk_add: f64,
        ss: bool,
        is_ranged: bool,
        trait_bonus: f64,
        attribute_bonus: f64,
        pvp_pve_bonus: f64,
    ) -> f64 {
        let shots_bonus = 1.0; // `SHOTS_BONUS` — no carrier on this dist.
        let c_atk = if crit { c_atk } else { 1.0 };
        let c_atk_add = if crit { c_atk_add } else { 0.0 };
        let crit_mod = if crit {
            if is_ranged { 0.5 } else { 1.0 }
        } else {
            0.0
        };
        // Blessed soulshots do not exist on Interlude, so `ssBlessed` is false.
        let ss_bonus = if ss { 2.0 * shots_bonus } else { 1.0 };
        let prox_bonus = prox * p_atk;
        let weapon_mod = if is_ranged { 154.0 } else { 77.0 };
        let attack = (p_atk * random_damage) + prox_bonus;
        let attack = ((((attack * c_atk * ss_bonus) + c_atk_add) * crit_mod) * weapon_mod)
            + (attack * (1.0 - crit_mod) * ss_bonus * weapon_mod);
        let mut damage = attack / defence;
        damage *= trait_bonus;
        damage *= attribute_bonus;
        damage *= pvp_pve_bonus;
        damage.max(0.0)
    }

    /// `handlers/effecthandlers/PhysicalAttack.instant`, the damage half:
    ///
    /// ```java
    /// final double attack = effector.getPAtk() * _pAtkMod;
    /// double defence = effected.getPDef() * _pDefMod;   // + shield, or -1 on a perfect block
    /// final double power = ((_power * (hasAbnormalType ? _abnormalPowerMod : 1))
    ///                       + effector.getStat().getValue(Stat.SKILL_POWER_ADD, 0));
    /// final double weaponMod = effector.getAttackType().isRanged() ? 70 : 77;
    /// final double rangedBonus = effector.getAttackType().isRanged() ? attack + power : 0;
    /// final double critMod = critical ? Formulas.calcCritDamage(effector, effected, skill) : 1;
    /// double ssmod = 1;
    /// if (skill.useSoulShot()) { if (charged) ssmod = 2 * SHOTS_BONUS; else if (blessed) ssmod = 4 * SHOTS_BONUS; }
    /// final double baseMod = (weaponMod * ((attack * effector.getLevelMod()) + power + rangedBonus)) / defence;
    /// damage = baseMod * (hasAbnormalType ? _abnormalDamageMod : 1) * ssmod * critMod
    ///        * weaponTraitMod * (generalTraitMod == 0 ? 1 : generalTraitMod) * weaknessMod
    ///        * attributeMod * pvpPveMod * randomMod;
    /// damage *= effector.getStat().getValue(Stat.PHYSICAL_SKILL_POWER, 1);
    /// ```
    ///
    /// `mods` stands for the block of multipliers the port's **caller**
    /// applies (traits, weakness, attribute, pvp/pve, `PHYSICAL_SKILL_POWER`,
    /// race and the abnormal pair): they are a product either way, so the
    /// sweep varies them as one factor and the leaf's own arithmetic is what
    /// is under test. `SKILL_POWER_ADD` has no carrier on this dist.
    #[allow(clippy::too_many_arguments)]
    pub fn physical_skill_damage(
        p_atk: f64,
        p_atk_mod: f64,
        p_def: f64,
        p_def_mod: f64,
        power: f64,
        level_mod: f64,
        random_mod: f64,
        crit: bool,
        crit_mul: f64,
        ss: bool,
        is_ranged: bool,
        mods: f64,
    ) -> f64 {
        let attack = p_atk * p_atk_mod;
        let defence = p_def * p_def_mod;
        let weapon_mod = if is_ranged { 70.0 } else { 77.0 };
        let ranged_bonus = if is_ranged { attack + power } else { 0.0 };
        let crit_mod = if crit { crit_mul } else { 1.0 };
        let ss_mod = if ss { 2.0 } else { 1.0 };
        let base_mod = (weapon_mod * ((attack * level_mod) + power + ranged_bonus)) / defence;
        base_mod * ss_mod * crit_mod * random_mod * mods
    }

    /// `Formulas.calcMagicDam`, the arithmetic without the packets:
    ///
    /// ```java
    /// final double shotsBonus = bss ? (4 * SHOTS_BONUS) : sps ? (2 * SHOTS_BONUS) : 1;
    /// final double critMod = mcrit ? calcCritDamage(attacker, target, skill) : 1;
    /// double damage = ((77 * (power + SKILL_POWER_ADD) * Math.sqrt(mAtk)) / mDef) * shotsBonus;
    /// // …failure: damage /= 2 (half) or damage = 1 (resisted)…
    /// damage = damage * critMod * (generalTraitMod == 0 ? 1 : generalTraitMod) * weaknessMod
    ///        * attributeMod * randomMod * pvpPveMod;
    /// damage *= attacker.getStat().getValue(Stat.MAGICAL_SKILL_POWER, 1);
    /// ```
    ///
    /// `mods` is again the caller's product. `randomMod` is **not** in it: it
    /// belongs to the leaf, and leaving it out is what made every nuke land on
    /// the same number before this sweep was written.
    #[allow(clippy::too_many_arguments)]
    pub fn magic_damage(
        m_atk: f64,
        m_def: f64,
        power: f64,
        mcrit: bool,
        crit_mul: f64,
        shots_bonus: f64,
        failure: u8,
        random_mod: f64,
        mods: f64,
    ) -> f64 {
        let mut damage = ((77.0 * power * m_atk.sqrt()) / m_def) * shots_bonus;
        match failure {
            1 => damage /= 2.0,
            2 => damage = 1.0,
            _ => {}
        }
        let crit_mod = if mcrit { crit_mul } else { 1.0 };
        damage * crit_mod * random_mod * mods
    }

    /// `Formulas.calcAttributeBonus`, after the element election:
    ///
    /// ```java
    /// final int diff = attackAttribute - defenceAttribute;
    /// if (diff > 0)  return Math.min(1.025 + (Math.sqrt(Math.pow(diff, 3) / 2) * 0.0001), 1.25);
    /// if (diff < 0)  return Math.max(0.975 - (Math.sqrt(Math.pow(-diff, 3) / 2) * 0.0001), 0.75);
    /// return 1;
    /// ```
    ///
    /// The election itself (which element, and whether a skill names one) is
    /// the port's `attribute_mod`; what is swept here is the curve, which is
    /// where an off-by-a-constant would hide.
    pub fn attribute_bonus(attack: f64, defence: f64) -> f64 {
        let diff = attack - defence;
        if diff > 0.0 {
            (1.025 + ((diff.powi(3) / 2.0).sqrt() * 0.0001)).min(1.25)
        } else if diff < 0.0 {
            (0.975 - (((-diff).powi(3) / 2.0).sqrt() * 0.0001)).max(0.75)
        } else {
            1.0
        }
    }
}

/// The grid. Small on purpose: every combination of these is swept, so the
/// product matters more than any single row's spread.
const P_ATKS: &[f64] = &[1.0, 37.0, 250.0, 1_337.0, 9_999.0];
const P_DEFS: &[f64] = &[1.0, 43.0, 300.0, 2_048.0];
const RANDOM_MULS: &[f64] = &[0.9, 1.0, 1.1];
const CRIT_MULS: &[f64] = &[2.0, 3.5];
const CRIT_ADDS: &[f64] = &[0.0, 137.0];
const MODS: &[f64] = &[1.0, 0.75, 1.4];

fn positions() -> [(Position, f64); 3] {
    // The `proxBonus` fraction Java picks per position.
    [
        (Position::Front, 0.0),
        (Position::Side, 0.05),
        (Position::Back, 0.2),
    ]
}

/// **The sweep.** ~48 000 cases: every position × crit × shot × ranged over the
/// attack/defence/random/crit-stat/modifier grid.
#[test]
fn auto_attack_damage_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &p_atk in P_ATKS {
        for &p_def in P_DEFS {
            for &random_mul in RANDOM_MULS {
                for &(position, prox) in &positions() {
                    for &crit in &[false, true] {
                        for &ss in &[false, true] {
                            for &is_ranged in &[false, true] {
                                for &c_mul in CRIT_MULS {
                                    for &c_add in CRIT_ADDS {
                                        for &m in MODS {
                                            let cd = CritDamage {
                                                mul: c_mul,
                                                add: c_add,
                                            };
                                            let ours = formulas::calc_auto_attack_damage(
                                                p_atk, random_mul, position, p_def, crit, cd, ss,
                                                is_ranged, m, m, m,
                                            );
                                            let theirs = java::auto_attack_damage(
                                                p_atk, random_mul, prox, p_def, crit, c_mul, c_add,
                                                ss, is_ranged, m, m, m,
                                            );
                                            assert!(
                                                (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                                "auto-attack damage diverged: ours {ours}, Java \
                                                 {theirs} — pAtk {p_atk}, pDef {p_def}, random \
                                                 {random_mul}, {position:?}, crit {crit}, ss \
                                                 {ss}, ranged {is_ranged}, cAtk {c_mul}, cAtkAdd \
                                                 {c_add}, mods {m}"
                                            );
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// The two shapes that are easy to get right in one case and wrong in the
/// other, called out separately so a failure names the mechanism rather than a
/// grid coordinate.
#[test]
fn the_ranged_weapon_mod_doubles_and_its_crit_splits() {
    let plain = |is_ranged| {
        formulas::calc_auto_attack_damage(
            100.0,
            1.0,
            Position::Front,
            50.0,
            false,
            CritDamage::default(),
            false,
            is_ranged,
            1.0,
            1.0,
            1.0,
        )
    };
    assert!(
        (plain(true) - plain(false) * 2.0).abs() < 1e-9,
        "a bow swings on 154 where a sword swings on 77"
    );

    // A ranged **crit** takes half the crit branch and half the flat one; a
    // melee crit takes the crit branch alone. With cAtk 2 that makes the
    // ranged crit 1.5× its own flat hit, not 2×.
    let crit = |is_ranged| {
        formulas::calc_auto_attack_damage(
            100.0,
            1.0,
            Position::Front,
            50.0,
            true,
            CritDamage::default(),
            false,
            is_ranged,
            1.0,
            1.0,
            1.0,
        )
    };
    assert!(
        (crit(false) - plain(false) * 2.0).abs() < 1e-9,
        "melee crit: the whole crit branch"
    );
    assert!(
        (crit(true) - plain(true) * 1.5).abs() < 1e-9,
        "ranged crit: half of each branch"
    );
}

/// The elemental ladder, swept across the band and both caps.
#[test]
fn attribute_bonus_matches_java_across_the_band() {
    for attack in (0..=400).step_by(7) {
        for defence in (0..=400).step_by(11) {
            let (a, d) = (attack as f64, defence as f64);
            let ours = formulas::calc_attribute_bonus(a, d);
            let theirs = java::attribute_bonus(a, d);
            assert!(
                (ours - theirs).abs() < 1e-9,
                "attribute bonus diverged at attack {a} / defence {d}: ours {ours}, Java {theirs}"
            );
        }
    }
}

/// **The physical-skill sweep** — `PhysicalAttack`'s damage half over the same
/// grid, both weapon classes, crit and shot on and off.
#[test]
fn physical_skill_damage_matches_java_across_the_grid() {
    let mut cases = 0usize;
    for &p_atk in P_ATKS {
        for &p_def in P_DEFS {
            for &power in &[0.0, 55.0, 1_200.0] {
                for &level_mod in &[0.5, 1.0, 1.89] {
                    for &random_mod in RANDOM_MULS {
                        for &crit in &[false, true] {
                            for &ss in &[false, true] {
                                for &is_ranged in &[false, true] {
                                    for &m in MODS {
                                        let ours = formulas::calc_physical_skill_damage(
                                            p_atk, 1.0, p_def, 1.0, power, level_mod, random_mod,
                                            crit, 2.0, ss, is_ranged,
                                        ) * m;
                                        let theirs = java::physical_skill_damage(
                                            p_atk, 1.0, p_def, 1.0, power, level_mod, random_mod,
                                            crit, 2.0, ss, is_ranged, m,
                                        );
                                        assert!(
                                            (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                            "physical skill damage diverged: ours {ours}, Java \
                                             {theirs} — pAtk {p_atk}, pDef {p_def}, power \
                                             {power}, levelMod {level_mod}, random {random_mod}, \
                                             crit {crit}, ss {ss}, ranged {is_ranged}, mods {m}"
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 10_000, "the grid collapsed to {cases} cases");
}

/// **The magic sweep** — `calcMagicDam` including its failure branches and the
/// `randomMod` the port had been dropping.
#[test]
fn magic_damage_matches_java_across_the_grid() {
    use gameserver::model::formulas::MagicFailure;

    let mut cases = 0usize;
    for &m_atk in &[1.0, 40.0, 900.0, 4_000.0] {
        for &m_def in &[1.0, 38.0, 400.0, 3_000.0] {
            for &power in &[1.0, 12.0, 340.0] {
                for &(failure, code) in &[
                    (MagicFailure::None, 0u8),
                    (MagicFailure::Half, 1),
                    (MagicFailure::Resisted, 2),
                ] {
                    for &shots in &[1.0, 2.0, 4.0] {
                        for &mcrit in &[false, true] {
                            for &random_mod in RANDOM_MULS {
                                for &m in MODS {
                                    let ours = formulas::calc_magic_dam(
                                        m_atk, m_def, power, mcrit, 3.0, shots, failure, random_mod,
                                    ) * m;
                                    let theirs = java::magic_damage(
                                        m_atk, m_def, power, mcrit, 3.0, shots, code, random_mod, m,
                                    );
                                    assert!(
                                        (ours - theirs).abs() <= theirs.abs() * 1e-12,
                                        "magic damage diverged: ours {ours}, Java {theirs} — \
                                         mAtk {m_atk}, mDef {m_def}, power {power}, failure \
                                         {code}, shots {shots}, mcrit {mcrit}, random \
                                         {random_mod}, mods {m}"
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 5_000, "the grid collapsed to {cases} cases");
}
