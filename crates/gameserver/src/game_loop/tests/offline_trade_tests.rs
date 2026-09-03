//! Offline shops (`OfflineTradeUtil` + `OfflineTraderTable`): logging out with
//! a store open leaves the shop standing, it keeps trading, and it survives a
//! restart.

use super::*;
use crate::game_loop::character::inventory;
use crate::game_loop::commerce::offline_trade;
use crate::game_loop::social::chat;
use crate::model::components::{PrivateStore, StoreItem};

/// Turn the feature on the way `Custom/OfflineTrade.ini` does on this dist.
fn enable_offline(world: &mut World) {
    let cfg = &mut world.cfg.offline_trade;
    cfg.trade_enable = true;
    cfg.craft_enable = true;
    // The peace-zone gate needs real zone data to satisfy; the tests that care
    // about it turn it on explicitly.
    cfg.mode_in_peace_zone = false;
    cfg.mode_no_damage = true;
    cfg.set_name_color = true;
    cfg.name_color = 0x0080_8080;
    cfg.restore_offliners = true;
    cfg.max_days = 0;
    cfg.disconnect_finished = true;
    cfg.store_in_realtime = true;
    cfg.enable_offline_command = true;
}

/// One inventory row: five D-grade crystals as object id 4242.
fn crystal_row() -> crate::db::ItemRow {
    crate::db::ItemRow {
        object_id: 4242,
        item_id: 1458,
        count: 5,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    }
}

/// Give `oid` a live sell store holding `count` of item 57 at `price`.
fn open_sell_store(world: &mut World, oid: i32, item_object_id: i32, count: i64, price: i64) {
    world.objects.add_components(
        &oid,
        PrivateStore {
            items: vec![StoreItem {
                object_id: item_object_id,
                item_id: 1458,
                count,
                price,
                enchant: 0,
            }],
            title: "cheap crystals".into(),
            packaged: false,
        },
    );
    if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
        p.store_type = 1;
    }
}

/// **Logging out with a store open leaves the shop standing.** The session is
/// gone (so the socket closed and the login server was told), but the `Player`
/// stays in the world with its store, and the rows are written straight away
/// because `StoreOfflineTradeInRealtime` is on.
#[test]
fn a_logout_with_a_store_open_leaves_the_shop_behind() {
    let (mut world, _db_tx, mut db_rx, mut link_rx) = test_world();
    enable_offline(&mut world);
    let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    // The fixture doesn't register the account the way `AuthLogin` does; the
    // logout notice is keyed off that registration.
    world.login.accounts_in_gameserver.insert("bob".into(), 1);
    open_sell_store(&mut world, 5001, 4242, 3, 100);
    drain_db(&mut db_rx);

    handle_logout(&mut world, 1);

    assert!(
        world.clients.get(&1).is_none(),
        "the client is disconnected…"
    );
    assert!(
        world.objects.get_component::<Player>(&5001).is_some(),
        "…but the shop is still standing in the world"
    );
    assert!(
        world.offline_traders.contains_key(&5001),
        "and it is tracked as an unattended shop"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .name_color,
        0x0080_8080,
        "OfflineSetNameColor recoloured the name"
    );
    // Java `GameClient.onDisconnection` sends the account logout either way.
    assert!(
        std::iter::from_fn(|| link_rx.try_recv().ok())
            .any(|c| matches!(c, LoginLinkCommand::PlayerLogout { .. })),
        "the login server is told the account left"
    );
    let cmds = drain_db(&mut db_rx);
    let stored = cmds
        .iter()
        .find_map(|c| match c {
            db::DbCommand::StoreOfflineTrader {
                char_id: 5001,
                store_type,
                title,
                items,
                ..
            } => Some((*store_type, title.clone(), items.clone())),
            _ => None,
        })
        .expect("the shop is persisted immediately (realtime storing)");
    assert_eq!(stored.0, 1, "PrivateStoreType.SELL");
    assert_eq!(stored.1, "cheap crystals");
    assert_eq!(
        stored.2,
        vec![(4242, 3, 100)],
        "the line rides as (object id, count, price)"
    );
}

/// **Without a store there is no shop to leave behind** — the ordinary logout
/// runs and the player leaves the world.
#[test]
fn a_logout_without_a_store_is_an_ordinary_logout() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    enable_offline(&mut world);
    let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);

    handle_logout(&mut world, 1);

    assert!(
        world.objects.get_component::<Player>(&5001).is_none(),
        "the player left the world"
    );
    assert!(world.offline_traders.is_empty());
    assert!(
        !drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::StoreOfflineTrader { .. })),
        "nothing to store"
    );
}

