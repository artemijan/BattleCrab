//! Cruma Tower's floor chain (`data/teleporters/others/CrumaTower.xml`).
//!
//! Carsus (30483) at the tower entrance serves the **3rd** basement floor and
//! nothing else. That is an operator deviation from retail, where he also
//! offers the 2nd floor — see `docs/CUSTOM_DIST_DEVIATIONS.md`; the 2nd floor
//! is now reached onward from the 3rd via Rombel (30487). The 1st floor was
//! never his to give: his page points at Ivory Tower Wizard Ian (30486), the
//! sole holder of that destination, in a remote area of the 2nd floor.
//!
//! These tests pin the route against the real dist data, so neither the
//! trimmed entrance list nor Ian's untouched route can be mistaken for a
//! dropped destination — and so a re-sync from the Java dist that restores the
//! 2nd-floor line (which would silently push the entrance button onto the
//! wrong floor, the buttons addressing destinations by index) fails loudly.

use super::*;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

const CARSUS_ID: i32 = 30483;
const IAN_ID: i32 = 30486;
const ROMBEL_ID: i32 = 30487;
const IAN_OID: i32 = NPC_OID + 1;
const CARSUS_OID: i32 = NPC_OID + 2;

/// 1st basement floor — Ian's only destination.
const FIRST_FLOOR: (i32, i32, i32) = (17616, 115436, -6584);
/// The 2nd floor, deliberately absent from the entrance list; Rombel's only
/// destination, and the sole way onto that floor now.
const SECOND_FLOOR: (i32, i32, i32) = (17708, 108308, -9056);
/// Carsus' one entrance destination.
const THIRD_FLOOR: (i32, i32, i32) = (17726, 114838, -11696);
/// The retail 2nd-floor entry that Carsus used to carry.
const RETAIL_ENTRANCE_SECOND_FLOOR: (i32, i32, i32) = (17664, 108288, -9056);

/// `teleporter_world` + the real dist html root and teleport lists, with Ian
/// and Carsus spawned alongside the fixture gatekeeper.
fn cruma_world(adena: i64) -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut rx) = teleporter_world(adena);
    world.data.root = DIST.to_string();
    world.data.teleporters = crate::data::teleporter_data::TeleporterData::load_from(DIST);
    add_test_npc(&mut world, IAN_OID, IAN_ID, "Teleporter", 70, 100, 0, 0);
    add_test_npc(
        &mut world,
        CARSUS_OID,
        CARSUS_ID,
        "Teleporter",
        70,
        110,
        0,
        0,
    );
    drain(&mut rx);
    (world, rx)
}

/// Talking to Ian serves his own page (not the "text is missing" stub) with a
/// working 1st-floor button, and pressing it puts the player on the 1st
/// basement floor free of charge — the `OTHER` list carries no fee.
#[test]
fn ian_talk_and_teleport_reach_the_first_basement_floor() {
    let (mut world, mut rx) = cruma_world(1_000);

    // First click selects the target, the second one talks (Java `onAction`).
    handle_action(&mut world, 1, &action_body(IAN_OID, 0));
    drain(&mut rx);
    handle_action(&mut world, 1, &action_body(IAN_OID, 0));
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("Ian's chat window");
    assert!(
        !contains_utf16(html, "My Text is missing"),
        "html/teleporter/30486.htm must be served"
    );
    assert!(
        contains_utf16(html, "1st basement floor"),
        "Ian's page offers the 1st floor"
    );
    assert!(
        contains_utf16(html, &format!("npc_{IAN_OID}_teleport OTHER 0")),
        "the button carries the OTHER-list bypass with the npc's object id"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{IAN_OID}_teleport OTHER 0")),
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "teleport packet sent"
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    // `teleport_player` grounds z on the geodata and lifts it 5 like Java's
    // `teleToLocation`; the test world has no geo loaded, so z passes through.
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (FIRST_FLOOR.0, FIRST_FLOOR.1, FIRST_FLOOR.2 + 5)
    );
    assert_eq!(
        adena_of(&world, 3001),
        1_000,
        "the OTHER list charges no fee"
    );
}

