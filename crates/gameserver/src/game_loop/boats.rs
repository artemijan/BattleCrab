//! Boat ferry engine (G24.5) — Java `model/actor/instance/Boat` +
//! `data/scripts/vehicles/*`. A ferry spawns and sails its route, snapping to
//! each waypoint and broadcasting `VehicleDeparture` (a move order the client
//! interpolates) / `VehicleInfo` (its authoritative position). At a dock it
//! anchors for a dwell — a staged schedule (`DockSchedule`) of `CreatureSay`
//! announcements ending in departure — during which players can board/disembark
//! and ride along. All four Interlude ferries (`BoatTalkingGludin`,
//! `BoatGiranTalking`, `BoatInnadrilTour`, `BoatRunePrimeval`) run with their
//! real announced cadences (the three point-to-point routes anchor 10 minutes;
//! Rune↔Primeval anchors 3). On departure each harbor collects the boat-ticket
//! fare (`payForRide`): riders holding the ticket have one consumed, ticketless
//! stowaways are told so and put ashore. Riders can walk around on deck
//! (`MoveToLocationInVehicle`), staying at their relative seat as the boat sails.
//! While under way it shouts the retail "the ferry will arrive in ~N minutes"
//! announcements (`DockSchedule.voyage`, scheduled at departure).
//!
//! SKIP(census): the busy-dock delay messages (`dockBusy`/`BUSY_*`) are
//! omitted — this dist runs one boat per route, so a harbor is never occupied
//! by another vehicle and Java's `dockBusy` branch is unreachable. Revive only
//! if a second boat ever shares a dock.

use crate::enums::ChatType;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::{send_inventory_update, set_position};
use crate::geo::distance::{dist3d_xyz, distance_2d_xy};
use crate::model::boat::{
    Boat, DockSchedule, DwellStage, Fare, InVehicle, RouteDef, RouteId, VehiclePathPoint,
};
use crate::model::components::{Position, RegionCell};
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
use crate::world::{World, region_of};

use super::helpers::broadcast_near_region;
use crate::game_loop::helpers::send_sm_bare_to_player;

