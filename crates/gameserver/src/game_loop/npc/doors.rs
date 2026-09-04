//! Door state changes — the port of `Door.openMe`/`closeMe`/
//! `broadcastStatusUpdate` plus the BY_TIME cycle (`startTimerOpen` /
//! `TimerOpen`) and the auto-close task. Open state lives in
//! `world.geo.doors` (the collision grid the geo queries — and the path
//! worker — read); this module flips it and broadcasts the client packets.
//! Group/child cascades are not ported (this dist declares none).

use crate::data::door_data::DoorOpenMethod;
use crate::game_loop::net::broadcast;
use crate::model::components::space::RegionCell;
use crate::model::door::Door;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use crate::scheduler::ms_to_ticks;

/// `Door.sendInfo(player)`: `StaticObjectInfo` + `DoorStatusUpdate` for one
/// door to one session (enter-world burst, region-cross deltas).
pub(crate) fn send_door_info(world: &World, session: &ClientSession, door_oid: i32) {
    let Some(door) = world.objects.get_component::<Door>(&door_oid) else {
        return;
    };
    let Some(t) = world.data.door_data.get(door.door_id) else {
        return;
    };
    let open = door_open_state(world, door_oid, door.door_id);
    session.send(server_packets::static_object_info_door(door, t, open));
    session.send(server_packets::door_status_update(door, t, open));
}

/// An instance door copy reads its own [`InstanceDoorOpen`] flag; every other
/// door reads the shared collision grid (Java's single global door state).
pub(crate) fn door_open_state(world: &World, door_oid: i32, door_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::space::InstanceDoorOpen>(&door_oid)
        .map(|s| s.0)
        .unwrap_or_else(|| world.geo.doors.is_open(door_id))
}

/// `Door.broadcastStatusUpdate()`: push the new state to everyone whose
/// region block contains the door.
pub(crate) fn broadcast_status(world: &World, door_oid: i32) {
    let Some(door) = world.objects.get_component::<Door>(&door_oid) else {
        return;
    };
    let Some(t) = world.data.door_data.get(door.door_id) else {
        return;
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&door_oid) else {
        return;
    };
    let open = world.geo.doors.is_open(door.door_id);
    broadcast::broadcast_near_region(
        world,
        region.0,
        &server_packets::static_object_info_door(door, t, open),
    );
    broadcast::broadcast_near_region(
        world,
        region.0,
        &server_packets::door_status_update(door, t, open),
    );
}

/// `Door.openMe()`: no-op when already open; otherwise flip, broadcast, and
/// arm the auto-close (script-triggered doors with a `closeTime` shut
/// themselves — BY_TIME doors are driven by their cycle task instead).
/// The object id of the **shared-grid** door carrying `door_id`.
///
/// Door object ids are allocated dynamically, so this scans the door regions.
/// Instance door copies are skipped: they carry their own
/// [`InstanceDoorOpen`](crate::model::components::space::InstanceDoorOpen) state and
/// are driven through [`crate::game_loop::space::instances::open_close_door`], so a
/// global open/close must not pick one up.
pub(crate) fn find_shared_door(world: &World, door_id: i32) -> Option<i32> {
    world.door_regions.values().flatten().copied().find(|&oid| {
        world
            .objects
            .get_component::<crate::model::components::space::InstanceDoorOpen>(&oid)
            .is_none()
            && world
                .objects
                .get_component::<Door>(&oid)
                .is_some_and(|d| d.door_id == door_id)
    })
}

/// Open the door with a given **door id** (Java `DoorData.getDoor(id).openMe()`).
pub(crate) fn open_door_by_id(world: &mut World, door_id: i32) {
    if let Some(oid) = find_shared_door(world, door_id) {
        open_door(world, oid);
    }
}

/// Open a door by id and arm a scripted close `close_ticks` later — the
/// `openDoor(id); startQuestTimer("Close_Door", …)` pattern (Pagan Temple's
/// mark-gated doors). Rides the auto-close seq, so a newer open or close
/// before the timer fires turns this close into a no-op.
pub(crate) fn open_door_timed(world: &mut World, door_id: i32, close_ticks: u64) {
    let Some(oid) = find_shared_door(world, door_id) else {
        return;
    };
    open_door(world, oid);
    if let Some(seq) = world
        .objects
        .get_component::<Door>(&oid)
        .map(|d| d.auto_close_seq)
    {
        world.scheduler.schedule(
            world.tick + close_ticks,
            ScheduledTask::DoorAutoClose {
                door_object_id: oid,
                seq,
            },
        );
    }
}

/// Open or close the door with a given **door id** (Java
/// `ClanHall.openCloseDoors` → `door.openMe()` / `closeMe()`).
pub(crate) fn set_door_by_id(world: &mut World, door_id: i32, open: bool) {
    if let Some(oid) = find_shared_door(world, door_id) {
        if open {
            open_door(world, oid);
        } else {
            close_door(world, oid);
        }
    }
}

