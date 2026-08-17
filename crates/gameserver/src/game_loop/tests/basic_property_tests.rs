//! Mesmerizing-debuff resistance — G34 S2 (`game_loop::basic_property`).
//!
//! Chain-stunning a mob gets harder and then stops working; chain-stunning a
//! *player* does not, because `Player.hasBasicPropertyResist()` is
//! `isInCategory(SIXTH_CLASS_GROUP)` and that category holds only awakened
//! classes this chronicle doesn't have. Getting that asymmetry backwards
//! silently rewrites PvP, so it is asserted directly.

use super::*;
use crate::game_loop::basic_property::{
    RESIST_DURATION_TICKS, has_resist, increase_resist_level, resist_bonus, resist_level,
};
use crate::model::skill::BasicProperty;

const MOB: i32 = crate::model::npc::FIRST_NPC_OBJECT_ID + 7701;
const PLAYER: i32 = 5801;

fn world_with_mob() -> World {
    let (mut world, ..) = test_world();
    // Through the shared helper: it `spawn`s the object into the ECS (a bare
    // `add_components` on an unregistered id silently no-ops) and reserves the
    // id against the runtime allocator ([[l2r-test-npc-oid-collision]]).
    add_test_npc(&mut world, MOB, 20001, "Monster", 50, 0, 0, 0);
    world
}

/// The ladder itself: 1.0 → 0.6 → 0.3 → 0, and **0 is a hard immunity**, not a
/// rate the clamp floor rescues — Java multiplies `basicPropertyResist` in
/// *after* `constrain(rate, minChance, maxChance)`.
#[test]
fn the_resist_ladder_is_1_0_6_0_3_then_immune() {
    let mut world = world_with_mob();

    assert_eq!(resist_bonus(&world, MOB, BasicProperty::Physical), 1.0);
    for (expected_level, expected_bonus) in [(1, 0.6), (2, 0.3), (3, 0.0), (4, 0.0)] {
        increase_resist_level(&mut world, MOB, BasicProperty::Physical);
        assert_eq!(
            resist_level(&world, MOB, BasicProperty::Physical),
            expected_level
        );
        assert_eq!(
            resist_bonus(&world, MOB, BasicProperty::Physical),
            expected_bonus,
            "resist level {expected_level}"
        );
    }

    // The two properties are independent slots: stunning something does not
    // make it harder to sleep.
    assert_eq!(resist_bonus(&world, MOB, BasicProperty::Magic), 1.0);
    // `NONE` never accrues and never resists.
    increase_resist_level(&mut world, MOB, BasicProperty::None);
    assert_eq!(resist_bonus(&world, MOB, BasicProperty::None), 1.0);
}

/// The window is 15 s **since the last landed debuff**, and an expired chain
/// restarts at 1 rather than continuing — Java's `increaseResistLevel` checks
/// `isExpired()` first. Expiry is evaluated on read; nothing sweeps it.
#[test]
fn the_chain_decays_fifteen_seconds_after_the_last_landed_debuff() {
    let mut world = world_with_mob();
    increase_resist_level(&mut world, MOB, BasicProperty::Physical);
    increase_resist_level(&mut world, MOB, BasicProperty::Physical);
    assert_eq!(resist_level(&world, MOB, BasicProperty::Physical), 2);

    // One tick before the window closes it still counts…
    world.tick += RESIST_DURATION_TICKS;
    assert_eq!(resist_level(&world, MOB, BasicProperty::Physical), 2);
    // …and one tick after, it is gone.
    world.tick += 1;
    assert_eq!(resist_level(&world, MOB, BasicProperty::Physical), 0);
    assert_eq!(resist_bonus(&world, MOB, BasicProperty::Physical), 1.0);

    // A fresh landing restarts the ladder at 1, not at 3.
    increase_resist_level(&mut world, MOB, BasicProperty::Physical);
    assert_eq!(resist_level(&world, MOB, BasicProperty::Physical), 1);
}

/// **The asymmetry.** `Creature.hasBasicPropertyResist()` is unconditionally
/// true; `Player` overrides it to `isInCategory(SIXTH_CLASS_GROUP)`, which this
/// dist populates with awakened classes (148+) only. So mobs accrue and players
/// never do — PvE stun-lock resistance without touching PvP chain-CC.
#[test]
fn mobs_accrue_resistance_and_players_do_not() {
    let mut world = world_with_mob();
    let _rx = ingame_player_access(&mut world, 1, PLAYER, 0);

    assert!(has_resist(&world, MOB), "every Creature has it by default");
    assert!(
        !has_resist(&world, PLAYER),
        "no Interlude class is in SIXTH_CLASS_GROUP, so no player accrues"
    );

    for _ in 0..4 {
        increase_resist_level(&mut world, MOB, BasicProperty::Physical);
        increase_resist_level(&mut world, PLAYER, BasicProperty::Physical);
    }
    assert_eq!(
        resist_bonus(&world, MOB, BasicProperty::Physical),
        0.0,
        "the mob is immune to a fourth stun"
    );
    assert_eq!(
        resist_bonus(&world, PLAYER, BasicProperty::Physical),
        1.0,
        "the player takes the fourth stun at full rate"
    );
}

