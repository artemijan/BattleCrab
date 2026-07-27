//! Mail / post (G30) — slice 3: the boot load, the offline name table, and
//! the inbox/outbox/attachable listings.

use super::*;

use crate::model::mail::{MailType, Message};
use crate::network::server_packets::opcodes;
use crate::network::server_packets::sm_ids;

const NOW: i64 = 1_700_000_000_000;
/// The real datapack — the test catalogue is empty, and the item packets need
/// real templates to serialize anything.
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn put_mail(world: &mut World, id: i32, sender: i32, receiver: i32, subject: &str) {
    world.mail.insert(Message::new_player_mail(
        id,
        sender,
        receiver,
        false,
        subject.into(),
        "body".into(),
        0,
        NOW,
    ));
}

/// Decode `ExShowReceivedPostList` into `(id, subject, counterparty, unread)`.
fn parse_inbox(p: &[u8]) -> Vec<(i32, String, String, bool)> {
    assert_eq!(p[0], opcodes::EX);
    assert_eq!(
        i16::from_le_bytes([p[1], p[2]]),
        opcodes::EX_SHOW_RECEIVED_POST_LIST
    );
    let mut r = commons::network::PacketReader::new(&p[3..]);
    r.read_i32().unwrap(); // now
    let count = r.read_i32().unwrap();
    let mut out = Vec::new();
    for _ in 0..count {
        r.read_i32().unwrap(); // mail type
        let id = r.read_i32().unwrap();
        let subject = r.read_string().unwrap();
        let who = r.read_string().unwrap();
        r.read_i32().unwrap(); // locked
        r.read_i32().unwrap(); // expiration
        let unread = r.read_i32().unwrap() == 1;
        r.read_i32().unwrap(); // deletable
        r.read_i32().unwrap(); // attachments
        r.read_i32().unwrap(); // returned
        r.read_i32().unwrap(); // sysstring
        out.push((id, subject, who, unread));
    }
    out
}

fn parse_outbox(p: &[u8]) -> Vec<(i32, String, String)> {
    assert_eq!(p[0], opcodes::EX);
    assert_eq!(
        i16::from_le_bytes([p[1], p[2]]),
        opcodes::EX_SHOW_SENT_POST_LIST
    );
    let mut r = commons::network::PacketReader::new(&p[3..]);
    r.read_i32().unwrap();
    let count = r.read_i32().unwrap();
    let mut out = Vec::new();
    for _ in 0..count {
        let id = r.read_i32().unwrap();
        let subject = r.read_string().unwrap();
        let who = r.read_string().unwrap();
        for _ in 0..5 {
            r.read_i32().unwrap();
        }
        out.push((id, subject, who));
    }
    out
}

fn ex_body_of(pkts: &[Vec<u8>], sub: i16) -> Option<Vec<u8>> {
    pkts.iter()
        .find(|p| p[0] == opcodes::EX && i16::from_le_bytes([p[1], p[2]]) == sub)
        .cloned()
}

#[test]
fn the_inbox_lists_mail_addressed_to_you_newest_first() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    put_mail(&mut world, 1, 9001, 3001, "first");
    put_mail(&mut world, 2, 9001, 3001, "second");
    put_mail(&mut world, 3, 9001, 4444, "not yours");
    world.char_ids_by_name.insert("sender".into(), 9001);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST_LIST, &[]),
    );

    let rows =
        parse_inbox(&ex_body_of(&drain(&mut rx), opcodes::EX_SHOW_RECEIVED_POST_LIST).unwrap());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, 2, "newest first");
    assert_eq!(rows[0].1, "second");
    assert_eq!(rows[1].1, "first");
    assert_eq!(
        rows[0].2, "sender",
        "an offline sender's name comes from the CharInfoTable equivalent"
    );
    assert!(rows[0].3, "fresh mail is unread");
}

#[test]
fn the_outbox_lists_mail_you_sent() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    put_mail(&mut world, 1, 3001, 9002, "to bob");
    put_mail(&mut world, 2, 9001, 3001, "inbound");
    world.char_ids_by_name.insert("bob".into(), 9002);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_SENT_POST_LIST, &[]),
    );

    let rows = parse_outbox(&ex_body_of(&drain(&mut rx), opcodes::EX_SHOW_SENT_POST_LIST).unwrap());
    assert_eq!(rows.len(), 1, "inbound mail is not in the outbox");
    assert_eq!(
        (rows[0].0, rows[0].1.as_str(), rows[0].2.as_str()),
        (1, "to bob", "bob")
    );
}

