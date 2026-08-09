//! Mail / post (G30) — slice 3: the boot load, the offline name table, and
//! the inbox/outbox/attachable listings.

use super::*;

use crate::model::inventory::Inventory;
use crate::model::mail::{MailType, Message};
use crate::network::server_packets::opcodes;
use crate::network::server_packets::sm_ids;

const NOW: i64 = 1_700_000_000_000;
/// The real datapack — the test catalogue is empty, and the item packets need
/// real templates to serialize anything.
const DIST: &str = crate::data::DIST_GAME;

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
fn entering_the_world_with_no_mail_sends_no_indicator_at_all() {
    // Regression: both enter-world packets were sent unconditionally, so a
    // character with an empty mailbox got the client's mail indicator lit and
    // found nothing behind it. Java gates both on `hasUnreadPost`.
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    crate::game_loop::mail::on_enter_world(&world, 3001);

    let pkts = drain(&mut rx);
    assert!(
        ex_body_of(&pkts, opcodes::EX_NOTICE_POST_ARRIVED).is_none(),
        "no mail must mean no indicator"
    );
    assert!(
        ex_body_of(&pkts, opcodes::EX_UN_READ_MAIL_COUNT).is_none(),
        "and no badge either"
    );
}

#[test]
fn entering_the_world_with_only_read_mail_sends_no_indicator() {
    // The inbox is non-empty but nothing in it is unread — still no indicator.
    let (mut world, ..) = test_world();
    put_mail(&mut world, 1, 9001, 3001, "already read");
    world.mail.get_mut(1).unwrap().unread = false;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    crate::game_loop::mail::on_enter_world(&world, 3001);
    assert!(ex_body_of(&drain(&mut rx), opcodes::EX_NOTICE_POST_ARRIVED).is_none());
}

#[test]
fn mail_deleted_by_the_receiver_does_not_light_the_indicator() {
    // Java's `hasUnreadPost` ignores `isDeletedByReceiver` and so *would* light
    // it here; the port uses the consistent inbox-based definition, matching
    // what the mailbox actually shows.
    let (mut world, ..) = test_world();
    put_mail(&mut world, 1, 9001, 3001, "deleted");
    world.mail.get_mut(1).unwrap().deleted_by_receiver = true;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    crate::game_loop::mail::on_enter_world(&world, 3001);
    assert!(ex_body_of(&drain(&mut rx), opcodes::EX_NOTICE_POST_ARRIVED).is_none());
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
        Vec::new(),
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

// ---------------------------------------------------------------------------
// Slice 4 — send, read, delete
// ---------------------------------------------------------------------------

/// A world where both players are in a peace zone with real item templates and
/// an id pool, i.e. able to actually mail each other.
fn mail_world() -> (
    World,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    db::CmdRx,
) {
    let (mut world, _tx, db_rx, _link) = test_world();
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    world.id_pool = 0x5000_0000..0x5000_1000;
    let a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let b_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    for oid in [3001, 3002] {
        world
            .objects
            .get_component_mut::<crate::model::components::ZoneFlags>(&oid)
            .unwrap()
            .mask = crate::data::zone_data::ZoneKind::Peace.bit();
        crate::game_loop::mail::on_character_created(&mut world, &format!("P{oid}"), oid);
    }
    (world, a_rx, b_rx, db_rx)
}

fn send_post_body(
    receiver: &str,
    is_cod: bool,
    subject: &str,
    text: &str,
    items: &[(i32, i64)],
    req_adena: i64,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(receiver);
    w.write_i32(is_cod as i32);
    w.write_string(subject);
    w.write_string(text);
    w.write_i32(items.len() as i32);
    for (oid, count) in items {
        w.write_i32(*oid);
        w.write_i64(*count);
    }
    w.write_i64(req_adena);
    w.into_bytes()
}

fn adena_of(world: &World, oid: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map_or(0, |inv| inv.adena())
}

fn give_adena(world: &mut World, oid: i32, count: i64) {
    crate::game_loop::items::add_inventory_item(world, oid, 57, count);
}

#[test]
fn sending_a_mail_charges_the_flat_fee_and_reaches_the_recipient() {
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", false, "hi", "there", &[], 0),
        ),
    );

    assert_eq!(adena_of(&world, 3001), 10_000 - 100, "flat 100 adena fee");
    let inbox = world.mail.inbox(3002);
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].subject, "hi");
    assert_eq!(inbox[0].content, "there");
    assert!(inbox[0].unread && !inbox[0].has_attachments);

    let a_pkts = drain(&mut a_rx);
    assert!(sm_ids_of(&a_pkts).contains(&sm_ids::MAIL_SUCCESSFULLY_SENT));
    assert!(ex_body_of(&a_pkts, opcodes::EX_REPLY_WRITE_POST).is_some());
    // The recipient is chimed and re-badged live.
    let b_pkts = drain(&mut b_rx);
    assert!(ex_body_of(&b_pkts, opcodes::EX_NOTICE_POST_ARRIVED).is_some());
    let badge = ex_body_of(&b_pkts, opcodes::EX_UN_READ_MAIL_COUNT).unwrap();
    assert_eq!(
        commons::network::PacketReader::new(&badge[3..])
            .read_i32()
            .unwrap(),
        1
    );
}

