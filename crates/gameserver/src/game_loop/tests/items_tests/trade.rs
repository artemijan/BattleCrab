//! Player-to-player commerce: private sell and buy stores, package stores,
//! direct trade, and the shop bypass.

use super::*;

/// A heal on another player: Heal.java's `power + sqrt(2·mAtk)` amount,
/// overheal-clamped, SM 1067 to the healed target.
#[test]
fn heal_on_other_restores_hp_with_formula() {
    let (mut world, ..) = cast_test_world();
    let mut _a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&3002)
        .unwrap()
        .cur_hp = 50.0;
    handle_action(&mut world, 1, &action_body(3002, 0));
    drain(&mut b_rx);

    // TARGET-type skills need no ctrl.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1015, false));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut b_rx); // ExRotation + MagicSkillUse

    advance_ticks(&mut world, 10); // hit 500 ms + cancel 500 ms

    let heal = formulas::heal::calc_heal(
        83.0,
        pcs(&world, 3001).m_atk,
        false,
        false,
        false,
        0,
        formulas::heal::HealCaster::PlayerMage,
        1.0,
    );
    assert!(
        heal > 50.0,
        "sanity: heal ({heal}) overflows the missing 50 HP"
    );
    assert_eq!(
        pvit(&world, 3002).cur_hp,
        100.0,
        "overheal clamped at max HP"
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MAGIC_SKILL_LAUNCHED
    );
    assert_eq!(
        sm_id(&b_rx.try_recv().unwrap()),
        server_packets::sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1
    );
    assert_eq!(
        b_rx.try_recv().unwrap()[0],
        server_packets::opcodes::STATUS_UPDATE
    );
}

/// The `Buy <listId>` bypass opens the buy window: the BUY tab (type 0,
/// list id + adena + both products) and the SELL tab (type 1).
#[test]
fn buy_bypass_opens_buy_and_sell_tabs() {
    let (mut world, _db_rx, mut rx) = shop_world();
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Buy 3")));
    let pkts = drain(&mut rx);
    let tabs: Vec<_> = pkts
        .iter()
        .filter(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
        .collect();
    assert_eq!(tabs.len(), 2, "buy + sell tab");
    // BUY tab: type 0, money 1000, list id 3, then the product table.
    let buy = tabs[0];
    assert_eq!(i32::from_le_bytes(buy[3..7].try_into().unwrap()), 0);
    assert_eq!(i64::from_le_bytes(buy[7..15].try_into().unwrap()), 1000);
    assert_eq!(i32::from_le_bytes(buy[15..19].try_into().unwrap()), 3);
    // SELL tab leads with type 1.
    assert_eq!(i32::from_le_bytes(tabs[1][3..7].try_into().unwrap()), 1);

    // A non-merchant NPC refuses the same bypass.
    add_test_npc(&mut world, NPC_OID + 1, 30002, "Folk", 5, 120, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Buy 3", NPC_OID + 1)),
    );
    let pkts = drain(&mut rx);
    assert!(
        !pkts
            .iter()
            .any(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
    );
}

/// A private sell store: the owner sets a list (store activates + store byte),
/// and a buyer purchases — items move seller→buyer, adena buyer→seller.
#[test]
fn private_store_sell_and_buy() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut seller_rx = ingame_player_access(&mut world, 1, 9600, 0);
    let mut buyer_rx = ingame_player_access(&mut world, 2, 9601, 0);
    drain(&mut seller_rx);
    drain(&mut buyer_rx);
    // Seller has 10 Crystal (D); buyer has 1000 adena.
    inventory::add_inventory_item(&mut world, 9600, 1458, 10).unwrap();
    inventory::add_inventory_item(&mut world, 9601, 57, 1000).unwrap();
    let crystal_oid = item_oid(&world, 9600, 1458);

    // Seller sets the store: sell 4 crystals at 100 adena each.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_SELL);
    w.write_i32(0); // not package
    w.write_i32(1); // one line
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9600)
            .unwrap()
            .store_type,
        1,
        "store active"
    );
    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::PrivateStore>(&9600)
            .unwrap()
            .items
            .len(),
        1
    );

    // Buyer buys all 4.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9600); // seller
    w.write_i32(1);
    w.write_i32(crystal_oid);
    w.write_i64(4);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());

    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9601)
            .unwrap()
            .count_of(1458),
        4,
        "buyer got 4 crystals"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9601)
            .unwrap()
            .count_of(57),
        600,
        "buyer paid 400"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(1458),
        6,
        "seller has 6 left"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9600)
            .unwrap()
            .count_of(57),
        400,
        "seller earned 400"
    );
    // Store emptied of its offered stock → closed.
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9600)
            .unwrap()
            .store_type,
        0,
        "store closed when sold out"
    );
}

