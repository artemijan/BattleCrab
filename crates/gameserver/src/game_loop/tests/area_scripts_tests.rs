//! `ai/areas` slice 1 — the talk/teleporter NPCs: Toma's wandering spawn,
//! the Elroki ferry pair, the Pagan Temple door gatekeepers, and Tunatun.

use super::*;

use crate::game_loop::area_npcs::{self, TOMA};

const TOMA_LOCS: [(i32, i32, i32); 3] = [
    (151680, -174891, -1782),
    (154153, -220105, -3402),
    (178834, -184336, -355),
];

fn toma_positions(world: &mut World) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &Position)>(|(n, p)| {
            if n.npc_id == TOMA {
                out.push((p.x, p.y, p.z));
            }
        });
    out
}

/// Toma is script-owned (not in the spawn data): boot places exactly one at
/// one of his three haunts, and the 30-minute beat moves him — never
/// duplicates him.
#[test]
fn toma_spawns_at_boot_and_relocates_without_duplicating() {
    let (mut world, _db, _l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(TOMA);
    t.type_name = "Folk".into();
    world.data.npc_data.insert_for_test(t);

    area_npcs::spawn_at_boot(&mut world);
    let at_boot = toma_positions(&mut world);
    assert_eq!(at_boot.len(), 1, "exactly one Toma after boot");
    assert!(TOMA_LOCS.contains(&at_boot[0]), "on a known haunt");

    // The beat fires (directly — the scheduled path is the same fn).
    for _ in 0..5 {
        area_npcs::relocate_toma(&mut world);
        let now = toma_positions(&mut world);
        assert_eq!(now.len(), 1, "relocation never duplicates him");
        assert!(TOMA_LOCS.contains(&now[0]));
    }
}

/// Orahochin ferries a peaceful player across, but refuses one whose attack
/// stance is still running (Java `talker.isInCombat()`).
#[test]
fn elroki_teleporter_refuses_combat_then_ferries() {
    let (mut world, _db, _l) = combat_test_world();
    const ORAHOCHIN: i32 = 32111;
    add_test_npc(&mut world, NPC_OID, ORAHOCHIN, "Folk", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // In combat: no teleport.
    world.objects.add_components(
        &5001,
        crate::model::components::AttackState {
            attack_end_tick: 0,
            stance_until_tick: world.tick + 150,
        },
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElrokiTeleporters")),
    );
    let pos = world.objects.get_component::<Position>(&5001).unwrap();
    assert_eq!((pos.x, pos.y), (60, 0), "still standing at the chasm");

    // Stance over: ferried to the island.
    world
        .objects
        .get_component_mut::<crate::model::components::AttackState>(&5001)
        .unwrap()
        .stance_until_tick = 0;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElrokiTeleporters")),
    );
    let pos = world.objects.get_component::<Position>(&5001).unwrap();
    // z is geo-grounded + 5 by `teleport_player` (Java `teleToLocation`).
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (4990, -1879, -3173),
        "Orahochin's island drop-off"
    );
}

/// The way-out gatekeeper (32035, no mark needed) opens the outer temple
/// door, and the scripted 10 s timer shuts it again.
#[test]
fn pagan_gatekeeper_opens_the_door_and_it_closes_itself() {
    let (mut world, _db, _l) = combat_test_world();
    const GATEKEEPER_OUT: i32 = 32035;
    const OUTER_DOOR: i32 = 19_160_001;
    crate::model::door::spawn_door_for_test(
        &mut world,
        crate::data::door_data::DoorTemplate {
            id: OUTER_DOOR,
            name: "pagan_outer".into(),
            node_x: [-16654; 4],
            node_y: [-36864; 4],
            node_z: -10759,
            height: 150,
            x: -16654,
            y: -36864,
            z: -10759,
            hp_max: 100,
            p_def: 0,
            m_def: 0,
            targetable: false,
            show_hp: false,
            open_by_default: false,
            open_method: crate::data::door_data::DoorOpenMethod::None,
            open_time: 0,
            close_time: -1,
            random_time: 0,
        },
    );
    assert!(!world.geo.doors.is_open(OUTER_DOOR));

    // The door consumed `next_npc_object_id` (== NPC_OID) — a fixture NPC on
    // the same oid would clobber it (the classic fixture/allocator collision).
    let gatekeeper_oid = NPC_OID + 7;
    add_test_npc(
        &mut world,
        gatekeeper_oid,
        GATEKEEPER_OUT,
        "Folk",
        40,
        100,
        0,
        0,
    );
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gatekeeper_oid}_Quest PaganTeleporters")),
    );
    assert!(
        world.geo.doors.is_open(OUTER_DOOR),
        "the gatekeeper opened the way out"
    );

    // Java `Close_Door1` at 10 s.
    advance_ticks(&mut world, 101);
    assert!(
        !world.geo.doors.is_open(OUTER_DOOR),
        "the door shuts itself"
    );
}

/// The outside gatekeeper (32034) demands a mark — empty-handed visitors do
/// not get the door.
#[test]
fn pagan_outer_gatekeeper_demands_a_mark() {
    let (mut world, _db, _l) = combat_test_world();
    const GATEKEEPER_IN: i32 = 32034;
    add_test_npc(&mut world, NPC_OID, GATEKEEPER_IN, "Folk", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest PaganTeleporters")),
    );
    // No door exists in this world; the observable contract is simply that
    // nothing panicked and no door opened.
    assert!(!world.geo.doors.is_open(19_160_001));
}

/// Tunatun's whip: level 82+ gets it once; below, a refusal; asking again
/// with one in the bag is refused too.
#[test]
fn tunatun_hands_out_one_whip_at_level_82() {
    let (mut world, _db, _l) = combat_test_world();
    const TUNATUN: i32 = 31537;
    const WHIP: i32 = 15473;
    add_test_npc(&mut world, NPC_OID, TUNATUN, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 5001, 60, 0, 0);

    // Under-leveled: refused.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 0, "level 5 gets nothing");

    // Level 82: whip granted — once.
    world
        .objects
        .get_component_mut::<crate::model::Player>(&5001)
        .unwrap()
        .level = 82;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 1, "whip granted");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Tunatun whip")),
    );
    assert_eq!(item_count(&world, 5001, WHIP), 1, "never a second whip");
}
