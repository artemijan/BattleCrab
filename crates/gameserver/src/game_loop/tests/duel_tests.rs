//! Duels (G20), 1v1: the challenge handshake, the countdown, and the
//! consequence-free ending.

use super::*;

use crate::game_loop::duel::{self, DuelResult};
use crate::model::Player;
use crate::model::components::{DuelRef, PendingDuel};

const A: i32 = 2001;
const B: i32 = 2002;
const A_CID: u32 = 1;
const B_CID: u32 = 2;

/// Two healthy players standing next to each other.
fn duelists(
    world: &mut World,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) {
    let a = ingame_caster(world, A_CID, A, 0, 0);
    let b = ingame_caster(world, B_CID, B, 100, 0);
    for oid in [A, B] {
        let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
    (a, b)
}

fn name_of(world: &World, oid: i32) -> String {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .name
        .clone()
}

fn challenge(world: &mut World, from_cid: u32, target: i32) {
    let mut w = PacketWriter::new();
    w.write_string(&name_of(world, target));
    w.write_i32(0); // 1v1
    duel::handle_request_duel_start(world, from_cid, &w.into_bytes());
}

fn answer(world: &mut World, cid: u32, accept: bool) {
    let mut w = PacketWriter::new();
    w.write_i32(0); // partyDuel
    w.write_i32(0); // unused
    w.write_i32(if accept { 1 } else { 0 });
    duel::handle_request_duel_answer(world, cid, &w.into_bytes());
}

fn has_sm(pkts: &[Vec<u8>], id: i16) -> bool {
    pkts.iter()
        .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE && sm_id(p) == id)
}

// ---------------------------------------------------------------------------
// The handshake
// ---------------------------------------------------------------------------

/// A challenge reaches the target as `ExDuelAskStart` and parks a pending
/// answer on them.
#[test]
fn challenge_asks_the_target() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut _oa, mut ob) = duelists(&mut world);
    drain(&mut ob);

    challenge(&mut world, A_CID, B);

    assert!(
        world.objects.has_component::<PendingDuel>(&B),
        "the challenge is pending on the target"
    );
    let pkts = drain(&mut ob);
    assert!(
        pkts.iter().any(|p| p[0] == 0xFE
            && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_DUEL_ASK_START),
        "the target is asked"
    );
}

/// Declining tells the challenger and starts nothing.
#[test]
fn declining_ends_it() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    drain(&mut oa);

    answer(&mut world, B_CID, false);

    assert!(
        !world.objects.has_component::<PendingDuel>(&B),
        "the pending challenge is cleared"
    );
    assert!(world.duels.is_empty(), "no duel started");
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::C1_HAS_DECLINED_YOUR_CHALLENGE_TO_A_DUEL
    ));
}

/// Accepting starts the countdown and marks both sides as dueling, so neither
/// can be challenged again.
#[test]
fn accepting_starts_the_countdown() {
    let (mut world, _db, _l) = cast_test_world();
    let (_oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);

    answer(&mut world, B_CID, true);

    assert_eq!(world.duels.len(), 1, "a duel exists");
    assert!(duel::is_in_duel(&world, A) && duel::is_in_duel(&world, B));
    assert!(duel::are_dueling(&world, A, B));
}

// ---------------------------------------------------------------------------
// canDuel gates
// ---------------------------------------------------------------------------

/// Below half HP, a player can't duel — and the challenger is told which
/// reason applies.
#[test]
fn low_hp_refuses_the_duel() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    {
        let v = world.objects.get_component_mut::<Vitals>(&B).unwrap();
        v.cur_hp = v.max_hp as f64 * 0.4;
    }
    drain(&mut oa);

    challenge(&mut world, A_CID, B);

    assert!(
        !world.objects.has_component::<PendingDuel>(&B),
        "no challenge is sent"
    );
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::C1_CANNOT_DUEL_BECAUSE_C1_S_HP_OR_MP_IS_BELOW_50
    ));
}

/// Someone already dueling can't be challenged again.
#[test]
fn already_dueling_refuses() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    drain(&mut oa);

    challenge(&mut world, A_CID, B);
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::YOU_ARE_UNABLE_TO_REQUEST_A_DUEL_AT_THIS_TIME
    ));
}

