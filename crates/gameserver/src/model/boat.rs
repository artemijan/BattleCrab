//! Boat (vehicle) model (G24.5) — a scheduled ferry that cycles a fixed route.
//! Java `model/actor/instance/Boat` + `VehiclePathPoint`. The boat is a world
//! object (position + heading) but neither a player nor an NPC, so it carries
//! only this component and does not appear in NPC/player queries.
//!
//! Route data (waypoints + dock schedules) is owned by the [`BoatRoutes`]
//! registry on `World`; the [`Boat`] component and boat scheduler tasks refer
//! to it by [`RouteId`]/index, so components stay plain `Copy` data and routes
//! can be built at runtime (boot, tests, or a future data loader).

use bevy_ecs::component::Component;

/// One announcement stage of a harbor dwell (Java boat-script `run()` `case`):
/// broadcast these system-message ids, then wait `then_ms` before the next
/// stage. After the final stage the ferry weighs anchor (its `then_ms` is
/// ignored — Java departs in the same `case` that shouts "leaving now").
#[derive(Debug, Clone)]
pub struct DwellStage {
    /// SystemMessageId ids to broadcast as `CreatureSay(BOAT, ...)`.
    pub messages: Vec<u32>,
    /// Delay before the next stage fires.
    pub then_ms: u64,
}

/// The fare collected when the ferry departs a harbor (Java `Vehicle.payForRide`):
/// the boat-ticket item id and where a rider without one is put ashore.
#[derive(Debug, Clone, Copy)]
pub struct Fare {
    /// The ticket item consumed on departure, or 0 for free passage (Innadril).
    pub ticket_item_id: i32,
    /// Where a ticketless rider is teleported (the `oust` args in the scripts).
    pub oust_x: i32,
    pub oust_y: i32,
    pub oust_z: i32,
}

/// The dwell schedule for a harbor waypoint (a boat script's per-dock cadence):
/// the `CreatureSay` sender id (Java uses 801 for every ferry), the ordered
/// announcement stages starting with the "arrived" shout fired on docking, and
/// the fare collected when the ferry finally departs.
#[derive(Debug, Clone)]
pub struct DockSchedule {
    pub char_id: i32,
    pub stages: Vec<DwellStage>,
    pub fare: Fare,
    /// In-transit "the ferry will arrive in ~N minutes" shouts fired after
    /// leaving this harbor: `(delay_ms_from_departure, message_ids)`. Empty for
    /// ferries that make no such announcement (Rune↔Primeval).
    pub voyage: Vec<(u64, Vec<u32>)>,
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
    /// Index into [`RouteDef::schedules`] of this harbor's staged departure
    /// announcements, if any. Docks without a schedule dwell silently for a
    /// default period.
    pub schedule: Option<u16>,
}

/// A ferry route: the whole cycle of waypoints (legs flattened; the boat loops
/// back to index 0) plus the dock schedules its harbor waypoints refer to.
#[derive(Debug, Clone)]
pub struct RouteDef {
    pub waypoints: Vec<VehiclePathPoint>,
    pub schedules: Vec<DockSchedule>,
}

impl RouteDef {
    /// The dwell schedule of a harbor waypoint, if it has one.
    pub fn schedule_of(&self, wp: &VehiclePathPoint) -> Option<&DockSchedule> {
        wp.schedule.map(|i| &self.schedules[i as usize])
    }
}

/// Handle to a route registered in [`BoatRoutes`] — what [`Boat`] components
/// and boat scheduler tasks store instead of borrowing the route data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteId(u32);

/// The ferry route registry (`World::boat_routes`): routes are registered once
/// (at boot by `spawn_boats`, or per-test for synthetic routes) and never
/// removed, so a `RouteId` stays valid for the life of the world.
#[derive(Debug, Default)]
pub struct BoatRoutes(Vec<RouteDef>);

impl BoatRoutes {
    pub fn register(&mut self, def: RouteDef) -> RouteId {
        self.0.push(def);
        RouteId(self.0.len() as u32 - 1)
    }

    pub fn get(&self, id: RouteId) -> &RouteDef {
        &self.0[id.0 as usize]
    }
}

/// A ferry and its progress along a cyclic route.
#[derive(Debug, Clone, Copy, Component)]
pub struct Boat {
    /// The route this ferry cycles, in `World::boat_routes`.
    pub route: RouteId,
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
    /// The waypoint the boat is heading to (`route` must be this boat's route).
    pub fn target(&self, route: &RouteDef) -> VehiclePathPoint {
        route.waypoints[self.leg]
    }
    /// Advance to the next waypoint (wrapping the cycle).
    pub fn advance(&mut self, route: &RouteDef) {
        self.leg = (self.leg + 1) % route.waypoints.len();
    }
}
