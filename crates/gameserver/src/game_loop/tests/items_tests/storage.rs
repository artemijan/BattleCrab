//! Warehouse, freight (including a send to an offline character) and
//! crystallizing.

use super::*;

/// Warehouse deposit → the item moves inventory→warehouse; withdraw moves it
/// back; and the save gathers both containers with the right `loc`s (so a
/// deposit survives relog).
#[test]
fn warehouse_deposit_withdraw_and_persist() {
    use crate::model::inventory::{Inventory, Warehouse};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9400, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9400, 57, 1000).expect("adena");
    let inv_adena = |w: &World| {
        w.objects
            .get_component::<Inventory>(&9400)
            .unwrap()
            .count_of(57)
    };
    let wh_adena = |w: &World| {
        w.objects
            .get_component::<Warehouse>(&9400)
            .unwrap()
            .0
            .count_of(57)
    };
    let adena_oid = item_oid(&world, 9400, 57);

    // Deposit 400.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SEND_WARE_HOUSE_DEPOSIT_LIST);
    w.write_i32(1);
    w.write_i32(adena_oid);
    w.write_i64(400);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(inv_adena(&world), 600, "400 left inventory");
    assert_eq!(wh_adena(&world), 400, "400 in warehouse");

    // The whole persisted set carries both containers with distinct locs.
    let save = build_save_data(&world, 9400).expect("save");
    let inv_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "INVENTORY")
        .expect("inv adena row");
    let wh_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "WAREHOUSE")
        .expect("wh adena row");
    assert_eq!((inv_row.count, wh_row.count), (600, 400));

    // Withdraw 150 back.
    let wh_oid = world
        .objects
        .get_component::<Warehouse>(&9400)
        .unwrap()
        .0
        .items()
        .iter()
        .find(|it| it.item_id == 57)
        .unwrap()
        .object_id;
    let mut w = PacketWriter::new();
    w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
    w.write_i32(1);
    w.write_i32(wh_oid);
    w.write_i64(150);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(wh_adena(&world), 250, "150 withdrawn");
    assert_eq!(inv_adena(&world), 750, "back in inventory");
}

/// Crystallizing a D-grade item destroys it and yields its `crystal_count` of
/// Crystal (D-grade) (1458) — but only with the Crystallize skill.
#[test]
fn crystallize_item_yields_crystals_when_skilled() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    // Leather Boots (40): D-grade with a crystal count.
    let cc = world.data.item_data.get(40).unwrap().crystal_count;
    assert!(cc > 0, "test item is crystallizable");

    let mut rx = ingame_player_access(&mut world, 1, 9500, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9500, 40, 1).expect("boots");
    let boots_oid = item_oid(&world, 9500, 40);
    let crystallize = |oid: i32| -> Vec<u8> {
        let mut w = PacketWriter::new();
        w.write_u8(cop::REQUEST_CRYSTALLIZE_ITEM);
        w.write_i32(oid);
        w.write_i64(1);
        w.into_bytes()
    };

    // No skill → refused, boots keep.
    on_packet(&mut world, 1, crystallize(boots_oid));
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9500)
            .unwrap()
            .count_of(40),
        1,
        "no skill, no crystallize"
    );

    // Grant Crystallize (248) level 1, then crystallize.
    world
        .objects
        .get_component_mut::<SkillBook>(&9500)
        .unwrap()
        .0
        .insert(248, 1);
    on_packet(&mut world, 1, crystallize(boots_oid));
    let inv = world.objects.get_component::<Inventory>(&9500).unwrap();
    assert_eq!(inv.count_of(40), 0, "boots crystallized away");
    assert_eq!(
        inv.count_of(1458),
        cc as i64,
        "got crystal_count Crystal (D-grade)"
    );
}

/// Freight (the account-package warehouse): the `package_withdraw` half. Seed
/// the container as if another character had sent items, withdraw part of it,
/// and confirm it persists with `loc="FREIGHT"`.
#[test]
fn freight_withdraw_and_persist() {
    use crate::model::inventory::{Freight, Inventory};
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 9600, 0);
    drain(&mut rx);

    // Seed 300 adena into the freight (as if sent by another character).
    let fr_oid = world.alloc_object_id().unwrap();
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Freight>(&9600)
            .unwrap()
            .0
            .add_item(&data.item_data, fr_oid, 57, 300);
    }

    // package_withdraw → active = freight, window opens.
    warehouse::open_freight_withdraw(&mut world, 1);
    let withdraw = {
        let mut w = PacketWriter::new();
        w.write_u8(cop::SEND_WARE_HOUSE_WITH_DRAW_LIST);
        w.write_i32(1);
        w.write_i32(fr_oid);
        w.write_i64(120);
        w.into_bytes()
    };
    on_packet(&mut world, 1, withdraw);

    assert_eq!(
        world
            .objects
            .get_component::<Freight>(&9600)
            .unwrap()
            .0
            .count_of(57),
        180,
        "180 left in freight"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(57),
        120,
        "120 withdrawn to inventory"
    );

    // Persisted with its own loc alongside inventory + warehouse.
    let save = build_save_data(&world, 9600).expect("save");
    let fr_row = save
        .items
        .iter()
        .find(|r| r.item_id == 57 && r.loc == "FREIGHT")
        .expect("freight row");
    assert_eq!(fr_row.count, 180);
    assert!(
        save.items
            .iter()
            .any(|r| r.item_id == 57 && r.loc == "INVENTORY" && r.count == 120),
        "inventory row"
    );
}

