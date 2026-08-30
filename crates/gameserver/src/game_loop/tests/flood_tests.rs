//! `FloodProtector.ini` end-to-end: the dispatch gate, the GM bypass and the
//! punishment arms, driven through `on_packet` rather than the counter's own
//! API — the counter is unit-tested in `game_loop::client::flood`, what these pin is
//! that it is actually *wired* to every packet Java protects.

use super::*;
use crate::config::flood_protector::{FloodAction, FloodProtectorsConfig};
use crate::game_loop::client::flood;
use crate::network::client_packets::ex_opcodes as exop;
use commons::config::PropertiesParser;

/// Overwrite the world's flood config from an ini body, so a test sets exactly
/// the keys it cares about and inherits Java's code defaults for the rest.
fn set_flood_config(world: &mut World, ini: &str) {
    world.cfg.flood_protector = FloodProtectorsConfig::from_parser(
        &PropertiesParser::from_content("FloodProtector.ini", ini),
    );
}

/// A long interval makes the window deterministic regardless of how fast the
/// test runs: 6000 ticks = 10 minutes, so both packets land inside it.
const LONG_WINDOW: &str = "FloodProtectorPlayerActionInterval = 6000\n";

/// The gate is reached from `on_packet`: two `Action` clicks inside one
/// interval, and only the first is answered.
#[test]
fn a_second_action_inside_the_window_is_dropped() {
    let (mut world, ..) = test_world();
    set_flood_config(&mut world, LONG_WINDOW);
    let mut rx = ingame_player(&mut world, 1, 5001, 0, 0, 0);
    add_test_npc(&mut world, 9001, 70001, "Folk", 10, 20, 0, 0);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9001, 0)].concat(),
    );
    assert!(
        !drain(&mut rx).is_empty(),
        "the first click is answered (target selected)"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9001, 0)].concat(),
    );
    assert!(
        drain(&mut rx).is_empty(),
        "the second click inside the interval is dropped by the flood gate"
    );
}

/// Java's `PlayerCondOverride.FLOOD_CONDITIONS` escape hatch — a GM is never
/// rate-limited. `is_gm()` stands in for the cond-override table here.
#[test]
fn a_gm_is_exempt_from_the_gate() {
    let (mut world, ..) = admin_world();
    set_flood_config(&mut world, LONG_WINDOW);
    let mut rx = ingame_player_access(&mut world, 1, 5002, 100);
    add_test_npc(&mut world, 9002, 70002, "Folk", 10, 20, 0, 0);
    drain(&mut rx);

    for i in 0..3 {
        on_packet(
            &mut world,
            1,
            [vec![cop::ACTION], action_body(9002, 0)].concat(),
        );
        assert!(
            !drain(&mut rx).is_empty(),
            "GM click {i} answered — no flood limit applies"
        );
    }
}

/// Slots are independent end-to-end, not just in the counter: flooding
/// `PlayerAction` must not spend the `Transaction` budget.
#[test]
fn flooding_one_slot_does_not_gate_another() {
    let (mut world, ..) = test_world();
    set_flood_config(
        &mut world,
        "FloodProtectorPlayerActionInterval = 6000\nFloodProtectorTransactionInterval = 6000\n",
    );
    let mut rx = ingame_player(&mut world, 1, 5003, 0, 0, 0);
    add_test_npc(&mut world, 9003, 70003, "Folk", 10, 20, 0, 0);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9003, 0)].concat(),
    );
    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9003, 0)].concat(),
    );
    drain(&mut rx);

    // A Transaction-slot packet still gets through on its own fresh window,
    // and that window is live — the *second* one is refused, which is what
    // separates "independent slot" from "gate not running at all".
    assert_eq!(
        flood::action_for_opcode(cop::TRADE_DONE),
        Some(FloodAction::Transaction)
    );
    assert!(
        flood::gate(&mut world, 1, FloodAction::Transaction),
        "the Transaction window is untouched by the PlayerAction flood"
    );
    assert!(
        !flood::gate(&mut world, 1, FloodAction::Transaction),
        "…but it is a real window: the second request inside it is refused"
    );
}

/// `PunishmentType = kick` closes the connection once `PunishmentLimit`
/// refusals pile up inside one interval (Java `kickPlayer`).
#[test]
fn the_kick_punishment_drops_the_session() {
    let (mut world, ..) = test_world();
    set_flood_config(
        &mut world,
        "FloodProtectorPlayerActionInterval = 6000\n\
         FloodProtectorPlayerActionPunishmentLimit = 2\n\
         FloodProtectorPlayerActionPunishmentType = kick\n",
    );
    let mut rx = ingame_player(&mut world, 1, 5004, 0, 0, 0);
    add_test_npc(&mut world, 9004, 70004, "Folk", 10, 20, 0, 0);
    drain(&mut rx);

    let click = |world: &mut World| {
        on_packet(world, 1, [vec![cop::ACTION], action_body(9004, 0)].concat());
    };
    click(&mut world); // allowed
    click(&mut world); // refusal 1
    assert!(
        world.clients.contains_key(&1),
        "under the limit, the client stays connected"
    );
    click(&mut world); // refusal 2 == limit
    assert!(
        !world.clients.contains_key(&1),
        "reaching the punishment limit kicks the client"
    );
}

