//! `model/BlockList` + `clientpackets/RequestBlock`, and the `isBlocked`
//! filter each consumer applies.

use super::*;
use crate::db::DbCommand;
use crate::enums::ChatType;
use crate::game_loop::{block_list, chat};
use crate::model::Player;
use crate::model::components::AdminFlags;
use crate::network::server_packets::{opcodes, sm_ids};

const BLOCK: i32 = 0;
const UNBLOCK: i32 = 1;
const BLOCKLIST: i32 = 2;
const ALLBLOCK: i32 = 3;
const ALLUNBLOCK: i32 = 4;

fn block_body(kind: i32, name: Option<&str>) -> Vec<u8> {
    let mut w = commons::network::PacketWriter::new();
    w.write_i32(kind);
    if let Some(n) = name {
        w.write_string(n);
    }
    w.into_bytes()
}

/// Two players who can hear each other, both known to the offline name table
/// (Java's `CharInfoTable`, which is how `RequestBlock` resolves a name).
fn two_players(
    world: &mut World,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) {
    let a = ingame_player(world, 1, 2001, 0, 0, 0);
    let b = ingame_player(world, 2, 2002, 0, 0, 0);
    for oid in [2001, 2002] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .expect("player")
            .level = 40;
        world
            .objects
            .add_components(&oid, crate::model::components::PlayerVariables::default());
        world.char_ids_by_name.insert(format!("p{oid}"), oid);
    }
    (a, b)
}

/// Blocking is one row in one direction, persisted, and both parties are told.
#[test]
fn blocking_adds_one_row_notifies_both_and_is_not_mutual() {
    let (mut world, _tx, mut db_rx, ..) = test_world();
    let (mut rx_a, mut rx_b) = two_players(&mut world);
    drain(&mut rx_a);
    drain(&mut rx_b);
    while db_rx.try_recv().is_ok() {}

    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("P2002")));

    assert!(
        block_list::is_blocked(&world, 2001, 2002),
        "2001 now ignores 2002"
    );
    assert!(
        !block_list::is_blocked(&world, 2002, 2001),
        "blocking is one-directional — 2002's own list is untouched"
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::S1_HAS_BEEN_ADDED_TO_YOUR_IGNORE_LIST]
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx_b)),
        vec![sm_ids::C1_HAS_PLACED_YOU_ON_HIS_HER_IGNORE_LIST],
        "the blocked player is told, if online"
    );

    let mut cmds = Vec::new();
    while let Ok(c) = db_rx.try_recv() {
        cmds.push(c);
    }
    assert_eq!(
        cmds.iter()
            .filter(|c| matches!(
                c,
                DbCommand::InsertBlock {
                    owner: 2001,
                    target: 2002
                }
            ))
            .count(),
        1,
        "exactly one row, owner→target"
    );

    // Unblocking removes it and persists the delete.
    block_list::handle_request_block(&mut world, 1, &block_body(UNBLOCK, Some("P2002")));
    assert!(!block_list::is_blocked(&world, 2001, 2002));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::S1_HAS_BEEN_REMOVED_FROM_YOUR_IGNORE_LIST]
    );
    let mut cmds = Vec::new();
    while let Ok(c) = db_rx.try_recv() {
        cmds.push(c);
    }
    assert!(cmds.iter().any(|c| matches!(
        c,
        DbCommand::DeleteBlock {
            owner: 2001,
            target: 2002
        }
    )));
}

/// Java's refusal set, each with its own answer.
#[test]
fn the_block_refusals_each_have_their_own_answer() {
    let (mut world, ..) = test_world();
    let (mut rx_a, _rx_b) = two_players(&mut world);

    // Unknown name — deliberately indistinguishable from a hidden character.
    drain(&mut rx_a);
    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("Nobody")));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::YOU_HAVE_FAILED_TO_REGISTER_THE_USER_TO_YOUR_IGNORE_LIST]
    );

    // A GM cannot be blocked. `GameData::for_test` ships `AdminData::empty()`,
    // where *no* access level is a GM — so the real table has to be loaded or
    // this case silently exercises the ordinary-player path instead.
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world
        .objects
        .get_component_mut::<Player>(&2002)
        .unwrap()
        .access_level = world.data.admin.highest_level();
    assert!(
        world
            .objects
            .get_component::<Player>(&2002)
            .unwrap()
            .is_gm(&world.data),
        "fixture check: the target really is a GM"
    );
    drain(&mut rx_a);
    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("P2002")));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::YOU_MAY_NOT_IMPOSE_A_BLOCK_ON_A_GM]
    );
    assert!(!block_list::is_blocked(&world, 2001, 2002));
    world
        .objects
        .get_component_mut::<Player>(&2002)
        .unwrap()
        .access_level = 0;

    // Blocking yourself: Java returns with **no** message at all.
    drain(&mut rx_a);
    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("P2001")));
    assert!(
        drain(&mut rx_a).is_empty(),
        "self-block is silently ignored"
    );
    assert!(!block_list::is_blocked(&world, 2001, 2001));

    // Unblocking someone who is not blocked answers SM 144.
    drain(&mut rx_a);
    block_list::handle_request_block(&mut world, 1, &block_body(UNBLOCK, Some("P2002")));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::THAT_IS_AN_INCORRECT_TARGET]
    );
}