#[test]
fn each_attachment_slot_adds_a_thousand_adena_to_the_fee() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    crate::game_loop::items::add_inventory_item(&mut world, 3001, 1060, 3); // healing potions
    let potion_oid = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 1060)
        .unwrap()
        .object_id;
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", false, "gift", "", &[(potion_oid, 2)], 0),
        ),
    );

    assert_eq!(
        adena_of(&world, 3001),
        10_000 - (100 + 1000),
        "100 flat + 1000 per attached slot"
    );
    let msg_id = world.mail.inbox(3002)[0].id;
    assert!(world.mail.get(msg_id).unwrap().has_attachments);
    let attached = world.mail.attachments.get(&msg_id).unwrap();
    assert_eq!(attached.items().len(), 1);
    assert_eq!(attached.items()[0].count, 2);
    // The partial stack stays with the sender under its original object id.
    let left = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .item_by_object_id(potion_oid);
    assert_eq!(left.map(|(_, c)| c), Some(1));
}

#[test]
fn a_sender_who_cannot_cover_the_fee_is_refused() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 50); // fee is 100
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", false, "hi", "", &[], 0),
        ),
    );
    assert!(world.mail.inbox(3002).is_empty());
    assert_eq!(adena_of(&world, 3001), 50, "nothing is charged on refusal");
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::YOU_CANNOT_FORWARD_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA)
    );
}

#[test]
fn attached_adena_cannot_also_pay_the_fee() {
    // 1000 adena on hand, all of it attached: the 1100 fee is unpayable.
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 1000);
    let adena_oid = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", false, "all of it", "", &[(adena_oid, 1000)], 0),
        ),
    );
    assert!(world.mail.inbox(3002).is_empty());
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::YOU_CANNOT_FORWARD_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA)
    );
}

#[test]
fn mail_to_an_unknown_name_or_to_yourself_is_refused() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("Nobody", false, "hi", "", &[], 0),
        ),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::WHEN_THE_RECIPIENT_DOESN_T_EXIST_SENDING_MAIL_IS_NOT_POSSIBLE)
    );

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3001", false, "hi", "", &[], 0),
        ),
    );
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::YOU_CANNOT_SEND_A_MAIL_TO_YOURSELF));
    assert_eq!(adena_of(&world, 3001), 10_000);
}

#[test]
fn mail_can_be_addressed_to_an_offline_character() {
    // The whole reason the CharInfoTable equivalent exists.
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    crate::game_loop::mail::on_character_created(&mut world, "Ghost", 4242);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("ghost", false, "knock", "", &[], 0),
        ),
    );
    assert_eq!(
        world.mail.inbox(4242).len(),
        1,
        "name lookup is case-insensitive and works offline"
    );
}

#[test]
fn a_cod_mail_needs_a_price_and_an_item() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", true, "pay", "", &[], 0),
        ),
    );
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(
        &sm_ids::WHEN_NOT_ENTERING_THE_AMOUNT_FOR_THE_PAYMENT_REQUEST_YOU_CANNOT_SEND_ANY_MAIL
    ));

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", true, "pay", "", &[], 500),
        ),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::IT_S_A_PAYMENT_REQUEST_TRANSACTION_PLEASE_ATTACH_THE_ITEM)
    );
    assert!(world.mail.inbox(3002).is_empty());
}

