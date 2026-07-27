//! Party matching rooms (G30) — slice 1: the waiting list, room creation and
//! editing, and the two browse lists.

use super::*;

use crate::model::components::InMatchingRoom;
use crate::model::matching_room::MatchingMemberType;
use crate::network::server_packets::opcodes;
use crate::network::server_packets::sm_ids;

/// `RequestPartyMatchConfig` body: page, location, level-filter.
fn match_config_body(page: i32, location: i32, level_filter: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(page);
    w.write_i32(location);
    w.write_i32(level_filter);
    w.into_bytes()
}

/// `RequestPartyMatchList` body: room id, max members, min/max level, loot, title.
fn match_list_body(
    room_id: i32,
    max_members: i32,
    min_level: i32,
    max_level: i32,
    loot: i32,
    title: &str,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(room_id);
    w.write_i32(max_members);
    w.write_i32(min_level);
    w.write_i32(max_level);
    w.write_i32(loot);
    w.write_string(title);
    w.into_bytes()
}

/// `RequestListPartyMatchingWaitingRoom` body.
fn waiting_list_body(
    page: i32,
    min_level: i32,
    max_level: i32,
    class_ids: &[i32],
    query: Option<&str>,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(page);
    w.write_i32(min_level);
    w.write_i32(max_level);
    w.write_i32(class_ids.len() as i32);
    for c in class_ids {
        w.write_i32(*c);
    }
    if let Some(q) = query {
        w.write_string(q);
    }
    w.into_bytes()
}

fn set_level(world: &mut World, oid: i32, level: i32) {
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.level = level;
    }
}

fn open_board(world: &mut World, client_id: u32) {
    on_packet(
        world,
        client_id,
        [
            vec![cop::REQUEST_PARTY_MATCH_CONFIG],
            match_config_body(1, -1, 1),
        ]
        .concat(),
    );
}

fn create_room(world: &mut World, client_id: u32, min: i32, max: i32, cap: i32, title: &str) {
    on_packet(
        world,
        client_id,
        [
            vec![cop::REQUEST_PARTY_MATCH_LIST],
            match_list_body(0, cap, min, max, 0, title),
        ]
        .concat(),
    );
}

/// Decode `ListPartyWaiting` (0x9C) into `(total, [(room id, title, leader)])`.
fn parse_room_list(p: &[u8]) -> (i32, Vec<(i32, String, String)>) {
    assert_eq!(p[0], opcodes::LIST_PARTY_WAITING);
    let mut r = commons::network::PacketReader::new(&p[1..]);
    let total = r.read_i32().unwrap();
    let page = r.read_i32().unwrap();
    let mut rooms = Vec::new();
    for _ in 0..page {
        let id = r.read_i32().unwrap();
        let title = r.read_string().unwrap();
        let _location = r.read_i32().unwrap();
        let _min = r.read_i32().unwrap();
        let _max = r.read_i32().unwrap();
        let _cap = r.read_i32().unwrap();
        let leader = r.read_string().unwrap();
        let members = r.read_i32().unwrap();
        for _ in 0..members {
            r.read_i32().unwrap();
            r.read_string().unwrap();
        }
        rooms.push((id, title, leader));
    }
    (total, rooms)
}

/// Decode `ExListPartyMatchingWaitingRoom` (0xFE 0x36) into `(total, names)`.
fn parse_waiting_list(p: &[u8]) -> (i32, Vec<String>) {
    assert_eq!(p[0], opcodes::EX);
    assert_eq!(
        i16::from_le_bytes([p[1], p[2]]),
        opcodes::EX_LIST_PARTY_MATCHING_WAITING_ROOM
    );
    let mut r = commons::network::PacketReader::new(&p[3..]);
    let total = r.read_i32().unwrap();
    let page = r.read_i32().unwrap();
    let mut names = Vec::new();
    for _ in 0..page {
        names.push(r.read_string().unwrap());
        r.read_i32().unwrap(); // class
        r.read_i32().unwrap(); // level
    }
    (total, names)
}