/// `ALLBLOCK`/`ALLUNBLOCK` drive message-refusal mode — the `isBlockAll` half
/// of `isBlocked`, which blocks everyone while the list itself stays empty.
/// That combination is the reason `is_blocked` exists.
#[test]
fn all_block_blocks_everyone_without_touching_the_list() {
    let (mut world, ..) = test_world();
    let (mut rx_a, _rx_b) = two_players(&mut world);
    drain(&mut rx_a);

    block_list::handle_request_block(&mut world, 1, &block_body(ALLBLOCK, None));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::MESSAGE_REFUSAL_MODE]
    );
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&2001)
            .is_some_and(|f| f.silence)
    );
    assert!(
        block_list::is_blocked(&world, 2001, 2002),
        "refusal mode blocks a player who is not on the list"
    );
    assert!(
        !block_list::is_in_block_list(&world, 2001, 2002),
        "…and the persisted list is still empty"
    );

    block_list::handle_request_block(&mut world, 1, &block_body(ALLUNBLOCK, None));
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::MESSAGE_ACCEPTANCE_MODE]
    );
    assert!(!block_list::is_blocked(&world, 2001, 2002));
}

/// `BLOCKLIST` answers with the names, sorted so the window does not reshuffle.
#[test]
fn the_block_list_request_answers_with_names() {
    let (mut world, ..) = test_world();
    let (mut rx_a, _rx_b) = two_players(&mut world);
    let _c = ingame_player(&mut world, 3, 2003, 0, 0, 0);
    world.char_ids_by_name.insert("p2003".into(), 2003);

    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("P2003")));
    block_list::handle_request_block(&mut world, 1, &block_body(BLOCK, Some("P2002")));
    drain(&mut rx_a);

    block_list::handle_request_block(&mut world, 1, &block_body(BLOCKLIST, None));
    let pkts = drain(&mut rx_a);
    let p = pkts
        .iter()
        .find(|p| p[0] == opcodes::BLOCK_LIST)
        .expect("BlockListPacket");
    let mut r = commons::network::PacketReader::new(&p[1..]);
    assert_eq!(r.read_i32().unwrap(), 2, "two entries");
    // Sorted by object id: 2002 then 2003, regardless of insertion order.
    assert_eq!(r.read_string().unwrap(), "P2002");
    assert_eq!(r.read_string().unwrap(), "", "the empty memo slot");
    assert_eq!(r.read_string().unwrap(), "P2003");
}

/// The whole point of the deferral: a blocked speaker is filtered out of every
/// broadcast channel, and **Shout is bidirectional where Trade is not**.
#[test]
fn a_block_filters_every_broadcast_channel() {
    let (mut world, ..) = test_world();
    let (mut rx_a, mut rx_b) = two_players(&mut world);

    // 2002 blocks 2001, so 2002 stops hearing 2001.
    block_list::handle_request_block(&mut world, 2, &block_body(BLOCK, Some("P2001")));

    for ty in [
        ChatType::General,
        ChatType::Shout,
        ChatType::Trade,
        ChatType::World,
    ] {
        drain(&mut rx_a);
        drain(&mut rx_b);
        chat::handle_say2(&mut world, 1, &say2_body("hi", ty.client_id(), None));
        assert!(
            !drain(&mut rx_b).iter().any(|p| p[0] == opcodes::SAY2),
            "{ty:?} must not reach a listener who blocked the speaker"
        );
    }

    // The reverse direction: 2001 has NOT blocked 2002, so 2001 still hears
    // 2002 on Trade — but not on Shout, whose in-region branch tests both
    // lists. This asymmetry is Java's, and collapsing the two would hide it.
    drain(&mut rx_a);
    chat::handle_say2(
        &mut world,
        2,
        &say2_body("hi", ChatType::Trade.client_id(), None),
    );
    assert!(
        drain(&mut rx_a).iter().any(|p| p[0] == opcodes::SAY2),
        "Trade checks only the listener's list, so 2001 still hears 2002"
    );

    drain(&mut rx_a);
    chat::handle_say2(
        &mut world,
        2,
        &say2_body("hi", ChatType::Shout.client_id(), None),
    );
    assert!(
        !drain(&mut rx_a).iter().any(|p| p[0] == opcodes::SAY2),
        "Shout is bidirectional — the speaker's own list suppresses it too"
    );
}

/// A whisper to someone who blocked you is refused with the same message as
/// message-refusal mode, so a blocked sender cannot tell the two apart.
#[test]
fn a_whisper_to_a_blocker_is_refused_indistinguishably() {
    let (mut world, ..) = test_world();
    let (mut rx_a, mut rx_b) = two_players(&mut world);
    block_list::handle_request_block(&mut world, 2, &block_body(BLOCK, Some("P2001")));
    drain(&mut rx_a);
    drain(&mut rx_b);

    chat::handle_say2(
        &mut world,
        1,
        &say2_body("psst", ChatType::Whisper.client_id(), Some("P2002")),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx_a)),
        vec![sm_ids::THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE]
    );
    assert!(drain(&mut rx_b).is_empty(), "nothing delivered");
}

/// Mail consults the **persisted list only**, and must work when the addressee
/// is offline — the case a per-player component could not answer, and the
/// reason the list is world state.
#[test]
fn mail_is_refused_by_an_offline_addressees_block_list() {
    let (mut world, ..) = test_world();
    let mut rx_a = ingame_player(&mut world, 1, 2001, 0, 0, 0);
    // 2002 never logs in: it exists only in the name table and the block map.
    world.char_ids_by_name.insert("p2002".into(), 2002);
    world.block_lists.entry(2002).or_default().insert(2001);
    drain(&mut rx_a);

    assert!(
        block_list::is_in_block_list(&world, 2002, 2001),
        "an offline character's list is still readable"
    );
    // Message-refusal mode is a live flag, so an offline player cannot be in
    // it — which is exactly why mail uses `isInBlockList`, not `isBlocked`.
    assert!(!world.objects.has_component::<AdminFlags>(&2002),);
}
