//! The NPC shop window: buying, selling and the refund container.

use super::*;

/// A purchase debits adena, adds the items, and answers with the
/// InventoryUpdate/inven-weight/sell-refresh/SM-4358 tail; the guards
/// (wrong quantity, empty purse, no merchant target) refuse cleanly.
#[test]
fn request_buy_item_purchases_and_guards() {
    let (mut world, _db_rx, mut rx) = shop_world();

    // 1 Cloth Cap (100) + 5 potions (50) = 150 adena.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1), (1061, 5)]));
    assert_eq!(adena_of(&world, 3001), 850);
    assert_eq!(count_of_item(&world, 3001, 41), 1);
    assert_eq!(count_of_item(&world, 3001, 1061), 5);
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == 0x21), "InventoryUpdate");
    assert!(
        pkts.iter().any(|p| is_ex(p, 0x166)),
        "ExUserInfoInvenWeight"
    );
    let sell_done = pkts
        .iter()
        .find(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
        .expect("sell refresh");
    assert_eq!(*sell_done.last().unwrap(), 1, "done flag");
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::EXCHANGE_IS_SUCCESSFUL)
    );

    // Non-stackable quantity > 1: SM 1036, nothing purchased.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 2)]));
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED)
    );
    assert_eq!(adena_of(&world, 3001), 850);

    // Too expensive: SM 279.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 100)]));
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA)
    );
    assert_eq!(adena_of(&world, 3001), 850);

    // Off-list item: no charge, and the probe now punishes (the illegal-action
    // warning line is the only reply — no inventory or trade packet).
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(702, 1)]));
    let pkts = drain(&mut rx);
    assert!(!pkts.iter().any(|p| p[0] == 0x21), "no InventoryUpdate");
    assert_eq!(adena_of(&world, 3001), 850);

    // No merchant targeted: ActionFailed.
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    drain(&mut rx);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 1)]));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL)
    );
    assert_eq!(adena_of(&world, 3001), 850);
}

/// Parse the `ExBuySellList` SELL window: the sell tab as `(item_id, price)`
/// and the refund tab as `(item_id, count, index, price)`.
fn parse_sell_window(p: &[u8]) -> (Vec<(i32, i64)>, Vec<(i32, i64, i32, i64)>) {
    assert_eq!(p[0], 0xFE);
    assert_eq!(
        i16::from_le_bytes(p[1..3].try_into().unwrap()),
        crate::network::trade::EX_BUY_SELL_LIST
    );
    let i16at = |o: usize| i16::from_le_bytes(p[o..o + 2].try_into().unwrap());
    let i32at = |o: usize| i32::from_le_bytes(p[o..o + 4].try_into().unwrap());
    let i64at = |o: usize| i64::from_le_bytes(p[o..o + 8].try_into().unwrap());
    assert_eq!(i32at(3), 1, "type SELL");
    let mut o = 11usize; // type + inventory slots
    let mut sell = Vec::new();
    let n = i16at(o) as usize;
    o += 2;
    for _ in 0..n {
        // write_item_entry is 41 bytes: item id at +5, the price follows.
        sell.push((i32at(o + 5), i64at(o + 41)));
        o += 49;
    }
    let mut refund = Vec::new();
    let rn = i16at(o) as usize;
    o += 2;
    for _ in 0..rn {
        refund.push((i32at(o + 5), i64at(o + 10), i32at(o + 41), i64at(o + 45)));
        o += 53;
    }
    (sell, refund)
}

/// The SELL-window packet (type 1) among the drained packets.
fn sell_window_of(pkts: &[Vec<u8>]) -> Vec<u8> {
    pkts.iter()
        .filter(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST))
        .find(|p| i32::from_le_bytes(p[3..7].try_into().unwrap()) == 1)
        .expect("sell window")
        .clone()
}

