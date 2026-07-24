//! Boat ferry engine (G24.5): a ferry cycles its route, snapping to each
//! waypoint in order.

use super::*;

use crate::model::boat::{Boat, VehiclePathPoint};
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