#[test]
fn attachments_off_still_delivers_the_message_without_them() {
    // Java coerces rather than rejecting.
    let (mut world, mut a_rx, _b, _db) = mail_world();
    world.cfg.general.allow_attachments = false;
    give_adena(&mut world, 3001, 10_000);
    let adena_oid = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", true, "hi", "", &[(adena_oid, 500)], 500),
        ),
    );
    let inbox = world.mail.inbox(3002);
    assert_eq!(inbox.len(), 1, "the message still goes");
    assert!(!inbox[0].has_attachments);
    assert_eq!(inbox[0].req_adena, 0, "the payment request is stripped too");
    assert_eq!(adena_of(&world, 3001), 10_000 - 100, "no per-slot fee");
}

#[test]
fn opening_an_inbox_message_marks_it_read_and_refreshes_the_badge() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "read me");
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST, &int_body(77)),
    );

    let pkts = drain(&mut b_rx);
    let body = ex_body_of(&pkts, opcodes::EX_SHOW_RECEIVED_POST).expect("the opened message");
    let mut r = commons::network::PacketReader::new(&body[3..]);
    r.read_i32().unwrap(); // mail type
    assert_eq!(r.read_i32().unwrap(), 77);
    r.read_i32().unwrap(); // locked
    r.read_i32().unwrap(); // unknown
    assert_eq!(r.read_string().unwrap(), "P3001", "sender name");
    assert_eq!(r.read_string().unwrap(), "read me");
    assert_eq!(r.read_string().unwrap(), "body");

    assert!(!world.mail.get(77).unwrap().unread);
    assert!(ex_body_of(&pkts, opcodes::EX_CHANGE_POST_STATE).is_some());
    let badge = ex_body_of(&pkts, opcodes::EX_UN_READ_MAIL_COUNT).unwrap();
    assert_eq!(
        commons::network::PacketReader::new(&badge[3..])
            .read_i32()
            .unwrap(),
        0
    );
}

#[test]
fn you_cannot_open_someone_elses_message() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    put_mail(&mut world, 77, 9999, 3002, "private");
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_RECEIVED_POST, &int_body(77)),
    );
    // No mail packet comes back — only the illegal-action warning (the probe
    // punishes with `DefaultPunish` now, as in Java).
    let pkts = drain(&mut a_rx);
    assert!(ex_body_of(&pkts, opcodes::EX_SHOW_RECEIVED_POST).is_none());
    assert!(world.mail.get(77).unwrap().unread, "still unread");
    advance_ticks(&mut world, 51);
    assert!(!world.clients.contains_key(&1), "kicked for the probe");
}

#[test]
fn opening_a_sent_message_does_not_mark_it_read() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "sent");
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_SENT_POST, &int_body(77)),
    );
    let pkts = drain(&mut a_rx);
    assert!(ex_body_of(&pkts, opcodes::EX_SHOW_SENT_POST).is_some());
    assert!(
        ex_body_of(&pkts, opcodes::EX_CHANGE_POST_STATE).is_none(),
        "Java sends no state change for the outbox"
    );
    assert!(world.mail.get(77).unwrap().unread);
}

#[test]
fn deleting_from_the_inbox_hides_it_and_keeps_the_senders_copy() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "bye");
    drain(&mut b_rx);

    let mut w = PacketWriter::new();
    w.write_i32(1);
    w.write_i32(77);
    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_DELETE_RECEIVED_POST,
            &w.into_bytes(),
        ),
    );

    let m = world.mail.get(77).expect("the row survives for the sender");
    assert!(m.deleted_by_receiver && !m.deleted_by_sender);
    assert!(world.mail.inbox(3002).is_empty());
    assert_eq!(world.mail.outbox(3001).len(), 1);
    assert!(ex_body_of(&drain(&mut b_rx), opcodes::EX_CHANGE_POST_STATE).is_some());
}

#[test]
fn a_message_both_sides_deleted_is_dropped_entirely() {
    let (mut world, mut a_rx, mut b_rx, mut db_rx) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "bye");
    drain(&mut a_rx);
    drain(&mut b_rx);
    drain_db(&mut db_rx);

    let del = |id: i32| {
        let mut w = PacketWriter::new();
        w.write_i32(1);
        w.write_i32(id);
        w.into_bytes()
    };
    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_DELETE_RECEIVED_POST, &del(77)),
    );
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_DELETE_SENT_POST, &del(77)),
    );

    assert!(
        world.mail.get(77).is_none(),
        "row gone once both sides drop it"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::DeleteMail { message_id: 77 })),
        "and the delete is persisted"
    );
}