#[test]
fn a_side_that_deleted_a_message_stops_seeing_it() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    put_mail(&mut world, 1, 9001, 3001, "gone");
    world.mail.get_mut(1).unwrap().deleted_by_receiver = true;

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST_LIST, &[]),
    );
    let rows =
        parse_inbox(&ex_body_of(&drain(&mut rx), opcodes::EX_SHOW_RECEIVED_POST_LIST).unwrap());
    assert!(rows.is_empty());
}

#[test]
fn system_mail_shows_a_literal_system_sender() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    world.mail.insert(Message::new_system_mail(
        1,
        3001,
        "Happy Birthday!".into(),
        "".into(),
        MailType::Birthday,
        NOW,
    ));

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST_LIST, &[]),
    );
    let rows =
        parse_inbox(&ex_body_of(&drain(&mut rx), opcodes::EX_SHOW_RECEIVED_POST_LIST).unwrap());
    assert_eq!(rows[0].2, "System");
}

#[test]
fn entering_the_world_sends_the_unread_badge_and_the_mail_notice() {
    let (mut world, ..) = test_world();
    // Two unread + one already read.
    put_mail(&mut world, 1, 9001, 3001, "a");
    put_mail(&mut world, 2, 9001, 3001, "b");
    put_mail(&mut world, 3, 9001, 3001, "c");
    world.mail.get_mut(3).unwrap().unread = false;

    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    crate::game_loop::mail::on_enter_world(&world, 3001);
    let pkts = drain(&mut rx);

    let badge = ex_body_of(&pkts, opcodes::EX_UN_READ_MAIL_COUNT).expect("unread badge");
    let mut r = commons::network::PacketReader::new(&badge[3..]);
    assert_eq!(r.read_i32().unwrap(), 2);
    assert!(ex_body_of(&pkts, opcodes::EX_NOTICE_POST_ARRIVED).is_some());
}

#[test]
fn mail_off_suppresses_the_listings_entirely() {
    let (mut world, ..) = test_world();
    world.cfg.general.allow_mail = false;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    put_mail(&mut world, 1, 9001, 3001, "hidden");

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST_LIST, &[]),
    );
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_SENT_POST_LIST, &[]),
    );
    assert!(drain(&mut rx).is_empty());
}

#[test]
fn the_attachable_item_list_needs_a_peace_zone() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);
    // `ingame_player` spawns outside any zone, so this is the non-peace path.
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ITEM_LIST, &[]),
    );
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![sm_ids::YOU_CANNOT_RECEIVE_OR_SEND_MAIL_WITH_ATTACHED_ITEMS_IN_NON_PEACE_ZONE_REGIONS]
    );
}

#[test]
fn the_attachable_item_list_returns_unequipped_non_quest_items_in_a_peace_zone() {
    let (mut world, ..) = test_world();
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::ZoneFlags>(&3001)
        .unwrap()
        .mask = crate::data::zone_data::ZoneKind::Peace.bit();
    world.id_pool = 0x5000_0000..0x5000_0100;
    crate::game_loop::items::add_inventory_item(&mut world, 3001, 57, 5000);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ITEM_LIST, &[]),
    );
    let pkt = ex_body_of(&drain(&mut rx), opcodes::EX_REPLY_POST_ITEM_LIST).expect("item list");
    let mut r = commons::network::PacketReader::new(&pkt[3..]);
    assert!(r.read_i32().unwrap() >= 1, "adena is attachable");
}

#[test]
fn the_boot_load_installs_messages_attachments_and_the_name_table() {
    let (mut world, ..) = test_world();
    let rows = vec![crate::character::ItemRow {
        object_id: 7001,
        item_id: 57,
        count: 100,
        enchant_level: 0,
        loc: "MAIL".into(),
        loc_data: 5,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    }];
    crate::game_loop::mail::on_loaded(
        &mut world,
        vec![Message::new_player_mail(
            5,
            9001,
            3001,
            true,
            "cod".into(),
            "pay up".into(),
            500,
            NOW,
        )],
        vec![(5, rows)],
        vec![("Alice".to_lowercase(), 9001)],
    );

    let m = world.mail.get(5).expect("message restored");
    assert!(m.is_locked() && m.req_adena == 500);
    assert_eq!(world.mail.attachments.get(&5).unwrap().items().len(), 1);
    assert_eq!(
        crate::game_loop::mail::char_id_by_name(&world, "ALICE"),
        Some(9001),
        "name lookup is case-insensitive"
    );
    assert_eq!(
        crate::game_loop::mail::char_name_by_id(&world, 9001),
        "alice"
    );
}