/// Decode `ExPartyRoomMember` (0xFE 0x08) into `(recipient type, [(oid, type)])`.
fn parse_room_members(p: &[u8]) -> (i32, Vec<(i32, i32)>) {
    assert_eq!(p[0], opcodes::EX);
    assert_eq!(
        i16::from_le_bytes([p[1], p[2]]),
        opcodes::EX_PARTY_ROOM_MEMBER
    );
    let mut r = commons::network::PacketReader::new(&p[3..]);
    let recipient_type = r.read_i32().unwrap();
    let count = r.read_i32().unwrap();
    let mut members = Vec::new();
    for _ in 0..count {
        let oid = r.read_i32().unwrap();
        r.read_string().unwrap(); // name
        r.read_i32().unwrap(); // class
        r.read_i32().unwrap(); // level
        r.read_i32().unwrap(); // location
        let mtype = r.read_i32().unwrap();
        members.push((oid, mtype));
    }
    (recipient_type, members)
}

#[test]
fn opening_the_board_registers_the_player_as_looking_for_party() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    assert!(!world.matching_rooms.is_waiting(3001));
    open_board(&mut world, 1);

    assert!(
        world.matching_rooms.is_waiting(3001),
        "RequestPartyMatchConfig is Java's only LFP-registration entry point"
    );
    let pkts = drain(&mut rx);
    let list = pkts
        .iter()
        .find(|p| p[0] == opcodes::LIST_PARTY_WAITING)
        .expect("the board answers with the room list");
    assert_eq!(parse_room_list(list).0, 0, "no rooms exist yet");
}

#[test]
fn a_party_member_who_is_not_the_leader_cannot_browse() {
    let (mut world, ..) = test_world();
    let mut leader_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut member_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut leader_rx);
    drain(&mut member_rx);

    open_board(&mut world, 2);
    assert_eq!(
        sm_ids_of(&drain(&mut member_rx)),
        vec![sm_ids::THE_LIST_OF_PARTY_ROOMS_CAN_ONLY_BE_VIEWED_BY_A_PERSON_WHO_IS_NOT_PART_OF_A_PARTY]
    );
    assert!(!world.matching_rooms.is_waiting(3002));

    // The leader may browse.
    open_board(&mut world, 1);
    assert!(world.matching_rooms.is_waiting(3001));
}

#[test]
fn exit_waiting_room_deregisters() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    open_board(&mut world, 1);
    assert!(world.matching_rooms.is_waiting(3001));

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_EXIT_PARTY_MATCHING_WAITING_ROOM,
            &[],
        ),
    );
    assert!(!world.matching_rooms.is_waiting(3001));
}

#[test]
fn creating_a_room_leaves_the_waiting_list_and_sends_the_room_windows() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    drain(&mut rx);
    open_board(&mut world, 1);
    drain(&mut rx);

    create_room(&mut world, 1, 20, 60, 9, "hunting");

    let room_id = world
        .matching_rooms
        .room_id_of(3001)
        .expect("the creator leads the new room");
    let room = world.matching_rooms.get(room_id).unwrap();
    assert_eq!(room.title, "hunting");
    assert_eq!((room.min_level, room.max_level), (20, 60));
    assert_eq!(room.max_members, 9);
    assert_eq!(room.all_members(), vec![3001]);
    assert!(
        !world.matching_rooms.is_waiting(3001),
        "a room leader is no longer looking for a party"
    );

    let pkts = drain(&mut rx);
    assert!(has_opcode(&pkts, opcodes::PARTY_ROOM_INFO));
    assert!(has_opcode(&pkts, opcodes::LIST_PARTY_WAITING));
    assert!(ex_subs_of(&pkts).contains(&opcodes::EX_PARTY_ROOM_MEMBER));
    assert_eq!(
        sm_ids_of(&pkts),
        vec![sm_ids::YOU_HAVE_CREATED_A_PARTY_ROOM]
    );

    // The creator is the room's PARTY_LEADER in its own member list.
    let members = pkts
        .iter()
        .find(|p| {
            p[0] == opcodes::EX && i16::from_le_bytes([p[1], p[2]]) == opcodes::EX_PARTY_ROOM_MEMBER
        })
        .map(|p| parse_room_members(p))
        .unwrap();
    assert_eq!(members.0, MatchingMemberType::PartyLeader.id());
    assert_eq!(
        members.1,
        vec![(3001, MatchingMemberType::PartyLeader.id())]
    );
}