/// A non-sellable item (`is_sellable=false`, e.g. the event Agathion
/// bracelets) is hidden from the sell tab and refused by `RequestSellItem`;
/// sellable items sell for reference-price/2, land in the refund tab, and
/// `RequestRefundItem` buys them back for the same price (Java
/// `Config.ALLOW_REFUND=True` on this dist).
#[test]
fn sell_list_hides_unsellable_and_refund_round_trips() {
    let (mut world, _db_rx, mut rx) = shop_world();
    for (item_id, name, sellable, stackable, price) in [
        (9001, "Agathion - Shiny (Event)", false, false, 0i64),
        (9002, "Sword of Test", true, false, 200),
        (9003, "Test Potion", true, true, 60),
    ] {
        world
            .data
            .item_data
            .insert_for_test(crate::data::item_data::template::ItemTemplate {
                item_id,
                name: name.into(),
                kind: crate::data::item_data::kinds::ItemKind::Etc,
                is_stackable: stackable,
                is_infinite: false,
                is_sellable: sellable,
                is_freightable: false,
                price,
                ..Default::default()
            });
    }
    inventory::add_inventory_item(&mut world, 3001, 9001, 1);
    inventory::add_inventory_item(&mut world, 3001, 9002, 1);
    inventory::add_inventory_item(&mut world, 3001, 9003, 10);
    let obj_of = |world: &World, item_id: i32| {
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .items()
            .iter()
            .find(|i| i.item_id == item_id)
            .unwrap()
            .object_id
    };
    let sword_oid = obj_of(&world, 9002);
    world
        .objects
        .get_component_mut::<Inventory>(&3001)
        .unwrap()
        .set_item_enchant(sword_oid, 5);
    drain(&mut rx);

    // The bracelet is invisible in the sell tab; the sellables are priced /2.
    shop::show_buy_window(&mut world, 1, 3001, NPC_OID, 3);
    let (sell, refund) = parse_sell_window(&sell_window_of(&drain(&mut rx)));
    assert!(!sell.iter().any(|&(id, _)| id == 9001), "unsellable hidden");
    assert!(sell.contains(&(9002, 100)), "sword at price/2");
    assert!(sell.contains(&(9003, 30)), "potion at price/2");
    assert!(refund.is_empty(), "refund tab starts empty");

    // Selling the non-sellable bracelet is refused (Java skips it silently).
    let bracelet_oid = obj_of(&world, 9001);
    shop::handle_request_sell_item(&mut world, 1, &sell_body(3, &[(bracelet_oid, 9001, 1)]));
    assert!(drain(&mut rx).is_empty(), "nothing sold, no refresh");
    assert_eq!(count_of_item(&world, 3001, 9001), 1);
    assert_eq!(adena_of(&world, 3001), 1000);

    // Sell the +5 sword and 4 potions: 100 + 4*30 = 220 adena, both chunks
    // appear in the refund tab (the partial stack under a fresh object id).
    let potion_oid = obj_of(&world, 9003);
    shop::handle_request_sell_item(
        &mut world,
        1,
        &sell_body(3, &[(sword_oid, 9002, 1), (potion_oid, 9003, 4)]),
    );
    assert_eq!(adena_of(&world, 3001), 1220);
    assert_eq!(count_of_item(&world, 3001, 9002), 0);
    assert_eq!(count_of_item(&world, 3001, 9003), 6);
    let (_, refund) = parse_sell_window(&sell_window_of(&drain(&mut rx)));
    assert_eq!(refund, vec![(9002, 1, 0, 100), (9003, 4, 1, 120)]);
    let refund_items = shop::refund_items_of(&world, 3001);
    assert_eq!(refund_items[0].object_id, sword_oid, "identity kept");
    assert_eq!(refund_items[0].enchant_level, 5, "enchant kept");
    assert_ne!(
        refund_items[1].object_id, potion_oid,
        "split stack gets a fresh object id"
    );

    // Buy the potions back: 120 adena, the chunk merges into the stack.
    shop::handle_request_refund_item(&mut world, 1, &refund_body(3, &[1]));
    assert_eq!(adena_of(&world, 3001), 1100);
    assert_eq!(count_of_item(&world, 3001, 9003), 10);
    let (_, refund) = parse_sell_window(&sell_window_of(&drain(&mut rx)));
    assert_eq!(refund, vec![(9002, 1, 0, 100)], "sword re-indexed at 0");

    // A bad slot refuses the whole request (Java: illegal-action punish).
    shop::handle_request_refund_item(&mut world, 1, &refund_body(3, &[5]));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL)
    );
    assert_eq!(adena_of(&world, 3001), 1100);

    // Buy the sword back: same object id, enchant intact, refund tab empty.
    shop::handle_request_refund_item(&mut world, 1, &refund_body(3, &[0]));
    assert_eq!(adena_of(&world, 3001), 1000);
    assert_eq!(count_of_item(&world, 3001, 9002), 1);
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    let sword = inv
        .items()
        .iter()
        .find(|i| i.item_id == 9002)
        .expect("sword restored");
    assert_eq!(sword.object_id, sword_oid);
    assert_eq!(sword.enchant_level, 5);
    let (_, refund) = parse_sell_window(&sell_window_of(&drain(&mut rx)));
    assert!(refund.is_empty());

    // No merchant targeted: refund refused.
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    drain(&mut rx);
    shop::handle_request_refund_item(&mut world, 1, &refund_body(3, &[0]));
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL)
    );
}