fn vp(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> VehiclePathPoint {
    VehiclePathPoint {
        x,
        y,
        z,
        move_speed,
        rotation_speed,
        dock: false,
        schedule: None,
    }
}

/// A harbor waypoint with a staged departure-announcement schedule, by index
/// into the route's `schedules`. (Every harbor on the four Interlude ferries
/// has one; a dock without a schedule would fall back to a silent `DWELL_MS`
/// dwell.)
fn dka(
    x: i32,
    y: i32,
    z: i32,
    move_speed: i32,
    rotation_speed: i32,
    schedule: u16,
) -> VehiclePathPoint {
    VehiclePathPoint {
        x,
        y,
        z,
        move_speed,
        rotation_speed,
        dock: true,
        schedule: Some(schedule),
    }
}

/// Fallback dwell for a harbor with no announcement schedule. Every Interlude
/// ferry dock has one, so this only applies to synthetic/test routes.
const DWELL_MS: u64 = 60_000;

/// The `CreatureSay` sender id every ferry uses for its announcements (Java
/// passes `801` to every `new CreatureSay(ChatType.BOAT, 801, …)`).
const BOAT_CHAR_ID: i32 = 801;

/// One dwell stage: shout `messages`, then wait `then_ms`.
fn stage(messages: &[u32], then_ms: u64) -> DwellStage {
    DwellStage {
        messages: messages.to_vec(),
        then_ms,
    }
}

/// A literal shout table in owned `DockSchedule.voyage` form.
fn shouts(list: &[(u64, &[u32])]) -> Vec<(u64, Vec<u32>)> {
    list.iter()
        .map(|&(delay, ids)| (delay, ids.to_vec()))
        .collect()
}

/// The standard 10-minute harbor dwell shared by the Talking/Gludin/Giran
/// ferries (Java `case` cadence 5 min → 4 min → 40 s → 20 s): the "arrived"
/// lines, then the 5-minute / 1-minute / "leaving soon" warnings, then the
/// "leaving now" shout after which the ferry departs.
fn ten_minute_dwell(
    fare: Fare,
    voyage: &[(u64, &[u32])],
    arrival: &[u32],
    leave_5min: &[u32],
    leave_1min: &[u32],
    leaving_soon: &[u32],
    leaving_now: &[u32],
) -> DockSchedule {
    DockSchedule {
        char_id: BOAT_CHAR_ID,
        fare,
        voyage: shouts(voyage),
        stages: vec![
            stage(arrival, 300_000),
            stage(leave_5min, 240_000),
            stage(leave_1min, 40_000),
            stage(leaving_soon, 20_000),
            stage(leaving_now, 0),
        ],
    }
}

/// The 3-minute dwell of the Rune ↔ Primeval ferry (Java `BoatRunePrimeval`):
/// the "arrived" lines, then after 3 minutes the "now departing" shout + depart.
fn three_minute_dwell(fare: Fare, arrival: &[u32], leaving: &[u32]) -> DockSchedule {
    DockSchedule {
        char_id: BOAT_CHAR_ID,
        fare,
        voyage: Vec::new(), // Rune↔Primeval makes no in-transit announcements
        stages: vec![stage(arrival, 180_000), stage(leaving, 0)],
    }
}

/// A boat-ticket fare (`payForRide(itemId, 1, oustX, oustY, oustZ)`).
const fn fare(ticket_item_id: i32, oust_x: i32, oust_y: i32, oust_z: i32) -> Fare {
    Fare {
        ticket_item_id,
        oust_x,
        oust_y,
        oust_z,
    }
}

// In-transit "arriving in ~N minutes" shouts, `(delay_from_departure, [ids])`.
const V_TALKING_TO_GLUDIN: &[(u64, &[u32])] =
    &[(300_000, &[1159]), (600_000, &[1160]), (840_000, &[1161])];
const V_GLUDIN_TO_TALKING: &[(u64, &[u32])] =
    &[(150_000, &[1191]), (450_000, &[1192]), (690_000, &[1193])];
const V_GIRAN_TO_TALKING: &[(u64, &[u32])] = &[
    (0, &[1162]),
    (250_000, &[1163]),
    (550_000, &[1164]),
    (790_000, &[1165]),
];
const V_TALKING_TO_GIRAN: &[(u64, &[u32])] = &[
    (200_000, &[1166]),
    (500_000, &[1167]),
    (800_000, &[1168]),
    (1_100_000, &[1169]),
    (1_340_000, &[1170]),
];
const V_INNADRIL_TOUR: &[(u64, &[u32])] = &[
    (650_000, &[1171]),
    (950_000, &[1172]),
    (1_250_000, &[1173]),
    (1_550_000, &[1174]),
    (1_790_000, &[1175]),
];

/// The Talking Island ↔ Gludin ferry (Java `BoatTalkingGludin`), all legs
/// flattened into one cycle: Talking → Gludin dock → Gludin → Talking dock.
/// 983 = "make haste to board".
fn talking_gludin() -> RouteDef {
    let gludin = ten_minute_dwell(
        fare(1075, -90015, 150422, -3610),
        V_GLUDIN_TO_TALKING,
        &[986, 987],
        &[988],
        &[989, 983],
        &[990],
        &[991],
    ); // leaves for Talking
    let talking = ten_minute_dwell(
        fare(1074, -96777, 258970, -3623),
        V_TALKING_TO_GLUDIN,
        &[979, 980],
        &[981],
        &[982, 983],
        &[984],
        &[985],
    ); // leaves for Gludin
    RouteDef {
        waypoints: vec![
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
            dka(-95686, 150514, -3610, 150, 800, 0), // Gludin dock
            vp(-95686, 155514, -3610, 180, 800),
            vp(-95686, 185514, -3610, 250, 800),
            vp(-60136, 238816, -3610, 200, 800),
            vp(-60520, 259609, -3610, 180, 1800),
            vp(-65344, 261460, -3610, 180, 1800),
            vp(-83344, 261560, -3610, 180, 1800),
            vp(-88344, 261660, -3610, 180, 1800),
            vp(-92344, 261660, -3610, 150, 1800),
            vp(-94242, 261659, -3610, 150, 1800),
            dka(-96622, 261660, -3610, 150, 1800, 1), // Talking dock
        ],
        schedules: vec![gludin, talking],
    }
}

/// Giran <-> Talking Island ferry (`BoatGiranTalking`) — no "make haste" line.
fn giran_talking() -> RouteDef {
    let talking = ten_minute_dwell(
        fare(3945, -96777, 258970, -3623),
        V_TALKING_TO_GIRAN,
        &[979, 993],
        &[994],
        &[995],
        &[996],
        &[997],
    ); // leaves for Giran
    let giran = ten_minute_dwell(
        fare(3946, 46763, 187041, -3451),
        V_GIRAN_TO_TALKING,
        &[992, 987],
        &[988],
        &[989],
        &[990],
        &[991],
    ); // leaves for Talking
    RouteDef {
        waypoints: vec![
            vp(51914, 189023, -3610, 150, 800),
            vp(60567, 189789, -3610, 150, 800),
            vp(63732, 197457, -3610, 200, 800),
            vp(63732, 219946, -3610, 250, 800),
            vp(62008, 222240, -3610, 250, 1200),
            vp(56115, 226791, -3610, 250, 1200),
            vp(40384, 226432, -3610, 300, 800),
            vp(37760, 226432, -3610, 300, 800),
            vp(27153, 226791, -3610, 300, 800),
            vp(12672, 227535, -3610, 300, 800),
            vp(-1808, 228280, -3610, 300, 800),
            vp(-22165, 230542, -3610, 300, 800),
            vp(-42523, 235205, -3610, 300, 800),
            vp(-68451, 259560, -3610, 250, 800),
            vp(-70848, 261696, -3610, 200, 800),
            vp(-83344, 261610, -3610, 200, 800),
            vp(-88344, 261660, -3610, 180, 800),
            vp(-92344, 261660, -3610, 180, 800),
            vp(-94242, 261659, -3610, 150, 800),
            dka(-96622, 261660, -3610, 150, 800, 0), // Talking dock (→ Giran)
            vp(-113925, 261660, -3610, 150, 800),
            vp(-126107, 249116, -3610, 180, 800),
            vp(-126107, 234499, -3610, 180, 800),
            vp(-126107, 219882, -3610, 180, 800),
            vp(-109414, 204914, -3610, 180, 800),
            vp(-92807, 204914, -3610, 180, 800),
            vp(-80425, 216450, -3610, 250, 800),
            vp(-68043, 227987, -3610, 250, 800),
            vp(-63744, 231168, -3610, 250, 800),
            vp(-60844, 231369, -3610, 250, 1800),
            vp(-44915, 231369, -3610, 200, 800),
            vp(-28986, 231369, -3610, 200, 800),
            vp(8233, 207624, -3610, 200, 800),
            vp(21470, 201503, -3610, 180, 800),
            vp(40058, 195383, -3610, 180, 800),
            vp(43022, 193793, -3610, 150, 800),
            vp(45986, 192203, -3610, 150, 800),
            dka(48950, 190613, -3610, 150, 800, 1), // Giran dock (→ Talking)
        ],
        schedules: vec![talking, giran],
    }
}

/// Innadril scenic tour (`BoatInnadrilTour`) — a single-harbor loop, free
/// passage (ticket id 0).
fn innadril_tour() -> RouteDef {
    let innadril = ten_minute_dwell(
        fare(0, 107092, 219098, -3952),
        V_INNADRIL_TOUR,
        &[998],
        &[999],
        &[1000],
        &[1001],
        &[1002],
    );
    RouteDef {
        waypoints: vec![
            vp(105129, 226240, -3610, 150, 800),
            vp(90604, 238797, -3610, 150, 800),
            vp(74853, 237943, -3610, 150, 800),
            vp(68207, 235399, -3610, 150, 800),
            vp(63226, 230487, -3610, 150, 800),
            vp(61843, 224797, -3610, 150, 800),
            vp(61822, 203066, -3610, 150, 800),
            vp(59051, 197685, -3610, 150, 800),
            vp(54048, 195298, -3610, 150, 800),
            vp(41609, 195687, -3610, 150, 800),
            vp(35821, 200284, -3610, 150, 800),
            vp(35567, 205265, -3610, 150, 800),
            vp(35617, 222471, -3610, 150, 800),
            vp(37932, 226588, -3610, 150, 800),
            vp(42932, 229394, -3610, 150, 800),
            vp(74324, 245231, -3610, 150, 800),
            vp(81872, 250314, -3610, 150, 800),
            vp(101692, 249882, -3610, 150, 800),
            vp(107907, 256073, -3610, 150, 800),
            vp(112317, 257133, -3610, 150, 800),
            vp(126273, 255313, -3610, 150, 800),
            vp(128067, 250961, -3610, 150, 800),
            vp(128520, 238249, -3610, 150, 800),
            vp(126428, 235072, -3610, 150, 800),
            vp(121843, 234656, -3610, 150, 800),
            vp(120096, 234268, -3610, 150, 800),
            vp(118572, 233046, -3610, 150, 800),
            vp(117671, 228951, -3610, 150, 800),
            vp(115936, 226540, -3610, 150, 800),
            vp(113628, 226240, -3610, 150, 800),
            vp(111300, 226240, -3610, 150, 800),
            dka(111264, 226240, -3610, 150, 800, 0), // Innadril dock
        ],
        schedules: vec![innadril],
    }
}

/// Rune <-> Primeval Isle ferry (`BoatRunePrimeval`). 1620 = "Welcome to Rune
/// Harbor".
fn rune_primeval() -> RouteDef {
    let primeval = three_minute_dwell(fare(8924, 10447, -24982, -3664), &[1988, 1991], &[1990]); // leaves for Rune
    let rune = three_minute_dwell(fare(8925, 34513, -38009, -3640), &[1620, 1989], &[1992]); // leaves for Primeval
    RouteDef {
        waypoints: vec![
            vp(32750, -39300, -3610, 180, 800),
            vp(27440, -39328, -3610, 250, 1000),
            vp(19616, -39360, -3610, 270, 1000),
            vp(3840, -38528, -3610, 270, 1000),
            vp(1664, -37120, -3610, 270, 1000),
            vp(896, -34560, -3610, 180, 1800),
            vp(832, -31104, -3610, 180, 180),
            vp(2240, -29132, -3610, 150, 1800),
            vp(4160, -27828, -3610, 150, 1800),
            vp(5888, -27279, -3610, 150, 1800),
            vp(7000, -27279, -3610, 150, 1800),
            dka(10342, -27279, -3610, 150, 1800, 0), // Primeval dock (→ Rune)
            vp(15528, -27279, -3610, 180, 800),
            vp(22304, -29664, -3610, 290, 800),
            vp(33824, -26880, -3610, 290, 800),
            vp(38848, -21792, -3610, 240, 1200),
            vp(43424, -22080, -3610, 180, 1800),
            vp(44320, -25152, -3610, 180, 1800),
            vp(40576, -31616, -3610, 250, 800),
            vp(36819, -35315, -3610, 220, 800),
            dka(34381, -37680, -3610, 220, 800, 1), // Rune dock (→ Primeval)
        ],
        schedules: vec![primeval, rune],
    }
}

/// `BoatManager.load` — spawn the four Interlude ferries at boot and set them
/// on their routes.
pub(crate) fn spawn_boats(world: &mut World) {
    // `BoatManager.load()` opens with `if (!Config.ALLOW_BOAT) return;` — the
    // docks are never registered, so with it off no boat exists at all rather
    // than existing and standing still.
    if !world.cfg.general.allow_boat {
        tracing::info!("BoatManager: disabled by AllowBoat.");
        return;
    }
    for def in [
        talking_gludin(),
        giran_talking(),
        innadril_tour(),
        rune_primeval(),
    ] {
        let route = world.boat_routes.register(def);
        spawn_boat(world, route);
    }
}

/// Spawn one ferry docked at its last waypoint and set it sailing; returns the
/// boat's object id.
pub(crate) fn spawn_boat(world: &mut World, route: RouteId) -> i32 {
    let oid = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    // Start at the last waypoint. If it's a harbor, anchor there (leg points at
    // that dock so its dwell schedule resolves) and depart toward index 0;
    // otherwise the boat is mid-route and heads for index 0 at once.
    let waypoints = &world.boat_routes.get(route).waypoints;
    let last = waypoints.len() - 1;
    let start = waypoints[last];
    let leg = if start.dock { last } else { 0 };
    world.objects.spawn(
        oid,
        (
            Boat {
                route,
                leg,
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
        begin_dwell(world, oid);
    } else {
        move_to_next(world, oid);
    }
    oid
}

/// The waypoint the ferry is currently sailing toward (or anchored at).
fn target_of(world: &World, boat_oid: i32) -> Option<VehiclePathPoint> {
    let b = world.objects.get_component::<Boat>(&boat_oid)?;
    Some(b.target(world.boat_routes.get(b.route)))
}

/// Advance the ferry to the next waypoint of its cycle.
fn advance_boat(world: &mut World, boat_oid: i32) {
    if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
        b.advance(world.boat_routes.get(b.route));
    }
}

/// Begin the harbor dwell: run the announcement schedule if the anchored dock
/// has one, otherwise dwell silently for the default period.
fn begin_dwell(world: &mut World, boat_oid: i32) {
    let has_schedule = target_of(world, boat_oid).is_some_and(|wp| wp.schedule.is_some());
    if has_schedule {
        run_dwell_stage(world, boat_oid, 0);
    } else {
        schedule_depart(world, boat_oid);
    }
}

/// The next harbor the ferry sails to after the one it's anchored at — its
/// coordinates, so departure shouts also reach players waiting there (Java
/// `broadcastPacket(fromDock, toDock, ...)`). A single-harbor tour returns
/// its own dock.
fn next_dock(world: &World, boat_oid: i32) -> Option<(i32, i32)> {
    let boat = world.objects.get_component::<Boat>(&boat_oid)?;
    let waypoints = &world.boat_routes.get(boat.route).waypoints;
    let n = waypoints.len();
    (1..=n)
        .map(|i| waypoints[(boat.leg + i) % n])
        .find(|wp| wp.dock)
        .map(|wp| (wp.x, wp.y))
}

/// One stage of a harbor's dwell schedule: broadcast its announcements (to this
/// harbor and the destination harbor), then schedule the next stage — or depart
/// after the last one. Driven by the `BoatDwellStage` scheduler task.
pub(crate) fn run_dwell_stage(world: &mut World, boat_oid: i32, stage_idx: usize) {
    let Some(boat) = world.objects.get_component::<Boat>(&boat_oid) else {
        return;
    };
    let route = world.boat_routes.get(boat.route);
    let dock = boat.target(route);
    let Some(sched) = route.schedule_of(&dock) else {
        return;
    };
    let Some(stage) = sched.stages.get(stage_idx) else {
        return;
    };

    let here = region_of(dock.x, dock.y);
    let there = next_dock(world, boat_oid).map(|(x, y)| region_of(x, y));
    for &mid in &stage.messages {
        let say = sp::creature_say_system(ChatType::Boat, sched.char_id, mid as i32);
        broadcast_near_region(world, here, &say);
        if let Some(there) = there
            && there != here
        {
            broadcast_near_region(world, there, &say);
        }
    }
    let then_ms = stage.then_ms;
    let is_last = stage_idx + 1 >= sched.stages.len();

    if !is_last {
        let fire_at = world.tick + then_ms.div_ceil(100);
        world.scheduler.schedule(
            fire_at,
            ScheduledTask::BoatDwellStage {
                boat_object_id: boat_oid,
                stage: stage_idx + 1,
            },
        );
    } else {
        depart(world, boat_oid);
    }
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

/// Weigh anchor: collect the fare from everyone aboard, then advance to the
/// next leg of the cycle and set sail. The fare is charged while still docked
/// (before `move_to_next` sets `moving`), matching Java's `payForRide` in the
/// same `case` that departs. Driven by the last dwell stage, or by the
/// `BoatDepart` task when the harbor has no announcements to make.
pub(crate) fn depart(world: &mut World, boat_oid: i32) {
    // Capture the departing dock's fare and voyage shout delays (by schedule
    // index, for the scheduler tasks) before advancing off it.
    let dock_info: Option<(u16, Fare, Vec<u64>)> = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .and_then(|b| {
            let route = world.boat_routes.get(b.route);
            b.target(route).schedule.map(|si| {
                let s = &route.schedules[si as usize];
                (
                    si,
                    s.fare,
                    s.voyage.iter().map(|&(delay, _)| delay).collect(),
                )
            })
        });
    if let Some((_, fare, _)) = &dock_info {
        pay_for_ride(world, boat_oid, *fare);
    }
    advance_boat(world, boat_oid);
    move_to_next(world, boat_oid);
    // Schedule the in-transit "arriving in ~N minutes" shouts for this leg.
    if let Some((schedule, _, delays)) = dock_info {
        for (shout, delay_ms) in delays.into_iter().enumerate() {
            let fire_at = world.tick + delay_ms.div_ceil(100);
            world.scheduler.schedule(
                fire_at,
                ScheduledTask::BoatVoyageShout {
                    boat_object_id: boat_oid,
                    schedule,
                    shout: shout as u16,
                },
            );
        }
    }
}

/// The `BoatVoyageShout` task: an in-transit arrival announcement, resolved by
/// schedule/shout index into the boat's route. Skipped if the ferry has already
/// docked (a late shout from the previous leg).
pub(crate) fn handle_voyage_shout(world: &mut World, boat_oid: i32, schedule: u16, shout: u16) {
    let Some(boat) = world.objects.get_component::<Boat>(&boat_oid) else {
        return;
    };
    if !boat.moving {
        return;
    }
    let route = world.boat_routes.get(boat.route);
    let Some(sched) = route.schedules.get(schedule as usize) else {
        return;
    };
    let Some((_, messages)) = sched.voyage.get(shout as usize) else {
        return;
    };
    for &mid in messages {
        let say = sp::creature_say_system(ChatType::Boat, BOAT_CHAR_ID, mid as i32);
        broadcast_to_route_docks(world, boat_oid, &say);
    }
}

/// Broadcast to the region around every harbor on the ferry's route (Java sends
/// arrival announcements to both the source and destination docks).
fn broadcast_to_route_docks(world: &World, boat_oid: i32, packet: &[u8]) {
    let Some(boat) = world.objects.get_component::<Boat>(&boat_oid) else {
        return;
    };
    let mut regions: Vec<(i32, i32)> = Vec::new();
    let waypoints = &world.boat_routes.get(boat.route).waypoints;
    for wp in waypoints.iter().filter(|w| w.dock) {
        let r = region_of(wp.x, wp.y);
        if !regions.contains(&r) {
            regions.push(r);
        }
    }
    for r in regions {
        broadcast_near_region(world, r, packet);
    }
}

/// `Vehicle.payForRide`: charge each rider the boat ticket as the ferry leaves.
/// A rider who holds the ticket has one consumed (free for Innadril, ticket id
/// 0); one who does not is told so and put ashore at the harbor.
fn pay_for_ride(world: &mut World, boat_oid: i32, fare: Fare) {
    // The ticket item id 0 means free passage — nobody is charged or ousted.
    if fare.ticket_item_id == 0 {
        return;
    }
    let riders: Vec<i32> = collect_riders(world, boat_oid);
    for player in riders {
        let changes = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&player)
            .map(|inv| inv.remove_item(fare.ticket_item_id, 1))
            .unwrap_or_default();
        if changes.is_empty() {
            // No ticket: Java sends the message and teleports them off.
            send_sm_bare_to_player(
                world,
                player,
                sp::sm_ids::YOU_DO_NOT_POSSESS_THE_CORRECT_TICKET,
            );
            oust_rider(world, player, boat_oid, fare);
        } else {
            send_inventory_update(world, player, changes);
        }
    }
}

/// Every player currently riding `boat_oid`.
fn collect_riders(world: &mut World, boat_oid: i32) -> Vec<i32> {
    let mut riders = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &InVehicle)>(|(p, v)| {
            if v.boat_object_id == boat_oid {
                riders.push(p.object_id);
            }
        });
    riders
}

/// Put a ticketless rider ashore: drop them from the boat and teleport them to
/// the harbor `oust` location.
fn oust_rider(world: &mut World, player: i32, boat_oid: i32, fare: Fare) {
    world.objects.remove_component::<InVehicle>(&player);
    let off = sp::get_off_vehicle(player, boat_oid, fare.oust_x, fare.oust_y, fare.oust_z);
    crate::game_loop::helpers::broadcast_including_self(world, player, &off);
    crate::game_loop::death::teleport_player(world, player, fare.oust_x, fare.oust_y, fare.oust_z);
}

/// `Boat.moveToNextRoutePoint`: head for the current waypoint — face it,
/// broadcast the move order, and schedule arrival by travel time.
fn move_to_next(world: &mut World, boat_oid: i32) {
    let Some((target, cur)) = target_of(world, boat_oid).zip(maybe_position(world, boat_oid))
    else {
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
    let dist = distance_2d_xy(target.x, target.y, cur.x, cur.y);
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
    let Some(target) = target_of(world, boat_oid) else {
        return;
    };
    let heading = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .map(|b| b.heading)
        .unwrap_or(0);
    set_position(world, boat_oid, (target.x, target.y, target.z));
    if let Some(cell) = world.objects.get_component_mut::<RegionCell>(&boat_oid) {
        cell.0 = region_of(target.x, target.y);
    }
    let info = sp::vehicle_info(boat_oid, target.x, target.y, target.z, heading);
    broadcast_near_region(world, region_of(target.x, target.y), &info);

    // Passengers rode along — snap them to the boat's new position (+ their seat).
    move_passengers(world, boat_oid, target.x, target.y, target.z);

    // At a harbor: anchor (leg stays on the dock so its dwell schedule resolves)
    // and begin the dwell. Elsewhere: advance the cycle and sail on.
    if target.dock {
        if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
            b.moving = false;
        }
        begin_dwell(world, boat_oid);
    } else {
        advance_boat(world, boat_oid);
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
        set_position(world, player_oid, (bx + sx, by + sy, bz + sz));
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
    let Some(pp) = maybe_position(world, player) else {
        return;
    };
    if dist3d_xyz(pp.x, pp.y, pp.z, bx, by, bz) > 1000.0 {
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
    set_position(world, player, (bx + seat.0, by + seat.1, bz + seat.2));
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
    set_position(world, player, (exit.0, exit.1, exit.2));
    let pkt = sp::get_off_vehicle(player, boat_oid, exit.0, exit.1, exit.2);
    crate::game_loop::helpers::broadcast_including_self(world, player, &pkt);
}

/// `RequestMoveToLocationInVehicle` → `Player.setInVehiclePosition` +
/// `MoveToLocationInVehicle`: the player walks around on the boat's deck.
/// `dest`/`origin` are positions relative to the boat. A no-op move (dest ==
/// origin) just stops them; otherwise their seat is updated to `dest` so they
/// ride at the new spot, and the walk is broadcast for everyone to see.
pub(crate) fn move_in_vehicle(
    world: &mut World,
    player: i32,
    boat_oid: i32,
    dest: (i32, i32, i32),
    origin: (i32, i32, i32),
) {
    // Must actually be aboard this boat (Java resolves/repairs the vehicle ref;
    // we require the rider to already hold the matching InVehicle).
    let Some(iv) = world.objects.get_component::<InVehicle>(&player).copied() else {
        return;
    };
    if iv.boat_object_id != boat_oid {
        return;
    }

    // A zero-length move is the client asking to stop where it is.
    if dest == origin {
        let heading = world
            .objects
            .get_component::<Position>(&player)
            .map(|p| p.heading)
            .unwrap_or(0);
        let stop = sp::stop_move_in_vehicle(player, boat_oid, dest.0, dest.1, dest.2, heading);
        crate::game_loop::helpers::send_to_player(world, player, stop);
        return;
    }

    // Update the rider's seat so `move_passengers` keeps them at the new spot,
    // and snap their absolute position to it now.
    if let Some(v) = world.objects.get_component_mut::<InVehicle>(&player) {
        v.seat_x = dest.0;
        v.seat_y = dest.1;
        v.seat_z = dest.2;
    }
    if let Some((bx, by, bz, _)) = boat_state(world, boat_oid)
        && let Some(p) = world.objects.get_component_mut::<Position>(&player)
    {
        p.x = bx + dest.0;
        p.y = by + dest.1;
        p.z = bz + dest.2;
    }
    let mov = sp::move_to_location_in_vehicle(player, boat_oid, dest, origin);
    crate::game_loop::helpers::broadcast_including_self(world, player, &mov);
}

/// A boat's `(x, y, z, moving)`, if the object is a boat.
fn boat_state(world: &World, boat_oid: i32) -> Option<(i32, i32, i32, bool)> {
    let moving = world.objects.get_component::<Boat>(&boat_oid)?.moving;
    let p = world.objects.get_component::<Position>(&boat_oid)?;
    Some((p.x, p.y, p.z, moving))
}

/// L2 heading (`0..65536`) from `(x1,y1)` toward `(x2,y2)`.
fn heading_toward(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let angle = ((y2 - y1) as f64).atan2((x2 - x1) as f64); // -π..π
    let units = angle / std::f64::consts::TAU * 65536.0;
    (units as i32).rem_euclid(65536)
}

// The packet readers, moved out of dispatch.rs to live with the flows they feed.
/// `RequestGetOnVehicle` / `RequestGetOffVehicle`: read `boatId + (x, y, z)` and
/// board / disembark for the in-game player (G24.5).
pub(crate) fn handle_get_on_off_vehicle(
    world: &mut World,
    client_id: u32,
    body: &[u8],
    boarding: bool,
) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let Some([boat_oid, x, y, z]) = r.read_i32_array::<4>() else {
        return;
    };
    if boarding {
        board(world, player, boat_oid, (x, y, z));
    } else {
        disembark(world, player, boat_oid, (x, y, z));
    }
}

/// `RequestMoveToLocationInVehicle`: the player walks around on a ferry's deck.
/// Reads boatId + target (x,y,z) + origin (x,y,z), all relative to the boat.
pub(crate) fn handle_move_in_vehicle(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let (Some(boat_oid), Some(tx), Some(ty), Some(tz), Some(ox), Some(oy), Some(oz)) = (
        r.read_i32(),
        r.read_i32(),
        r.read_i32(),
        r.read_i32(),
        r.read_i32(),
        r.read_i32(),
        r.read_i32(),
    ) else {
        return;
    };
    move_in_vehicle(world, player, boat_oid, (tx, ty, tz), (ox, oy, oz));
}