#[test]
fn a_second_player_sees_the_room_in_the_list() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);

    create_room(&mut world, 1, 20, 60, 9, "hunting");
    drain(&mut b_rx);

    open_board(&mut world, 2);
    let pkts = drain(&mut b_rx);
    let (total, rooms) = parse_room_list(
        pkts.iter()
            .find(|p| p[0] == opcodes::LIST_PARTY_WAITING)
            .unwrap(),
    );
    assert_eq!(total, 1);
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].1, "hunting");
    assert_eq!(rooms[0].2, "P3001", "the leader's name is carried");
}

#[test]
fn my_level_range_filter_hides_rooms_that_do_not_span_the_browser() {
    // Guards the fix for Java's inverted MY_LEVEL_RANGE test, which would show
    // no room here at all.
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 70);
    set_level(&mut world, 3003, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    create_room(&mut world, 1, 20, 60, 9, "low"); // spans 40
    create_room(&mut world, 2, 65, 80, 9, "high"); // does not
    drain(&mut c_rx);

    // level_filter 0 = MY_LEVEL_RANGE
    on_packet(
        &mut world,
        3,
        [
            vec![cop::REQUEST_PARTY_MATCH_CONFIG],
            match_config_body(1, -1, 0),
        ]
        .concat(),
    );
    let pkts = drain(&mut c_rx);
    let (total, rooms) = parse_room_list(
        pkts.iter()
            .find(|p| p[0] == opcodes::LIST_PARTY_WAITING)
            .unwrap(),
    );
    assert_eq!(total, 1, "only the room whose band contains level 40");
    assert_eq!(rooms[0].1, "low");

    // ALL shows both.
    open_board(&mut world, 3);
    let pkts = drain(&mut c_rx);
    let (total, _) = parse_room_list(
        pkts.iter()
            .find(|p| p[0] == opcodes::LIST_PARTY_WAITING)
            .unwrap(),
    );
    assert_eq!(total, 2);
}

#[test]
fn the_leader_can_edit_the_room_and_everyone_gets_the_new_info() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    drain(&mut rx);
    create_room(&mut world, 1, 20, 60, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::REQUEST_PARTY_MATCH_LIST],
            match_list_body(room_id, 5, 30, 45, 2, "renamed"),
        ]
        .concat(),
    );

    let room = world.matching_rooms.get(room_id).unwrap();
    assert_eq!(room.title, "renamed");
    assert_eq!((room.min_level, room.max_level), (30, 45));
    assert_eq!(room.max_members, 5);
    assert_eq!(room.loot, 2);
    assert!(has_opcode(&drain(&mut rx), opcodes::PARTY_ROOM_INFO));
    assert_eq!(
        world.matching_rooms.rooms.len(),
        1,
        "editing must not create a second room"
    );
}

#[test]
fn a_non_leader_cannot_edit_the_room() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    create_room(&mut world, 1, 20, 60, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    world
        .matching_rooms
        .get_mut(room_id)
        .unwrap()
        .members
        .push(3002);

    on_packet(
        &mut world,
        2,
        [
            vec![cop::REQUEST_PARTY_MATCH_LIST],
            match_list_body(room_id, 5, 30, 45, 2, "hijacked"),
        ]
        .concat(),
    );
    assert_eq!(world.matching_rooms.get(room_id).unwrap().title, "hunting");
}

#[test]
fn browsing_the_waiting_list_filters_by_level_class_and_name() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 70);
    set_level(&mut world, 3003, 40);
    for (oid, class) in [(3001, 10), (3002, 10), (3003, 20)] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .class_id = class;
    }
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);
    for cid in [1, 2, 3] {
        open_board(&mut world, cid);
    }
    drain(&mut a_rx);

    // Level band 1..=50 keeps 3001 and 3003.
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM,
            &waiting_list_body(1, 1, 50, &[], None),
        ),
    );
    let (total, names) = parse_waiting_list(&drain(&mut a_rx)[0]);
    assert_eq!(total, 2);
    assert_eq!(names, vec!["P3001", "P3003"]);

    // Class filter narrows to 3001.
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM,
            &waiting_list_body(1, 1, 50, &[10], None),
        ),
    );
    assert_eq!(parse_waiting_list(&drain(&mut a_rx)[0]).1, vec!["P3001"]);

    // Name query is case-insensitive on both sides (Java only lowercases the
    // name, so an upper-case query never matched there).
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM,
            &waiting_list_body(1, 1, 80, &[], Some("P3002")),
        ),
    );
    assert_eq!(parse_waiting_list(&drain(&mut a_rx)[0]).1, vec!["P3002"]);
}

