//! Boat ferry engine (G24.5): a ferry cycles its route, snapping to each
//! waypoint in order.

use super::*;

use crate::model::boat::VehiclePathPoint;
use crate::model::components::Position;

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
    },
    VehiclePathPoint {
        x: 1400,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
    },
    VehiclePathPoint {
        x: 1600,
        y: 1000,
        z: -3600,
        move_speed: 200,
        rotation_speed: 800,
        dock: false,
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