/// The entrance list is the 3rd floor alone (custom): Carsus offers neither
/// the 2nd floor — Rombel's job, from the 3rd floor — nor the 1st, which is
/// Ian's.
#[test]
fn the_entrance_list_is_the_third_floor_alone() {
    let data = crate::data::teleporter_data::TeleporterData::load_from(DIST);

    let carsus = data.holder(CARSUS_ID, "OTHER").expect("Carsus OTHER list");
    let dests: Vec<_> = carsus
        .locations
        .iter()
        .map(|l| (l.x, l.y, l.z))
        .collect::<Vec<_>>();
    assert_eq!(
        dests,
        vec![THIRD_FLOOR],
        "the entrance drops players on the 3rd floor and nowhere else"
    );
    assert!(
        !dests.contains(&RETAIL_ENTRANCE_SECOND_FLOOR),
        "the retail 2nd-floor destination is deliberately removed from the entrance"
    );
    assert!(
        !dests.contains(&FIRST_FLOOR),
        "the 1st floor is deliberately absent from the entrance"
    );

    let ian = data.holder(IAN_ID, "OTHER").expect("Ian OTHER list");
    assert_eq!(
        ian.locations
            .iter()
            .map(|l| (l.x, l.y, l.z))
            .collect::<Vec<_>>(),
        vec![FIRST_FLOOR],
        "Ian is the 1st-floor elevator"
    );

    let rombel = data.holder(ROMBEL_ID, "OTHER").expect("Rombel OTHER list");
    assert_eq!(
        rombel
            .locations
            .iter()
            .map(|l| (l.x, l.y, l.z))
            .collect::<Vec<_>>(),
        vec![SECOND_FLOOR],
        "Rombel still carries the 2nd floor — the only route onto it now"
    );
}

/// Carsus' page offers exactly one teleport button, and it lands on the 3rd
/// floor. Trimming the xml shifted the 3rd floor from index 1 to index 0, so
/// this walks the real html end to end: a stale `OTHER 1` button would fail
/// the bypass outright, and a stale index against a restored 2nd-floor line
/// would silently teleport to the wrong floor.
#[test]
fn carsus_offers_only_the_third_basement_floor() {
    let (mut world, mut rx) = cruma_world(1_000);

    // First click selects the target, the second one talks (Java `onAction`).
    handle_action(&mut world, 1, &action_body(CARSUS_OID, 0));
    drain(&mut rx);
    handle_action(&mut world, 1, &action_body(CARSUS_OID, 0));
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("Carsus' chat window");
    assert!(
        !contains_utf16(html, "My Text is missing"),
        "html/teleporter/30483.htm must be served"
    );
    assert!(
        contains_utf16(html, "3rd basement floor"),
        "his page offers the 3rd floor"
    );
    assert!(
        !contains_utf16(html, "go to the 2nd basement floor"),
        "the 2nd-floor button is removed from his page"
    );
    assert!(
        contains_utf16(html, &format!("npc_{CARSUS_OID}_teleport OTHER 0")),
        "the surviving button addresses index 0 — the 3rd floor after the trim"
    );
    assert!(
        !contains_utf16(html, &format!("npc_{CARSUS_OID}_teleport OTHER 1")),
        "no button may address the index the removed 2nd floor used to share"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{CARSUS_OID}_teleport OTHER 0")),
    );
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "teleport packet sent"
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    // z passes through +5 like Ian's route above: no geodata in the test world.
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (THIRD_FLOOR.0, THIRD_FLOOR.1, THIRD_FLOOR.2 + 5),
        "the button lands on the 3rd floor, not the removed 2nd"
    );
    assert_eq!(
        adena_of(&world, 3001),
        1_000,
        "the OTHER list charges no fee"
    );
}

/// Ian is actually placed on the map — his spawn line lives in
/// `spawns/Dion/Dion.xml`, a subdirectory file, so this also guards the
/// recursive spawn walk that makes him reachable at all.
#[test]
fn ian_is_spawned_on_the_second_floor() {
    let data = crate::data::spawn_data::SpawnData::load_from(DIST);
    let ian = data
        .spawns
        .iter()
        .flat_map(|s| s.groups.iter())
        .flat_map(|g| g.npcs.iter())
        .find(|n| n.npc_id == IAN_ID)
        .expect("Ian must be spawned");
    let loc = ian.loc.expect("fixed loc");
    assert_eq!((loc.x, loc.y, loc.z), (17722, 119749, -9068));
}

/// Rombel is actually placed on the 3rd floor. With the 2nd floor gone from
/// the entrance he is the only way onto it, so an unspawned Rombel would
/// strand the 2nd floor (and Ian behind it) rather than merely inconvenience.
#[test]
fn rombel_is_spawned_on_the_third_floor() {
    let data = crate::data::spawn_data::SpawnData::load_from(DIST);
    let rombel = data
        .spawns
        .iter()
        .flat_map(|s| s.groups.iter())
        .flat_map(|g| g.npcs.iter())
        .find(|n| n.npc_id == ROMBEL_ID)
        .expect("Rombel must be spawned");
    let loc = rombel.loc.expect("fixed loc");
    assert_eq!((loc.x, loc.y, loc.z), (17811, 114750, -11680));
}