/// A full player-to-player trade: request → accept → both add items → both
/// confirm → the offered items swap.
#[test]
fn player_trade_swaps_items() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut a_rx = ingame_player_access(&mut world, 1, 9700, 0);
    let mut b_rx = ingame_player_access(&mut world, 2, 9701, 0);
    drain(&mut a_rx);
    drain(&mut b_rx);
    inventory::add_inventory_item(&mut world, 9700, 1458, 10).unwrap(); // A: Crystal D
    inventory::add_inventory_item(&mut world, 9701, 1459, 10).unwrap(); // B: Crystal C
    let a_oid = item_oid(&world, 9700, 1458);
    let b_oid = item_oid(&world, 9701, 1459);
    let one_int = |op: u8, v: i32| {
        let mut w = PacketWriter::new();
        w.write_u8(op);
        w.write_i32(v);
        w.into_bytes()
    };
    let add = |oid: i32, n: i64| {
        let mut w = PacketWriter::new();
        w.write_u8(cop::ADD_TRADE_ITEM);
        w.write_i32(0);
        w.write_i32(oid);
        w.write_i64(n);
        w.into_bytes()
    };

    // A requests, B accepts → both in a trade.
    on_packet(&mut world, 1, one_int(cop::TRADE_REQUEST, 9701));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::PendingTrade>(&9701)
            .map(|p| p.from),
        Some(9700)
    );
    on_packet(&mut world, 2, one_int(cop::ANSWER_TRADE_REQUEST, 1));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::Trade>(&9700)
            .unwrap()
            .partner,
        9701
    );

    // A offers 4 Crystal D, B offers 3 Crystal C.
    on_packet(&mut world, 1, add(a_oid, 4));
    on_packet(&mut world, 2, add(b_oid, 3));
    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::Trade>(&9700)
            .unwrap()
            .items[0]
            .count,
        4
    );

    // Both confirm → swap.
    on_packet(&mut world, 1, one_int(cop::TRADE_DONE, 1));
    on_packet(&mut world, 2, one_int(cop::TRADE_DONE, 1));

    let a_inv = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&9700)
            .unwrap()
            .count_of(id)
    };
    let b_inv = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(id)
    };
    assert_eq!(
        (a_inv(&world, 1458), a_inv(&world, 1459)),
        (6, 3),
        "A: -4 D, +3 C"
    );
    assert_eq!(
        (b_inv(&world, 1458), b_inv(&world, 1459)),
        (4, 7),
        "B: +4 D, -3 C"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::Trade>(&9700),
        "trade closed"
    );
    assert!(
        !world
            .objects
            .has_component::<model::components::commerce::Trade>(&9701),
        "trade closed"
    );
}

