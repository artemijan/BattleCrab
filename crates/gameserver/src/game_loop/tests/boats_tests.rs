//! Boat ferry engine (G24.5): a ferry cycles its route, snapping to each
//! waypoint in order.

use super::*;
use crate::game_loop::boats;

use crate::model::boat::{
    Boat, DockSchedule, DwellStage, Fare, InVehicle, RouteDef, RouteId, VehiclePathPoint,
};
use crate::model::components::Position;

/// Scan `packets` for a ferry `CreatureSay` (SAY2, `ChatType::Boat`) carrying
/// `msg_id` in its message-id slot (opcode, i32 objId=0, i32 chatType,
/// i32 charId, i32 messageId).
fn boat_announced(packets: &[Vec<u8>], msg_id: u32) -> bool {
    packets.iter().any(|p| {
        p.len() >= 17
            && p[0] == server_packets::opcodes::SAY2
            && i32::from_le_bytes([p[5], p[6], p[7], p[8]])
                == crate::enums::ChatType::Boat.client_id()
            && i32::from_le_bytes([p[13], p[14], p[15], p[16]]) == msg_id as i32
    })
}

/// A non-dock waypoint at `(x, y)` with test-scale speeds.
fn wp(x: i32, y: i32) -> VehiclePathPoint {
    VehiclePathPoint {
        x,
        y,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
        schedule: None,
    }
}

/// A harbor waypoint at `(x, y)` carrying the route's schedule 0.
fn dock(x: i32, y: i32) -> VehiclePathPoint {
    VehiclePathPoint {
        dock: true,
        schedule: Some(0),
        ..wp(x, y)
    }
}

/// Register `def` and spawn a ferry on it.
fn spawn_on(world: &mut World, def: RouteDef) -> i32 {
    let route = world.boat_routes.register(def);
    boats::spawn_boat(world, route)
}

/// A free (no-fare) harbor.
fn free_fare() -> Fare {
    Fare {
        ticket_item_id: 0,
        oust_x: 0,
        oust_y: 0,
        oust_z: 0,
    }
}

/// A short synthetic route (the real ferry legs are thousands of units apart,
/// so their travel times would need thousands of test ticks).
fn test_route() -> RouteDef {
    RouteDef {
        waypoints: vec![wp(1200, 1000), wp(1400, 1000), wp(1600, 1000)],
        schedules: vec![],
    }
}