/// The formula wiring, both terms: `getAbnormalResist` is **subtracted inside
/// `baseMod`** (before the clamp), `getBasicPropertyResistBonus` is
/// **multiplied after** it. The second is what lets a rate the clamp floored at
/// 10 reach 0 — port it before the clamp and a level-3 chain leaves a 10 %
/// stun through instead of none.
#[test]
fn the_two_basic_property_terms_enter_the_formula_at_different_points() {
    use crate::model::formulas::calc_effect_land_rate;

    // baseMod = (magicLevel − targetLevel + 3) × lvlBonusRate + activateRate + 30.
    // With lvlBonusRate 0 the level gap drops out: 0 + 50 + 30 = 80, inside the
    // 10–90 clamp so it passes through untouched.
    let base = calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default());
    assert!((base - 80.0).abs() < 1e-9, "un-clamped baseMod: {base}");

    // The stat term is subtracted *inside* baseMod, so it moves the pre-clamp
    // value one-for-one.
    let stat_20 =
        calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 20.0, 1.0, Default::default());
    assert!((stat_20 - 60.0).abs() < 1e-9, "80 − 20: {stat_20}");
    // …and it is subject to the clamp, unlike the chain term below: 80 − 75 = 5
    // is floored back up to 10.
    let stat_75 =
        calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 75.0, 1.0, Default::default());
    assert!((stat_75 - 10.0).abs() < 1e-9, "floored at 10: {stat_75}");

    // The chain term multiplies *after* the clamp.
    assert!(
        (calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 0.0, 0.6, Default::default()) - 48.0)
            .abs()
            < 1e-9
    );
    assert!(
        (calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 0.0, 0.3, Default::default()) - 24.0)
            .abs()
            < 1e-9
    );
    assert_eq!(
        calc_effect_land_rate(40, 50, 0, 40, 1.0, 1.0, 1.0, 0.0, 0.0, Default::default()),
        0.0,
        "a level-3 chain is a hard 0"
    );

    // The distinction that matters: start from a rate the **floor already
    // rescued** — lvlBonusRate 2 against a level-90 target gives
    // (40 − 90 + 3) × 2 + 80 = −14, clamped up to 10 — and the chain still
    // takes it to 0. Multiply before the clamp instead and a level-3 chain
    // would leave a 10 % stun landing forever.
    let floored = calc_effect_land_rate(40, 50, 2, 90, 1.0, 1.0, 1.0, 0.0, 1.0, Default::default());
    assert!((floored - 10.0).abs() < 1e-9, "floored at 10: {floored}");
    assert_eq!(
        calc_effect_land_rate(40, 50, 2, 90, 1.0, 1.0, 1.0, 0.0, 0.0, Default::default()),
        0.0,
        "…and the chain still takes it to 0"
    );
}

/// End-to-end: a **real dist stun** landing on a mob accrues, and the accrual
/// sits on the *landed* path — a resisted stun builds nothing. Java puts it
/// inside `applyEffects`' `if (addContinuousEffects)` branch, past
/// `calcEffectSuccess`, precisely so a debuff you keep failing to land can't
/// lock you out of it.
///
/// Stun Attack (100): `<basicProperty>PHYSICAL`, `<abnormalType>STUN`,
/// `activateRate 50` — so the roll is live and `forced_rolls` decides it.
#[test]
fn a_landed_stun_accrues_and_a_resisted_one_does_not() {
    const STUN_ATTACK: i32 = 100;
    let (mut world, ..) = test_world();
    world.data = dist::game_data_owned();
    add_test_npc(&mut world, MOB, 20001, "Monster", 20, 0, 0, 0);
    let _rx = ingame_player_access(&mut world, 1, PLAYER, 0);
    let skill = world
        .data
        .skill_data
        .get(STUN_ATTACK, 1)
        .expect("Stun Attack 1")
        .clone();
    assert_eq!(
        skill.basic_property,
        BasicProperty::Physical,
        "the dist declares <basicProperty>PHYSICAL</basicProperty>"
    );

    // `forced_rolls` is a queue shared by *every* roll the cast makes — a
    // physical skill rolls for crit and more before the effect-land roll, so
    // seeding one value seeds the wrong one ([[l2r-forced-rolls-flake]]).
    // Filling the queue with 99 makes the outcome independent of ordering:
    // the land rate is clamped to at most 90, and `resisted = rate <= roll`,
    // so a 99 resists wherever in the sequence it lands.
    world.force_rolls([99; 8]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, PLAYER, MOB, &skill);
    assert_eq!(
        resist_level(&world, MOB, BasicProperty::Physical),
        0,
        "a resisted stun must not build the resistance that would lock it out"
    );

    // Same trick inverted: the rate is floored at 10, so a 0 never resists.
    world.clear_forced_rolls();
    world.force_rolls([0; 8]);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, PLAYER, MOB, &skill);
    assert_eq!(
        resist_level(&world, MOB, BasicProperty::Physical),
        1,
        "a landed stun accrues one level"
    );
    assert_eq!(resist_bonus(&world, MOB, BasicProperty::Physical), 0.6);
}