/// **The config gate is load-bearing**: with `OfflineTradeEnable` off, a store
/// owner logging out just logs out (Java's `canSetShop` stays false).
#[test]
fn the_feature_switch_refuses_the_shop() {
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    world.cfg.offline_trade.trade_enable = false;
    let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    open_sell_store(&mut world, 5001, 4242, 3, 100);

    handle_logout(&mut world, 1);

    assert!(
        world.objects.get_component::<Player>(&5001).is_none(),
        "no shop is left behind when the feature is off"
    );
}

/// **A shopper sees the unattended shop.** It has no session, so the
/// session-driven visibility scan would miss it — the `offline_traders` index
/// is what puts its `CharInfo` on an arriving player's screen.
#[test]
fn an_arriving_player_sees_the_unattended_shop() {
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    let _seller_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    open_sell_store(&mut world, 5001, 4242, 3, 100);
    handle_logout(&mut world, 1);

    // A second player logs in right next to it.
    let mut buyer_rx = ingame_player(&mut world, 2, 5002, 120, 200, 0);
    drain(&mut buyer_rx);
    visibility::on_enter_world(&world, 2, 5002);

    let saw = drain(&mut buyer_rx)
        .into_iter()
        .any(|p| p[0] == server_packets::opcodes::CHAR_INFO);
    assert!(saw, "the shop is drawn for the arriving player");
}

/// **The shop keeps trading, and selling out ends it.** A purchase rewrites the
/// stored rows (`onTransaction`); emptying the store closes it, and
/// `OfflineDisconnectFinished` takes the seller out of the world.
#[test]
fn buying_out_an_unattended_shop_sends_it_home() {
    use crate::model::inventory::Inventory;

    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    enable_offline(&mut world);
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4100_0000..0x4100_0200;
    let _seller_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    let _buyer_rx = ingame_player(&mut world, 2, 5002, 120, 200, 0);
    inventory::add_inventory_item(&mut world, 5001, 1458, 4).unwrap();
    inventory::add_inventory_item(&mut world, 5002, 57, 1000).unwrap();
    let crystal = world
        .objects
        .get_component::<Inventory>(&5001)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == 1458)
        .unwrap()
        .object_id;
    open_sell_store(&mut world, 5001, crystal, 4, 100);
    handle_logout(&mut world, 1);
    assert!(world.offline_traders.contains_key(&5001));
    drain_db(&mut db_rx);

    // Buy 2 of the 4: the shop stays, its rows are rewritten with what's left.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(5001);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(2);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());

    assert!(
        world.offline_traders.contains_key(&5001),
        "a partial sale leaves the shop open"
    );
    let rewritten = drain_db(&mut db_rx)
        .into_iter()
        .find_map(|c| match c {
            db::DbCommand::StoreOfflineTrader {
                char_id: 5001,
                items,
                ..
            } => Some(items),
            _ => None,
        })
        .expect("the rows follow the sale");
    assert_eq!(rewritten, vec![(crystal, 2, 100)], "two crystals left");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&5002)
            .unwrap()
            .count_of(1458),
        2,
        "the buyer got them"
    );

    // Buy the rest: the store empties, so the shop goes home.
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_PRIVATE_STORE_BUY);
    w.write_i32(5001);
    w.write_i32(1);
    w.write_i32(crystal);
    w.write_i64(2);
    w.write_i64(100);
    on_packet(&mut world, 2, w.into_bytes());

    assert!(
        !world.offline_traders.contains_key(&5001),
        "OfflineDisconnectFinished: a sold-out shop leaves"
    );
    assert!(
        world.objects.get_component::<Player>(&5001).is_none(),
        "…and the player is out of the world"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::ClearOfflineTrader { char_id: 5001 })),
        "its rows are cleared"
    );
}

/// **`OfflineModeNoDamage`**: an unattended shop cannot be hurt.
#[test]
fn an_unattended_shop_takes_no_damage() {
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    let _rx2 = ingame_player(&mut world, 2, 5002, 120, 200, 0);
    open_sell_store(&mut world, 5001, 4242, 3, 100);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&5001) {
        v.cur_hp = 500.0;
    }
    handle_logout(&mut world, 1);

    combat::player_receive_damage(&mut world, 5001, 5002, 300.0);
    assert_eq!(
        world.objects.get_component::<Vitals>(&5001).unwrap().cur_hp,
        500.0,
        "the hit is nullified"
    );

    // With the config off the same hit lands — and Java's `reduceHp` then runs
    // its "attacked players in craft/shops stand up" branch, which clears the
    // store type; with `OfflineDisconnectFinished` that takes the whole
    // unattended shop out of the world. So one hit ends it.
    world.cfg.offline_trade.mode_no_damage = false;
    combat::player_receive_damage(&mut world, 5001, 5002, 300.0);
    assert!(
        world.objects.get_component::<Player>(&5001).is_none(),
        "a damageable shop is closed and sent home by the first hit"
    );
    assert!(!world.offline_traders.contains_key(&5001));
}