#[test]
fn ferry_cycles_through_its_waypoints() {
    let (mut world, _tx, _db, _l) = test_world();
    let boat = spawn_on(&mut world, test_route());

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
    boats::spawn_boats(&mut world);

    // Collect every spawned ferry's route.
    let mut routes: Vec<RouteId> = Vec::new();
    world.objects.for_each_mut::<&Boat>(|boat| {
        // Each ferry begins anchored at a harbor, so it is boardable at boot.
        assert!(!boat.moving, "ferry starts anchored at a dock");
        routes.push(boat.route);
    });

    assert_eq!(routes.len(), 4, "all four Interlude ferries spawn");

    for &id in &routes {
        assert!(
            world.boat_routes.get(id).waypoints.last().unwrap().dock,
            "every route ends at a dock (the ferry spawns anchored there)"
        );
    }

    // The three point-to-point ferries have two harbors; the Innadril scenic
    // tour loops through a single harbor. Match the multiset of dock counts.
    let mut dock_counts: Vec<usize> = routes
        .iter()
        .map(|&id| {
            world
                .boat_routes
                .get(id)
                .waypoints
                .iter()
                .filter(|p| p.dock)
                .count()
        })
        .collect();
    dock_counts.sort_unstable();
    assert_eq!(
        dock_counts,
        vec![1, 2, 2, 2],
        "Innadril tour has one harbor, the other three ferries two each"
    );

    // Every harbor on every ferry now carries a departure-announcement schedule
    // whose final stage departs silently (empty) — never; it always shouts.
    for &id in &routes {
        let def = world.boat_routes.get(id);
        for dock_wp in def.waypoints.iter().filter(|p| p.dock) {
            let sched = def
                .schedule_of(dock_wp)
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

/// A dock schedule short enough (test-scale) that the staged dwell can be
/// driven in a handful of ticks instead of ten minutes.
fn test_dock_sched() -> DockSchedule {
    DockSchedule {
        char_id: 801,
        fare: free_fare(), // free passage — no fare interaction in this test
        voyage: vec![],
        stages: vec![
            DwellStage {
                messages: vec![7001, 7002], // arrival shout
                then_ms: 500,
            },
            DwellStage {
                messages: vec![7003], // "leaving soon"
                then_ms: 300,
            },
            DwellStage {
                messages: vec![7004], // "leaving now" → depart
                then_ms: 0,
            },
        ],
    }
}

/// A two-waypoint route whose harbor (1000, 1000) carries `sched`.
fn route(sched: DockSchedule) -> RouteDef {
    RouteDef {
        waypoints: vec![wp(1200, 1000), dock(1000, 1000)],
        schedules: vec![sched],
    }
}

#[test]
fn dock_dwell_announces_each_stage_then_departs() {
    let (mut world, _tx, _db, _l) = test_world();
    // A player waiting at the harbor hears the ferry announcements.
    let mut rx = ingame_player(&mut world, 42, 500, 1000, 1000, -3600);

    let boat = spawn_on(&mut world, route(test_dock_sched()));

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

/// A dock schedule whose harbor charges a boat ticket on departure and puts
/// ticketless riders ashore at (900, 900).
fn fare_dock_sched() -> DockSchedule {
    DockSchedule {
        char_id: 801,
        fare: Fare {
            ticket_item_id: BOAT_TICKET,
            oust_x: 900,
            oust_y: 900,
            oust_z: -3600,
        },
        voyage: vec![],
        stages: vec![
            DwellStage {
                messages: vec![7001],
                then_ms: 300,
            },
            DwellStage {
                messages: vec![7002],
                then_ms: 0,
            },
        ],
    }
}

#[test]
fn departing_ferry_collects_tickets_and_ousts_stowaways() {
    let (mut world, _tx, _db, _l) = test_world();
    // Two players board at the harbor: one holds a ticket, one is a stowaway.
    let _rx_payer = ingame_player(&mut world, 1, 100, 1000, 1000, -3600);
    let _rx_stow = ingame_player(&mut world, 2, 200, 1000, 1000, -3600);
    {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&100)
            .unwrap()
            .add_item(&data.item_data, 9_000_100, BOAT_TICKET, 1);
    }
    let boat = spawn_on(&mut world, route(fare_dock_sched()));
    boats::board(&mut world, 100, boat, (0, 0, 0));
    boats::board(&mut world, 200, boat, (0, 0, 0));
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
        .get_component::<Inventory>(&100)
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

#[test]
fn walking_on_deck_moves_the_seat_and_broadcasts() {
    use crate::network::server_packets::opcodes;
    let (mut world, _tx, _db, _l) = test_world();
    let mut rx = ingame_player(&mut world, 1, 100, 1000, 1000, -3600);

    // Board the anchored ferry at seat origin, then drain the boarding packets.
    let boat = spawn_on(&mut world, route(test_dock_sched()));
    boats::board(&mut world, 100, boat, (0, 0, 0));
    let _ = drain(&mut rx);

    // Walk across the deck: seat origin (0,0,0) → (50, -20, 0).
    boats::move_in_vehicle(&mut world, 100, boat, (50, -20, 0), (0, 0, 0));
    let iv = world.objects.get_component::<InVehicle>(&100).unwrap();
    assert_eq!(
        (iv.seat_x, iv.seat_y, iv.seat_z),
        (50, -20, 0),
        "the rider's seat moved to the new deck spot"
    );
    // Absolute position = boat position (1000,1000) + the new seat.
    let pos = world.objects.get_component::<Position>(&100).unwrap();
    assert_eq!((pos.x, pos.y), (1050, 980), "rider stands at boat + seat");
    let p = drain(&mut rx);
    assert!(
        has_opcode(&p, opcodes::MOVE_TO_LOCATION_IN_VEHICLE),
        "the on-deck walk is broadcast"
    );

    // A zero-length move is a stop request → StopMoveInVehicle, seat unchanged.
    boats::move_in_vehicle(&mut world, 100, boat, (50, -20, 0), (50, -20, 0));
    let iv = world.objects.get_component::<InVehicle>(&100).unwrap();
    assert_eq!(
        (iv.seat_x, iv.seat_y),
        (50, -20),
        "stop leaves the seat put"
    );
    let p = drain(&mut rx);
    assert!(
        has_opcode(&p, opcodes::STOP_MOVE_IN_VEHICLE),
        "a no-op move stops the rider"
    );
    assert!(
        !has_opcode(&p, opcodes::MOVE_TO_LOCATION_IN_VEHICLE),
        "a stop does not broadcast a walk"
    );
}

/// A dock whose ferry, after departing, shouts an in-transit arrival message
/// 200 ms into the voyage.
fn voyage_dock_sched() -> DockSchedule {
    DockSchedule {
        char_id: 801,
        fare: free_fare(),
        voyage: vec![(200, vec![8001])],
        stages: vec![
            DwellStage {
                messages: vec![7001],
                then_ms: 100,
            },
            DwellStage {
                messages: vec![7002],
                then_ms: 0,
            },
        ],
    }
}

fn voyage_route() -> RouteDef {
    RouteDef {
        // A distant waypoint so the ferry is still sailing when the shout fires.
        waypoints: vec![wp(3000, 1000), dock(1000, 1000)],
        schedules: vec![voyage_dock_sched()],
    }
}

#[test]
fn in_transit_arrival_shout_fires_while_sailing_then_stops_once_docked() {
    let (mut world, _tx, _db, _l) = test_world();
    let mut rx = ingame_player(&mut world, 1, 100, 1000, 1000, -3600);

    let boat = spawn_on(&mut world, voyage_route());
    // Drain the arrival dwell shout; advance through the 100 ms dwell → depart.
    let _ = drain(&mut rx);
    advance_ticks(&mut world, 2);
    assert!(
        world.objects.get_component::<Boat>(&boat).unwrap().moving,
        "ferry is sailing"
    );

    // ~200 ms into the voyage the in-transit shout fires (still sailing).
    advance_ticks(&mut world, 3);
    let p = drain(&mut rx);
    assert!(
        boat_announced(&p, 8001),
        "the in-transit arrival shout was broadcast"
    );

    // A voyage shout that arrives after the ferry has docked is dropped.
    if let Some(b) = world.objects.get_component_mut::<Boat>(&boat) {
        b.moving = false;
    }
    boats::handle_voyage_shout(&mut world, boat, 0, 0);
    let p = drain(&mut rx);
    assert!(
        !boat_announced(&p, 8001),
        "no in-transit shout once the ferry has docked"
    );
}