/// `PunishmentType = ban` records an ACCOUNT/BAN punishment for the flooding
/// account (Java `banAccount`).
#[test]
fn the_ban_punishment_records_an_account_ban() {
    let (mut world, ..) = test_world();
    set_flood_config(
        &mut world,
        "FloodProtectorPlayerActionInterval = 6000\n\
         FloodProtectorPlayerActionPunishmentLimit = 1\n\
         FloodProtectorPlayerActionPunishmentType = ban\n\
         FloodProtectorPlayerActionPunishmentTime = 5\n",
    );
    let mut rx = ingame_player(&mut world, 1, 5005, 0, 0, 0);
    add_test_npc(&mut world, 9005, 70005, "Folk", 10, 20, 0, 0);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9005, 0)].concat(),
    );
    on_packet(
        &mut world,
        1,
        [vec![cop::ACTION], action_body(9005, 0)].concat(),
    );

    assert!(
        world.punishments.has_punishment(
            "bob",
            model::punishment::PunishmentAffect::Account,
            model::punishment::PunishmentType::Ban,
        ),
        "the flooding account is banned"
    );
}

/// The extended (`0xD0`) path is gated too — the sub-opcode decides the slot,
/// and the `0xD0` envelope itself must not be charged separately.
#[test]
fn the_ex_packet_path_is_gated_by_sub_opcode() {
    assert_eq!(
        flood::action_for_ex_opcode(exop::REQUEST_SEND_POST),
        Some(FloodAction::SendMail)
    );
    assert_eq!(
        flood::action_for_opcode(cop::EX_PACKET),
        None,
        "the envelope is not a protected packet"
    );

    let (mut world, ..) = test_world();
    set_flood_config(&mut world, "FloodProtectorSendMailInterval = 6000\n");
    let _rx = ingame_player(&mut world, 1, 5006, 0, 0, 0);

    assert!(
        flood::gate(&mut world, 1, FloodAction::SendMail),
        "first mail allowed"
    );
    assert!(
        !flood::gate(&mut world, 1, FloodAction::SendMail),
        "second mail inside the window refused"
    );
}

/// The shipped `dist/game/config/FloodProtector.ini`, end to end: a legitimate
/// sequence *is* throttled under the real values, and the window really does
/// reopen. This is the case the (deliberately protector-less) game-loop
/// fixtures no longer cover, so it is pinned here against the actual file.
#[test]
fn the_dist_config_throttles_a_burst_and_then_reopens() {
    let (mut world, ..) = test_world();
    world.cfg.flood_protector = FloodProtectorsConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(
        world
            .cfg
            .flood_protector
            .get(FloodAction::Transaction)
            .interval,
        10,
        "the shipped Transaction window is 10 ticks = 1 s"
    );
    let _rx = ingame_player(&mut world, 1, 5009, 0, 0, 0);

    assert!(flood::gate(&mut world, 1, FloodAction::Transaction));
    assert!(
        !flood::gate(&mut world, 1, FloodAction::Transaction),
        "a second transaction in the same second is refused"
    );

    // One second later (the game loop's own 100 ms tick), it is allowed again.
    world.tick += 10;
    assert!(
        flood::gate(&mut world, 1, FloodAction::Transaction),
        "the window reopens after the configured interval"
    );

    // …and `UseItem` ships disabled ("to match retail"), so it never refuses.
    for _ in 0..20 {
        assert!(
            flood::gate(&mut world, 1, FloodAction::UseItem),
            "UseItem ships at interval 0 and must stay unlimited"
        );
    }
}

/// `FloodProtectorSubclass` is the one Java call site driven from a bypass
/// rather than a packet (`VillageMaster` cases 4/5/7), so the opcode table
/// cannot reach it — it is wired by hand and must still hold.
#[test]
fn the_subclass_bypass_is_rate_limited() {
    let (mut world, ..) = test_world();
    set_flood_config(&mut world, "FloodProtectorSubclassInterval = 6000\n");
    let mut rx = ingame_player(&mut world, 1, 5008, 0, 0, 0);
    add_test_npc(&mut world, 9008, 70008, "VillageMaster", 70, 20, 0, 0);
    drain(&mut rx);

    // Case 5 (change class) targeting the class the character is already on:
    // Java answers with `SubClass_Current.htm`, so an allowed call always
    // replies — which makes "no reply" a clean signal that the gate fired.
    crate::game_loop::character::subclass::handle_village_master_bypass(
        &mut world, 1, 5008, 9008, "5 0",
    );
    assert!(
        !drain(&mut rx).is_empty(),
        "the first subclass bypass is answered"
    );

    crate::game_loop::character::subclass::handle_village_master_bypass(
        &mut world, 1, 5008, 9008, "5 0",
    );
    assert!(
        drain(&mut rx).is_empty(),
        "the second inside the interval is refused before any handling"
    );
}

/// The protectors hang off the connection, not the player: Java keeps them on
/// `GameClient`, so bouncing to character select and back must not hand the
/// client a fresh budget.
#[test]
fn the_flood_budget_survives_a_state_transition() {
    let (mut world, ..) = test_world();
    set_flood_config(&mut world, "FloodProtectorCharacterSelectInterval = 6000\n");
    let _rx = ingame_player(&mut world, 1, 5007, 0, 0, 0);

    assert!(flood::gate(&mut world, 1, FloodAction::CharacterSelect));
    assert!(!flood::gate(&mut world, 1, FloodAction::CharacterSelect));

    // InGame → Authenticated, the `RequestRestart` path.
    let ClientSession::InGame(session) = world.clients.remove(&1).unwrap() else {
        panic!("in-game session expected");
    };
    world.clients.insert(
        1,
        ClientSession::Authenticated(session.into_authenticated()),
    );

    assert!(
        !flood::gate(&mut world, 1, FloodAction::CharacterSelect),
        "the window carried across the transition — a relog cannot reset it"
    );
}
