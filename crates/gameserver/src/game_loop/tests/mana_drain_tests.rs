//! `MagicalAttackMp` — MP drain (G19).
//!
//! Mana Burn 1398 and Mana Storm 1399 carry **only** this effect, so before it
//! was ported both parsed to an empty effect list and were dropped whole: the
//! nukes cast, played their animation and drained nothing. Aura Sink 1102 and
//! Seal of Gloom 1210 pair it with an already-ported `ManaDamOverTime`, so they
//! landed but did none of the up-front damage.

use super::*;

use crate::model::formulas::{self, MagicFailure};
use crate::model::skill::SkillEffect;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

// ---------------------------------------------------------------------------
// calcManaDam
// ---------------------------------------------------------------------------

/// `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef` — worth pinning term by
/// term, because it shares no structure with the HP magic formula.
#[test]
fn mana_dam_follows_the_java_formula() {
    // sqrt(100)=10, ×power 20 = 200, ×(970/97 = 10) = 2000, ÷mDef 50 = 40.
    let d = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    assert!((d - 40.0).abs() < 1e-9, "expected 40, got {d}");
}

/// The target's **max MP is a direct multiplier** — the same nuke drains far
/// more from a high-MP mage than from a fighter. This is the term that makes
/// the effect feel different from an HP nuke.
#[test]
fn a_bigger_mp_pool_is_drained_harder() {
    let small = formulas::calc_mana_dam(
        100.0,
        50.0,
        500.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    let big = formulas::calc_mana_dam(
        100.0,
        50.0,
        1000.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    assert!(
        (big - small * 2.0).abs() < 1e-9,
        "double the pool, double the drain: {small} -> {big}"
    );
}

/// A crit triples the drain and then clamps to the skill's own
/// `criticalLimit` — a cap with no equivalent in any HP formula.
#[test]
fn a_crit_triples_the_drain_and_then_clamps_to_the_limit() {
    let base = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    let crit = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        true,
        7000.0,
    );
    assert!((crit - base * 3.0).abs() < 1e-9, "×3 when under the cap");

    // Same crit against a 100 cap: clamped, not tripled.
    let capped = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        true,
        100.0,
    );
    assert!(
        (capped - 100.0).abs() < 1e-9,
        "clamped to criticalLimit, got {capped}"
    );
}

/// Spiritshots scale `mAtk` **before** the square root, so the drain grows by
/// `sqrt(bonus)` rather than the bonus itself — the opposite of how
/// `calc_magic_dam` applies them.
#[test]
fn spiritshots_scale_matk_before_the_square_root() {
    let plain = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    let sps = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        2.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    assert!(
        (sps - plain * 2.0_f64.sqrt()).abs() < 1e-9,
        "×sqrt(2), not ×2: {plain} -> {sps}"
    );
}

/// Java's failure block here only ever *halves* — unlike `calcMagicDam` there
/// is no `damage = 1` floor on a full resist, so both verdicts do the same
/// thing. Ported as written.
#[test]
fn a_resisted_drain_is_halved_not_floored() {
    let full = formulas::calc_mana_dam(
        100.0,
        50.0,
        970.0,
        20.0,
        1.0,
        MagicFailure::None,
        false,
        7000.0,
    );
    for failure in [MagicFailure::Half, MagicFailure::Resisted] {
        let d = formulas::calc_mana_dam(100.0, 50.0, 970.0, 20.0, 1.0, failure, false, 7000.0);
        assert!((d - full / 2.0).abs() < 1e-9, "{failure:?} halves, got {d}");
    }
}

// ---------------------------------------------------------------------------
// calcMagicAffected
// ---------------------------------------------------------------------------

/// The drain's own landing roll: a noisy mAtk-vs-mDef comparison. With the
/// gaussian pinned to 0 the deterministic term decides, so equal attack and
/// defence sits exactly on the boundary (`d > 0` is strict → refused).
#[test]
fn magic_affected_compares_attack_against_defence() {
    // attack = 2*mAtk = 200 vs defence 200 → d = 0, refused (strict >).
    assert!(
        !formulas::calc_magic_affected(100.0, 200.0, 0.0),
        "a dead heat does not land"
    );
    // A clear mAtk edge lands.
    assert!(
        formulas::calc_magic_affected(100.0, 50.0, 0.0),
        "more attack than defence lands"
    );
    // And a clear deficit does not.
    assert!(
        !formulas::calc_magic_affected(10.0, 500.0, 0.0),
        "far more defence than attack fails"
    );
}

