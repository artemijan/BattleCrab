//! Community board (G30) — synthetic-world tests over the real dist htmls and
//! config. The board button (`_bbshome`), the offline gate, and the heal /
//! teleport actions are driven end-to-end through `handle_parse_command`.

use super::*;

use crate::config::community_board::{scan_available_teleports, CommunityBoardConfig};
use crate::game_loop::community_board::handle_parse_command;
use crate::model::components::{Buffs, Position, Vitals};
use crate::model::skill::ActiveBuff;

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

/// Give the player an active buff so the scheme-create snapshot has something
/// to capture.
fn push_buff(world: &mut World, oid: i32, skill_id: i32) {
    let buffs = world.objects.get_component_mut::<Buffs>(&oid).expect("player has a Buffs component");
    buffs.0.push(ActiveBuff {
        skill_id,
        skill_level: 1,
        abnormal_type_client_id: 0,
        abnormal_type: "NONE".to_string(),
        abnormal_level: 0,
        slot: crate::model::skill::BuffSlot::Buff,
        expires_at_tick: u64::MAX,
        passive: false,
        effect_flags: 0,
        effects: Vec::new(),
    });
}

#[test]
fn premium_buy_grants_status_and_serves_thankyou() {
    let (mut world, _tx, _rx, _l) = test_world();
    enable_board(&mut world);
    // `EnablePremiumSystem` is False in Java's `Config` defaults (and so in
    // `PremiumConfig::default`); this dist's PremiumSystem.ini turns it on.
    world.cfg.premium.enabled = true;
    world.cfg.community_board.premium_price_per_day = 0; // free — isolate the grant path
    let mut rx = ingame_player(&mut world, 1, 7101, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbspremium;7");

    // The account behind the test session is "bob" (lowercased in the store).
    assert!(world.premium.get("bob").copied().unwrap_or(0) > 0, "premium granted for the account");
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::SHOW_BOARD),
        "the thank-you page is served"
    );
    assert!(count_system_messages(&pkts) >= 1, "the status message is sent");
}

#[test]
fn premium_buy_refused_without_currency() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    // dist price 1,000,000/day; the fresh test player cannot pay.
    let mut rx = ingame_player(&mut world, 1, 7102, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbspremium;1");
    assert!(world.premium.is_empty(), "no premium granted when the player can't pay");
}

#[test]
fn premium_buy_rejects_out_of_range_days() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    world.cfg.community_board.premium_price_per_day = 0;
    let mut rx = ingame_player(&mut world, 1, 7103, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbspremium;40"); // > 30 → refused
    assert!(world.premium.is_empty(), "an out-of-range day count is refused");
}

#[test]
fn scheme_create_from_active_buffs_persists() {
    let (mut world, _tx, mut db_rx, _l) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7110, 0, 0, 0);
    push_buff(&mut world, 7110, 1204); // Wind Walk — on the dist whitelist
    drain(&mut rx);
    let _ = drain_db(&mut db_rx);

    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_create Windy buffer/schemes.html");

    let schemes = world.buffer_schemes.get(&7110).expect("scheme registered");
    assert_eq!(schemes.len(), 1);
    assert_eq!(schemes[0].0, "Windy", "scheme stored under its name");
    assert_eq!(schemes[0].1, vec![1204], "the active whitelisted buff was captured");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreBufferScheme { object_id, scheme_name, .. }
                if *object_id == 7110 && scheme_name == "Windy"
        )),
        "the scheme is written through to buffer_schemes"
    );
}

#[test]
fn scheme_create_without_buffs_shows_error() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7111, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_create Empty buffer/schemes.html");
    assert!(
        world.buffer_schemes.get(&7111).map_or(true, |s| s.is_empty()),
        "no scheme created without active buffs"
    );
    let pkts = drain(&mut rx);
    let content = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .map(|p| cb_content(p))
        .unwrap_or_default();
    assert!(content.contains("You don't have any buffs applied."), "the error banner is rendered");
}

#[test]
fn scheme_delete_removes_it() {
    let (mut world, _tx, mut db_rx, _l) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7112, 0, 0, 0);
    push_buff(&mut world, 7112, 1204);
    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_create Gone buffer/schemes.html");
    assert_eq!(world.buffer_schemes.get(&7112).unwrap().len(), 1);
    let _ = drain_db(&mut db_rx);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_delete Gone buffer/schemes.html");
    assert!(world.buffer_schemes.get(&7112).unwrap().is_empty(), "scheme removed");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::DeleteBufferScheme { scheme_name, .. } if scheme_name == "Gone"
        )),
        "the delete is written through"
    );
}