pub(crate) fn open_door(world: &mut World, door_oid: i32) {
    let Some((door_id, seq)) = world.objects.get_component_mut::<Door>(&door_oid).map(|d| {
        d.auto_close_seq += 1;
        (d.door_id, d.auto_close_seq)
    }) else {
        return;
    };
    if world.geo.doors.is_open(door_id) {
        return;
    }
    world.geo.doors.set_open(door_id, true);
    broadcast_status(world, door_oid);

    let Some((close_time, method)) = world
        .data
        .door_data
        .get(door_id)
        .map(|t| (t.close_time, t.open_method))
    else {
        return;
    };
    if close_time >= 0 && method != DoorOpenMethod::ByTime {
        let delay = ms_to_ticks(close_time * 1000);
        world.scheduler.schedule(
            world.tick + delay,
            ScheduledTask::DoorAutoClose {
                door_object_id: door_oid,
                seq,
            },
        );
    }
}

/// `Door.closeMe()`: cancels the pending auto-close (seq bump), flips,
/// broadcasts.
pub(crate) fn close_door(world: &mut World, door_oid: i32) {
    let Some(door_id) = world.objects.get_component_mut::<Door>(&door_oid).map(|d| {
        d.auto_close_seq += 1;
        d.door_id
    }) else {
        return;
    };
    if !world.geo.doors.is_open(door_id) {
        return;
    }
    world.geo.doors.set_open(door_id, false);
    broadcast_status(world, door_oid);
}

/// The `AutoClose` task: shut the door unless a newer open/close superseded
/// this schedule.
pub(crate) fn handle_door_auto_close(world: &mut World, door_oid: i32, seq: u64) {
    if world
        .objects
        .get_component::<Door>(&door_oid)
        .is_none_or(|d| d.auto_close_seq != seq)
    {
        return;
    }
    close_door(world, door_oid);
}

/// `Door()` constructor → `startTimerOpen()` for every BY_TIME door: arm the
/// first toggle. Initial delay is `openTime` while open / `closeTime` while
/// closed (+ a 0..randomTime spread), exactly Java's (asymmetric vs the
/// running cycle) choice.
pub(crate) fn start_time_cycles(world: &mut World) {
    let mut doors: Vec<(i32, i32)> = Vec::new(); // (door_oid, door_id)
    world
        .objects
        .for_each_mut::<&Door>(|d| doors.push((d.object_id, d.door_id)));
    for (door_oid, door_id) in doors {
        let Some((method, open_time, close_time, random_time)) = world
            .data
            .door_data
            .get(door_id)
            .map(|t| (t.open_method, t.open_time, t.close_time, t.random_time))
        else {
            continue;
        };
        if method != DoorOpenMethod::ByTime {
            continue;
        }
        let open = world.geo.doors.is_open(door_id);
        let mut delay = if open { open_time } else { close_time };
        if random_time > 0 {
            delay += world.roll(random_time);
        }
        let ticks = ms_to_ticks(delay.max(0) * 1000);
        world.scheduler.schedule(
            world.tick + ticks,
            ScheduledTask::DoorTimerToggle {
                door_object_id: door_oid,
            },
        );
    }
}

/// The `TimerOpen` task: toggle and re-arm. Post-toggle delay is
/// `closeTime` while open / `openTime` while closed (Java's `TimerOpen.run`
/// — note the inversion against `startTimerOpen`, kept as-is).
pub(crate) fn handle_door_timer_toggle(world: &mut World, door_oid: i32) {
    let Some(door_id) = world
        .objects
        .get_component::<Door>(&door_oid)
        .map(|d| d.door_id)
    else {
        return;
    };
    let open = world.geo.doors.is_open(door_id);
    if open {
        close_door(world, door_oid);
    } else {
        open_door(world, door_oid);
    }
    let Some((random_time, close_time, open_time)) = world
        .data
        .door_data
        .get(door_id)
        .map(|t| (t.random_time, t.close_time, t.open_time))
    else {
        return;
    };
    let open_now = world.geo.doors.is_open(door_id);
    let mut delay = if open_now { close_time } else { open_time };
    if random_time > 0 {
        delay += world.roll(random_time);
    }
    let ticks = ms_to_ticks(delay.max(0) * 1000);
    world.scheduler.schedule(
        world.tick + ticks,
        ScheduledTask::DoorTimerToggle {
            door_object_id: door_oid,
        },
    );
}

/// Convenience for scripts/systems: open or close a door by its template id
/// (Java `DoorData.getDoor(id).openMe()` call sites).
#[allow(dead_code)]
pub(crate) fn open_close_by_door_id(world: &mut World, door_id: i32, open: bool) {
    let mut door_oid = None;
    world.objects.for_each_mut::<&Door>(|d| {
        if d.door_id == door_id {
            door_oid = Some(d.object_id);
        }
    });
    let Some(oid) = door_oid else { return };
    if open {
        open_door(world, oid);
    } else {
        close_door(world, oid);
    }
}