/// Build a `SetPrivateStoreListBuy` body: the wanted lines, keyed by item id
/// with the client's enchant/augment/element tail.
fn set_buy_list(lines: &[(i32, i64, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_BUY);
    w.write_i32(lines.len() as i32);
    for &(item_id, count, price) in lines {
        w.write_i32(item_id);
        w.write_i16(0); // enchant
        w.write_i16(0); // unknown
        w.write_i64(count);
        w.write_i64(price);
        w.write_i32(0); // augment option 1
        w.write_i32(0); // augment option 2
        for _ in 0..8 {
            w.write_i16(0); // attack element + six defences
        }
        w.write_i32(0); // visual id
    }
    w.into_bytes()
}

/// Build a `RequestPrivateStoreSell` body: the store owner and the offered
/// lines, with the soul-crystal/SA tails empty.
fn store_sell_body(store_player: i32, lines: &[(i32, i32, i64, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_SELL);
    w.write_i32(store_player);
    w.write_i32(lines.len() as i32);
    for &(object_id, item_id, count, price) in lines {
        w.write_i32(object_id);
        w.write_i32(item_id);
        w.write_i16(0); // enchant
        w.write_i16(0); // unknown
        w.write_i64(count);
        w.write_i64(price);
        w.write_i32(0); // visual
        w.write_i32(0); // option 1
        w.write_i32(0); // option 2
        w.write_u8(0); // soul-crystal options
        w.write_u8(0); // SA effects
    }
    w.into_bytes()
}

/// A private **buy** store: the owner posts what they want, a customer sells
/// into it — items customer→owner, adena owner→customer.
#[test]
fn private_buy_store_takes_items_and_pays_out() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut owner_rx = ingame_player_access(&mut world, 1, 9610, 0);
    let mut seller_rx = ingame_player_access(&mut world, 2, 9611, 0);
    drain(&mut owner_rx);
    drain(&mut seller_rx);
    // The buyer has 1000 adena to spend; the seller has 10 D-grade crystals.
    inventory::add_inventory_item(&mut world, 9610, 57, 1000).unwrap();
    inventory::add_inventory_item(&mut world, 9611, 1458, 10).unwrap();
    let crystal_oid = item_oid(&world, 9611, 1458);

    // Wanted: 4 crystals at 100 adena each (400 total, affordable).
    on_packet(&mut world, 1, set_buy_list(&[(1458, 4, 100)]));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        3,
        "the buy store is open"
    );

    // The customer offers ten, but only four are wanted — the rest stay put.
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 10, 100)]),
    );
    {
        let seller_inv = world.objects.get_component::<Inventory>(&9611).unwrap();
        assert_eq!(
            seller_inv.count_of(1458),
            6,
            "only the four wanted changed hands"
        );
        assert_eq!(
            seller_inv.count_of(57),
            400,
            "and were paid for at 100 each"
        );
    }
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        0,
        "a filled buy store closes"
    );

    // Re-open a smaller store and fill it in two goes.
    on_packet(&mut world, 1, set_buy_list(&[(1458, 4, 100)]));
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 3, 100)]),
    );
    let seller_inv = world.objects.get_component::<Inventory>(&9611).unwrap();
    assert_eq!(
        seller_inv.count_of(1458),
        3,
        "three more crystals handed over"
    );
    assert_eq!(seller_inv.count_of(57), 700, "paid 300 more adena");
    let owner_inv = world.objects.get_component::<Inventory>(&9610).unwrap();
    assert_eq!(owner_inv.count_of(1458), 7, "the owner received them");
    assert_eq!(owner_inv.count_of(57), 300, "and spent 300 more");
    // One line still wanted, so the store stays open.
    assert_eq!(
        world
            .objects
            .get_component::<model::components::commerce::PrivateBuyStore>(&9610)
            .unwrap()
            .items[0]
            .count,
        1,
        "one crystal still wanted"
    );

    // Filling the last one closes the store.
    on_packet(
        &mut world,
        2,
        store_sell_body(9610, &[(crystal_oid, 1458, 1, 100)]),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9610)
            .unwrap()
            .store_type,
        0,
        "a filled buy store closes"
    );
}

