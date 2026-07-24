//! Boat ferry engine (G24.5) — Java `model/actor/instance/Boat` +
//! `data/scripts/vehicles/*`. Slice 1: a ferry spawns and sails its route,
//! snapping to each waypoint and broadcasting `VehicleDeparture` (a move order
//! the client interpolates) / `VehicleInfo` (its authoritative position).
//!
//! TODO(G24.5): the dock anchor dwell + departure `CreatureSay` announcements
//! (the boat loops continuously here), and boarding/disembark (slice 2).

use crate::model::boat::{Boat, InVehicle, VehiclePathPoint};
use crate::model::components::{Position, RegionCell};
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
use crate::world::{region_of, World};

use super::helpers::broadcast_near_region;

const fn vp(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> VehiclePathPoint {
    VehiclePathPoint {
        x,
        y,
        z,
        move_speed,
        rotation_speed,
        dock: false,
    }
}

/// A harbor waypoint (the ferry anchors here; boarding is allowed).
const fn dk(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> VehiclePathPoint {
    VehiclePathPoint {
        x,
        y,
        z,
        move_speed,
        rotation_speed,
        dock: true,
    }
}

/// How long the ferry anchors at a harbor before departing.
/// TODO(G24.5): Java's is 10 minutes; shortened until the schedule is wired.
const DWELL_MS: u64 = 60_000;

/// The Talking Island ↔ Gludin ferry (Java `BoatTalkingGludin`), all legs
/// flattened into one cycle: Talking → Gludin dock → Gludin → Talking dock.
const TALKING_GLUDIN: &[VehiclePathPoint] = &[
    vp(-121385, 261660, -3610, 180, 800),
    vp(-127694, 253312, -3610, 200, 800),
    vp(-129274, 237060, -3610, 250, 800),
    vp(-114688, 139040, -3610, 200, 800),
    vp(-109663, 135704, -3610, 180, 800),
    vp(-102151, 135704, -3610, 180, 800),
    vp(-96686, 140595, -3610, 180, 800),
    vp(-95686, 147718, -3610, 180, 800),
    vp(-95686, 148718, -3610, 180, 800),
    vp(-95686, 149718, -3610, 150, 800),
    dk(-95686, 150514, -3610, 150, 800), // Gludin dock
    vp(-95686, 155514, -3610, 180, 800),
    vp(-95686, 185514, -3610, 250, 800),
    vp(-60136, 238816, -3610, 200, 800),
    vp(-60520, 259609, -3610, 180, 1800),
    vp(-65344, 261460, -3610, 180, 1800),
    vp(-83344, 261560, -3610, 180, 1800),
    vp(-88344, 261660, -3610, 180, 1800),
    vp(-92344, 261660, -3610, 150, 1800),
    vp(-94242, 261659, -3610, 150, 1800),
    dk(-96622, 261660, -3610, 150, 1800), // Talking dock
];

/// `BoatManager.load` — spawn the ferries at boot and set them sailing.
pub(crate) fn spawn_boats(world: &mut World) {
    spawn_boat(world, TALKING_GLUDIN);
}

/// Spawn one ferry docked at its last waypoint and set it sailing; returns the
/// boat's object id.
pub(crate) fn spawn_boat(world: &mut World, route: &'static [VehiclePathPoint]) -> i32 {
    let oid = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    // Start docked at the last waypoint, sailing toward index 0.
    let start = route[route.len() - 1];
    world.objects.spawn(
        oid,
        (
            Boat {
                route,
                leg: 0,
                heading: 0,
                moving: false,
            },
            Position {
                x: start.x,
                y: start.y,
                z: start.z,
                heading: 0,
            },
            RegionCell(region_of(start.x, start.y)),
        ),
    );
    let info = sp::vehicle_info(oid, start.x, start.y, start.z, 0);
    broadcast_near_region(world, region_of(start.x, start.y), &info);
    // Anchored at a harbor → dwell (boardable); otherwise set sail at once.
    if start.dock {
        schedule_depart(world, oid);
    } else {
        move_to_next(world, oid);
    }
    oid
}

fn schedule_depart(world: &mut World, boat_oid: i32) {
    let fire_at = world.tick + DWELL_MS.div_ceil(100);
    world.scheduler.schedule(
        fire_at,
        ScheduledTask::BoatDepart {
            boat_object_id: boat_oid,
        },
    );
}

/// The `BoatDepart` task: weigh anchor and sail to the next waypoint.
pub(crate) fn handle_depart(world: &mut World, boat_oid: i32) {
    move_to_next(world, boat_oid);
}

/// `Boat.moveToNextRoutePoint`: head for the current waypoint — face it,
/// broadcast the move order, and schedule arrival by travel time.
fn move_to_next(world: &mut World, boat_oid: i32) {
    let Some((target, cur)) = ({
        let boat = world.objects.get_component::<Boat>(&boat_oid);
        let pos = world.objects.get_component::<Position>(&boat_oid).copied();
        boat.map(|b| b.target()).zip(pos)
    }) else {
        return;
    };
    let heading = heading_toward(cur.x, cur.y, target.x, target.y);
    if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
        b.heading = heading;
        b.moving = true;
    }
    if let Some(p) = world.objects.get_component_mut::<Position>(&boat_oid) {
        p.heading = heading;
    }

    let departure = sp::vehicle_departure(
        boat_oid,
        target.move_speed,
        target.rotation_speed,
        target.x,
        target.y,
        target.z,
    );
    broadcast_near_region(world, region_of(cur.x, cur.y), &departure);

    // Travel time: distance / speed (units per second → ms).
    let dist = (((target.x - cur.x) as f64).powi(2) + ((target.y - cur.y) as f64).powi(2)).sqrt();
    let travel_ms = (dist / target.move_speed.max(1) as f64 * 1000.0).max(100.0) as u64;
    let fire_at = world.tick + travel_ms.div_ceil(100);
    world.scheduler.schedule(
        fire_at,
        ScheduledTask::BoatArrive {
            boat_object_id: boat_oid,
        },
    );
}

/// The `BoatArrive` task: snap to the waypoint, broadcast the position, then set
/// sail for the next one.
pub(crate) fn handle_arrive(world: &mut World, boat_oid: i32) {
    let Some(target) = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .map(|b| b.target())
    else {
        return;
    };
    let heading = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .map(|b| b.heading)
        .unwrap_or(0);
    if let Some(p) = world.objects.get_component_mut::<Position>(&boat_oid) {
        p.x = target.x;
        p.y = target.y;
        p.z = target.z;
    }
    if let Some(cell) = world.objects.get_component_mut::<RegionCell>(&boat_oid) {
        cell.0 = region_of(target.x, target.y);
    }
    let info = sp::vehicle_info(boat_oid, target.x, target.y, target.z, heading);
    broadcast_near_region(world, region_of(target.x, target.y), &info);

    // Passengers rode along — snap them to the boat's new position (+ their seat).
    move_passengers(world, boat_oid, target.x, target.y, target.z);

    // Advance the cycle; anchor at harbors, sail on elsewhere.
    if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
        b.advance();
    }
    if target.dock {
        if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
            b.moving = false;
        }
        schedule_depart(world, boat_oid);
    } else {
        move_to_next(world, boat_oid);
    }
}

