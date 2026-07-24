//! Boat ferry engine (G24.5): a ferry cycles its route, snapping to each
//! waypoint in order.

use super::*;

use crate::model::boat::{Boat, DockSchedule, DwellStage, InVehicle, VehiclePathPoint};
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

    // Every harbor on every ferry now carries a departure-announcement schedule
    // whose final stage departs silently (empty) — never; it always shouts.
    for route in &routes {
        for wp in route.iter().filter(|p| p.dock) {
            let sched = wp
                .schedule
                .expect("every ferry dock has an announcement schedule");
            assert!(
                sched.stages.len() >= 2,
                "a dwell has an arrival stage and a departure stage"
            );
            assert!(
                sched.stages.iter().all(|s| !s.messages.is_empty()),
                "every dwell stage announces something"
            );
            assert_eq!(
                sched.stages.last().unwrap().then_ms,
                0,
                "the final stage departs immediately after its shout"
            );
        }
    }
}

/// A route whose dock carries a short (test-scale) announcement schedule so the
/// staged dwell can be driven in a handful of ticks instead of ten minutes.
static TEST_DOCK_SCHED: DockSchedule = DockSchedule {
    char_id: 801,
    fare: crate::model::boat::Fare {
        ticket_item_id: 0, // free passage — no fare interaction in this test
        oust_x: 0,
        oust_y: 0,
        oust_z: 0,
    },
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

/// "Boat Ticket: Talking Island to Gludin" — a real EtcItem used as the fare.
const BOAT_TICKET: i32 = 1074;

/// A route whose harbor charges a boat ticket on departure and puts ticketless
/// riders ashore at (900, 900).
static FARE_DOCK_SCHED: DockSchedule = DockSchedule {
    char_id: 801,
    fare: crate::model::boat::Fare {
        ticket_item_id: BOAT_TICKET,
        oust_x: 900,
        oust_y: 900,
        oust_z: -3600,
    },
    stages: &[
        DwellStage {
            messages: &[7001],
            then_ms: 300,
        },
        DwellStage {
            messages: &[7002],
            then_ms: 0,
        },
    ],
};

const FARE_ROUTE: &[VehiclePathPoint] = &[
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
        x: 1000,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: true,
        schedule: Some(&FARE_DOCK_SCHED),
    },
];

#[test]
fn departing_ferry_collects_tickets_and_ousts_stowaways() {
    let (mut world, _tx, _db, _l) = test_world();
    // Two players board at the harbor: one holds a ticket, one is a stowaway.
    let _rx_payer = ingame_player(&mut world, 1, 100, 1000, 1000, -3600);
    let _rx_stow = ingame_player(&mut world, 2, 200, 1000, 1000, -3600);
    {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<crate::model::inventory::Inventory>(&100)
            .unwrap()
            .add_item(&data.item_data, 9_000_100, BOAT_TICKET, 1);
    }

    let boat = crate::game_loop::boats::spawn_boat(&mut world, FARE_ROUTE);
    crate::game_loop::boats::board(&mut world, 100, boat, (0, 0, 0));
    crate::game_loop::boats::board(&mut world, 200, boat, (0, 0, 0));
    assert!(world.objects.get_component::<InVehicle>(&100).is_some());
    assert!(world.objects.get_component::<InVehicle>(&200).is_some());

    // Advance through the dwell (300 ms + final stage) → departure charges fare.
    advance_ticks(&mut world, 5);
    assert!(
        world.objects.get_component::<Boat>(&boat).unwrap().moving,
        "ferry departed"
    );

    // The ticket-holder rode on, their ticket consumed.
    assert!(
        world.objects.get_component::<InVehicle>(&100).is_some(),
        "paying passenger rode on"
    );
    let tickets_left = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&100)
        .unwrap()
        .count_of(BOAT_TICKET);
    assert_eq!(tickets_left, 0, "the ticket was collected");

    // The stowaway was put ashore.
    assert!(
        world.objects.get_component::<InVehicle>(&200).is_none(),
        "stowaway removed from the boat"
    );
    let pos = world.objects.get_component::<Position>(&200).unwrap();
    assert_eq!((pos.x, pos.y), (900, 900), "stowaway teleported ashore");
}
