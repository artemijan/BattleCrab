//! `Npc.onBypassFeedback` verb routing.
//!
//! Java resolves an NPC bypass through `BypassHandler.getHandler`, which
//! lower-cases both the registry keys and the incoming command — so every
//! `bypasshandlers/` verb is case-insensitive, and this dist relies on it:
//! Giran's luxury shop (Galladucci 30097 / Alexandria 30098, plus five more
//! merchant htmls) spells the verb `Multisell`, while the rest of the dist
//! writes `multisell`.
//!
//! The same handler set includes `Link`, which the dist reaches in *both*
//! forms — bare (`Link <page>`, the support-magic pages) and NPC-scoped
//! (`npc_<id>_Link <page>`, the fisherman manuals, warehouse and pet-manager
//! info pages, craft and skill-enchant help: 73 files).

use super::*;
use crate::data::multisell_data::MultisellData;

const DIST: &str = crate::data::DIST_GAME;

/// Galladucci — the Giran luxury shop's weapon trader. `merchant/30097.htm`:
/// `bypass -h npc_%objectId%_Multisell 3009701`.
const GALLADUCCI_ID: i32 = 30097;
/// Alexandria, his wife — the armor half of the same shop, two lists.
const ALEXANDRIA_ID: i32 = 30098;
/// Any fisherman: the manual pages hang off `Link`.
const FISHERMAN_ID: i32 = 30845;

const NPC_OID: i32 = 5101;
const PLAYER_OID: i32 = 3101;

/// The real item catalog and multisell lists — the lists are the thing under
/// test's payload, so a synthetic pair would prove nothing about the dist.
fn shop_world(npc_id: i32) -> (World, db::CmdRx, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, db_rx, _link_rx) = quest_test_world();
    world.data.item_data = dist::items_owned();
    world.data.multisells = MultisellData::load_from(DIST, &world.data.item_data);
    add_test_npc(&mut world, NPC_OID, npc_id, "Merchant", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);
    (world, db_rx, rx)
}

/// `MultiSellList`'s list id: opcode, the Helios byte, then the id.
fn multisell_list_id(pkt: &[u8]) -> i32 {
    i32::from_le_bytes(pkt[2..6].try_into().unwrap())
}

fn opened_list(pkts: &[Vec<u8>]) -> Option<i32> {
    pkts.iter()
        .find(|p| p[0] == server_packets::opcodes::MULTI_SELL_LIST)
        .map(|p| multisell_list_id(p))
}

// ---------------------------------------------------------------------------
// The reported bug: the Giran luxury shop's buttons did nothing
// ---------------------------------------------------------------------------

/// Every button on the two luxury-shop htmls, verbatim as the client sends it.
#[test]
fn the_giran_luxury_shop_buttons_open_their_exchange_windows() {
    for (npc_id, command, expected) in [
        (GALLADUCCI_ID, "Multisell 3009701", 3009701),
        (ALEXANDRIA_ID, "Multisell 3009801", 3009801),
        (ALEXANDRIA_ID, "Multisell 3009802", 3009802),
    ] {
        let (mut world, _db_rx, mut rx) = shop_world(npc_id);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_{command}")),
        );
        assert_eq!(
            opened_list(&drain(&mut rx)),
            Some(expected),
            "npc {npc_id}: [{command}] should open the exchange window"
        );
    }
}

/// `BypassHandler` is a lower-cased map lookup, so the spelling in the html is
/// free. Both dist spellings must land, and so must anything in between.
#[test]
fn a_registered_handler_verb_routes_whatever_its_casing() {
    for spelling in ["multisell", "Multisell", "MULTISELL", "MuLtIsElL"] {
        let (mut world, _db_rx, mut rx) = shop_world(GALLADUCCI_ID);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_{spelling} 3009701")),
        );
        assert_eq!(
            opened_list(&drain(&mut rx)),
            Some(3009701),
            "[{spelling}] is the same handler as `multisell`"
        );
    }
}

/// A verb an NPC subclass answers is *not* in that map: only
/// `VillageMaster.onBypassFeedback` handles `create_clan`, and it compares with
/// a case-sensitive `startsWith`. Folding case there would be a deviation, not
/// a fix — so the pair must disagree.
#[test]
fn an_npc_instance_verb_stays_case_sensitive() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, PLAYER_OID, 0, 0, 0);
    drain(&mut rx);

    // Level < 10 — the first guard in `ClanTable.createClan`, and proof the
    // verb reached the handler at all.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_create_clan Myclan")),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).contains(
            &server_packets::sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN
        ),
        "the exact spelling routes"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Create_Clan Myclan")),
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE).is_empty(),
        "a re-cased instance verb is unhandled, like Java's startsWith"
    );
}

// ---------------------------------------------------------------------------
// `Link` in its NPC-scoped form
// ---------------------------------------------------------------------------

/// The fisherman manual's own page buttons. Java routes `npc_<id>_Link` and
/// bare `Link` to the same `bypasshandlers/Link`, and the window belongs to
/// the NPC that was clicked (`%objectId%` must be substituted or the page's
/// own buttons come back inert).
#[test]
fn an_npc_scoped_link_serves_the_whitelisted_page() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Link fisherman/fishing_manual002.htm"
        )),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("the link page is sent");
    assert!(
        html.contains("Fishing Manual"),
        "served the requested page, got: {html}"
    );
    assert!(
        html.contains(&format!("npc_{NPC_OID}_Link")),
        "the page's %objectId% is substituted with the clicked NPC, got: {html}"
    );
}