#[test]
fn scheme_execute_pet_without_pet_errors() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7113, 0, 0, 0);
    push_buff(&mut world, 7113, 1204);
    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_create Petless buffer/schemes.html");
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_execute Petless buffer/schemes.html pet");
    let pkts = drain(&mut rx);
    let content = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .map(|p| cb_content(p))
        .unwrap_or_default();
    assert!(content.contains("You don't have a pet."), "the pet execute reports no pet");
}

#[test]
fn scheme_create_enforces_max_schemes() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7114, 0, 0, 0);
    push_buff(&mut world, 7114, 1204);
    for i in 0..5 {
        handle_parse_command(&mut world, 1, &format!("_bbs_buff_scheme_create S{i} buffer/schemes.html"));
    }
    assert_eq!(world.buffer_schemes.get(&7114).unwrap().len(), 5, "five schemes created");
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_buff_scheme_create S6 buffer/schemes.html");
    assert_eq!(world.buffer_schemes.get(&7114).unwrap().len(), 5, "the sixth is rejected at the cap");
    let pkts = drain(&mut rx);
    let content = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .map(|p| cb_content(p))
        .unwrap_or_default();
    assert!(content.contains("Maximum schemes amount is already reached."), "the cap error is shown");
}

// --- FavoriteBoard / HomepageBoard ----------------------------------------

/// Grab the concatenated board html (all SHOW_BOARD chunks joined).
fn board_html(pkts: &[Vec<u8>]) -> String {
    pkts.iter()
        .filter(|p| p[0] == server_packets::opcodes::SHOW_BOARD)
        .map(|p| cb_content(p))
        .collect()
}

#[test]
fn homepage_link_serves_homepage() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7200, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbslink");
    let pkts = drain(&mut rx);
    assert_eq!(
        pkts.iter().filter(|p| p[0] == server_packets::opcodes::SHOW_BOARD).count(),
        3,
        "the homepage is sent as three chunks"
    );
    assert!(board_html(&pkts).contains("bbs_Webfolder"), "homepage.html body rendered");
}

#[test]
fn getfav_on_empty_renders_the_list_page() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7201, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsgetfav");
    let html = board_html(&drain(&mut rx));
    assert!(html.contains("Bookmark list"), "favorite.html rendered");
    assert!(!html.contains("%fav_list%"), "the (empty) list placeholder was substituted");
}

#[test]
fn add_favorite_from_home_persists_and_renders() {
    let (mut world, _tx, mut db_rx, _l) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7202, 0, 0, 0);
    // Opening the board home queues the "Home" bypass (Java `addBypass`).
    handle_parse_command(&mut world, 1, "_bbshome");
    drain(&mut rx);
    let _ = drain_db(&mut db_rx);

    // The client toolbar's "add to favorites" button.
    handle_parse_command(&mut world, 1, "bbs_add_fav");

    let favs = world.bbs_favorites.get(&7202).expect("favorite registered");
    assert_eq!(favs.len(), 1);
    assert_eq!(favs[0].title, "Home", "favorite bookmarks the board home");
    assert_eq!(favs[0].bypass, "_bbshome");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreFavorite { player_id, title, .. } if *player_id == 7202 && title == "Home"
        )),
        "the favorite is written through to bbs_favorites"
    );
    // The callback re-renders the favorites list with the new row.
    let html = board_html(&drain(&mut rx));
    assert!(html.contains("Home"), "the new favorite row is rendered");
    assert!(html.contains("_bbsdelfav_"), "the delete button carries the fav id");
}

#[test]
fn add_favorite_without_queued_bypass_is_noop() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7203, 0, 0, 0);
    drain(&mut rx);

    // No `_bbshome` first → nothing queued, nothing added (Java logs & returns).
    handle_parse_command(&mut world, 1, "bbs_add_fav");
    assert!(world.bbs_favorites.get(&7203).map_or(true, |f| f.is_empty()), "no favorite added");
}

