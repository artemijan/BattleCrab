//! Boat ferry engine (G24.5): a ferry cycles its route, snapping to each
//! waypoint in order.

use super::*;

use crate::model::boat::{Boat, DockSchedule, DwellStage, VehiclePathPoint};
use crate::model::components::Position;

/// Scan `packets` for a ferry `CreatureSay` (SAY2, `ChatType::Boat`) carrying
/// `msg_id` in its message-id slot (opcode, i32 objId=0, i32 chatType,
/// i32 charId, i32 messageId).
fn boat_announced(packets: &[Vec<u8>], msg_id: u32) -> bool {
    packets.iter().any(|p| {
        p.len() >= 17
            && p[0] == crate::network::server_packets::opcodes::SAY2
            && i32::from_le_bytes([p[5], p[6], p[7], p[8]])
                == crate::enums::ChatType::Boat.client_id()
            && i32::from_le_bytes([p[13], p[14], p[15], p[16]]) == msg_id as i32
    })
}

/// A short synthetic route (the real ferry legs are thousands of units apart,
/// so their travel times would need thousands of test ticks).
const TEST_ROUTE: &[VehiclePathPoint] = &[
    VehiclePathPoint {
        x: 1200,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
        schedule: None,
    },
    VehiclePathPoint {
        x: 1400,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
        schedule: None,
    },
    VehiclePathPoint {
        x: 1600,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
        schedule: None,
    },
];

#[test]
fn ferry_cycles_through_its_waypoints() {
    let (mut world, _tx, _db, _l) = test_world();
    let boat = crate::game_loop::boats::spawn_boat(&mut world, TEST_ROUTE);

    let pos = |w: &World| -> (i32, i32) {
        let p = w.objects.get_component::<Position>(&boat).unwrap();
        (p.x, p.y)
    };

    // Spawns docked at the last waypoint, already sailing toward waypoint 0.
    assert_eq!(
        pos(&world),
        (1600, 1000),
        "starts at the dock (last waypoint)"
    );

    // Waypoint 0 is 400 units off at speed 200 → ~2 s → ~20 ticks.
    advance_ticks(&mut world, 21);
    assert_eq!(pos(&world), (1200, 1000), "arrived at waypoint 0");

    // Each remaining leg is 200 units → ~10 ticks.
    advance_ticks(&mut world, 11);
    assert_eq!(pos(&world), (1400, 1000), "→ waypoint 1");
    advance_ticks(&mut world, 11);
    assert_eq!(pos(&world), (1600, 1000), "→ waypoint 2");

    // The cycle wraps back to waypoint 0.
    advance_ticks(&mut world, 21);
    assert_eq!(pos(&world), (1200, 1000), "loops back to waypoint 0");
}

#[test]
fn all_four_ferries_spawn_docked() {
    let (mut world, _tx, _db, _l) = test_world();
    crate::game_loop::boats::spawn_boats(&mut world);

    // Collect every spawned ferry's route.
    let mut routes: Vec<&'static [VehiclePathPoint]> = Vec::new();
    world.objects.for_each_mut::<&Boat>(|boat| {
        // Each ferry begins anchored at a harbor (its last waypoint is a dock),
        // so it is boardable at boot.
        assert!(!boat.moving, "ferry starts anchored at a dock");
        assert!(
            boat.route[boat.route.len() - 1].dock,
            "every route ends at a dock"
        );
        routes.push(boat.route);
    });

    assert_eq!(routes.len(), 4, "all four Interlude ferries spawn");

    // The three point-to-point ferries have two harbors; the Innadril scenic
    // tour loops through a single harbor. Match the multiset of dock counts.
    let mut dock_counts: Vec<usize> = routes
        .iter()
        .map(|r| r.iter().filter(|p| p.dock).count())
        .collect();
    dock_counts.sort_unstable();
    assert_eq!(
        dock_counts,
        vec![1, 2, 2, 2],
        "Innadril tour has one harbor, the other three ferries two each"
    );
}

/// A route whose dock carries a short (test-scale) announcement schedule so the
/// staged dwell can be driven in a handful of ticks instead of ten minutes.
static TEST_DOCK_SCHED: DockSchedule = DockSchedule {
    char_id: 801,
    stages: &[
        DwellStage {
            messages: &[7001, 7002], // arrival shout
            then_ms: 500,
        },
        DwellStage {
            messages: &[7003], // "leaving soon"
            then_ms: 300,
        },
        DwellStage {
            messages: &[7004], // "leaving now" → depart
            then_ms: 0,
        },
    ],
};

const SCHED_ROUTE: &[VehiclePathPoint] = &[
    // A mid-route waypoint the ferry sails to after leaving the dock.
    VehiclePathPoint {
        x: 1200,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
        schedule: None,
    },
    // The harbor with the staged departure schedule (spawn anchors here).
    VehiclePathPoint {
        x: 1000,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: true,
        schedule: Some(&TEST_DOCK_SCHED),
    },
];

#[test]
fn dock_dwell_announces_each_stage_then_departs() {
    let (mut world, _tx, _db, _l) = test_world();
    // A player waiting at the harbor hears the ferry announcements.
    let mut rx = ingame_player(&mut world, 42, 500, 1000, 1000, -3600);

    let boat = crate::game_loop::boats::spawn_boat(&mut world, SCHED_ROUTE);

    let moving = |w: &World| w.objects.get_component::<Boat>(&boat).unwrap().moving;

    // Spawns anchored at the harbor and immediately shouts its arrival stage.
    assert!(!moving(&world), "anchored on arrival");
    let p = drain(&mut rx);
    assert!(boat_announced(&p, 7001), "arrival announcement broadcast");
    assert!(boat_announced(&p, 7002), "second arrival line broadcast");
    assert!(!boat_announced(&p, 7003), "later stages have not fired yet");

    // Stage 1 fires after 500 ms (~5 ticks).
    advance_ticks(&mut world, 6);
    let p = drain(&mut rx);
    assert!(boat_announced(&p, 7003), "\"leaving soon\" stage broadcast");
    assert!(!moving(&world), "still anchored between stages");

    // Stage 2 (final) fires after a further 300 ms → shout, then weigh anchor.
    advance_ticks(&mut world, 4);
    let p = drain(&mut rx);
    assert!(boat_announced(&p, 7004), "\"leaving now\" stage broadcast");
    assert!(moving(&world), "ferry set sail after the last stage");
    assert_eq!(
        world.objects.get_component::<Boat>(&boat).unwrap().leg,
        0,
        "departed toward the next leg (index 0)"
    );
}