/// `PlayerRefund` capacity: the container holds 12 entries — the 13th sale
/// silently destroys the oldest (Java `PlayerRefund.addItem`).
#[test]
fn refund_container_caps_at_twelve() {
    let (mut world, _db_rx, mut rx) = shop_world();
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::template::ItemTemplate {
            item_id: 9003,
            name: "Test Potion".into(),
            kind: crate::data::item_data::kinds::ItemKind::Etc,
            is_stackable: true,
            is_infinite: false,
            is_sellable: true,
            is_freightable: false,
            price: 60,
            ..Default::default()
        });
    inventory::add_inventory_item(&mut world, 3001, 9003, 20);
    let potion_oid = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == 9003)
        .unwrap()
        .object_id;
    for _ in 0..13 {
        shop::handle_request_sell_item(&mut world, 1, &sell_body(3, &[(potion_oid, 9003, 1)]));
    }
    drain(&mut rx);
    assert_eq!(count_of_item(&world, 3001, 9003), 7);
    assert_eq!(shop::refund_items_of(&world, 3001).len(), 12, "capped");
}

/// `RequestSellItem` (0x37) sells inventory items to the targeted merchant for
/// reference-price/2 adena each.
#[test]
fn request_sell_item_pays_adena() {
    let (mut world, _db_rx, mut rx) = shop_world();
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::template::ItemTemplate {
            trade_flags: Default::default(),
            pre_conditions: Vec::new(),
            is_oly_restricted: false,
            is_event_restricted: false,
            for_npc: false,
            time: -1,
            duration: -1,
            immediate_effect: false,
            ex_immediate_effect: false,
            default_action: crate::data::item_data::kinds::ActionType::Other,
            item_id: 5000,
            name: "Trophy".into(),
            kind: crate::data::item_data::kinds::ItemKind::Etc,
            body_part: 0,
            weight: 0,
            is_stackable: true,
            is_infinite: false,
            type1: 4,
            type2: 5,
            is_quest_item: false,
            is_sellable: true,
            is_freightable: false,
            price: 200, // sells for 100 each
            handler: crate::data::item_data::kinds::ItemHandler::None,
            crystal_type: crate::data::item_data::kinds::CrystalType::None,
            crystal_count: 0,
            attack_radius: 40,
            attack_angle: 0,
            mp_consume: 0,
            reduced_mp_consume: 0,
            reduced_mp_consume_chance: 0,
            capsuled_items: Vec::new(),
            extractable_count_min: 0,
            extractable_count_max: 0,
            item_skills: Vec::new(),
            etc_item_type: crate::data::item_data::kinds::EtcItemType::Other,
            enchant_enabled: false,
            enchant_limit: 0,
            is_magic_weapon: false,
        });
    inventory::add_inventory_item(&mut world, 3001, 5000, 10).expect("trophies");
    let oid = item_oid(&world, 3001, 5000);
    drain(&mut rx);

    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_SELL_ITEM);
    w.write_i32(3); // list id
    w.write_i32(1); // one line
    w.write_i32(oid);
    w.write_i32(5000);
    w.write_i64(4);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(count_of_item(&world, 3001, 5000), 6, "4 sold");
    assert_eq!(adena_of(&world, 3001), 1000 + 400, "paid 4 × (200/2)");
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x21),
        "InventoryUpdate sent"
    );
}