/// **The `.offline` command**: it asks for confirmation, and only the "yes"
/// reply hands the player over to offline mode. Without a store it refuses.
#[test]
fn the_offline_command_confirms_before_detaching() {
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    let mut rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    drain(&mut rx);

    // No store yet: "Private store already closed." and nothing happens.
    chat::handle_say2(
        &mut world,
        1,
        &say2_body(
            ".offline",
            crate::enums::ChatType::General.client_id(),
            None,
        ),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::PRIVATE_STORE_ALREADY_CLOSED),
        "refused without a store"
    );
    assert!(world.clients.contains_key(&1));

    // With a store open: a ConfirmDlg, and the player is still connected.
    open_sell_store(&mut world, 5001, 4242, 3, 100);
    chat::handle_say2(
        &mut world,
        1,
        &say2_body(
            ".offline",
            crate::enums::ChatType::General.client_id(),
            None,
        ),
    );
    let dlg = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::CONFIRM_DLG)
        .expect("the exit-game confirmation");
    assert_eq!(
        i32::from_le_bytes([dlg[1], dlg[2], dlg[3], dlg[4]]),
        server_packets::sm_ids::DO_YOU_WISH_TO_EXIT_THE_GAME as i32
    );
    assert!(world.clients.contains_key(&1), "asking is not leaving");

    // "No" changes nothing.
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(125, 0, 0)].concat(),
    );
    assert!(world.clients.contains_key(&1), "answer 0 is a no-op");

    // "Yes" hands over to offline mode.
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(125, 1, 0)].concat(),
    );
    assert!(!world.clients.contains_key(&1), "the client detached");
    assert!(
        world.offline_traders.contains_key(&5001),
        "and the shop is standing"
    );
}

/// **A restart is invisible to shoppers.** The stored rows rebuild the whole
/// character and re-open its store; an expired one (`OfflineMaxDays`) is
/// dropped instead.
#[test]
fn stored_shops_come_back_at_boot() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    enable_offline(&mut world);
    let now = offline_trade::now_millis();

    let mut chr = dummy_char(5001, "Shopkeeper");
    chr.x = 100;
    chr.y = 200;
    chr.items = vec![crystal_row()];
    let mut expired = dummy_char(5002, "Gone");
    expired.x = 100;
    expired.y = 200;

    world.cfg.offline_trade.max_days = 7;
    offline_trade::restore_offline_traders(
        &mut world,
        vec![
            db::OfflineTraderRow {
                char: chr,
                time: now - 3_600_000, // an hour ago
                store_type: 1,
                title: "cheap crystals".into(),
                items: vec![(4242, 5, 250)],
            },
            db::OfflineTraderRow {
                char: expired,
                time: now - 8 * 86_400_000, // eight days ago
                store_type: 1,
                title: "stale".into(),
                items: vec![],
            },
        ],
    );

    assert!(
        world.offline_traders.contains_key(&5001),
        "the fresh shop is back"
    );
    let store = world
        .objects
        .get_component::<PrivateStore>(&5001)
        .expect("with its store re-opened");
    assert_eq!(store.title, "cheap crystals");
    assert_eq!(
        store.items.first().map(|i| (i.object_id, i.count, i.price)),
        Some((4242, 5, 250)),
        "the line is rebuilt from the stored row"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&5001)
            .unwrap()
            .store_type,
        1,
        "and the store byte is set, so shoppers can click it"
    );
    assert_eq!(
        world.offline_traders[&5001].start_time_millis,
        now - 3_600_000,
        "the *original* detach time is kept, so OfflineMaxDays keeps counting"
    );

    assert!(
        !world.offline_traders.contains_key(&5002),
        "the eight-day-old shop is past OfflineMaxDays"
    );
    assert!(
        world.objects.get_component::<Player>(&5002).is_none(),
        "…and never enters the world"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::ClearOfflineTrader { char_id: 5002 })),
        "its rows are dropped"
    );

    // `RestoreOffliners = False` — the rows are read (the DB thread has no
    // config) but nothing comes back.
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    world.cfg.offline_trade.restore_offliners = false;
    offline_trade::restore_offline_traders(
        &mut world,
        vec![db::OfflineTraderRow {
            char: dummy_char(5003, "NotComing"),
            time: now,
            store_type: 1,
            title: String::new(),
            items: vec![],
        }],
    );
    assert!(
        world.offline_traders.is_empty(),
        "RestoreOffliners off means no shop comes back"
    );
}

