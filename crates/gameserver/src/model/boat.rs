//! Boat (vehicle) model (G24.5) — a scheduled ferry that cycles a fixed route.
//! Java `model/actor/instance/Boat` + `VehiclePathPoint`. The boat is a world
//! object (position + heading) but neither a player nor an NPC, so it carries
//! only this component and does not appear in NPC/player queries.

use bevy_ecs::component::Component;

/// One announcement stage of a harbor dwell (Java boat-script `run()` `case`):
/// broadcast these system-message ids, then wait `then_ms` before the next
/// stage. After the final stage the ferry weighs anchor (its `then_ms` is
/// ignored — Java departs in the same `case` that shouts "leaving now").
#[derive(Debug, Clone, Copy)]
pub struct DwellStage {
    /// SystemMessageId ids to broadcast as `CreatureSay(BOAT, ...)`.
    pub messages: &'static [u32],
    /// Delay before the next stage fires.
    pub then_ms: u64,
}

/// The dwell schedule for a harbor waypoint (a boat script's per-dock cadence):
/// the `CreatureSay` sender id (Java uses 801 for every ferry) and the ordered
/// announcement stages, starting with the "arrived" shout fired on docking.
#[derive(Debug, Clone, Copy)]
pub struct DockSchedule {
    pub char_id: i32,
    pub stages: &'static [DwellStage],
}

/// One waypoint of a route (Java `VehiclePathPoint`): the target position, the
/// move speed used to reach it, and the rotation speed.
#[derive(Debug, Clone, Copy)]
pub struct VehiclePathPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub move_speed: i32,
    pub rotation_speed: i32,
    /// A harbor: the ferry anchors here for a dwell, and only while anchored can
    /// players board or disembark (Java gates both on `!boat.isMoving()`).
    pub dock: bool,
    /// The staged departure announcements for this harbor, if any. Docks without
    /// a schedule dwell silently for a default period.
    pub schedule: Option<&'static DockSchedule>,
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
    /// Whether the boat is under way (Java `Vehicle.isMoving`). Boarding is
    /// only allowed while `false` (anchored at a dock).
    pub moving: bool,
}

/// A player riding a boat (Java `Player._vehicle` + `_inVehiclePosition`): the
/// boat they're on and their seat position **relative to** the boat's origin.
#[derive(Debug, Clone, Copy, Component)]
pub struct InVehicle {
    pub boat_object_id: i32,
    pub seat_x: i32,
    pub seat_y: i32,
    pub seat_z: i32,
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
