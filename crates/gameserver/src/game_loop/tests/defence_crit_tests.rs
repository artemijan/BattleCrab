//! `DefenceCriticalRate` — the defender's side of the autoattack crit roll
//! (G19).
//!
//! The mirror of the crit-*damage* slice: Light Armor Mastery 233 (`-15% PER`)
//! and Pa'agrio's Eye 1364 (`-30%`) make their wearer harder to crit. The port
//! computed the crit chance as a bare `crit_stat / 10`, so both were inert.

use super::*;

use crate::model::formulas::physical::calc_auto_attack_crit;
use crate::model::movement::Position;
use crate::model::stats::Stat;

const DIST: &str = crate::data::DIST_GAME;

/// The identity defaults reproduce exactly what the formula did before, which
/// is what keeps every existing combat test honest.
///
/// The numbers below were once inflated by a height bonus of **1.1**: the port
/// evaluated Java's `((z*4/5 + 10) / 100) + 1` in floating point, where Java
/// evaluates it in `int` and gets a flat 1 for every z (see
/// `calc_critical_height_bonus`). Front position is 1.0 as well, so crit_stat
/// 440 is `(440/10) * 1.0 * 1.0 = 44`.
///
/// The levels are both sub-78, which keeps `calcCrit`'s level-difference term
/// out of these rows; `high_level_attackers_get_the_level_term` covers it.
#[test]
fn identity_defences_reproduce_the_old_formula() {
    assert!(calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        43
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        44
    ));
}

/// A defender's multiplier scales the **attacker's** rate — that is what the
/// two-arg `getValue(DEFENCE_CRITICAL_RATE, rate)` means, and getting it
/// backwards would make the stat a flat chance instead of a reduction.
#[test]
fn the_defenders_multiplier_scales_the_attackers_rate() {
    // Light Armor Mastery's -15% → x0.85: (374/10) = 37.4.
    assert!(calc_auto_attack_crit(
        440.0,
        0.85,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        37
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        0.85,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        38
    ));
    // Pa'agrio's Eye's -30% → x0.70: (308/10) = 30.8.
    assert!(calc_auto_attack_crit(
        440.0,
        0.70,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        30
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        0.70,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        31
    ));
}

/// The `_ADD` term is applied after the multiply and before the `/10`, so it is
/// worth ten times its face value in percentage points.
#[test]
fn the_add_term_lands_before_the_divide() {
    // ((1.0 * 440) + 100) / 10 = 54.
    assert!(calc_auto_attack_crit(
        440.0,
        1.0,
        100.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        53
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        1.0,
        100.0,
        Position::Front,
        1.0,
        0,
        0,
        40,
        40,
        54
    ));
}

/// The 3..97 clamp still bounds the result, so no defence stat can take an
/// attacker below a 3% chance.
#[test]
fn the_clamp_still_bounds_a_heavily_defended_target() {
    assert!(
        calc_auto_attack_crit(440.0, 0.0, 0.0, Position::Front, 1.0, 0, 0, 40, 40, 2),
        "floored at 3, so roll 2 crits"
    );
    assert!(
        !calc_auto_attack_crit(440.0, 0.0, 0.0, Position::Front, 1.0, 0, 0, 40, 40, 3),
        "and roll 3 does not"
    );
}

/// Java adds `sqrt(attackerLevel) * (attackerLevel - targetLevel) * 0.125`
/// whenever **either** side is 78 or over — the port had no such term, so an
/// 80 fighting anything below itself was critting at the low-level rate.
#[test]
fn high_level_attackers_get_the_level_term() {
    // 80 vs 70: 44 + sqrt(80)*10*0.125 = 44 + 11.18 = 55.18.
    assert!(calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        80,
        70,
        55
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        80,
        70,
        56
    ));
    // It cuts the other way, and the gate is an OR: a level-70 attacker
    // swinging at a level-78 target loses the same 11.18 points.
    // 44 + sqrt(70)*(-8)*0.125 = 44 - 8.37 = 35.63.
    assert!(calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        70,
        78,
        35
    ));
    assert!(!calc_auto_attack_crit(
        440.0,
        1.0,
        0.0,
        Position::Front,
        1.0,
        0,
        0,
        70,
        78,
        36
    ));
}

/// Both carriers parse to the `PER` stat with their real negative amounts.
#[test]
fn real_dist_carriers_parse() {
    assert_eq!(
        stat_value_of(233, 1, Stat::DefenceCriticalRate),
        Some(-15.0),
        "Light Armor Mastery is -15%"
    );
    assert_eq!(
        stat_value_of(1364, 1, Stat::DefenceCriticalRate),
        Some(-30.0),
        "Pa'agrio's Eye is -30%"
    );
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
    let skills = dist::skills_owned();
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
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let mut chr = dummy_char(9702, "Light");
    chr.skills = vec![(233, 1, 0)];
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
    let skills = dist::skills_owned();
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
