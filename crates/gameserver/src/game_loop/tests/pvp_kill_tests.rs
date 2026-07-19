//! PvP / PK kill consequences (G20): counters, karma, and the zone exemptions.

use super::*;

use crate::game_loop::pvp;
use crate::model::components::{PvpState, ZoneFlags};
use crate::model::Player;

const KILLER: i32 = 2001;
const VICTIM: i32 = 2002;
const KILLER_CID: u32 = 1;
const VICTIM_CID: u32 = 2;

fn rep(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).unwrap().reputation
}
fn pvp_kills(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).unwrap().pvp_kills
}
fn pk_kills(world: &World, oid: i32) -> i32 {
    world.objects.get_component::<Player>(&oid).unwrap().pk_kills
}

fn two_players(world: &mut World) {
    let _k = ingame_caster(world, KILLER_CID, KILLER, 0, 0);
    let _v = ingame_caster(world, VICTIM_CID, VICTIM, 50, 0);
}

fn kill(world: &mut World) {
    pvp::on_kill_update_pvp_reputation(world, KILLER, VICTIM);
}

// ---------------------------------------------------------------------------
// The karma-gain curve
// ---------------------------------------------------------------------------

/// `Formulas.calculateKarmaGain`'s three brackets.
#[test]
fn karma_gain_follows_the_pk_count_brackets() {
    // < 99: ((pk * 0.5 + 1) * 60) * 12
    assert_eq!(pvp::calculate_karma_gain(0), 720);
    assert_eq!(pvp::calculate_karma_gain(1), 1080);
    assert_eq!(pvp::calculate_karma_gain(98), 36_000);
    // < 180: ((pk * 0.125 + 37.75) * 60) * 12
    assert_eq!(pvp::calculate_karma_gain(99), 36_090);
    // >= 180: flat.
    assert_eq!(pvp::calculate_karma_gain(180), 43_200);
    assert_eq!(pvp::calculate_karma_gain(500), 43_200);
    // Karma rises with the body count.
    assert!(pvp::calculate_karma_gain(50) > pvp::calculate_karma_gain(10));
}

// ---------------------------------------------------------------------------
// The three kill outcomes
// ---------------------------------------------------------------------------

/// Killing a **flagged** player is lawful: a PvP kill, no karma.
#[test]
fn killing_a_flagged_player_is_a_pvp_kill() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world.objects.add_components(&VICTIM, PvpState { flag: 1, ..Default::default() });

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 1, "counted as a PvP kill");
    assert_eq!(pk_kills(&world, KILLER), 0, "not a PK");
    assert_eq!(rep(&world, KILLER), 0, "no karma taken");
}

/// Killing a **clean** player with positive reputation and no prior PKs is the
/// "first offence": reputation resets to 0 rather than going negative.
#[test]
fn first_offence_resets_positive_reputation() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world.objects.get_component_mut::<Player>(&KILLER).unwrap().reputation = 500;

    kill(&mut world);

    assert_eq!(rep(&world, KILLER), 0, "reputation reset, not driven negative");
    assert_eq!(pk_kills(&world, KILLER), 1);
    assert_eq!(pvp_kills(&world, KILLER), 0);
}

/// Otherwise a clean kill is a **PK**: karma is taken and the counter rises.
#[test]
fn killing_a_clean_player_costs_karma() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    // Reputation 0 → falls past the first-offence branch (which needs > 0).

    kill(&mut world);

    assert_eq!(pk_kills(&world, KILLER), 1);
    assert_eq!(pvp_kills(&world, KILLER), 0);
    assert_eq!(rep(&world, KILLER), -pvp::calculate_karma_gain(0), "karma for the first PK");

    // A second PK costs more, from the now-higher pk count.
    let after_first = rep(&world, KILLER);
    kill(&mut world);
    assert_eq!(pk_kills(&world, KILLER), 2);
    assert_eq!(rep(&world, KILLER), after_first - pvp::calculate_karma_gain(1));
}

/// Killing a **PK** is lawful and counts as PvP.
#[test]
fn killing_a_pk_is_lawful() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    world.objects.get_component_mut::<Player>(&VICTIM).unwrap().reputation = -5000;

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 1);
    assert_eq!(pk_kills(&world, KILLER), 0);
    assert!(rep(&world, KILLER) >= 0, "no karma for killing a PK");
}

// ---------------------------------------------------------------------------
// Exemptions
// ---------------------------------------------------------------------------

/// Inside a PVP zone nothing is counted at all — neither side takes karma or
/// gains a kill ("Do nothing when in PVP zone").
#[test]
fn pvp_zone_kills_count_for_nothing() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    for oid in [KILLER, VICTIM] {
        world.objects.get_component_mut::<ZoneFlags>(&oid).unwrap().mask =
            crate::data::zone_data::ZoneKind::Pvp.bit();
    }

    kill(&mut world);

    assert_eq!(pvp_kills(&world, KILLER), 0, "arena kills are not counted");
    assert_eq!(pk_kills(&world, KILLER), 0);
    assert_eq!(rep(&world, KILLER), 0);
}

/// A monster killer moves none of this (the block is player-on-player only).
#[test]
fn monster_kills_move_no_counters() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 40, 0, 0);

    pvp::on_kill_update_pvp_reputation(&mut world, NPC_OID, VICTIM);

    assert_eq!(pk_kills(&world, VICTIM), 0);
    assert_eq!(rep(&world, VICTIM), 0);
}

/// `check_if_pvp` itself: flagged and PK targets are lawful, a clean one is
/// not, and self never is.
#[test]
fn check_if_pvp_classifies_targets() {
    let (mut world, _db, _l) = cast_test_world();
    two_players(&mut world);

    assert!(!pvp::check_if_pvp(&world, KILLER, VICTIM), "a clean stranger is not a lawful target");
    assert!(!pvp::check_if_pvp(&world, KILLER, KILLER), "self is never PvP");

    world.objects.add_components(&VICTIM, PvpState { flag: 1, ..Default::default() });
    assert!(pvp::check_if_pvp(&world, KILLER, VICTIM), "a flagged target is lawful");

    world.objects.add_components(&VICTIM, PvpState::default());
    world.objects.get_component_mut::<Player>(&VICTIM).unwrap().reputation = -1;
    assert!(pvp::check_if_pvp(&world, KILLER, VICTIM), "a PK is lawful");
}