// ---------------------------------------------------------------------------
// Slice 2 — join, leave, kick, disband, invite
// ---------------------------------------------------------------------------

/// Drive a join of `room_id` (or, with 0, a location+level lookup).
fn join_room(world: &mut World, client_id: u32, room_id: i32, location: i32, level: i32) {
    let mut w = PacketWriter::new();
    w.write_i32(room_id);
    w.write_i32(location);
    w.write_i32(level);
    on_packet(
        world,
        client_id,
        [vec![cop::REQUEST_PARTY_MATCH_DETAIL], w.into_bytes()].concat(),
    );
}

fn two_player_room(world: &mut World) -> i32 {
    create_room(world, 1, 1, 80, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    join_room(world, 2, room_id, -1, 40);
    room_id
}

#[test]
fn joining_a_room_notifies_both_sides() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);

    create_room(&mut world, 1, 20, 60, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    open_board(&mut world, 2);
    drain(&mut a_rx);
    drain(&mut b_rx);

    join_room(&mut world, 2, room_id, -1, 40);

    let room = world.matching_rooms.get(room_id).unwrap();
    assert_eq!(room.all_members(), vec![3001, 3002]);
    assert!(
        !world.matching_rooms.is_waiting(3002),
        "joining leaves the looking-for-party list"
    );
    assert!(world.objects.has_component::<InMatchingRoom>(&3002));

    // The leader is told someone entered and gets a refreshed member list.
    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&sm_ids::C1_HAS_ENTERED_THE_PARTY_ROOM));
    assert!(ex_subs_of(&a_pkts).contains(&opcodes::EX_PARTY_ROOM_MEMBER));

    // The joiner gets the room window + the member list, listed as a plain
    // waiting player (they are not in the leader's party).
    let b_pkts = drain(&mut b_rx);
    assert!(has_opcode(&b_pkts, opcodes::PARTY_ROOM_INFO));
    let (recipient_type, members) = b_pkts
        .iter()
        .find(|p| {
            p[0] == opcodes::EX && i16::from_le_bytes([p[1], p[2]]) == opcodes::EX_PARTY_ROOM_MEMBER
        })
        .map(|p| parse_room_members(p))
        .unwrap();
    assert_eq!(recipient_type, MatchingMemberType::WaitingPlayer.id());
    assert_eq!(
        members,
        vec![
            (3001, MatchingMemberType::PartyLeader.id()),
            (3002, MatchingMemberType::WaitingPlayer.id()),
        ]
    );
}

#[test]
fn a_joiner_outside_the_level_band_is_refused() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 75);
    drain(&mut a_rx);
    drain(&mut b_rx);
    create_room(&mut world, 1, 20, 60, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    drain(&mut b_rx);

    join_room(&mut world, 2, room_id, -1, 75);

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
    assert_eq!(
        sm_ids_of(&drain(&mut b_rx)),
        vec![sm_ids::YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_ENTER_THAT_PARTY_ROOM]
    );
}

#[test]
fn a_full_room_refuses_further_joiners() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    for oid in [3001, 3002, 3003] {
        set_level(&mut world, oid, 40);
    }
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    create_room(&mut world, 1, 1, 80, 2, "duo"); // cap 2, leader included
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    join_room(&mut world, 2, room_id, -1, 40);
    drain(&mut c_rx);
    join_room(&mut world, 3, room_id, -1, 40);

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001, 3002]
    );
    assert_eq!(
        sm_ids_of(&drain(&mut c_rx)),
        vec![sm_ids::YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_ENTER_THAT_PARTY_ROOM]
    );
}

#[test]
fn withdrawing_returns_the_member_to_the_waiting_list() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_WITHDRAW_PARTY_ROOM,
            &int_body(room_id),
        ),
    );

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
    assert!(
        world.matching_rooms.is_waiting(3002),
        "leaving a room re-registers you as looking-for-party"
    );
    assert!(!world.objects.has_component::<InMatchingRoom>(&3002));

    let b_pkts = drain(&mut b_rx);
    assert!(sm_ids_of(&b_pkts).contains(&sm_ids::YOU_HAVE_EXITED_THE_PARTY_ROOM));
    assert!(ex_subs_of(&b_pkts).contains(&opcodes::EX_CLOSE_PARTY_ROOM));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::C1_HAS_LEFT_THE_PARTY_ROOM));
}

