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
use crate::geo::distance::{dist3d_xyz, distance_2d_xy};
use crate::model::boat::{Boat, DockSchedule, DwellStage, Fare, InVehicle, VehiclePathPoint};
use crate::model::components::{Position, RegionCell};
use crate::network::server_packets as sp;
use crate::scheduler::ScheduledTask;
use crate::world::{World, region_of};

use super::helpers::broadcast_near_region;

const fn vp(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> VehiclePathPoint {
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

/// A harbor waypoint with a staged departure-announcement schedule. (Every
/// harbor on the four Interlude ferries has one; a dock without a schedule
/// would fall back to a silent `DWELL_MS` dwell.)
const fn dka(
    x: i32,
    y: i32,
    z: i32,
    move_speed: i32,
    rotation_speed: i32,
    schedule: &'static DockSchedule,
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

/// The standard 10-minute harbor dwell shared by the Talking/Gludin/Giran
/// ferries (Java `case` cadence 5 min → 4 min → 40 s → 20 s): the "arrived"
/// lines, then the 5-minute / 1-minute / "leaving soon" warnings, then the
/// "leaving now" shout after which the ferry departs.
macro_rules! ten_minute_dwell {
    ($fare:expr, $voyage:expr, $arrival:expr, $leave_5min:expr, $leave_1min:expr, $leaving_soon:expr, $leaving_now:expr) => {
        DockSchedule {
            char_id: 801,
            fare: $fare,
            voyage: $voyage,
            stages: &[
                DwellStage {
                    messages: $arrival,
                    then_ms: 300_000,
                },
                DwellStage {
                    messages: $leave_5min,
                    then_ms: 240_000,
                },
                DwellStage {
                    messages: $leave_1min,
                    then_ms: 40_000,
                },
                DwellStage {
                    messages: $leaving_soon,
                    then_ms: 20_000,
                },
                DwellStage {
                    messages: $leaving_now,
                    then_ms: 0,
                },
            ],
        }
    };
}

/// The 3-minute dwell of the Rune ↔ Primeval ferry (Java `BoatRunePrimeval`):
/// the "arrived" lines, then after 3 minutes the "now departing" shout + depart.
macro_rules! three_minute_dwell {
    ($fare:expr, $arrival:expr, $leaving:expr) => {
        DockSchedule {
            char_id: 801,
            fare: $fare,
            voyage: &[], // Rune↔Primeval makes no in-transit announcements
            stages: &[
                DwellStage {
                    messages: $arrival,
                    then_ms: 180_000,
                },
                DwellStage {
                    messages: $leaving,
                    then_ms: 0,
                },
            ],
        }
    };
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

// Talking Island ↔ Gludin (`BoatTalkingGludin`). 983 = "make haste to board".
static TALKING_DOCK_SCHED: DockSchedule = ten_minute_dwell!(
    fare(1074, -96777, 258970, -3623),
    V_TALKING_TO_GLUDIN,
    &[979, 980],
    &[981],
    &[982, 983],
    &[984],
    &[985]
); // leaves for Gludin
static GLUDIN_DOCK_SCHED: DockSchedule = ten_minute_dwell!(
    fare(1075, -90015, 150422, -3610),
    V_GLUDIN_TO_TALKING,
    &[986, 987],
    &[988],
    &[989, 983],
    &[990],
    &[991]
); // leaves for Talking

// Giran ↔ Talking Island (`BoatGiranTalking`) — no "make haste" line.
static GT_GIRAN_DOCK_SCHED: DockSchedule = ten_minute_dwell!(
    fare(3946, 46763, 187041, -3451),
    V_GIRAN_TO_TALKING,
    &[992, 987],
    &[988],
    &[989],
    &[990],
    &[991]
); // leaves for Talking
static GT_TALKING_DOCK_SCHED: DockSchedule = ten_minute_dwell!(
    fare(3945, -96777, 258970, -3623),
    V_TALKING_TO_GIRAN,
    &[979, 993],
    &[994],
    &[995],
    &[996],
    &[997]
); // leaves for Giran

// Innadril pleasure boat (`BoatInnadrilTour`) — a single-harbor scenic loop,
// free passage (ticket id 0).
static INNADRIL_DOCK_SCHED: DockSchedule = ten_minute_dwell!(
    fare(0, 107092, 219098, -3952),
    V_INNADRIL_TOUR,
    &[998],
    &[999],
    &[1000],
    &[1001],
    &[1002]
);

// Rune Harbor ↔ Primeval Isle (`BoatRunePrimeval`). 1620 = "Welcome to Rune Harbor".
static RUNE_DOCK_SCHED: DockSchedule =
    three_minute_dwell!(fare(8925, 34513, -38009, -3640), &[1620, 1989], &[1992]); // leaves for Primeval
static PRIMEVAL_DOCK_SCHED: DockSchedule =
    three_minute_dwell!(fare(8924, 10447, -24982, -3664), &[1988, 1991], &[1990]); // leaves for Rune

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
    dka(-95686, 150514, -3610, 150, 800, &GLUDIN_DOCK_SCHED), // Gludin dock
    vp(-95686, 155514, -3610, 180, 800),
    vp(-95686, 185514, -3610, 250, 800),
    vp(-60136, 238816, -3610, 200, 800),
    vp(-60520, 259609, -3610, 180, 1800),
    vp(-65344, 261460, -3610, 180, 1800),
    vp(-83344, 261560, -3610, 180, 1800),
    vp(-88344, 261660, -3610, 180, 1800),
    vp(-92344, 261660, -3610, 150, 1800),
    vp(-94242, 261659, -3610, 150, 1800),
    dka(-96622, 261660, -3610, 150, 1800, &TALKING_DOCK_SCHED), // Talking dock
];

/// Giran <-> Talking Island ferry (`BoatGiranTalking`).
const GIRAN_TALKING: &[VehiclePathPoint] = &[
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
    dka(-96622, 261660, -3610, 150, 800, &GT_TALKING_DOCK_SCHED), // Talking dock (→ Giran)
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
    dka(48950, 190613, -3610, 150, 800, &GT_GIRAN_DOCK_SCHED), // Giran dock (→ Talking)
];

/// Innadril scenic tour (`BoatInnadrilTour`) — a single-harbor loop.
const INNADRIL_TOUR: &[VehiclePathPoint] = &[
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
    dka(111264, 226240, -3610, 150, 800, &INNADRIL_DOCK_SCHED), // Innadril dock
];

/// Rune <-> Primeval Isle ferry (`BoatRunePrimeval`).
const RUNE_PRIMEVAL: &[VehiclePathPoint] = &[
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
    dka(10342, -27279, -3610, 150, 1800, &PRIMEVAL_DOCK_SCHED), // Primeval dock (→ Rune)
    vp(15528, -27279, -3610, 180, 800),
    vp(22304, -29664, -3610, 290, 800),
    vp(33824, -26880, -3610, 290, 800),
    vp(38848, -21792, -3610, 240, 1200),
    vp(43424, -22080, -3610, 180, 1800),
    vp(44320, -25152, -3610, 180, 1800),
    vp(40576, -31616, -3610, 250, 800),
    vp(36819, -35315, -3610, 220, 800),
    dka(34381, -37680, -3610, 220, 800, &RUNE_DOCK_SCHED), // Rune dock (→ Primeval)
];

/// `BoatManager.load` — spawn the four Interlude ferries at boot and set them
/// on their routes.
pub(crate) fn spawn_boats(world: &mut World) {
    for route in [TALKING_GLUDIN, GIRAN_TALKING, INNADRIL_TOUR, RUNE_PRIMEVAL] {
        spawn_boat(world, route);
    }
}

/// Spawn one ferry docked at its last waypoint and set it sailing; returns the
/// boat's object id.
pub(crate) fn spawn_boat(world: &mut World, route: &'static [VehiclePathPoint]) -> i32 {
    let oid = world.next_npc_object_id;
    world.next_npc_object_id += 1;
    // Start at the last waypoint. If it's a harbor, anchor there (leg points at
    // that dock so its dwell schedule resolves) and depart toward index 0;
    // otherwise the boat is mid-route and heads for index 0 at once.
    let last = route.len() - 1;
    let start = route[last];
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

/// Begin the harbor dwell: run the announcement schedule if the anchored dock
/// (`boat.target()`) has one, otherwise dwell silently for the default period.
fn begin_dwell(world: &mut World, boat_oid: i32) {
    let has_schedule = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .map(|b| b.target().schedule.is_some())
        .unwrap_or(false);
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
    let n = boat.route.len();
    (1..=n)
        .map(|i| boat.route[(boat.leg + i) % n])
        .find(|wp| wp.dock)
        .map(|wp| (wp.x, wp.y))
}

/// One stage of a harbor's dwell schedule: broadcast its announcements (to this
/// harbor and the destination harbor), then schedule the next stage — or depart
/// after the last one.
fn run_dwell_stage(world: &mut World, boat_oid: i32, stage_idx: usize) {
    let Some((dock, sched)) = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .and_then(|b| b.target().schedule.map(|s| (b.target(), s)))
    else {
        return;
    };
    let Some(stage) = sched.stages.get(stage_idx).copied() else {
        return;
    };

    let here = region_of(dock.x, dock.y);
    let there = next_dock(world, boat_oid).map(|(x, y)| region_of(x, y));
    for &mid in stage.messages {
        let say = sp::creature_say_system(ChatType::Boat, sched.char_id, mid as i32);
        broadcast_near_region(world, here, &say);
        if let Some(there) = there
            && there != here
        {
            broadcast_near_region(world, there, &say);
        }
    }

    if stage_idx + 1 < sched.stages.len() {
        let fire_at = world.tick + stage.then_ms.div_ceil(100);
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

/// The `BoatDwellStage` task: run the next announcement stage of a dwell.
pub(crate) fn handle_dwell_stage(world: &mut World, boat_oid: i32, stage: usize) {
    run_dwell_stage(world, boat_oid, stage);
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
/// same `case` that departs.
fn depart(world: &mut World, boat_oid: i32) {
    // Capture the departing dock's schedule before advancing off it.
    let sched = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .and_then(|b| b.target().schedule);
    if let Some(fare) = sched.map(|s| s.fare) {
        pay_for_ride(world, boat_oid, fare);
    }
    if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
        b.advance();
    }
    move_to_next(world, boat_oid);
    // Schedule the in-transit "arriving in ~N minutes" shouts for this leg.
    if let Some(voyage) = sched.map(|s| s.voyage) {
        for &(delay_ms, messages) in voyage {
            let fire_at = world.tick + delay_ms.div_ceil(100);
            world.scheduler.schedule(
                fire_at,
                ScheduledTask::BoatVoyageShout {
                    boat_object_id: boat_oid,
                    messages,
                },
            );
        }
    }
}

/// The `BoatVoyageShout` task: an in-transit arrival announcement. Skipped if
/// the ferry has already docked (a late shout from the previous leg).
pub(crate) fn handle_voyage_shout(world: &mut World, boat_oid: i32, messages: &'static [u32]) {
    let moving = world
        .objects
        .get_component::<Boat>(&boat_oid)
        .is_some_and(|b| b.moving);
    if !moving {
        return;
    }
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
    for wp in boat.route.iter().filter(|w| w.dock) {
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
            send_boat_sm(
                world,
                player,
                sp::sm_ids::YOU_DO_NOT_POSSESS_THE_CORRECT_TICKET,
            );
            oust_rider(world, player, boat_oid, fare);
        } else if let Some(cid) = crate::game_loop::helpers::client_for_player(world, player) {
            let iu = crate::network::enter_world::inventory_update_changes(&world.data, &changes);
            crate::game_loop::helpers::send_inventory_update(world, cid, player, iu);
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

/// Send a bare system message to a player if online.
fn send_boat_sm(world: &World, player: i32, sm_id: i16) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, player)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(sp::system_message_with(sm_id, &[]));
    }
}

/// The `BoatDepart` task (silent-dwell fallback): weigh anchor and sail on.
pub(crate) fn handle_depart(world: &mut World, boat_oid: i32) {
    depart(world, boat_oid);
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

    // At a harbor: anchor (leg stays on the dock so its dwell schedule resolves)
    // and begin the dwell. Elsewhere: advance the cycle and sail on.
    if target.dock {
        if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
            b.moving = false;
        }
        begin_dwell(world, boat_oid);
    } else {
        if let Some(b) = world.objects.get_component_mut::<Boat>(&boat_oid) {
            b.advance();
        }
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
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, player)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(stop);
        }
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
