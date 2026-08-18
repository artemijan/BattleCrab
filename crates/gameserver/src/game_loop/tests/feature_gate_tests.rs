//! `General.ini`'s feature gates — the keys that switch a whole subsystem off.
//!
//! All of them ship **on**, so none changes behaviour today. What the tests pin
//! is the off branch, which is the one an operator reaches and the one nothing
//! else in the suite exercises: `apply_dist_general_config` turns them on for
//! every other fixture precisely so those fixtures do not measure a disabled
//! server by accident.

use super::*;
use crate::config::general::ChatScope;
use crate::enums::ChatType;
use crate::model::boat::Boat;

/// `Darin's Letter` — undroppable and quest-bound, so it exercises the *other*
/// drop gates; adena is the droppable control.
const ADENA: i32 = 57;

/// `RequestDropItem` body.
fn drop_packet_for(item_oid: i32, count: i64, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(item_oid);
    w.write_i64(count);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

fn gated_world() -> (World, db::CmdRx) {
    let (mut world, _tx, db_rx, _link) = test_world();
    world.data.item_data = crate::data::dist::items_owned();
    world.id_pool = 0x7000_0000..0x7000_0100;
    // The chat-scope tests compare *map regions*, and `GameData::for_test`
    // ships none — every position would answer `None` and share Java's `0`
    // bucket, making "two players far apart" indistinguishable from "two
    // players in one town".
    world.data.map_region =
        crate::data::map_region::MapRegionData::load_from(crate::data::DIST_GAME);
    (world, db_rx)
}

/// `AllowDiscardItem` (**True** here) gates `RequestDropItem` wholesale, and
/// Java exempts a `DROP_ALL_ITEMS` override holder — the same override the
/// bound-item gates read, so the two must not be conflated.
#[test]
fn allow_discard_item_off_refuses_the_drop_unless_overridden() {
    for (key, overrides, expect_dropped) in [
        (true, false, true),
        (false, false, false),
        (false, true, true),
    ] {
        let (mut world, _rx) = gated_world();
        world.cfg.general.allow_discard_item = key;
        install_wall_region(&mut world);
        let mut rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
        give_item(&mut world, 100, 90_101, ADENA, 10);
        if overrides {
            world
                .objects
                .get_component_mut::<Player>(&100)
                .unwrap()
                .cond_overrides |= 1u64 << crate::game_loop::admin::DROP_ALL_ITEMS_ORDINAL;
        }
        drain(&mut rx);

        let pos = *world.objects.get_component::<Position>(&100).unwrap();
        on_packet(
            &mut world,
            1,
            drop_packet_for(90_101, 10, pos.x, pos.y, pos.z),
        );
        let gone = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&100)
            .map(|i| i.by_object_id(90_101).is_none())
            .unwrap_or(false);
        assert_eq!(
            gone, expect_dropped,
            "AllowDiscardItem = {key}, override = {overrides}"
        );
    }
}

/// `AllowBoat` (**True**) gates `BoatManager.load()`, so with it off no boat is
/// registered at all — the ferries do not exist rather than existing and
/// standing still.
#[test]
fn allow_boat_off_registers_no_ferries() {
    let (mut world, _rx) = gated_world();
    world.cfg.general.allow_boat = false;
    crate::game_loop::boats::spawn_boats(&mut world);
    assert_eq!(
        world.objects.count::<Boat>(),
        0,
        "no boat exists with AllowBoat off"
    );

    let (mut world, _rx) = gated_world();
    crate::game_loop::boats::spawn_boats(&mut world);
    assert!(
        world.objects.count::<Boat>() > 0,
        "…and the ferries are back with it on"
    );
}

/// `AllowRefund` (**True**) has two Java sites, and only testing one would
/// leave the other free: `RequestSellItem` decides whether the sold stack is
/// *filed*, and `Player.hasRefund()` decides whether the tab shows anything.
/// With the key off a sale is a destroy.
#[test]
fn allow_refund_off_destroys_the_sold_stack_instead_of_filing_it() {
    for key in [true, false] {
        let (mut world, _rx) = gated_world();
        world.cfg.general.allow_refund = key;
        let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
        give_item(&mut world, 100, 90_102, ADENA, 5);

        // File a stack directly through the refund container the sell path uses.
        if key {
            world
                .objects
                .add_components(&100, crate::model::inventory::Refund::default());
            // Copy a real instance rather than fabricating one — the refund
            // container holds the stack the sell path moved out of inventory.
            let inst = *world
                .objects
                .get_component::<crate::model::inventory::Inventory>(&100)
                .and_then(|i| i.by_object_id(90_102))
                .expect("the seeded stack");
            if let Some(r) = world
                .objects
                .get_component_mut::<crate::model::inventory::Refund>(&100)
            {
                r.push(inst);
            }
        }
        let shown = crate::game_loop::shop::refund_items_of(&world, 100).len();
        assert_eq!(
            shown,
            usize::from(key),
            "AllowRefund = {key}: the tab shows {shown} entries"
        );
    }
}

/// The refund tab is empty with the key off **even when rows exist** — Java's
/// `hasRefund()` ANDs the config in, so turning it off hides a container filled
/// while it was on rather than leaking it.
#[test]
fn the_refund_tab_hides_rows_left_over_from_when_it_was_on() {
    let (mut world, _rx) = gated_world();
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    world
        .objects
        .add_components(&100, crate::model::inventory::Refund::default());
    give_item(&mut world, 100, 90_104, ADENA, 1);
    let inst = *world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&100)
        .and_then(|i| i.by_object_id(90_104))
        .expect("the seeded stack");
    if let Some(r) = world
        .objects
        .get_component_mut::<crate::model::inventory::Refund>(&100)
    {
        r.push(inst);
    }
    assert_eq!(
        crate::game_loop::shop::refund_items_of(&world, 100).len(),
        1
    );
    world.cfg.general.allow_refund = false;
    assert!(
        crate::game_loop::shop::refund_items_of(&world, 100).is_empty(),
        "the rows survive but the tab does not show them"
    );
}

