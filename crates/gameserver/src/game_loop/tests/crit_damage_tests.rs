//! Critical-damage stats (G19).
//!
//! `CriticalDamage`/`CriticalDamageAdd` were parsed into `StatModifiers` and
//! then read by **nobody** — all three damage formulas hard-coded a ×2 crit.
//! So Death Whisper 1242, Focus Attack 317, Vicious Stance 312, Frenzy 176,
//! Dance of Fire 274 and 13 other learnable skills were completely inert.
//! Found by scanning for `Stat` variants with no consumer, the check the
//! previous slice's post-mortem called for.

use super::*;

use crate::model::components::StatModifiers;
use crate::model::formulas::{self, CritDamage};
use crate::model::movement::Position;
use crate::model::skill::SkillEffect;
use crate::model::stats::{Stat, StatQualifier};

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn dist_skills() -> crate::data::skill_data::SkillData {
    crate::data::skill_data::SkillData::load_from(DIST)
}

/// The `(stat, amount)` pairs a skill contributes with no qualifier.
fn plain_mods(
    skills: &crate::data::skill_data::SkillData,
    id: i32,
    level: i32,
) -> Vec<(Stat, f64)> {
    skills
        .get(id, level)
        .unwrap_or_else(|| panic!("skill {id} loads"))
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) if m.qualifier.is_none() => Some((m.stat, m.amount)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The formula itself
// ---------------------------------------------------------------------------

/// A non-crit swing must not read the crit stats at all — otherwise every
/// crit-damage buff would silently become a flat damage buff.
#[test]
fn crit_stats_do_not_touch_a_normal_hit() {
    let huge = CritDamage {
        mul: 10.0,
        add: 1000.0,
    };
    let plain = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        false,
        CritDamage::default(),
        false,
    );
    let with_stats =
        formulas::calc_auto_attack_damage(100.0, 1.0, Position::Front, 50.0, false, huge, false);
    assert_eq!(
        plain, with_stats,
        "a non-crit ignores cAtk/cAtkAdd entirely"
    );
}

/// `CritDamage::default()` is Java's stat-free `2 * 1 * 1 * 1` / `0`, so the
/// whole refactor is behaviour-preserving for an actor with no crit buffs —
/// which is what every pre-existing damage test relies on.
#[test]
fn default_crit_damage_reproduces_the_old_hard_coded_double() {
    let base = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        false,
        CritDamage::default(),
        false,
    );
    let crit = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage::default(),
        false,
    );
    assert!(
        (crit - base * 2.0).abs() < 1e-9,
        "default crit is exactly ×2: {base} -> {crit}"
    );
}

/// The multiplier scales the crit, and the flat add lands **after** the
/// soulshot multiply and **inside** the ×77 / ÷pDef — Java's bracketing, which
/// is what makes `cAtkAdd` worth far more than its face value.
#[test]
fn crit_multiplier_and_flat_add_follow_javas_bracketing() {
    // pAtk 100, no prox bonus, pDef 50 → base attack term is 100.
    let doubled = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage { mul: 4.0, add: 0.0 },
        false,
    );
    let default_crit = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage::default(),
        false,
    );
    assert!(
        (doubled - default_crit * 2.0).abs() < 1e-9,
        "cAtk 4 is twice cAtk 2"
    );

    // cAtkAdd = 50 → attack becomes (100*2 + 50) = 250, ×77 / 50.
    let with_add = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage {
            mul: 2.0,
            add: 50.0,
        },
        false,
    );
    assert!(
        (with_add - (250.0 * 77.0 / 50.0)).abs() < 1e-9,
        "cAtkAdd lands inside the ×77, got {with_add}"
    );

    // With soulshots the add is applied *after* the ss multiply, so it is
    // NOT doubled: (100*2*2 + 50) rather than ((100*2 + 50)*2).
    let ss = formulas::calc_auto_attack_damage(
        100.0,
        1.0,
        Position::Front,
        50.0,
        true,
        CritDamage {
            mul: 2.0,
            add: 50.0,
        },
        true,
    );
    assert!(
        (ss - (450.0 * 77.0 / 50.0)).abs() < 1e-9,
        "soulshots do not scale cAtkAdd, got {ss}"
    );
}

/// The magic branch takes its own multiplier (`MAGIC_CRITICAL_DAMAGE`), and
/// only when the cast actually crit.
#[test]
fn magic_crit_multiplier_applies_only_on_a_magic_crit() {
    let none = formulas::MagicFailure::None;
    let plain = formulas::calc_magic_dam(100.0, 60.0, 12.0, false, 3.0, 1.0, none);
    let base = formulas::calc_magic_dam(100.0, 60.0, 12.0, false, 2.0, 1.0, none);
    assert_eq!(plain, base, "a non-crit cast ignores the crit multiplier");

    let crit = formulas::calc_magic_dam(100.0, 60.0, 12.0, true, 3.0, 1.0, none);
    assert!(
        (crit - base * 3.0).abs() < 1e-9,
        "a magic crit takes the full multiplier"
    );
}

// ---------------------------------------------------------------------------
// Stat plumbing
// ---------------------------------------------------------------------------