#[test]
fn delete_favorite_removes_and_writes_through() {
    let (mut world, _tx, mut db_rx, _l) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7204, 0, 0, 0);
    handle_parse_command(&mut world, 1, "_bbshome");
    handle_parse_command(&mut world, 1, "bbs_add_fav");
    let fav_id = world.bbs_favorites.get(&7204).unwrap()[0].fav_id;
    let _ = drain_db(&mut db_rx);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, &format!("_bbsdelfav_{fav_id}"));
    assert!(world.bbs_favorites.get(&7204).unwrap().is_empty(), "favorite removed");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::DeleteFavorite { player_id, fav_id: id } if *player_id == 7204 && *id == fav_id)),
        "the delete is written through"
    );
}

// --- Merchant (multisell) ---------------------------------------------------

use crate::data::item_data::ADENA_ID;
use crate::data::MultisellData;
use crate::game_loop::multisell::handle_multi_sell_choose;
use crate::model::components::ActiveMultisell;
use crate::model::inventory::Inventory;

/// Load the real item catalog + multisell lists (the empty test data has no
/// lists, and the loader validates ingredients/products against item templates).
fn load_real_multisell_data(world: &mut World) {
    world.data.item_data = crate::data::ItemData::load_from(DIST);
    world.data.multisells = MultisellData::load_from(DIST, &world.data.item_data);
}

/// Build a `MultiSellChoose` body (list id, entry id, amount + the ignored
/// enchant/augment/elemental tail).
fn multisell_choose_body(list_id: i32, entry_id: i32, amount: i64) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(list_id);
    w.write_i32(entry_id);
    w.write_i64(amount);
    w.write_i16(0); // enchant level
    w.write_i32(0); // augment 1
    w.write_i32(0); // augment 2
    for _ in 0..8 {
        w.write_i16(0); // attack element + six defences
    }
    w.into_bytes()
}

#[test]
fn merchant_multisell_opens_the_exchange_window() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    load_real_multisell_data(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7205, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsmultisell;600026,_bbstop");
    let pkts = drain(&mut rx);

    // `_bbstop` names no page file, so the board is not re-rendered...
    assert!(
        !pkts.iter().any(|p| p[0] == server_packets::opcodes::SHOW_BOARD),
        "no board re-render (the page file is absent, like Java's null returnHtml)"
    );
    // ...but the multisell window opens, and the open list is recorded.
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST),
        "the MultiSellList window is sent"
    );
    assert_eq!(
        world.objects.get_component::<ActiveMultisell>(&7205).map(|a| a.list_id),
        Some(600026),
        "the open list is tracked on the player"
    );
}

#[test]
fn multisell_choose_exchanges_adena_for_the_product() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    load_real_multisell_data(&mut world);
    world.id_pool = 0x7000_0000..0x7000_1000;
    let mut rx = ingame_player(&mut world, 1, 7206, 0, 0, 0);
    // 600026 entry 1: 50,000,000 adena → 1 Cloth Belt (13894).
    super::items::add_inventory_item(&mut world, 7206, ADENA_ID, 50_000_000);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsmultisell;600026,_bbstop");
    drain(&mut rx);

    let body = multisell_choose_body(600026, 1, 1);
    handle_multi_sell_choose(&mut world, 1, &body);
    let pkts = drain(&mut rx);

    let inv = world.objects.get_component::<Inventory>(&7206).unwrap();
    assert_eq!(inv.count_of(ADENA_ID), 0, "adena was spent");
    assert_eq!(inv.count_of(13894), 1, "the Cloth Belt was granted");
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::EX && {
            let sub = i16::from_le_bytes([p[1], p[2]]);
            sub == server_packets::opcodes::EX_MULTISELL_RESULT
        }),
        "an ExMultiSellResult ack is sent"
    );
}

#[test]
fn multisell_choose_refused_without_enough_adena() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    load_real_multisell_data(&mut world);
    world.id_pool = 0x7000_0000..0x7000_1000;
    let mut rx = ingame_player(&mut world, 1, 7207, 0, 0, 0);
    super::items::add_inventory_item(&mut world, 7207, ADENA_ID, 1_000); // far short
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbsmultisell;600026,_bbstop");
    drain(&mut rx);

    let body = multisell_choose_body(600026, 1, 1);
    handle_multi_sell_choose(&mut world, 1, &body);
    drain(&mut rx);

    let inv = world.objects.get_component::<Inventory>(&7207).unwrap();
    assert_eq!(inv.count_of(ADENA_ID), 1_000, "adena untouched on a shortfall");
    assert_eq!(inv.count_of(13894), 0, "no belt granted");
}