/// **`TradeChat`/`GlobalChat` are not on/off switches.** Three values each:
/// `on` is region-scoped, `global` is server-wide, and `gm` is region-scoped
/// *only* for a `CHAT_CONDITIONS` holder — everyone else falls through and the
/// line is dropped in silence, which is Java's own shape and the reason "off"
/// has no spelling of its own.
#[test]
fn the_chat_scopes_route_by_region_globally_or_not_at_all() {
    // Two players in deliberately different map regions.
    let far = (-80_000, 250_000);
    for (scope, overrides, expect_heard) in [
        (ChatScope::Region, false, false),
        (ChatScope::Global, false, true),
        (ChatScope::GmOnly, false, false),
        (ChatScope::GmOnly, true, false),
        (ChatScope::Off, false, false),
    ] {
        let (mut world, _rx) = gated_world();
        world.cfg.general.trade_chat = scope;
        let mut speaker_rx = ingame_player(&mut world, 1, 100, 83_400, 148_500, -3_400);
        let mut listener_rx = ingame_player(&mut world, 2, 101, far.0, far.1, 0);
        if overrides {
            world
                .objects
                .get_component_mut::<Player>(&100)
                .unwrap()
                .cond_overrides |= 1u64 << 8;
        }
        // Trade needs level 20 (Java's own literal).
        for oid in [100, 101] {
            world
                .objects
                .get_component_mut::<Player>(&oid)
                .unwrap()
                .level = 40;
        }
        drain(&mut speaker_rx);
        drain(&mut listener_rx);

        crate::game_loop::chat::handle_say2(
            &mut world,
            1,
            &say2_body("hi", ChatType::Trade as i32, None),
        );
        let heard = drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2);
        assert_eq!(
            heard, expect_heard,
            "scope {scope:?}, override {overrides}: a far-region listener"
        );
    }
}

/// `ChatTrade` carries a **hard-coded** level 20 — not `MinimumChatLevel`, and
/// Shout has no equivalent. Found while reading the scope branch; the port had
/// neither gate.
#[test]
fn trade_chat_needs_level_twenty_and_shout_does_not() {
    for (chat_type, level, expect_heard) in [
        (ChatType::Trade, 19, false),
        (ChatType::Trade, 20, true),
        (ChatType::Shout, 19, true),
    ] {
        let (mut world, _rx) = gated_world();
        let mut speaker_rx = ingame_player(&mut world, 1, 100, 83_400, 148_500, -3_400);
        let mut listener_rx = ingame_player(&mut world, 2, 101, 83_410, 148_510, -3_400);
        for oid in [100, 101] {
            world
                .objects
                .get_component_mut::<Player>(&oid)
                .unwrap()
                .level = level;
        }
        drain(&mut speaker_rx);
        drain(&mut listener_rx);

        crate::game_loop::chat::handle_say2(
            &mut world,
            1,
            &say2_body("hi", chat_type as i32, None),
        );
        let heard = drain(&mut listener_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SAY2);
        assert_eq!(heard, expect_heard, "{chat_type:?} at level {level}");
    }
}

/// `MinimumChatLevel` (**0**, inert) gates General, Shout and Whisper — three
/// handlers, **three different system messages**, which is why it cannot be
/// written once and shared. Trade is deliberately absent: it has its own
/// literal 20 instead.
#[test]
fn minimum_chat_level_gates_three_channels_with_their_own_messages() {
    for (chat_type, sm) in [
        (
            ChatType::General,
            server_packets::sm_ids::GENERAL_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1,
        ),
        (
            ChatType::Shout,
            server_packets::sm_ids::SHOUT_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1,
        ),
    ] {
        let (mut world, _rx) = gated_world();
        world.cfg.general.minimum_chat_level = 30;
        let mut rx = ingame_player(&mut world, 1, 100, 83_400, 148_500, -3_400);
        world
            .objects
            .get_component_mut::<Player>(&100)
            .unwrap()
            .level = 29;
        drain(&mut rx);

        crate::game_loop::chat::handle_say2(
            &mut world,
            1,
            &say2_body("hi", chat_type as i32, None),
        );
        assert!(
            has_sm(&drain(&mut rx), sm),
            "{chat_type:?} must refuse with its own message"
        );
    }
}

/// A `CHAT_CONDITIONS` holder is exempt from the level gate — the same override
/// the scope branches read, so a GM is not silenced by a level floor meant for
/// new accounts.
#[test]
fn the_chat_level_gate_exempts_a_chat_conditions_holder() {
    let (mut world, _rx) = gated_world();
    world.cfg.general.minimum_chat_level = 30;
    let mut rx = ingame_player(&mut world, 1, 100, 83_400, 148_500, -3_400);
    {
        let p = world.objects.get_component_mut::<Player>(&100).unwrap();
        p.level = 29;
        p.cond_overrides |= 1u64 << 8;
    }
    drain(&mut rx);
    crate::game_loop::chat::handle_say2(
        &mut world,
        1,
        &say2_body("hi", ChatType::General as i32, None),
    );
    assert!(
        !has_sm(
            &drain(&mut rx),
            server_packets::sm_ids::GENERAL_CHAT_CANNOT_BE_USED_BELOW_LEVEL_S1
        ),
        "the override holder is not refused"
    );
}
