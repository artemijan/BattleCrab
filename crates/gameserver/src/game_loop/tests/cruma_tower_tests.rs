//! Cruma Tower's floor chain (`data/teleporters/others/CrumaTower.xml`).
//!
//! Carsus (30483) at the tower entrance serves **only** the 2nd and 3rd
//! basement floors — that is retail Classic Interlude, not a porting gap:
//! his own page says the 1st floor is reached "through Ivory Tower Wizard Ian
//! who you can find in a remote area of the 2nd basement floor", and Ian
//! (30486) is the sole holder of the 1st-floor destination. These tests pin
//! that route against the real dist data so the entrance list staying at two
//! options can't be mistaken for a dropped destination again.

use super::*;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

const CARSUS_ID: i32 = 30483;
const IAN_ID: i32 = 30486;
const IAN_OID: i32 = NPC_OID + 1;

/// 1st basement floor — Ian's only destination.
const FIRST_FLOOR: (i32, i32, i32) = (17616, 115436, -6584);
/// Carsus' two entrance destinations.
const SECOND_FLOOR: (i32, i32, i32) = (17664, 108288, -9056);
const THIRD_FLOOR: (i32, i32, i32) = (17726, 114838, -11696);

/// `teleporter_world` + the real dist html root and teleport lists, with Ian
/// spawned alongside the fixture gatekeeper.
fn cruma_world(adena: i64) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut rx) = teleporter_world(adena);
    world.data.root = DIST.to_string();
    world.data.teleporters = crate::data::teleporter_data::TeleporterData::load_from(DIST);
    add_test_npc(&mut world, IAN_OID, IAN_ID, "Teleporter", 70, 100, 0, 0);
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

/// The entrance list is the retail two floors: Carsus never offers the 1st
/// floor, and Ian is the only NPC in the dist that does.
#[test]
fn the_entrance_list_is_the_retail_two_floors() {
    let data = crate::data::teleporter_data::TeleporterData::load_from(DIST);

    let carsus = data.holder(CARSUS_ID, "OTHER").expect("Carsus OTHER list");
    let dests: Vec<_> = carsus
        .locations
        .iter()
        .map(|l| (l.x, l.y, l.z))
        .collect::<Vec<_>>();
    assert_eq!(dests, vec![SECOND_FLOOR, THIRD_FLOOR]);
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