#[test]
fn a_delete_batch_aborts_on_a_message_that_still_has_attachments() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "plain");
    put_mail(&mut world, 78, 3001, 3002, "with item");
    world.mail.get_mut(78).unwrap().has_attachments = true;
    drain(&mut b_rx);

    let mut w = PacketWriter::new();
    w.write_i32(2);
    w.write_i32(77);
    w.write_i32(78);
    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_DELETE_RECEIVED_POST,
            &w.into_bytes(),
        ),
    );

    // Java returns out of the whole loop, so *neither* is deleted.
    assert!(!world.mail.get(77).unwrap().deleted_by_receiver);
    assert!(!world.mail.get(78).unwrap().deleted_by_receiver);
    assert!(drain(&mut b_rx).is_empty());
}

// ---------------------------------------------------------------------------
// Slice 5 — attachments, COD, cancel/reject, expiry
// ---------------------------------------------------------------------------

/// Put a message carrying `count` of `item_id` in `receiver`'s inbox.
fn mail_with_item(
    world: &mut World,
    id: i32,
    sender: i32,
    receiver: i32,
    item_id: i32,
    count: i64,
    req_adena: i64,
) {
    let mut m = Message::new_player_mail(
        id,
        sender,
        receiver,
        req_adena > 0,
        "parcel".into(),
        "".into(),
        req_adena,
        commons::util::now_millis(),
    );
    m.has_attachments = true;
    world.mail.insert(m);
    let oid = world.alloc_object_id().unwrap();
    let catalog = &world.data.item_data;
    world
        .mail
        .attachments
        .entry(id)
        .or_default()
        .insert_instance(catalog, oid, item_id, count, 0, -1);
}

fn count_of(world: &World, oid: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map_or(0, |inv| inv.count_of(item_id))
}

#[test]
fn receiving_an_attachment_moves_the_item_and_clears_the_flag() {
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 5, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );

    assert_eq!(
        count_of(&world, 3002, 1060),
        5,
        "the receiver got the items"
    );
    let m = world.mail.get(77).unwrap();
    assert!(!m.has_attachments);
    assert!(world.mail.attachments.get(&77).is_none());

    let b_pkts = drain(&mut b_rx);
    let sms = sm_ids_of(&b_pkts);
    assert!(sms.contains(&sm_ids::YOU_HAVE_ACQUIRED_S2_S1));
    assert!(sms.contains(&sm_ids::MAIL_SUCCESSFULLY_RECEIVED));
    // The sender is told their parcel was collected.
    assert!(
        sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::S1_ACQUIRED_THE_ATTACHED_ITEM_TO_YOUR_MAIL)
    );
}

#[test]
fn a_cod_receiver_pays_and_the_sender_is_credited() {
    // The G30 mail gate: mail an item COD, receiver pays, sender gets adena.
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 1, 500);
    give_adena(&mut world, 3002, 1000);
    let sender_before = adena_of(&world, 3001);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );

    assert_eq!(count_of(&world, 3002, 1060), 1);
    assert_eq!(adena_of(&world, 3002), 500, "the COD price was charged");
    assert_eq!(
        adena_of(&world, 3001),
        sender_before + 500,
        "and paid to the sender"
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::S2_HAS_MADE_A_PAYMENT_OF_S1_ADENA_PER_YOUR_PAYMENT_REQUEST_MAIL)
    );
}

#[test]
fn a_cod_receiver_who_cannot_pay_gets_nothing() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 1, 500);
    give_adena(&mut world, 3002, 100);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );

    assert_eq!(count_of(&world, 3002, 1060), 0);
    assert_eq!(adena_of(&world, 3002), 100, "nothing was charged");
    assert!(world.mail.get(77).unwrap().has_attachments);
    assert!(
        sm_ids_of(&drain(&mut b_rx))
            .contains(&sm_ids::YOU_CANNOT_RECEIVE_BECAUSE_YOU_DON_T_HAVE_ENOUGH_ADENA)
    );
}

#[test]
fn an_offline_senders_cod_payment_is_delivered_by_mail() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    // 4242 has a name but no session/inventory: an offline sender.
    crate::game_loop::mail::on_character_created(&mut world, "Ghost", 4242);
    mail_with_item(&mut world, 77, 4242, 3002, 1060, 1, 500);
    give_adena(&mut world, 3002, 1000);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );

    assert_eq!(adena_of(&world, 3002), 500);
    let payout = world.mail.inbox(4242);
    assert_eq!(payout.len(), 1, "the offline sender is paid by mail");
    assert!(payout[0].has_attachments);
    let attached = world.mail.attachments.get(&payout[0].id).unwrap();
    assert_eq!(attached.items()[0].item_id, 57);
    assert_eq!(attached.items()[0].count, 500);
}

