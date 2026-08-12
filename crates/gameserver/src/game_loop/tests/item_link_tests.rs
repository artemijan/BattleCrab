//! Shift-clicked chat item links: `Say2.parseAndPublishItem` plus the
//! `RequestExRqItemLink` / `ExRpItemLink` pair that fills the "?" a reader
//! clicks in the chat line.

use super::*;

const LINKED_ITEM: i32 = 9101;

/// The markup the client wraps around a shift-clicked item: `\x08` … `\x08`,
/// with the inventory object id in the `ID=` field.
fn item_link(object_id: i32, title: &str) -> String {
    format!("\u{8}\tType=1\tID={object_id}\tColor=0\tUnderline=0\tTitle={title}\u{8}")
}

fn link_world() -> (
    World,
    db::CmdTx,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db_tx, db_rx, link_rx) = test_world();
    world.id_pool = 0x2000_0000..0x2000_1000; // object ids for the granted items
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::ItemTemplate {
            item_id: LINKED_ITEM,
            name: "Squire's Sword".into(),
            kind: crate::data::item_data::ItemKind::Weapon,
            type2: 0,
            body_part: 0x0080, // R-hand
            is_stackable: false,
            is_infinite: false,
            is_sellable: true,
            price: 100,
            ..Default::default()
        });
    (world, db_tx, db_rx, link_rx)
}

fn say(world: &mut World, client_id: u32, text: &str) {
    on_packet(
        world,
        client_id,
        [vec![cop::SAY2], say2_body(text, 0, None)].concat(),
    );
}

fn request_item_link(world: &mut World, client_id: u32, object_id: i32) {
    on_packet(world, client_id, ex_packet(0x1E, &object_id.to_le_bytes()));
}

/// The `ExRpItemLink` answer, decoded: (object id, item id, count, body part).
fn parse_item_link_answer(pkt: &[u8]) -> (i32, i32, i64, i64) {
    assert_eq!(pkt[0], server_packets::opcodes::EX, "an extended packet");
    assert_eq!(
        i16::from_le_bytes([pkt[1], pkt[2]]),
        0x6D,
        "ExRpItemLink sub-opcode"
    );
    // writeItem: mask u8, object id, item id, T1 u8, count i64, type2 u8,
    // customType1 u8, equipped i16, body part i64, …
    let oid = i32::from_le_bytes(pkt[4..8].try_into().unwrap());
    let item_id = i32::from_le_bytes(pkt[8..12].try_into().unwrap());
    let count = i64::from_le_bytes(pkt[13..21].try_into().unwrap());
    let body_part = i64::from_le_bytes(pkt[25..33].try_into().unwrap());
    (oid, item_id, count, body_part)
}

fn item_link_answers(pkts: &[Vec<u8>]) -> Vec<&Vec<u8>> {
    pkts.iter()
        .filter(|p| p[0] == server_packets::opcodes::EX && i16::from_le_bytes([p[1], p[2]]) == 0x6D)
        .collect()
}

/// The whole round trip: A shift-clicks an item into general chat, B hears the
/// line and clicks the link, and the server answers with that item's row.
#[test]
fn a_shift_clicked_item_can_be_inspected_by_the_reader() {
    let (mut world, ..) = link_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let item_oid = crate::game_loop::items::add_inventory_item(&mut world, 3001, LINKED_ITEM, 1)
        .expect("granted")[0];
    drain(&mut a_rx);
    drain(&mut b_rx);

    say(
        &mut world,
        1,
        &format!("look {}", item_link(item_oid, "Squire's Sword")),
    );
    assert!(
        has_opcode(&drain(&mut b_rx), server_packets::opcodes::SAY2),
        "the line carrying the link is broadcast"
    );
    assert_eq!(
        world.published_items.get(&item_oid),
        Some(&3001),
        "the linked item is published by its owner"
    );
    drain(&mut a_rx);

    request_item_link(&mut world, 2, item_oid);

    let pkts = drain(&mut b_rx);
    let answers = item_link_answers(&pkts);
    assert_eq!(answers.len(), 1, "exactly one ExRpItemLink");
    let (oid, item_id, count, body_part) = parse_item_link_answer(answers[0]);
    assert_eq!(oid, item_oid);
    assert_eq!(item_id, LINKED_ITEM);
    assert_eq!(count, 1);
    assert_eq!(
        body_part, 0x0080,
        "the template's slot, as ItemList writes it"
    );
}

