//! Boat (vehicle) model (G24.5) — a scheduled ferry that cycles a fixed route.
//! Java `model/actor/instance/Boat` + `VehiclePathPoint`. The boat is a world
//! object (position + heading) but neither a player nor an NPC, so it carries
//! only this component and does not appear in NPC/player queries.

use bevy_ecs::component::Component;

/// One waypoint of a route (Java `VehiclePathPoint`): the target position, the
/// move speed used to reach it, and the rotation speed.
#[derive(Debug, Clone, Copy)]
pub struct VehiclePathPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub move_speed: i32,
    pub rotation_speed: i32,
}

/// A ferry and its progress along a cyclic route.
#[derive(Debug, Clone, Component)]
pub struct Boat {
    /// The whole cycle, legs flattened; the boat loops back to index 0.
    pub route: &'static [VehiclePathPoint],
    /// The waypoint index the boat is currently sailing toward.
    pub leg: usize,
    /// Current facing (updated toward each waypoint).
    pub heading: i32,
}

impl Boat {
    /// The waypoint the boat is heading to.
    pub fn target(&self) -> VehiclePathPoint {
        self.route[self.leg]
    }
    /// Advance to the next waypoint (wrapping the cycle).
    pub fn advance(&mut self) {
        self.leg = (self.leg + 1) % self.route.len();
    }
}