/// A buy store may not ask for more than the owner can pay for.
#[test]
fn private_buy_store_refuses_an_unaffordable_list() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9612, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9612, 57, 100).unwrap();

    // 10 × 100 = 1000 adena wanted, but only 100 in the purse.
    on_packet(&mut world, 1, set_buy_list(&[(1458, 10, 100)]));

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9612)
            .unwrap()
            .store_type,
        0,
        "the store never opened"
    );
    assert!(
        has_system_message(
            &drain(&mut rx),
            server_packets::sm_ids::THE_PURCHASE_PRICE_IS_HIGHER_THAN_YOUR_MONEY
        ),
        "and the client is told why"
    );
}

/// The wanted list is capped by `MaxPvtStoreBuySlots*` (4 for a non-Dwarf).
#[test]
fn private_buy_store_enforces_the_slot_limit() {
    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4000_0000..0x4000_0200;
    let mut rx = ingame_player_access(&mut world, 1, 9613, 0);
    drain(&mut rx);
    inventory::add_inventory_item(&mut world, 9613, 57, 1_000_000).unwrap();

    let five = [
        (1458, 1, 100),
        (1459, 1, 100),
        (1460, 1, 100),
        (1461, 1, 100),
        (1462, 1, 100),
    ];
    on_packet(&mut world, 1, set_buy_list(&five));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9613)
            .unwrap()
            .store_type,
        0,
        "five lines is one over the limit"
    );

    on_packet(&mut world, 1, set_buy_list(&five[..4]));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9613)
            .unwrap()
            .store_type,
        3,
        "four lines is fine"
    );
}

/// **A package store sells its whole list as one lot.** `/packagesale` (player
/// action 61) opens the manage window in package mode; the store then reports
/// `PACKAGE_SELL` (8), and a buyer who asks for fewer lines than it holds is
/// refused outright — Java's anti-bot check. Taking every line goes through.
#[test]
fn package_store_is_all_or_nothing() {
    use crate::model::components::commerce::PrivateStore;
    use crate::model::inventory::Inventory;

    let (mut world, ..) = admin_world();
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4300_0000..0x4300_0200;
    let mut seller_rx = ingame_player_access(&mut world, 1, 9700, 0);
    let mut buyer_rx = ingame_player_access(&mut world, 2, 9701, 0);
    // Two distinct items so the package has two lines.
    inventory::add_inventory_item(&mut world, 9700, 1458, 5).unwrap(); // Crystal (D)
    inventory::add_inventory_item(&mut world, 9700, 1459, 5).unwrap(); // Crystal (C)
    inventory::add_inventory_item(&mut world, 9701, 57, 10_000).unwrap();
    let (a, b) = (item_oid(&world, 9700, 1458), item_oid(&world, 9700, 1459));
    drain(&mut seller_rx);
    drain(&mut buyer_rx);

    // `/packagesale` → the manage window opens with the package flag set.
    // The press dispatches through `ActionData.xml`'s handler table, which the
    // fixture world ships empty: without the row the packet finds no handler
    // and no window opens at all.
    world
        .data
        .action_data
        .insert_row_for_test(61, "PrivateStore", 8);
    let mut act = PacketWriter::new();
    act.write_u8(cop::REQUEST_ACTION_USE);
    act.write_i32(61);
    act.write_i32(0);
    act.write_u8(0);
    on_packet(&mut world, 1, act.into_bytes());
    let manage = drain(&mut seller_rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::PRIVATE_STORE_MANAGE_LIST)
        .expect("the manage window");
    assert_eq!(
        i32::from_le_bytes([manage[5], manage[6], manage[7], manage[8]]),
        1,
        "the window is flagged as a package sale"
    );

    // Open the package store with both lines.
    let mut w = PacketWriter::new();
    w.write_u8(cop::SET_PRIVATE_STORE_LIST_SELL);
    w.write_i32(1); // package sale
    w.write_i32(2);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    w.write_i32(b);
    w.write_i64(5);
    w.write_i64(200);
    on_packet(&mut world, 1, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9700)
            .unwrap()
            .store_type,
        8,
        "PACKAGE_SELL"
    );
    assert!(
        world
            .objects
            .get_component::<PrivateStore>(&9700)
            .unwrap()
            .packaged
    );

    // Buying only one of the two lines is refused — nothing moves.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9700);
    w.write_i32(1);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(57),
        10_000,
        "a partial package purchase pays nothing"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&9701)
            .unwrap()
            .count_of(1458),
        0,
        "…and delivers nothing"
    );

    // Taking the whole package works: 5×100 + 5×200 = 1500 adena.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(9700);
    w.write_i32(2);
    w.write_i32(a);
    w.write_i64(5);
    w.write_i64(100);
    w.write_i32(b);
    w.write_i64(5);
    w.write_i64(200);
    on_packet(&mut world, 2, w.into_bytes());

    let buyer = world.objects.get_component::<Inventory>(&9701).unwrap();
    assert_eq!(buyer.count_of(1458), 5, "first line delivered");
    assert_eq!(buyer.count_of(1459), 5, "second line delivered");
    assert_eq!(
        buyer.count_of(57),
        10_000 - 1_500,
        "and paid for as one lot"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&9700)
            .unwrap()
            .store_type,
        0,
        "the emptied store closes"
    );
}