/// Too far apart to hear the challenge.
#[test]
fn out_of_range_refuses() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    world.objects.get_component_mut::<Position>(&B).unwrap().x = 9000;
    drain(&mut oa);

    challenge(&mut world, A_CID, B);

    assert!(!world.objects.has_component::<PendingDuel>(&B));
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::C1_IS_TOO_FAR_AWAY_TO_RECEIVE_A_DUEL_CHALLENGE
    ));
}

// ---------------------------------------------------------------------------
// Running and ending
// ---------------------------------------------------------------------------

/// The countdown announces each second and then begins the duel.
#[test]
fn countdown_runs_down_and_starts() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    drain(&mut oa);

    // Five countdown steps take it to zero and start the duel.
    advance_ticks(&mut world, 60);

    let pkts = drain(&mut oa);
    assert!(
        has_sm(&pkts, server_packets::sm_ids::LET_THE_DUEL_BEGIN),
        "the duel began"
    );
    assert!(
        pkts.iter().any(|p| p[0] == 0xFE
            && i16::from_le_bytes([p[1], p[2]]) == server_packets::opcodes::EX_DUEL_START),
        "ExDuelStart went out"
    );
    let d = world.duels.values().next().expect("still running");
    assert!(d.ends_at_tick > world.tick, "the 120 s clock is set");
}

/// Surrendering hands the win to the opponent and clears the duel.
#[test]
fn surrender_ends_the_duel() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    advance_ticks(&mut world, 60);
    drain(&mut oa);

    duel::handle_request_duel_surrender(&mut world, B_CID);

    assert!(world.duels.is_empty(), "the duel is over");
    assert!(!duel::is_in_duel(&world, A) && !duel::is_in_duel(&world, B));
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::C1_HAS_WON_THE_DUEL
    ));
}

/// **A duel never kills.** The losing blow stops at 1 HP and ends the duel
/// instead — the loser is alive and restored afterwards.
#[test]
fn losing_blow_does_not_kill() {
    let (mut world, _db, _l) = cast_test_world();
    let (_oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    advance_ticks(&mut world, 60);

    // A blow far exceeding B's remaining HP.
    let huge = world.objects.get_component::<Vitals>(&B).unwrap().max_hp as f64 * 10.0;
    crate::game_loop::combat::apply_physical_damage(&mut world, A, B, huge, false, false);

    let v = world.objects.get_component::<Vitals>(&B).unwrap();
    assert!(!v.dead, "the duel loser is not killed");
    assert!(world.duels.is_empty(), "and the duel ended");
    // Restored on the way out.
    assert_eq!(v.cur_hp, v.max_hp as f64, "the loser is restored");
}

/// Drifting too far apart cancels the duel with no winner.
#[test]
fn drifting_apart_cancels() {
    let (mut world, _db, _l) = cast_test_world();
    let (mut oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    advance_ticks(&mut world, 60);
    drain(&mut oa);

    world.objects.get_component_mut::<Position>(&B).unwrap().x = 50_000;
    advance_ticks(&mut world, 20);

    assert!(world.duels.is_empty(), "the duel was cancelled");
    assert!(has_sm(
        &drain(&mut oa),
        server_packets::sm_ids::THE_DUEL_HAS_ENDED_IN_A_TIE
    ));
}

/// Ending clears the duel marker from both sides so they can duel again.
#[test]
fn ending_clears_the_duel_state() {
    let (mut world, _db, _l) = cast_test_world();
    let (_oa, _ob) = duelists(&mut world);
    challenge(&mut world, A_CID, B);
    answer(&mut world, B_CID, true);
    let duel_id = *world.duels.keys().next().unwrap();

    duel::end_duel(&mut world, duel_id, DuelResult::Canceled);

    assert!(world.objects.get_component::<DuelRef>(&A).is_none());
    assert!(world.objects.get_component::<DuelRef>(&B).is_none());
    assert!(duel::can_duel(&world, A).is_ok(), "free to duel again");
}