/// A sell line names an *instance*: if the character no longer holds it, the
/// line is dropped rather than resurrecting a phantom item (Java's
/// `addItem(...) == null → continue`).
#[test]
fn a_restored_line_needs_the_item_to_still_exist() {
    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    let mut chr = dummy_char(5001, "Shopkeeper");
    chr.items = vec![crystal_row()];

    offline_trade::restore_offline_traders(
        &mut world,
        vec![db::OfflineTraderRow {
            char: chr,
            time: offline_trade::now_millis(),
            store_type: 1,
            title: "half gone".into(),
            items: vec![(4242, 5, 250), (9999, 1, 100)],
        }],
    );

    let store = world.objects.get_component::<PrivateStore>(&5001).unwrap();
    assert_eq!(
        store.items.len(),
        1,
        "the vanished instance is not restored"
    );
    assert_eq!(store.items[0].object_id, 4242);
}

/// Logging back in for real clears the shop's rows (Java `EnterWorld`'s
/// `onTransaction(player, true, false)`), so a restart doesn't resurrect a shop
/// its owner already took down.
#[test]
fn entering_the_world_clears_the_stored_shop() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    enable_offline(&mut world);
    let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    drain_db(&mut db_rx);

    offline_trade::on_enter_world(&mut world, 5001);

    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::ClearOfflineTrader { char_id: 5001 })),
        "the rows go away when the owner is back"
    );
}

/// **Teleporting an unattended shop completes inline** (Java: `if (!isPlayer()
/// || client.isDetached()) onTeleported()`).
///
/// A detached character has no client to answer `Appearing`, so without this
/// the `teleporting` flag is set and never cleared — and it gates position
/// validation, while the watchdog cannot clear it either. A GM-teleported shop
/// was left in a state nothing short of a relog could resolve.
#[test]
fn teleporting_an_offline_shop_completes_without_a_client() {
    use crate::model::Player;

    let (mut world, _db_tx, ..) = test_world();
    enable_offline(&mut world);
    let mut rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    open_sell_store(&mut world, 5001, 4242, 3, 100);
    drain(&mut rx);

    // Detach: the shop stands, the session is gone.
    on_packet(
        &mut world,
        1,
        [vec![cop::DLG_ANSWER], dlg_answer_body(125, 1, 0)].concat(),
    );
    assert!(!world.clients.contains_key(&1), "detached");
    assert!(world.offline_traders.contains_key(&5001));

    // Teleport it, as `//teleportto` or a restart point would.
    crate::game_loop::death::teleport_player(&mut world, 5001, 9000, 9000, -1000);

    let p = world.objects.get_component::<Player>(&5001).unwrap();
    assert!(
        !p.teleporting,
        "the teleport completed inline — no client will ever send Appearing"
    );
    let pos = world.objects.get_component::<Position>(&5001).unwrap();
    assert_eq!((pos.x, pos.y), (9000, 9000), "and it actually moved");
    assert!(
        !world.teleport_watchdog_due.contains_key(&5001),
        "the watchdog is cancelled, not left to expire on a shop that cannot answer"
    );
}

/// `OfflineAbnormalEffect` marks an unattended shop with a visual effect —
/// **one** of the configured names, drawn at random per trader, which is what
/// gives a row of shops a mix rather than a uniform glow.
///
/// It lands on `AdminVisuals` because the effect has no buff behind it: the
/// shop shows the marker without gaining anything. The `visual_effects` fold
/// is what every outgoing packet reads, so asserting there covers the wire.
#[test]
fn an_offline_shop_wears_one_of_the_configured_abnormal_effects() {
    const FLAME: i16 = 1;
    const STIGMA: i16 = 2;

    let open_shop = |effects: Vec<i16>| {
        let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
        enable_offline(&mut world);
        world.cfg.offline_trade.abnormal_effects = effects;
        let _rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
        world.login.accounts_in_gameserver.insert("bob".into(), 1);
        open_sell_store(&mut world, 5001, 4242, 3, 100);
        drain_db(&mut db_rx);
        handle_logout(&mut world, 1);
        abnormal::visual_effects(&world, 5001)
    };

    // The zero case first: with the list empty — as this dist ships it — the
    // shop wears nothing, so the assertion below is about the config and not
    // about some effect the shop picks up on its own.
    assert!(
        open_shop(Vec::new()).is_empty(),
        "an empty list marks nothing"
    );

    // Exactly one is applied, even though two are configured.
    let worn = open_shop(vec![FLAME, STIGMA]);
    assert_eq!(worn.len(), 1, "Java picks one at random, not all of them");
    assert!(
        worn[0] == FLAME || worn[0] == STIGMA,
        "and it is one of the configured ones: {worn:?}"
    );

    // A single-entry list is deterministic, so this pins the id end to end.
    assert_eq!(open_shop(vec![STIGMA]), vec![STIGMA]);
}