#[test]
fn the_sender_can_cancel_and_take_the_items_back() {
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 3, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_CANCEL_POST_ATTACHMENT,
            &int_body(77),
        ),
    );

    assert_eq!(count_of(&world, 3001, 1060), 3, "items returned to sender");
    assert!(world.mail.get(77).is_none(), "the mail is gone entirely");
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::MAIL_SUCCESSFULLY_CANCELLED));
    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&sm_ids::S1_CANCELED_THE_SENT_MAIL));
}

#[test]
fn cancelling_after_the_receiver_took_the_items_is_refused() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 3, 0);
    // Receiver already collected.
    world.mail.attachments.remove(&77);
    world.mail.get_mut(77).unwrap().has_attachments = false;
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_CANCEL_POST_ATTACHMENT,
            &int_body(77),
        ),
    );
    assert!(
        sm_ids_of(&drain(&mut a_rx))
            .contains(&sm_ids::YOU_CANNOT_CANCEL_SENT_MAIL_SINCE_THE_RECIPIENT_RECEIVED_IT)
    );
    assert_eq!(count_of(&world, 3001, 1060), 0);
}

#[test]
fn rejecting_returns_the_parcel_to_the_sender_as_a_new_message() {
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 2, 500);
    drain(&mut a_rx);
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(
            cp::ex_opcodes::REQUEST_REJECT_POST_ATTACHMENT,
            &int_body(77),
        ),
    );

    // The original loses its items but the row stays.
    let original = world.mail.get(77).expect("original row survives");
    assert!(!original.has_attachments);
    // A returned message lands in the sender's inbox holding the container.
    let back = world.mail.inbox(3001);
    assert_eq!(back.len(), 1);
    assert!(back[0].returned && back[0].has_attachments);
    let return_id = back[0].id;
    let attached = world.mail.attachments.get(&return_id).unwrap();
    assert_eq!(attached.items()[0].item_id, 1060);
    assert_eq!(attached.items()[0].count, 2);

    assert!(sm_ids_of(&drain(&mut b_rx)).contains(&sm_ids::MAIL_SUCCESSFULLY_RETURNED));
    assert!(sm_ids_of(&drain(&mut a_rx)).contains(&sm_ids::S1_RETURNED_THE_MAIL));
}

#[test]
fn attachment_actions_need_a_peace_zone() {
    let (mut world, _a, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 1, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::ZoneFlags>(&3002)
        .unwrap()
        .mask = 0;
    drain(&mut b_rx);

    on_packet(
        &mut world,
        2,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );
    assert_eq!(count_of(&world, 3002, 1060), 0);
    assert!(
        sm_ids_of(&drain(&mut b_rx))
            .contains(&sm_ids::YOU_CANNOT_RECEIVE_IN_A_NON_PEACE_ZONE_LOCATION)
    );
}

#[test]
fn you_cannot_take_an_attachment_addressed_to_someone_else() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    mail_with_item(&mut world, 77, 9999, 3002, 1060, 1, 0);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_POST_ATTACHMENT, &int_body(77)),
    );
    assert_eq!(count_of(&world, 3001, 1060), 0);
    assert!(world.mail.get(77).unwrap().has_attachments);
}

#[test]
fn an_expired_mail_is_deleted_and_its_parcel_goes_to_the_senders_warehouse() {
    let (mut world, mut a_rx, mut b_rx, _db) = mail_world();
    mail_with_item(&mut world, 77, 3001, 3002, 1060, 4, 0);
    // Make it already due.
    world.mail.get_mut(77).unwrap().expiration = commons::util::now_millis() - 1;
    world
        .objects
        .add_components(&3001, crate::model::inventory::Warehouse::default());
    drain(&mut a_rx);
    drain(&mut b_rx);

    crate::game_loop::mail::handle_expiry(&mut world, 77);

    assert!(world.mail.get(77).is_none(), "the message is dropped");
    let wh = world
        .objects
        .get_component::<crate::model::inventory::Warehouse>(&3001)
        .unwrap();
    assert_eq!(wh.0.count_of(1060), 4, "the parcel went to the warehouse");
    for rx in [&mut a_rx, &mut b_rx] {
        assert!(
            sm_ids_of(&drain(rx))
                .contains(&sm_ids::THE_MAIL_WAS_RETURNED_DUE_TO_THE_EXCEEDED_WAITING_TIME)
        );
    }
}