/// **`SetPrivateStoreWholeMsg` (ex 0x47) titles the package store** and echoes
/// `ExPrivateStoreSetWholeMsg` back — the package-sell counterpart of
/// `PrivateStoreMsgSell`, which was missing entirely.
#[test]
fn package_store_title_round_trips() {
    use crate::model::components::commerce::PrivateStore;

    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 9702, 0);
    drain(&mut rx);

    let mut body = PacketWriter::new();
    body.write_string("Whole lot!");
    on_packet(
        &mut world,
        1,
        ex_packet(
            cp::ex_opcodes::SET_PRIVATE_STORE_WHOLE_MSG,
            &body.into_bytes(),
        ),
    );

    assert_eq!(
        world
            .objects
            .get_component::<PrivateStore>(&9702)
            .map(|s| s.title.clone()),
        Some("Whole lot!".to_string())
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_PRIVATE_STORE_WHOLE_MSG)),
        "the title is echoed back"
    );
}

/// **Opening a shop suppresses inventory refreshes for 1500 ms** (Java
/// `Player.setInventoryBlockingStatus` + `InventoryEnableTask`).
///
/// The client fires its own `RequestItemList` while a buy window is coming up;
/// answering it redraws the inventory over the window the player just asked
/// for. Java ignores those requests for 1.5 s, and so does this now.
#[test]
fn a_shop_window_suppresses_item_list_refreshes_briefly() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    // Not blocked to begin with: a plain request is answered.
    drain(&mut rx);
    items::handle_request_item_list(&mut world, 1);
    assert!(
        !drain(&mut rx).is_empty(),
        "an unblocked item-list request is answered"
    );

    // Block, as opening a shop/warehouse does.
    inventory::block_inventory(&mut world, 3001);
    items::handle_request_item_list(&mut world, 1);
    assert!(
        drain(&mut rx).is_empty(),
        "the request is dropped while the window is opening"
    );

    // 1500 ms later the scheduled task lifts it. Java's task clears the flag
    // unconditionally, so a second window opened inside the window is
    // unblocked by the *first* task rather than extending the block.
    advance_ticks(&mut world, 16); // 1.6 s at 10 ticks/s
    assert!(
        !world.inventory_blocked.contains(&3001),
        "InventoryEnableTask lifted the block"
    );
    items::handle_request_item_list(&mut world, 1);
    assert!(
        !drain(&mut rx).is_empty(),
        "refreshes are answered again once the window has settled"
    );
}