#[test]
fn the_leader_can_kick_a_member() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_OUST_FROM_PARTY_ROOM,
            &int_body(3002),
        ),
    );

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
    assert!(
        sm_ids_of(&drain(&mut b_rx)).contains(&sm_ids::YOU_HAVE_BEEN_OUSTED_FROM_THE_PARTY_ROOM)
    );
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::C1_HAS_BEEN_KICKED_FROM_THE_PARTY_ROOM));
}

#[test]
fn a_member_cannot_kick_and_a_partymate_cannot_be_kicked() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);

    // A non-leader kicking is ignored outright.
    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_OUST_FROM_PARTY_ROOM,
            &int_body(3001),
        ),
    );
    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001, 3002]
    );

    // Once they share a party, the leader is refused too. (Java compares the
    // kicker's party with itself, so this rule never fires there.)
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_OUST_FROM_PARTY_ROOM,
            &int_body(3002),
        ),
    );
    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001, 3002],
        "a party-mate must be removed through the party UI"
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::YOU_CANNOT_DISMISS_A_PARTY_MEMBER_BY_FORCE)
    );
}

#[test]
fn the_leader_can_disband_the_room() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    drain(&mut a_rx);
    drain(&mut b_rx);

    let mut w = PacketWriter::new();
    w.write_i32(room_id);
    w.write_i32(0); // Java reads and discards a second int
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_DISMISS_PARTY_ROOM, &w.into_bytes()),
    );

    assert!(world.matching_rooms.rooms.is_empty());
    for (rx, oid) in [(&mut a_rx, 3001), (&mut b_rx, 3002)] {
        let pkts = drain(rx);
        assert!(sm_ids_of(&pkts).contains(&sm_ids::THE_PARTY_ROOM_HAS_BEEN_DISBANDED));
        assert!(ex_subs_of(&pkts).contains(&opcodes::EX_CLOSE_PARTY_ROOM));
        assert!(world.matching_rooms.is_waiting(oid));
        assert!(!world.objects.has_component::<InMatchingRoom>(&oid));
    }
}

#[test]
fn a_non_leader_cannot_disband() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);

    let mut w = PacketWriter::new();
    w.write_i32(room_id);
    w.write_i32(0);
    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_DISMISS_PARTY_ROOM, &w.into_bytes()),
    );
    assert_eq!(world.matching_rooms.rooms.len(), 1);
}

#[test]
fn the_leader_leaving_promotes_the_next_member_and_announces_it() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_WITHDRAW_PARTY_ROOM,
            &int_body(room_id),
        ),
    );

    let room = world.matching_rooms.get(room_id).unwrap();
    assert_eq!(room.leader, 3002);
    assert_eq!(room.all_members(), vec![3002]);
    assert!(
        sm_ids_of(&drain(&mut b_rx)).contains(&sm_ids::THE_LEADER_OF_THE_PARTY_ROOM_HAS_CHANGED)
    );
}

#[test]
fn a_solo_leader_leaving_deletes_the_room_without_a_leader_change_message() {
    // Java leaks the room here (see MatchingRoomManager::remove_member) and
    // still announces a leader change.
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    drain(&mut rx);
    create_room(&mut world, 1, 1, 80, 9, "solo");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_WITHDRAW_PARTY_ROOM,
            &int_body(room_id),
        ),
    );

    assert!(world.matching_rooms.rooms.is_empty());
    let sms = sm_ids_of(&drain(&mut rx));
    assert!(sms.contains(&sm_ids::YOU_HAVE_EXITED_THE_PARTY_ROOM));
    assert!(!sms.contains(&sm_ids::THE_LEADER_OF_THE_PARTY_ROOM_HAS_CHANGED));
}

#[test]
fn inviting_a_player_and_accepting_puts_them_in_the_room() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    create_room(&mut world, 1, 1, 80, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_ASK_JOIN_PARTY_ROOM,
            &name_body("P3002"),
        ),
    );
    let b_pkts = drain(&mut b_rx);
    let ask = b_pkts
        .iter()
        .find(|p| {
            p[0] == opcodes::EX
                && i16::from_le_bytes([p[1], p[2]]) == opcodes::EX_ASK_JOIN_PARTY_ROOM
        })
        .expect("the invitation dialog");
    let mut r = commons::network::PacketReader::new(&ask[3..]);
    assert_eq!(r.read_string().unwrap(), "P3001");
    assert_eq!(r.read_string().unwrap(), "hunting");
    assert!(world.objects.has_component::<PendingRequest>(&3002));

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::ANSWER_JOIN_PARTY_ROOM, &int_body(1)),
    );
    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001, 3002]
    );
    assert!(
        !world.objects.has_component::<PendingRequest>(&3002),
        "the request slot is released on both sides"
    );
    assert!(!world.objects.has_component::<PendingRequest>(&3001));
}