#[test]
fn an_expiry_timer_that_fires_early_re_arms_instead_of_deleting() {
    let (mut world, _a, _b, _db) = mail_world();
    put_mail(&mut world, 77, 3001, 3002, "later");
    world.mail.get_mut(77).unwrap().expiration = commons::util::now_millis() + 3_600_000;

    crate::game_loop::mail::handle_expiry(&mut world, 77);
    assert!(
        world.mail.get(77).is_some(),
        "a message that is not due yet must survive its timer"
    );
}

#[test]
fn sending_a_mail_arms_its_expiry_timer() {
    let (mut world, mut a_rx, _b, _db) = mail_world();
    give_adena(&mut world, 3001, 10_000);
    drain(&mut a_rx);

    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::REQUEST_SEND_POST,
            &send_post_body("P3002", false, "hi", "", &[], 0),
        ),
    );
    let msg_id = world.mail.inbox(3002)[0].id;
    // 15 days out, so the message must still be alive well before then.
    let expiration = world.mail.get(msg_id).unwrap().expiration;
    assert!(expiration > commons::util::now_millis() + 14 * 86_400_000);
}

// ---------------------------------------------------------------------------
// Custom mail manager (`Custom/CustomMailManager.ini`)
// ---------------------------------------------------------------------------

fn custom_row(receiver: i32, items: &str) -> crate::db::CustomMailRow {
    crate::db::CustomMailRow {
        date: "2026-08-01 12:00:00".into(),
        receiver,
        subject: "A gift".into(),
        message: "Enjoy.".into(),
        items: items.into(),
    }
}

/// A row for an **online** character becomes a real message with attachments,
/// and the row is deleted. The recipient gets the arrival chime.
#[test]
fn a_custom_mail_row_is_delivered_and_deleted() {
    use crate::db::DbCommand;

    let (mut world, _tx, mut db_rx, _l) = test_world();
    world.id_pool = 0x4C00_0000..0x4C00_0100;
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = 57;
    t.name = "Adena".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    drain(&mut rx);
    drain_db(&mut db_rx);

    crate::game_loop::custom_mail::apply_loaded(&mut world, vec![custom_row(3001, "57 1000")]);

    let msg = world
        .mail
        .messages
        .values()
        .find(|m| m.receiver_id == 3001)
        .expect("a message was created");
    assert_eq!(msg.subject, "A gift");
    assert!(msg.has_attachments, "the item list makes it an attachment");
    let attached = world.mail.attachments.get(&msg.id).expect("attachments");
    assert_eq!(attached.items().len(), 1);
    assert_eq!(attached.items()[0].item_id, 57);
    assert_eq!(attached.items()[0].count, 1000);

    // The row is removed, keyed by (date, receiver) like Java.
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            DbCommand::DeleteCustomMail { receiver: 3001, date } if date == "2026-08-01 12:00:00"
        )),
        "the delivered row is deleted"
    );
    assert!(
        !drain(&mut rx).is_empty(),
        "the recipient is told mail arrived"
    );
}

/// **An offline recipient's row is left alone** — not delivered, not deleted —
/// so the gift waits for them to log in.
#[test]
fn an_offline_recipient_keeps_their_row() {
    use crate::db::DbCommand;

    let (mut world, _tx, mut db_rx, _l) = test_world();
    world.id_pool = 0x4C00_0200..0x4C00_0300;
    drain_db(&mut db_rx);

    crate::game_loop::custom_mail::apply_loaded(&mut world, vec![custom_row(9999, "57 1000")]);

    assert!(
        world.mail.messages.is_empty(),
        "nothing delivered to an offline character"
    );
    assert!(
        !drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::DeleteCustomMail { .. })),
        "and the row survives for a later poll"
    );
}

/// A row with no items is an ordinary letter: delivered, no attachments.
#[test]
fn a_row_without_items_is_a_plain_letter() {
    let (mut world, _tx, _db, _l) = test_world();
    world.id_pool = 0x4C00_0400..0x4C00_0500;
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    crate::game_loop::custom_mail::apply_loaded(&mut world, vec![custom_row(3001, "")]);

    let msg = world
        .mail
        .messages
        .values()
        .find(|m| m.receiver_id == 3001)
        .expect("delivered");
    assert!(!msg.has_attachments);
    assert!(world.mail.attachments.get(&msg.id).is_none());
}