/// The gaussian is what makes it a *roll* rather than a threshold: a big enough
/// swing flips either verdict.
#[test]
fn the_gaussian_can_flip_the_verdict_either_way() {
    // Overwhelming attack, but a bad enough draw still fails.
    assert!(
        formulas::calc_magic_affected(100.0, 0.0, 0.0),
        "baseline lands"
    );
    assert!(
        !formulas::calc_magic_affected(100.0, 0.0, -4.0),
        "a bad draw loses it"
    );
    // Overwhelming defence, but a good enough draw lands it.
    assert!(
        !formulas::calc_magic_affected(10.0, 500.0, 0.0),
        "baseline fails"
    );
    assert!(
        formulas::calc_magic_affected(10.0, 500.0, 4.0),
        "a good draw wins it"
    );
}

/// A zero-mAtk actor would divide by zero in Java and get NaN (which compares
/// false); the port guards explicitly and reaches the same verdict.
#[test]
fn a_zero_attack_actor_never_lands_the_drain() {
    assert!(
        !formulas::calc_magic_affected(0.0, 0.0, 5.0),
        "no attack, no drain, whatever the roll"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// All four learnable skills parse, with the real per-skill `criticalLimit`
/// split (1600 on the two debuffs, 7000 on the two nukes).
#[test]
fn real_dist_mana_drain_skills_parse() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for (id, expected_limit) in [
        (1102, 1600.0),
        (1210, 1600.0),
        (1398, 7000.0),
        (1399, 7000.0),
    ] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        let found = skill.effects.iter().find_map(|e| match e {
            SkillEffect::MagicalAttackMp {
                power,
                critical,
                critical_limit,
            } => Some((*power, *critical, *critical_limit)),
            _ => None,
        });
        let (power, critical, limit) =
            found.unwrap_or_else(|| panic!("skill {id} carries MagicalAttackMp"));
        assert!(power > 0.0, "skill {id} has a real power, got {power}");
        assert!(critical, "skill {id} declares critical=true");
        assert!(
            (limit - expected_limit).abs() < 1e-9,
            "skill {id} criticalLimit {expected_limit}, got {limit}"
        );
    }
}

/// Mana Burn 1398 and Mana Storm 1399 carry *only* this effect — which is why
/// they were dropped whole. Pinning it keeps the regression visible.
#[test]
fn the_two_nukes_carry_only_the_drain_effect() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for id in [1398, 1399] {
        let skill = skills.get(id, 1).unwrap();
        assert_eq!(
            skill.effects.len(),
            1,
            "skill {id} has exactly one effect: {:?}",
            skill.effects
        );
        assert!(matches!(
            skill.effects[0],
            SkillEffect::MagicalAttackMp { .. }
        ));
    }
}

/// All four are `<isMagic>1</isMagic>`. That matters for the crit path: Java's
/// `calcCrit` magic branch **discards the `magicCriticalRate` it was passed**
/// and reads the caster's `MAGIC_CRITICAL_RATE` stat instead, so the drain's
/// crit is exactly the port's existing per-cast `mcrit` — gated only by the
/// effect's own `critical` flag.
///
/// (Worth pinning: `magic_type` is parsed from `<isMagic>`, *not* a
/// `<magicType>` tag — which this dist's schema doesn't have at all.)
#[test]
fn the_drain_skills_are_magic_skills() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for id in [1102, 1210, 1398, 1399] {
        assert_eq!(
            skills.get(id, 1).unwrap().magic_type,
            1,
            "skill {id} is a magic skill"
        );
    }
}

/// Aura Sink and Seal of Gloom are debuffs (so their crit caps at 200‰), the
/// two nukes are not (320‰) — the `is_bad` split `calc_magic_crit` keys on.
#[test]
fn the_two_debuffs_and_the_two_nukes_differ_in_badness() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for id in [1102, 1210] {
        assert!(
            skills.get(id, 1).unwrap().is_debuff,
            "skill {id} is a debuff"
        );
    }
}