/// `CriticalDamagePosition` is **multiplicative with identity 1.0**, unlike the
/// additive move-type map — mixing the two up would make an unqualified stat
/// read as a ×0 (or a missing one as +1).
#[test]
fn position_qualified_stats_multiply_from_one() {
    let mut mods = StatModifiers::default();
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Back),
        1.0,
        "absent reads as 1.0, not 0.0"
    );

    crate::model::apply_modifier(
        &mut mods,
        &crate::model::skill::StatModifierEffect {
            stat: Stat::CriticalDamage,
            mode: crate::model::stats::StatModifierType::Per,
            amount: 30.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: Some(StatQualifier::Position(Position::Back)),
            two_handed: false,
        },
    );
    assert!(
        mods.mul.is_empty(),
        "a position-qualified effect must not leak into the plain mul map"
    );
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Back) - 1.3).abs() < 1e-9,
        "+30% → ×1.3"
    );
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Front),
        1.0,
        "and only from behind"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// Death Whisper 1242 — the buff this whole slice is really about — parses to a
/// `PER` `CriticalDamage` modifier. Before the consumers landed it pumped this
/// stat and nothing read it.
#[test]
fn death_whisper_grants_a_critical_damage_multiplier() {
    let skills = dist_skills();
    let mods = plain_mods(&skills, 1242, 1);
    let crit = mods
        .iter()
        .find(|(s, _)| *s == Stat::CriticalDamage)
        .expect("Death Whisper pumps CriticalDamage");
    assert!(crit.1 > 0.0, "and by a positive amount, got {}", crit.1);
}

/// A representative spread of the 18 learnable `CriticalDamage` skills all
/// reach `Stat::CriticalDamage` (PER) or `CriticalDamageAdd` (DIFF).
#[test]
fn learnable_critical_damage_skills_all_reach_a_crit_stat() {
    let skills = dist_skills();
    for id in [
        176, 193, 274, 312, 317, 401, 414, 420, 1242, 1253, 1356, 1363,
    ] {
        let mods = plain_mods(&skills, id, 1);
        assert!(
            mods.iter()
                .any(|(s, _)| matches!(s, Stat::CriticalDamage | Stat::CriticalDamageAdd)),
            "skill {id} contributes a crit-damage stat, got {mods:?}"
        );
    }
}

/// Focus Death 355 carries **two** position-qualified entries with opposite
/// signs — front `-30%` and back `+90%` — so the skill makes you worse at
/// crit-damage head-on and far better from behind. That asymmetry is the
/// whole point of the effect, and it only survives because the position map is
/// multiplicative: `-30` becomes ×0.7, not a subtraction.
#[test]
fn focus_death_penalises_frontal_crits_and_rewards_backstabs() {
    let skills = dist_skills();
    let qualified: Vec<_> = skills
        .get(355, 1)
        .expect("Focus Death loads")
        .effects
        .iter()
        .filter_map(|e| match e {
            SkillEffect::StatModifier(m) => match m.qualifier {
                Some(StatQualifier::Position(p)) => Some((m.stat, p, m.amount)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        qualified,
        vec![
            (Stat::CriticalDamage, Position::Front, -30.0),
            (Stat::CriticalDamage, Position::Back, 90.0),
        ],
        "both halves of the effect parse, with their real signs"
    );

    // And they fold to the multipliers the formula reads.
    let mut mods = StatModifiers::default();
    for e in &skills.get(355, 1).unwrap().effects {
        if let SkillEffect::StatModifier(m) = e {
            crate::model::apply_modifier(&mut mods, m);
        }
    }
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Front) - 0.7).abs() < 1e-9,
        "front -30% → ×0.7"
    );
    assert!(
        (mods.position_value(Stat::CriticalDamage, Position::Back) - 1.9).abs() < 1e-9,
        "back +90% → ×1.9"
    );
    assert_eq!(
        mods.position_value(Stat::CriticalDamage, Position::Side),
        1.0,
        "side is untouched"
    );
}

/// Prophecy of Wind 1357 grants the magic-crit multiplier — the one branch
/// besides autoattacks with a real learnable grantor.
#[test]
fn prophecy_of_wind_grants_magic_critical_damage() {
    let skills = dist_skills();
    let mods = plain_mods(&skills, 1357, 1);
    assert!(
        mods.iter().any(|(s, _)| *s == Stat::MagicCriticalDamage),
        "Prophecy of Wind pumps MagicCriticalDamage, got {mods:?}"
    );
}

/// End to end through the passive path: a player who has learned a
/// `CriticalDamage` skill carries the multiplier in `StatModifiers.mul`, which
/// is exactly what `crit_damage_auto` reads.
#[test]
fn learned_crit_damage_passive_folds_into_stat_modifiers() {
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = Player::from_char(&world.data, &dummy_char(4301, "Bare"));
    assert_eq!(
        bare.stat_modifiers.add.get(&Stat::CriticalDamageAdd),
        None,
        "no skill: no modifier at all"
    );

    // Skill 193 "Critical Damage" — a genuine `operateType=P` passive, and a
    // `mode=DIFF` one, so it feeds the *flat* `CriticalDamageAdd` rather than
    // the multiplier. (Most of the headline crit skills — Vicious Stance 312,
    // Focus Attack 317 — are *toggles*, which land through the buff path and
    // so are correctly absent from a freshly built `Player`.)
    let mut chr = dummy_char(4302, "Crit");
    chr.skills = vec![(193, 1, 0)];
    let bundle = Player::from_char(&world.data, &chr);
    let add = bundle
        .stat_modifiers
        .add
        .get(&Stat::CriticalDamageAdd)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (add - 32.0).abs() < 1e-9,
        "Critical Damage lvl 1 is a flat +32 cAtkAdd, got {add}"
    );
    // Which, per the bracketing test above, is worth 32·77/pDef on a crit —
    // far more than its face value suggests.
    assert_eq!(
        bundle.stat_modifiers.mul.get(&Stat::CriticalDamage),
        None,
        "a DIFF effect never touches the multiplier"
    );
}
