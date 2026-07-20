//! `DefenceCriticalRate` — the defender's side of the autoattack crit roll
//! (G19).
//!
//! The mirror of the crit-*damage* slice: Light Armor Mastery 233 (`-15% PER`)
//! and Pa'agrio's Eye 1364 (`-30%`) make their wearer harder to crit. The port
//! computed the crit chance as a bare `crit_stat / 10`, so both were inert.

use super::*;

use crate::model::formulas::calc_auto_attack_crit;
use crate::model::movement::Position;
use crate::model::stats::Stat;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn dist_skills() -> crate::data::skill_data::SkillData {
    crate::data::skill_data::SkillData::load_from(DIST)
}

/// The identity defaults reproduce exactly what the formula did before, which
/// is what keeps every existing combat test honest.
///
/// The arithmetic below folds in two bonuses that are **not** 1.0 even in the
/// plainest case: `calc_critical_height_bonus(0, 0)` is **1.1** (Java's `+10`
/// before the `/100`), and only the front position bonus is 1.0. So crit_stat
/// 440 gives `(440/10) * 1.0 * 1.1 = 48.4`, not 44.
#[test]
fn identity_defences_reproduce_the_old_formula() {
    assert!(calc_auto_attack_crit(440.0, 1.0, 0.0, Position::Front, 0, 0, 48));
    assert!(!calc_auto_attack_crit(440.0, 1.0, 0.0, Position::Front, 0, 0, 49));
}

/// A defender's multiplier scales the **attacker's** rate — that is what the
/// two-arg `getValue(DEFENCE_CRITICAL_RATE, rate)` means, and getting it
/// backwards would make the stat a flat chance instead of a reduction.
#[test]
fn the_defenders_multiplier_scales_the_attackers_rate() {
    // Light Armor Mastery's -15% → x0.85: (374/10) * 1.1 = 41.14.
    assert!(calc_auto_attack_crit(440.0, 0.85, 0.0, Position::Front, 0, 0, 41));
    assert!(!calc_auto_attack_crit(440.0, 0.85, 0.0, Position::Front, 0, 0, 42));
    // Pa'agrio's Eye's -30% → x0.70: (308/10) * 1.1 = 33.88.
    assert!(calc_auto_attack_crit(440.0, 0.70, 0.0, Position::Front, 0, 0, 33));
    assert!(!calc_auto_attack_crit(440.0, 0.70, 0.0, Position::Front, 0, 0, 34));
}

/// The `_ADD` term is applied after the multiply and before the `/10`, so it is
/// worth ten times its face value in percentage points.
#[test]
fn the_add_term_lands_before_the_divide() {
    // ((1.0 * 440) + 100) / 10 = 54, then x1.1 = 59.4.
    assert!(calc_auto_attack_crit(440.0, 1.0, 100.0, Position::Front, 0, 0, 59));
    assert!(!calc_auto_attack_crit(440.0, 1.0, 100.0, Position::Front, 0, 0, 60));
}

/// The 3..97 clamp still bounds the result, so no defence stat can take an
/// attacker below a 3% chance.
#[test]
fn the_clamp_still_bounds_a_heavily_defended_target() {
    assert!(calc_auto_attack_crit(440.0, 0.0, 0.0, Position::Front, 0, 0, 2), "floored at 3, so roll 2 crits");
    assert!(!calc_auto_attack_crit(440.0, 0.0, 0.0, Position::Front, 0, 0, 3), "and roll 3 does not");
}

/// Both carriers parse to the `PER` stat with their real negative amounts.
#[test]
fn real_dist_carriers_parse() {
    let skills = dist_skills();
    let amount_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.stat_modifier_effects()
                .iter()
                .find(|m| m.stat == Stat::DefenceCriticalRate)
                .map(|m| m.amount)
        })
    };
    assert_eq!(amount_of(233, 1), Some(-15.0), "Light Armor Mastery is -15%");
    assert_eq!(amount_of(1364, 1), Some(-30.0), "Pa'agrio's Eye is -30%");
}

/// Light Armor Mastery is **armor-conditioned** — Java gates it on
/// `<armorType>LIGHT</armorType>`, and this port folds conditioned passives
/// only when the gear matches. So a naked character correctly gets *nothing*,
/// and the modifier has to be checked at the parsed-effect level instead.
///
/// (My first version of this test expected it to fold with no armour equipped
/// and failed — the gate is right, the expectation was not.)
#[test]
fn light_armor_mastery_is_armor_conditioned() {
    let skills = dist_skills();
    let effect = skills
        .get(233, 1)
        .expect("Light Armor Mastery loads")
        .stat_modifier_effects()
        .into_iter()
        .find(|m| m.stat == Stat::DefenceCriticalRate)
        .expect("it carries the defence-crit modifier");

    assert_eq!(effect.amount, -15.0);
    assert_ne!(effect.armor_condition, 0, "gated on wearing light armour");

    // A naked character gets no modifier at all — the gate doing its job.
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = crate::data::player_template::PlayerTemplateData::load_from(DIST);
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = crate::data::item_data::ItemData::load_from(DIST);
    data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let mut chr = dummy_char(9702, "Light");
    chr.skills = vec![(233, 1)];
    let bundle = Player::from_char(&world.data, &chr);
    assert_eq!(
        bundle.stat_modifiers.mul.get(&Stat::DefenceCriticalRate),
        None,
        "no light armour equipped, so the passive contributes nothing"
    );
}

/// Pa'agrio's Eye 1364 is **not** armor-conditioned, so it folds
/// unconditionally — the contrast that shows the gate above is the skill's
/// own property and not a limitation of the plumbing.
#[test]
fn paagrios_eye_folds_unconditionally() {
    let skills = dist_skills();
    let effect = skills
        .get(1364, 1)
        .expect("Pa'agrio's Eye loads")
        .stat_modifier_effects()
        .into_iter()
        .find(|m| m.stat == Stat::DefenceCriticalRate)
        .expect("it carries the defence-crit modifier");
    assert_eq!(effect.amount, -30.0);
    assert_eq!(effect.armor_condition, 0, "no armour gate");
}