/// **Freighting items to another character on the account.** `package_deposit`
/// offers the account's other characters, the send window lists only
/// `is_freightable` items, and the send itself charges `FreightPrice` per slot
/// and writes the items to the (offline) recipient's freight rows.
#[test]
fn freight_send_delivers_to_an_offline_character() {
    use crate::model::components::player::LastFolkNpc;
    use crate::model::inventory::Inventory;

    let (mut world, mut db, _link) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4400_0000..0x4400_0200;

    // The sender, with a second character (9902 "Alt") on the account.
    let chr = dummy_char(9901, "Sender");
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(9901, "Sender"), dummy_char(9902, "Alt")])
        .into_entering(bundle);
    let (session, bundle) = session.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));

    // A freight manager in range (the send checks the last folk NPC).
    add_test_npc(&mut world, NPC_OID, 30001, "Warehouse", 70, 0, 0, 0);
    world.objects.add_components(&9901, LastFolkNpc(NPC_OID));

    // **No item below id 10000 declares `is_freightable` on this dist** — every
    // one of the 3416 that do is later-chronicle (10649+). Java's gate is the
    // same, so the freight can only ever carry those; 10649 (Feather of
    // Blessing) is the lowest and stands in for the mechanism here.
    const FREIGHTABLE: i32 = 10649;
    assert!(
        world
            .data
            .item_data
            .get(FREIGHTABLE)
            .unwrap()
            .is_freightable,
        "fixture assumption: 10649 is freightable"
    );
    inventory::add_inventory_item(&mut world, 9901, FREIGHTABLE, 10).unwrap();
    inventory::add_inventory_item(&mut world, 9901, 57, 5_000).unwrap();
    let crystal = item_oid(&world, 9901, FREIGHTABLE);
    drain(&mut rx);

    // `package_deposit` → the account's other characters.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_package_deposit")),
    );
    let list = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::PACKAGE_TO_LIST)
        .expect("the send-to list");
    assert_eq!(
        i32::from_le_bytes([list[1], list[2], list[3], list[4]]),
        1,
        "one other character on the account"
    );

    // The send window lists the freightable item.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SENDABLE_ITEM_LIST);
    w.write_i32(9902);
    on_packet(&mut world, 1, w.into_bytes());
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::PACKAGE_SENDABLE_LIST),
        "the sendable-item window opens"
    );

    // Send 4 crystals: they leave the inventory, the 1000-adena slot fee is
    // charged, and the delivery is written to the offline recipient's rows.
    drain_db(&mut db);
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(9902);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(4);
    on_packet(&mut world, 1, w.into_bytes());

    let inv = world.objects.get_component::<Inventory>(&9901).unwrap();
    assert_eq!(inv.count_of(FREIGHTABLE), 6, "4 items left the sender");
    assert_eq!(inv.count_of(57), 4_000, "the 1000-adena fee was charged");
    let delivered = drain_db(&mut db).into_iter().find_map(|c| match c {
        db::DbCommand::AddFreightItems {
            owner_id: 9902,
            items,
        } => Some(items),
        _ => None,
    });
    let items = delivered.expect("the freight rows were written");
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].item_id, items[0].count), (FREIGHTABLE, 4));
}

/// **The freight refuses what it may not carry.** A non-freightable item and a
/// recipient who isn't on the account both leave everything where it is.
#[test]
fn freight_send_refuses_bad_items_and_strangers() {
    use crate::model::components::player::LastFolkNpc;
    use crate::model::inventory::Inventory;

    let (mut world, _db, _link) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4500_0000..0x4500_0200;
    let chr = dummy_char(9903, "Sender");
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let session = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(9903, "Sender"), dummy_char(9904, "Alt")])
        .into_entering(bundle);
    let (session, bundle) = session.into_ingame();
    bundle.spawn_into(&mut world);
    world.clients.insert(1, ClientSession::InGame(session));
    add_test_npc(&mut world, NPC_OID, 30001, "Warehouse", 70, 0, 0, 0);
    world.objects.add_components(&9903, LastFolkNpc(NPC_OID));

    // Adena — like every other Interlude-range item on this dist — is not
    // freightable; 10649 is, and stands in for a legal cargo below.
    inventory::add_inventory_item(&mut world, 9903, 57, 5_000).unwrap();
    inventory::add_inventory_item(&mut world, 9903, 10649, 5).unwrap();
    assert!(
        !world.data.item_data.get(1458).unwrap().is_freightable,
        "Crystal (D) — an Interlude item — may not be freighted"
    );
    let (adena_oid, crystal) = (item_oid(&world, 9903, 57), item_oid(&world, 9903, 10649));
    drain(&mut rx);

    // A non-freightable line aborts the whole send.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(9904);
    w.write_i32(1);
    w.write_i32(adena_oid);
    w.write_i64(100);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9903)
            .unwrap()
            .count_of(57),
        5_000,
        "a non-freightable item is refused, fee included"
    );

    // A recipient who isn't on the account is refused too.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PACKAGE_SEND);
    w.write_i32(7777);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(5);
    on_packet(&mut world, 1, w.into_bytes());
    let inv = world.objects.get_component::<Inventory>(&9903).unwrap();
    assert_eq!(inv.count_of(10649), 5, "nothing was sent to a stranger");
    assert_eq!(inv.count_of(57), 5_000, "and no fee was taken");
}
