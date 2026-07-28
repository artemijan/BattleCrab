//! GM petitions (G31 slice 3): submit → GM accept (the gate) → consultation
//! chat → end + feedback.

use super::*;

use crate::db::DbCommand;
use crate::game_loop::petition;
use crate::model::Player;
use crate::model::petition::PetitionState;

const SAY2_OPCODE: u8 = 0x4A; // server CreatureSay
const PETITION_VOTE_OPCODE: u8 = 0xFC;

/// Promote an online player to GM so `any_gm_online` / `is_gm` see them.
fn make_gm(world: &mut World, oid: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .access_level = 70;
}

fn petition_body(content: &str, type_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(content);
    w.write_i32(type_id);
    w.into_bytes()
}

fn feedback_body(rate: i32, message: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(0); // unknown
    w.write_i32(rate);
    w.write_string(message);
    w.into_bytes()
}

fn has_opcode(pkts: &[Vec<u8>], op: u8) -> bool {
    pkts.iter().any(|p| p.first() == Some(&op))
}

#[test]
fn submit_creates_a_pending_petition_and_pings_gms() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let mut p_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut gm_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    drain(&mut p_rx);
    drain(&mut gm_rx);

    petition::on_request_petition(&mut world, 1, &petition_body("help me", 3));

    assert_eq!(world.petitions.pending_count(), 1);
    let (_id, pet) = world.petitions.pending.iter().next().unwrap();
    assert_eq!(pet.petitioner, 3001);
    assert_eq!(pet.state, PetitionState::Pending);
    // Petitioner got acceptance SMs; GMs got the "new petition" broadcast.
    assert!(!drain(&mut p_rx).is_empty(), "petitioner notified");
    assert!(
        has_opcode(&drain(&mut gm_rx), SAY2_OPCODE),
        "GM gets the broadcast"
    );
}

#[test]
fn submitting_needs_a_gm_online() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    assert_eq!(world.petitions.pending_count(), 0, "no GM → no petition");
}

#[test]
fn only_one_pending_petition_per_player() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _gm = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("first", 3));
    petition::on_request_petition(&mut world, 1, &petition_body("second", 3));
    assert_eq!(world.petitions.pending_count(), 1, "the second is refused");
}

#[test]
fn gm_accept_starts_the_consultation_and_records_the_gm() {
    // The slice gate: a player files a petition and a GM answers it.
    let (mut world, _tx, _rx, _link) = admin_world();
    let mut p_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut gm_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    let id = *world.petitions.pending.keys().next().unwrap();
    drain(&mut p_rx);
    drain(&mut gm_rx);

    petition::accept_petition(&mut world, 3002, id);

    let pet = &world.petitions.pending[&id];
    assert_eq!(pet.state, PetitionState::InProcess);
    assert_eq!(pet.responder, Some(3002));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .last_petition_gm_name
            .as_deref(),
        Some("P3002"),
        "petitioner remembers the responding GM"
    );
    assert!(
        !drain(&mut p_rx).is_empty(),
        "petitioner told it was accepted"
    );
    assert!(!drain(&mut gm_rx).is_empty(), "GM gets receipt + start");
}

#[test]
fn consultation_chat_reaches_both_participants_and_is_logged() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let mut p_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut gm_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    let id = *world.petitions.pending.keys().next().unwrap();
    petition::accept_petition(&mut world, 3002, id);
    drain(&mut p_rx);
    drain(&mut gm_rx);

    // Petitioner speaks on the petition channel (type 6 = PETITION_PLAYER).
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body("are you there?", 6, None)].concat(),
    );
    assert!(
        has_opcode(&drain(&mut p_rx), SAY2_OPCODE),
        "petitioner echo"
    );
    assert!(has_opcode(&drain(&mut gm_rx), SAY2_OPCODE), "GM hears it");
    assert_eq!(world.petitions.pending[&id].log.len(), 1, "line logged");
}

#[test]
fn gm_ending_the_consultation_completes_it_and_prompts_feedback() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let mut p_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _gm = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    let id = *world.petitions.pending.keys().next().unwrap();
    petition::accept_petition(&mut world, 3002, id);
    drain(&mut p_rx);

    // The GM ends the consultation (RequestPetitionCancel from a GM).
    petition::on_request_petition_cancel(&mut world, 2);

    assert!(!world.petitions.pending.contains_key(&id), "left pending");
    assert!(world.petitions.completed.contains_key(&id), "now completed");
    assert!(
        has_opcode(&drain(&mut p_rx), PETITION_VOTE_OPCODE),
        "petitioner is prompted for feedback"
    );
}

#[test]
fn feedback_after_a_consultation_is_persisted() {
    let (mut world, _tx, mut db_rx, _link) = admin_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _gm = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    let id = *world.petitions.pending.keys().next().unwrap();
    petition::accept_petition(&mut world, 3002, id); // sets last_petition_gm_name
    drain_db(&mut db_rx);

    petition::on_request_petition_feedback(&mut world, 1, &feedback_body(4, "great help"));

    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            DbCommand::StorePetitionFeedback { char_name, gm_name, rate, .. }
                if char_name == "P3001" && gm_name == "P3002" && *rate == 4
        )),
        "feedback row persisted"
    );
}

#[test]
fn feedback_without_a_prior_consultation_is_dropped() {
    let (mut world, _tx, mut db_rx, _link) = admin_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    // No last_petition_gm_name → Java drops it.
    petition::on_request_petition_feedback(&mut world, 1, &feedback_body(3, "hi"));
    assert!(
        !drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::StorePetitionFeedback { .. }))
    );
}

#[test]
fn petitioner_can_cancel_a_pending_petition() {
    let (mut world, _tx, _rx, _link) = admin_world();
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _gm = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_gm(&mut world, 3002);
    petition::on_request_petition(&mut world, 1, &petition_body("help", 3));
    assert_eq!(world.petitions.pending_count(), 1);

    petition::on_request_petition_cancel(&mut world, 1);
    assert_eq!(world.petitions.pending_count(), 0, "cancelled");
}
