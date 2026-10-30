//! `DefenceCriticalRate` — the defender's side of the autoattack crit roll
//! (G19).
//!
//! Salvation's `Formulas.calcCrit` reads it as a **reduction**: `(rate −
//! DEFENCE_CRITICAL_RATE_ADD) · (1 − min(0.8, DEFENCE_CRITICAL_RATE − 1))`.
//! Light Armor Mastery 233 (`+15% PER`) and Pa'agrio's Eye 1364 (`+30%`) carry
//! positive amounts on this dist, so they push the stat above 1 and strip that
//! share of the attacker's rate. The roll is `Rnd.get(1000)`.

use super::*;

use crate::model::formulas::physical::calc_auto_attack_crit;
use crate::model::movement::Position;
use crate::model::stats::Stat;

const DIST: &str = crate::data::DIST_GAME;

/// `MaxPCritRate`'s default.
const CAP: f64 = 500.0;

fn crits(stat: f64, def_rate: f64, def_add: f64, lvl: i32, tlvl: i32, roll: i32) -> bool {
    calc_auto_attack_crit(
        stat,
        def_rate,
        def_add,
        Position::Front,
        1.0,
        0,
        0,
        lvl,
        tlvl,
        CAP,
        roll,
    )
}

/// With identity defences the rate is the attacker's stat: front position and
/// Java's integer height bonus are both 1, and the levels stay under 78.
#[test]
fn identity_defences_leave_the_attackers_rate() {
    assert!(crits(440.0, 1.0, 0.0, 40, 40, 439));
    assert!(!crits(440.0, 1.0, 0.0, 40, 40, 440));
}

/// The defender's stat strips `value − 1` of the attacker's rate.
#[test]
fn the_defenders_stat_reduces_the_attackers_rate() {
    // Light Armor Mastery's +15% → 1.15: 440 · 0.85 = 374.000…06 in doubles
    // (`1.15 − 1` is a hair under 0.15), so 374 still crits and 375 doesn't.
    assert!(crits(440.0, 1.15, 0.0, 40, 40, 374));
    assert!(!crits(440.0, 1.15, 0.0, 40, 40, 375));
    // Pa'agrio's Eye's +30% → 1.30: 440 · 0.70 = 308.
    assert!(crits(440.0, 1.30, 0.0, 40, 40, 307));
    assert!(!crits(440.0, 1.30, 0.0, 40, 40, 308));
    // A debuff below 1 raises it: 0.8 → 440 · 1.2 = 528, capped at 500.
    assert!(crits(400.0, 0.8, 0.0, 40, 40, 479));
    assert!(!crits(400.0, 0.8, 0.0, 40, 40, 480));
}

/// The reduction never exceeds 80 %.
#[test]
fn the_reduction_stops_at_eighty_percent() {
    // 3.0 would be −200 %; it stops at 440 · 0.2 = 88.
    assert!(crits(440.0, 3.0, 0.0, 40, 40, 87));
    assert!(!crits(440.0, 3.0, 0.0, 40, 40, 88));
}

/// `_ADD` is subtracted from the attacker's rate before the reduction.
#[test]
fn the_add_term_is_subtracted_first() {
    // (440 − 100) · 0.7 = 238.
    assert!(crits(440.0, 1.3, 100.0, 40, 40, 237));
    assert!(!crits(440.0, 1.3, 100.0, 40, 40, 238));
}

/// `MIN_CRIT_RATE` 30 still bounds the result, so no defence takes an attacker
/// below a 3 % chance.
#[test]
fn the_floor_still_bounds_a_heavily_defended_target() {
    assert!(
        crits(440.0, 1.0, 1000.0, 40, 40, 29),
        "floored at 30, so roll 29 crits"
    );
    assert!(
        !crits(440.0, 1.0, 1000.0, 40, 40, 30),
        "and roll 30 does not"
    );
}

/// Java adds `sqrt(attackerLevel) · (attackerLevel − targetLevel) · 0.125`
/// whenever **either** side is 78 or over.
#[test]
fn high_level_attackers_get_the_level_term() {
    // 80 vs 70: 440 + √80 · 10 · 0.125 = 451.18.
    assert!(crits(440.0, 1.0, 0.0, 80, 70, 451));
    assert!(!crits(440.0, 1.0, 0.0, 80, 70, 452));
    // The gate is an OR and the term cuts both ways: 70 vs 78 is
    // 440 + √70 · (−8) · 0.125 = 431.63.
    assert!(crits(440.0, 1.0, 0.0, 70, 78, 431));
    assert!(!crits(440.0, 1.0, 0.0, 70, 78, 432));
}

/// Both carriers parse to the `PER` stat with their real negative amounts.
#[test]
fn real_dist_carriers_parse() {
    assert_eq!(
        stat_value_of(233, 1, Stat::DefenceCriticalRate),
        Some(15.0),
        "Light Armor Mastery is +15%"
    );
    assert_eq!(
        stat_value_of(1364, 1, Stat::DefenceCriticalRate),
        Some(30.0),
        "Pa'agrio's Eye is +30%"
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

    assert_eq!(effect.amount, 15.0);
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
    assert_eq!(effect.amount, 30.0);
    assert_eq!(effect.armor_condition, 0, "no armour gate");
}