/// Java's `item.isPublished()` gate: an object id nobody linked is never
/// answered, so a client cannot read a stranger's inventory by guessing ids.
#[test]
fn an_unpublished_item_is_never_answered() {
    let (mut world, ..) = link_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let item_oid = crate::game_loop::items::add_inventory_item(&mut world, 3001, LINKED_ITEM, 1)
        .expect("granted")[0];
    drain(&mut a_rx);
    drain(&mut b_rx);

    request_item_link(&mut world, 2, item_oid);

    assert!(
        item_link_answers(&drain(&mut b_rx)).is_empty(),
        "never shift-clicked into chat, so no answer"
    );
}

/// Java logs "trying publish item which does not own" and returns false, which
/// drops the whole line — a player cannot link someone else's item.
#[test]
fn linking_an_item_you_do_not_own_drops_the_line() {
    let (mut world, ..) = link_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let other_oid = crate::game_loop::items::add_inventory_item(&mut world, 3002, LINKED_ITEM, 1)
        .expect("granted")[0];
    drain(&mut a_rx);
    drain(&mut b_rx);

    say(
        &mut world,
        1,
        &format!("mine {}", item_link(other_oid, "Squire's Sword")),
    );

    assert!(
        !has_opcode(&drain(&mut b_rx), server_packets::opcodes::SAY2),
        "the line is dropped whole"
    );
    assert!(
        !world.published_items.contains_key(&other_oid),
        "and nothing is published"
    );
}

/// Java: "Allow higher limit if player shift some item (text is longer then)"
/// — 500 chars with a link, 105 without.
#[test]
fn an_item_link_raises_the_chat_length_cap() {
    let (mut world, ..) = link_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let item_oid = crate::game_loop::items::add_inventory_item(&mut world, 3001, LINKED_ITEM, 1)
        .expect("granted")[0];
    drain(&mut a_rx);
    drain(&mut b_rx);

    // A link plus enough padding to clear the 105-char cap but stay under 500.
    let line = format!(
        "{}{}",
        "x".repeat(120),
        item_link(item_oid, "Squire's Sword")
    );
    assert!(line.chars().count() > 105 && line.chars().count() < 500);
    say(&mut world, 1, &line);
    assert!(
        has_opcode(&drain(&mut b_rx), server_packets::opcodes::SAY2),
        "a linked line over 105 chars is still said"
    );
    drain(&mut a_rx);

    // Past 500 even a linked line is spam.
    let long = format!(
        "{}{}",
        "x".repeat(500),
        item_link(item_oid, "Squire's Sword")
    );
    say(&mut world, 1, &long);
    assert_eq!(
        ids_after_opcode(&drain(&mut a_rx), server_packets::opcodes::SYSTEM_MESSAGE),
        vec![server_packets::sm_ids::KEYBOARD_INPUT_SPAM_WARNING]
    );
    assert!(
        !has_opcode(&drain(&mut b_rx), server_packets::opcodes::SAY2),
        "and it is not broadcast"
    );
}

/// The publish flag dies with the player, as Java's per-`Item` `_published`
/// does when the instance is dropped at logout.
#[test]
fn logging_out_kills_the_publishers_links() {
    let (mut world, ..) = link_world();
    let mut a_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut b_rx = ingame_player(&mut world, 2, 3002, 100, 0, 0);
    let item_oid = crate::game_loop::items::add_inventory_item(&mut world, 3001, LINKED_ITEM, 1)
        .expect("granted")[0];
    drain(&mut a_rx);
    drain(&mut b_rx);
    say(
        &mut world,
        1,
        &format!("wts {}", item_link(item_oid, "Squire's Sword")),
    );
    drain(&mut a_rx);
    drain(&mut b_rx);

    crate::game_loop::net::on_disconnect(&mut world, 1);

    assert!(
        !world.published_items.contains_key(&item_oid),
        "the publisher's links are forgotten"
    );
    request_item_link(&mut world, 2, item_oid);
    assert!(
        item_link_answers(&drain(&mut b_rx)).is_empty(),
        "and the link no longer resolves"
    );
}