/// `Link.java` answers a page outside its whitelist with an empty html — the
/// window opens blank rather than serving an arbitrary file.
#[test]
fn an_npc_scoped_link_refuses_a_page_outside_the_whitelist() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Link merchant/30097.htm")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a window is still sent");
    assert!(html.is_empty(), "off-whitelist page is not served: {html}");
}

// ---------------------------------------------------------------------------
// `player_help` and `TerritoryStatus` (measured-gaps rows 7 and 8)
// ---------------------------------------------------------------------------

/// The text a `NpcHtmlMessage` carries: opcode, object id, then the string.
fn html_body(pkt: &[u8]) -> String {
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    let _object_id = r.read_i32().unwrap();
    r.read_string().unwrap()
}

fn html_packet(pkts: &[Vec<u8>]) -> Option<String> {
    pkts.iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .map(|p| html_body(p))
}

/// `bypasshandlers/PlayerHelp` — the help book's pages link to each other
/// through this bypass, and 92 files under `data/html/help/` use it. It was
/// unhandled, so every "Next Page" button in the book was dead.
#[test]
fn player_help_opens_a_help_page() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    world.data.root = DIST.to_string();
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("player_help 7100.htm"));

    let html = html_packet(&drain(&mut rx)).expect("the page opens");
    assert!(!html.is_empty(), "and carries the file's text");
}

/// The `#<itemId>` suffix — Java turns it into `NpcHtmlMessage(0, itemId)`, an
/// item-bound dialog the client keeps open when a button inside is pressed.
/// That is what lets the book page through its own links.
#[test]
fn player_help_marks_the_dialog_item_bound() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    world.data.root = DIST.to_string();
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("player_help classchange/7096-2.htm#7096"),
    );

    let pkt = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("the page opens");
    // …objectId, html string, then the item id.
    let mut r = commons::network::PacketReader::new(&pkt[1..]);
    let _object_id = r.read_i32().unwrap();
    let _html = r.read_string().unwrap();
    assert_eq!(
        r.read_i32().unwrap(),
        7096,
        "bound to the book that opened it"
    );
}

/// Java's traversal guard: a path with `..` is refused outright.
#[test]
fn player_help_refuses_a_traversal_path() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    world.data.root = DIST.to_string();
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("player_help ../../config/Server.ini"),
    );

    assert!(
        html_packet(&drain(&mut rx)).is_none(),
        "nothing is served for a traversal attempt"
    );
}

/// `bypasshandlers/TerritoryStatus` — the "local lord and tax rate" button that
/// 254 of the dist's folk htmls carry. Unowned castle → the no-clan page.
#[test]
fn territory_status_answers_for_an_unowned_castle() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    world.data.root = DIST.to_string();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_TerritoryStatus")),
    );

    let html = html_packet(&drain(&mut rx)).expect("the page opens");
    // `territorynoclan.htm` — Java's other branch, and it words itself
    // differently from the owned page.
    assert!(
        html.contains("not currently under the rule of any clan"),
        "the no-clan page, got: {html}"
    );
    // A literal `%` survives in the owned page's "Tax Rate : N %", so the
    // check names the placeholders rather than banning the character.
    for token in ["%castlename%", "%territory%", "%objectId%"] {
        assert!(!html.contains(token), "{token} is still unfilled: {html}");
    }
}

/// With an owner, the page names the clan and its leader, and fills the tax
/// rate and kingdom in.
#[test]
fn territory_status_names_the_lord_of_an_owned_castle() {
    let (mut world, _db_rx, mut rx) = shop_world(FISHERMAN_ID);
    world.data.root = DIST.to_string();
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST);
    // The NPC sits at (0,0,0); `findNearestCastle` picks whichever that is.
    let castle_id = world.data.zone_data.nearest_castle_at(0, 0, 0).unwrap();
    world.castles = vec![model::castle::Castle {
        show_npc_crest: false,
        id: castle_id,
        name: "Giran".into(),
        side: model::castle::CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    let mut clan = Clan {
        id: 900,
        name: "Holders".into(),
        leader_id: 4242,
        level: 5,
        reputation_score: 0,
        castle_id,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    };
    clan.members.push(model::clan::ClanMember {
        char_id: 4242,
        name: "Lordy".into(),
        level: 80,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 1,
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
        title: String::new(),
    });
    world.clans.insert(900, clan);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_TerritoryStatus")),
    );

    let html = html_packet(&drain(&mut rx)).expect("the page opens");
    assert!(html.contains("Lordy"), "the lord is named: {html}");
    assert!(html.contains("Holders"), "and their clan: {html}");
    assert!(html.contains("Giran"), "and the castle: {html}");
    for token in [
        "%castlename%",
        "%clanname%",
        "%clanleadername%",
        "%taxpercent%",
        "%territory%",
        "%objectId%",
    ] {
        assert!(!html.contains(token), "{token} is still unfilled: {html}");
    }
}
