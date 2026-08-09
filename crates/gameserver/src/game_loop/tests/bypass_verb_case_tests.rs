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
fn shop_world(
    npc_id: i32,
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
) {
    let (mut world, db_rx, _link_rx) = quest_test_world();
    world.data.item_data = crate::data::ItemData::load_from(DIST);
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
        sm_ids_of(&drain(&mut rx)).contains(
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
        sm_ids_of(&drain(&mut rx)).is_empty(),
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
