//! Cruma Tower's floor chain (`data/teleporters/others/CrumaTower.xml`).
//!
//! In retail Classic Interlude — and in upstream Mobius — Carsus (30483) at
//! the tower entrance serves **only** the 2nd and 3rd basement floors, with
//! the 1st floor reachable solely from Ian (30486) at the far end of the 2nd
//! floor. **This dist deviates deliberately**: Carsus carries a third,
//! operator-added 1st-floor destination (appended last, so his retail 0/1
//! indices stay put) and `html/teleporter/30483.htm` grew a button for it.
//! Ian's route is untouched and still works. These tests pin both routes so
//! the custom line can't be dropped by a dist re-sync without a red test.

use super::*;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

const CARSUS_ID: i32 = 30483;
const IAN_ID: i32 = 30486;
const CARSUS_OID: i32 = NPC_OID + 2;
const IAN_OID: i32 = NPC_OID + 1;

/// 1st basement floor — Ian's only destination, and Carsus' custom index 2.
const FIRST_FLOOR: (i32, i32, i32) = (17616, 115436, -6584);
/// Carsus' two retail entrance destinations.
const SECOND_FLOOR: (i32, i32, i32) = (17664, 108288, -9056);
const THIRD_FLOOR: (i32, i32, i32) = (17726, 114838, -11696);

/// `teleporter_world` + the real dist html root and teleport lists, with Ian
/// and Carsus spawned alongside the fixture gatekeeper.
fn cruma_world(adena: i64) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
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
        100,
        0,
        0,
    );
    drain(&mut rx);
    (world, rx)
}

/// The custom entrance route: Carsus' page offers the 1st basement floor and
/// the button lands the player there, without disturbing the retail 2nd/3rd
/// floor buttons that still sit at indices 0 and 1.
#[test]
fn carsus_entrance_offers_the_custom_first_floor_button() {
    let (mut world, mut rx) = cruma_world(1_000);

    handle_action(&mut world, 1, &action_body(CARSUS_OID, 0));
    drain(&mut rx);
    handle_action(&mut world, 1, &action_body(CARSUS_OID, 0));
    let pkts = drain(&mut rx);
    let html = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE)
        .expect("Carsus' chat window");
    assert!(
        contains_utf16(html, &format!("npc_{CARSUS_OID}_teleport OTHER 2")),
        "the custom 1st-floor button"
    );
    assert!(
        contains_utf16(html, &format!("npc_{CARSUS_OID}_teleport OTHER 0"))
            && contains_utf16(html, &format!("npc_{CARSUS_OID}_teleport OTHER 1")),
        "the retail 2nd/3rd floor buttons keep their indices"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{CARSUS_OID}_teleport OTHER 2")),
    );
    drain(&mut rx);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
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

/// The entrance list is the retail two floors *plus* the custom 1st floor,
/// in that order — the retail lines must keep indices 0/1 or the shipped
/// html buttons would send players to the wrong floor.
#[test]
fn the_entrance_list_keeps_retail_indices_and_appends_the_first_floor() {
    let data = crate::data::teleporter_data::TeleporterData::load_from(DIST);

    let carsus = data.holder(CARSUS_ID, "OTHER").expect("Carsus OTHER list");
    let dests: Vec<_> = carsus
        .locations
        .iter()
        .map(|l| (l.x, l.y, l.z))
        .collect::<Vec<_>>();
    assert_eq!(dests, vec![SECOND_FLOOR, THIRD_FLOOR, FIRST_FLOOR]);

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