#[test]
fn declining_an_invitation_tells_the_inviter_and_frees_both_slots() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    create_room(&mut world, 1, 1, 80, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_ASK_JOIN_PARTY_ROOM,
            &name_body("P3002"),
        ),
    );
    drain(&mut a_rx);
    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::ANSWER_JOIN_PARTY_ROOM, &int_body(0)),
    );

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(
        &sm_ids::THE_RECIPIENT_OF_YOUR_INVITATION_DID_NOT_ACCEPT_THE_PARTY_MATCHING_INVITATION
    ));
    // Java's decline path leaves `activeRequester` set on one branch; both
    // sides must be free here.
    assert!(!world.objects.has_component::<PendingRequest>(&3001));
    assert!(!world.objects.has_component::<PendingRequest>(&3002));
}

#[test]
fn inviting_without_a_room_is_dropped_rather_than_crashing() {
    // Java builds ExAskJoinPartyRoom from `getMatchingRoom().getTitle()` with
    // no null check.
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_ASK_JOIN_PARTY_ROOM,
            &name_body("P3002"),
        ),
    );
    assert!(drain(&mut b_rx).is_empty());
    assert!(!world.objects.has_component::<PendingRequest>(&3002));
}

#[test]
fn logging_out_removes_the_player_from_the_room_and_the_waiting_list() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    drain(&mut a_rx);

    crate::game_loop::net::store_and_remove_player(&mut world, 3002);

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
    assert!(
        !world.matching_rooms.is_waiting(3002),
        "the leaver must not be left advertising themselves"
    );
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::C1_HAS_LEFT_THE_PARTY_ROOM));
}

#[test]
fn leaving_the_party_also_leaves_the_room() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    let room_id = two_player_room(&mut world);
    make_party(&mut world, &[3001, 3002], LootRule::FindersKeepers);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(&mut world, 2, vec![cop::REQUEST_WITH_DRAWAL_PARTY]);

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001]
    );
}

#[test]
fn accepting_a_party_invite_from_a_room_leader_joins_the_room() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    set_level(&mut world, 3001, 40);
    set_level(&mut world, 3002, 40);
    drain(&mut a_rx);
    drain(&mut b_rx);
    create_room(&mut world, 1, 1, 80, 9, "hunting");
    let room_id = world.matching_rooms.room_id_of(3001).unwrap();
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::REQUEST_JOIN_PARTY], join_party_body("P3002", 1)].concat(),
    );
    on_packet(
        &mut world,
        2,
        [vec![cop::REQUEST_ANSWER_JOIN_PARTY], int_body(1)].concat(),
    );

    assert_eq!(
        world.matching_rooms.get(room_id).unwrap().all_members(),
        vec![3001, 3002],
        "a party invite from the room leader also joins his room"
    );
    // ...and now they read as a real party member in the room.
    let party_id = world
        .objects
        .get_component::<PartyRef>(&3002)
        .map(|PartyRef(id)| *id);
    assert!(party_id.is_some());
}

#[test]
fn room_chat_reaches_every_member_and_nobody_else() {
    let (mut world, ..) = test_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let mut c_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    for oid in [3001, 3002, 3003] {
        set_level(&mut world, oid, 40);
    }
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);
    two_player_room(&mut world);
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain(&mut c_rx);

    let mut w = PacketWriter::new();
    w.write_string("hello room");
    w.write_i32(crate::enums::ChatType::PartyMatchRoom.client_id());
    on_packet(&mut world, 1, [vec![cop::SAY2], w.into_bytes()].concat());

    // Both members hear it (the speaker included), the outsider does not.
    for rx in [&mut a_rx, &mut b_rx] {
        let pkts = drain(rx);
        assert!(
            has_opcode(&pkts, opcodes::SAY2),
            "every room member hears room chat"
        );
    }
    assert!(!has_opcode(&drain(&mut c_rx), opcodes::SAY2));
}