/// Snap every player riding this boat to `(bx,by,bz)` plus their seat offset.
fn move_passengers(world: &mut World, boat_oid: i32, bx: i32, by: i32, bz: i32) {
    let mut riders: Vec<(i32, (i32, i32, i32))> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &InVehicle)>(|(p, v)| {
            if v.boat_object_id == boat_oid {
                riders.push((p.object_id, (v.seat_x, v.seat_y, v.seat_z)));
            }
        });
    for (player_oid, (sx, sy, sz)) in riders {
        if let Some(p) = world.objects.get_component_mut::<Position>(&player_oid) {
            p.x = bx + sx;
            p.y = by + sy;
            p.z = bz + sz;
        }
    }
}

/// `RequestGetOnVehicle`: board a ferry. Java gates it on the boat existing,
/// being **anchored** (not moving), and the player being within 1000 units;
/// `seat` is the seat position relative to the boat.
pub(crate) fn board(world: &mut World, player: i32, boat_oid: i32, seat: (i32, i32, i32)) {
    // Not already aboard.
    if world.objects.get_component::<InVehicle>(&player).is_some() {
        return;
    }
    let Some((bx, by, bz, moving)) = boat_state(world, boat_oid) else {
        return;
    };
    if moving {
        return;
    }
    let Some(pp) = world.objects.get_component::<Position>(&player).copied() else {
        return;
    };
    if dist3d(pp.x, pp.y, pp.z, bx, by, bz) > 1000.0 {
        return;
    }
    world.objects.add_components(
        &player,
        InVehicle {
            boat_object_id: boat_oid,
            seat_x: seat.0,
            seat_y: seat.1,
            seat_z: seat.2,
        },
    );
    if let Some(p) = world.objects.get_component_mut::<Position>(&player) {
        p.x = bx + seat.0;
        p.y = by + seat.1;
        p.z = bz + seat.2;
    }
    let pkt = sp::get_on_vehicle(player, boat_oid, seat.0, seat.1, seat.2);
    crate::game_loop::helpers::broadcast_including_self(world, player, &pkt);
}

/// `RequestGetOffVehicle`: step off onto the dock at `(x,y,z)`. Only while the
/// boat this player rides is anchored.
pub(crate) fn disembark(world: &mut World, player: i32, boat_oid: i32, exit: (i32, i32, i32)) {
    let Some(iv) = world.objects.get_component::<InVehicle>(&player).copied() else {
        return;
    };
    if iv.boat_object_id != boat_oid {
        return;
    }
    if boat_state(world, boat_oid).is_some_and(|(_, _, _, moving)| moving) {
        return;
    }
    world.objects.remove_component::<InVehicle>(&player);
    if let Some(p) = world.objects.get_component_mut::<Position>(&player) {
        p.x = exit.0;
        p.y = exit.1;
        p.z = exit.2;
    }
    let pkt = sp::get_off_vehicle(player, boat_oid, exit.0, exit.1, exit.2);
    crate::game_loop::helpers::broadcast_including_self(world, player, &pkt);
}

/// A boat's `(x, y, z, moving)`, if the object is a boat.
fn boat_state(world: &World, boat_oid: i32) -> Option<(i32, i32, i32, bool)> {
    let moving = world.objects.get_component::<Boat>(&boat_oid)?.moving;
    let p = world.objects.get_component::<Position>(&boat_oid)?;
    Some((p.x, p.y, p.z, moving))
}

fn dist3d(x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> f64 {
    (((x2 - x1) as f64).powi(2) + ((y2 - y1) as f64).powi(2) + ((z2 - z1) as f64).powi(2)).sqrt()
}

/// L2 heading (`0..65536`) from `(x1,y1)` toward `(x2,y2)`.
fn heading_toward(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let angle = ((y2 - y1) as f64).atan2((x2 - x1) as f64); // -π..π
    let units = angle / std::f64::consts::TAU * 65536.0;
    (units as i32).rem_euclid(65536)
}
