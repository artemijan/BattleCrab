//! Community board (G30) — synthetic-world tests over the real dist htmls and
//! config. The board button (`_bbshome`), the offline gate, and the heal /
//! teleport actions are driven end-to-end through `handle_parse_command`.

use super::*;

use crate::config::community_board::{scan_available_teleports, CommunityBoardConfig};
use crate::game_loop::community_board::handle_parse_command;
use crate::model::components::{Position, Vitals};

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// Point the world at the real dist htmls and load the real community-board
/// config (custom board, buff/teleport whitelists).
fn enable_board(world: &mut World) {
    let general = commons::config::PropertiesParser::load(format!("{DIST}config/General.ini"));
    let cb = commons::config::PropertiesParser::load(format!("{DIST}config/Custom/CommunityBoard.ini"));
    let mut c = CommunityBoardConfig::from_parsers(&general, &cb);
    c.available_teleports = scan_available_teleports(true, &format!("{DIST}data/html"));
    world.cfg.community_board = c;
    world.data.root = DIST.to_string();
}

/// Decode a `ShowBoard` packet's content (skip opcode + show/hide byte + the 8
/// fixed nav strings).
fn cb_content(pkt: &[u8]) -> String {
    assert_eq!(pkt[0], server_packets::opcodes::SHOW_BOARD, "a ShowBoard packet");
    let mut r = commons::network::PacketReader::new(&pkt[2..]);
    for _ in 0..8 {
        r.read_string().unwrap();
    }
    r.read_string().unwrap()
}

#[test]
fn board_button_opens_custom_home_with_navigation() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7001, 0, 0, 0);
    drain(&mut rx);

    // The board button opens at `BBSDefault` (`_bbshome`).
    handle_parse_command(&mut world, 1, "_bbshome");
    let pkts = drain(&mut rx);

    let board: Vec<_> = pkts
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .collect();
    assert_eq!(board.len(), 3, "sendCBHtml sends three ShowBoard chunks (101/102/103)");

    let content = cb_content(board[0]);
    assert!(content.contains("Community Board"), "home page body rendered");
    assert!(!content.contains("%navigation%"), "navigation panel was injected");
    // The navigation markup links back through `_bbs*` bypasses.
    assert!(content.contains("_bbs"), "nav buttons wired to community-board bypasses");
}

#[test]
fn board_offline_when_disabled_sends_system_message() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    world.cfg.community_board.enabled = false;
    let mut rx = ingame_player(&mut world, 1, 7002, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbshome");
    let pkts = drain(&mut rx);

    assert!(
        !pkts.iter().any(|p| p[0] == server_packets::opcodes::SHOW_BOARD),
        "no board window when the community server is offline"
    );
    assert_eq!(count_system_messages(&pkts), 1, "the offline SystemMessage is sent");
}

#[test]
fn heal_action_restores_hp_mp_cp() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7003, 0, 0, 0);

    // Wound the player.
    {
        let v = world.objects.get_component_mut::<Vitals>(&7003).unwrap();
        v.cur_hp = 1.0;
        v.cur_mp = 1.0;
    }
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsheal;");
    let v = pvit(&world, 7003);
    assert_eq!(v.cur_hp, v.max_hp as f64, "HP restored to max");
    assert_eq!(v.cur_mp, v.max_mp as f64, "MP restored to max");
}

#[test]
fn heal_blocked_without_currency() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    world.cfg.community_board.heal_price = 1000; // charge adena the player lacks
    let mut rx = ingame_player(&mut world, 1, 7006, 0, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&7006).unwrap();
        v.cur_hp = 1.0;
    }
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsheal;");
    assert_eq!(pvit(&world, 7006).cur_hp, 1.0, "no heal when the player can't pay");
}

#[test]
fn teleport_action_moves_player_and_hides_board() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7004, 0, 0, 0);
    drain(&mut rx);

    // A destination whitelisted by the gatekeeper htmls (Giran gatekeeper).
    let key = "207320 87617 -1112";
    assert!(
        world.cfg.community_board.available_teleports.contains_key(key),
        "the destination is in the scanned whitelist"
    );
    handle_parse_command(&mut world, 1, &format!("_bbsteleport;{key}"));

    let pos = *world.objects.get_component::<Position>(&7004).unwrap();
    assert_eq!((pos.x, pos.y), (207320, 87617), "player teleported to the destination x/y");

    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::SHOW_BOARD && p[1] == 0),
        "the board is hidden (ShowBoard show=0) around the teleport"
    );
}

#[test]
fn teleport_to_unlisted_destination_is_refused() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7005, 100, 200, 0);
    drain(&mut rx);

    // Coordinates not present in any gatekeeper html — anti-exploit reject.
    handle_parse_command(&mut world, 1, "_bbsteleport;999999 999999 999999");
    let pos = *world.objects.get_component::<Position>(&7005).unwrap();
    assert_eq!((pos.x, pos.y), (100, 200), "player did not move for an unlisted destination");
}