#[test]
fn multisell_choose_ignored_for_a_stale_list() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    load_real_multisell_data(&mut world);
    world.id_pool = 0x7000_0000..0x7000_1000;
    let mut rx = ingame_player(&mut world, 1, 7208, 0, 0, 0);
    super::items::add_inventory_item(&mut world, 7208, ADENA_ID, 50_000_000);
    drain(&mut rx);

    // No multisell opened → a forged choose is dropped, nothing charged.
    let body = multisell_choose_body(600026, 1, 1);
    handle_multi_sell_choose(&mut world, 1, &body);

    let inv = world.objects.get_component::<Inventory>(&7208).unwrap();
    assert_eq!(inv.count_of(ADENA_ID), 50_000_000, "no exchange without an open list");
}

// --- DropSearchBoard --------------------------------------------------------

/// Load the real NPC + item datapack into the synthetic world so the drop
/// index and item catalog are populated (the empty test data has no drops).
fn load_real_drop_data(world: &mut World) {
    world.data.npc_data = crate::data::NpcData::load_from(DIST);
    world.data.item_data = crate::data::ItemData::load_from(DIST);
}

#[test]
fn drop_search_item_and_drop_list_render() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    load_real_drop_data(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7300, 0, 0, 0);
    drain(&mut rx);

    // The nav "Search" button (`_bbs_search_item;`) opens the empty page.
    handle_parse_command(&mut world, 1, "_bbs_search_item;");
    let html = board_html(&drain(&mut rx));
    assert!(html.contains("Drop Search"), "the drop-search page rendered with navigation");

    // An empty query matches all droppable items → the first 14 icon buttons.
    handle_parse_command(&mut world, 1, "_bbs_search_item ");
    let html = board_html(&drain(&mut rx));
    assert_eq!(html.matches("_bbs_search_drop").count(), 14, "14 item-icon buttons on a full page");
    assert!(!html.contains("No Match"));

    // A nonsense query → No Match.
    handle_parse_command(&mut world, 1, "_bbs_search_item zzzznotanitem");
    assert!(board_html(&drain(&mut rx)).contains("No Match"), "no matches reported");

    // Drop list for a real indexed item: rows link each NPC to `_bbs_npc_trace`.
    let item_id = *world.data.npc_data.drop_index().keys().next().expect("some item is dropped");
    handle_parse_command(&mut world, 1, &format!("_bbs_search_drop {item_id} 1 $order $level"));
    let html = board_html(&drain(&mut rx));
    assert!(html.contains("_bbs_npc_trace"), "drop rows link to the NPC trace");
    assert!(html.contains("Drop") || html.contains("Spoil"), "each row is tagged Drop or Spoil");
}

#[test]
fn npc_trace_marks_a_live_spawn() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7301, 0, 0, 0);
    // A live NPC (template 12345) spawned at a known location.
    let (npc, extra) = crate::model::npc::Npc::for_test(9001, 12345, 111, 222, 333, 100, 100);
    world.objects.spawn(9001, (npc, extra));
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_npc_trace 12345");
    let pkts = drain(&mut rx);
    let radars: Vec<_> = pkts.iter().filter(|p| p[0] == server_packets::opcodes::RADAR_CONTROL).collect();
    assert_eq!(radars.len(), 2, "addMarker sends two RadarControl packets");
    // The marker carries the spawn coordinates (x=111 after the opcode + showRadar + type ints).
    let mut r = commons::network::PacketReader::new(&radars[0][1..]);
    let _show = r.read_i32().unwrap();
    let _type = r.read_i32().unwrap();
    assert_eq!(r.read_i32().unwrap(), 111, "marker x = spawn x");
    assert_eq!(r.read_i32().unwrap(), 222, "marker y = spawn y");
    assert_eq!(r.read_i32().unwrap(), 333, "marker z = spawn z");
}

#[test]
fn npc_trace_without_a_spawn_messages_the_player() {
    let (mut world, ..) = test_world();
    enable_board(&mut world);
    let mut rx = ingame_player(&mut world, 1, 7302, 0, 0, 0);
    drain(&mut rx);

    handle_parse_command(&mut world, 1, "_bbs_npc_trace 999999");
    let pkts = drain(&mut rx);
    assert!(
        !pkts.iter().any(|p| p[0] == server_packets::opcodes::RADAR_CONTROL),
        "no marker when nothing is spawned"
    );
    assert_eq!(count_system_messages(&pkts), 1, "the player is told no spawn was found");
}
