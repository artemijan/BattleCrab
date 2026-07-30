//! Global, player-less lifecycles from the `ai/areas` scripts — beats that
//! cannot ride the quest-timer machinery because that is anchored to a
//! player (`QuestTimerSeqs`), while these run from boot with nobody online.
//!
//! First resident: **Toma** (`ai/areas/DwarvenVillage/Toma`). Java spawns him
//! at one of three haunts and relocates him every 30 minutes
//! (`RESPAWN_TOMA`); his chat window is `scripts::toma`.

use crate::scheduler::ScheduledTask;
use crate::world::World;

pub(crate) const TOMA: i32 = 30556;

/// Java's `LOCATIONS` — the three spots Toma wanders between.
const TOMA_LOCS: [(i32, i32, i32, i32); 3] = [
    (151680, -174891, -1782, 0),
    (154153, -220105, -3402, 0),
    (178834, -184336, -355, 41400),
];

/// `TELEPORT_DELAY` (30 min) in ticks.
const TOMA_RELOCATE_TICKS: u64 = 30 * 60 * 10;

/// Boot: Toma is not in the spawn data — the script owns him entirely (Java
/// ctor fires `RESPAWN_TOMA` immediately, then every 30 minutes). Eilhalder
/// von Hellmann's day/night watch starts here too, treating boot as a
/// transition so a night boot spawns him right away.
pub(crate) fn spawn_at_boot(world: &mut World) {
    relocate_toma(world);
    let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
    eilhalder_on_day_night_change(world, night);
    world.scheduler.schedule(
        world.tick + DAY_NIGHT_CHECK_TICKS,
        ScheduledTask::DayNightCheck { was_night: night },
    );
    world
        .scheduler
        .schedule(world.tick + FOG_REFRESH_TICKS, ScheduledTask::FogRefresh);
}

/// Forge of the Gods: the 15 s escalation-counter reset (Java's repeating
/// `"refresh"` quest timer).
const FOG_REFRESH_TICKS: u64 = 150;

pub(crate) fn handle_fog_refresh(world: &mut World) {
    world.fog_kill_count = 0;
    world
        .scheduler
        .schedule(world.tick + FOG_REFRESH_TICKS, ScheduledTask::FogRefresh);
}

/// The `RESPAWN_TOMA` beat: despawn the old Toma, spawn him at a random
/// haunt, re-arm.
pub(crate) fn relocate_toma(world: &mut World) {
    let old = find_toma(world);
    if let Some(oid) = old {
        let region = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&oid)
            .map(|r| r.0);
        if let Some(region) = region {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
    }
    let (x, y, z, heading) = TOMA_LOCS[world.roll(3) as usize];
    crate::model::npc::spawn_npc_at(world, TOMA, x, y, z, heading);
    world.scheduler.schedule(
        world.tick + TOMA_RELOCATE_TICKS,
        ScheduledTask::TomaRelocate,
    );
}

pub(crate) fn find_toma(world: &mut World) -> Option<i32> {
    find_by_npc_id(world, TOMA)
}

fn find_by_npc_id(world: &mut World, npc_id: i32) -> Option<i32> {
    let mut found = None;
    world
        .objects
        .for_each_mut::<(&crate::model::npc::Npc, &crate::model::components::Position)>(
            |(n, _)| {
                if n.npc_id == npc_id {
                    found = Some(n.object_id);
                }
            },
        );
    found
}

// ---------------------------------------------------------------------------
// Eilhalder von Hellmann — the Forest of the Dead night raid boss
// ---------------------------------------------------------------------------

pub(crate) const EILHALDER: i32 = 25328;
const EILHALDER_SPAWN: (i32, i32, i32) = (59090, -42188, -3003);

/// The day/night poll cadence (1 real minute = 6 game-minutes; transitions
/// land within one beat, like Java's minute-tick task manager).
const DAY_NIGHT_CHECK_TICKS: u64 = 600;
/// Java's `startQuestTimer("despawn", 30000, …)` retry while he fights.
const EILHALDER_DESPAWN_RETRY_TICKS: u64 = 300;

/// The minute beat: fire the transition handler when day/night flipped,
/// re-arm carrying the new state (state lives in the task itself).
pub(crate) fn handle_day_night_check(world: &mut World, was_night: bool) {
    let night = crate::game_loop::game_time::is_night_at(commons::util::now_millis());
    if night != was_night {
        eilhalder_on_day_night_change(world, night);
    }
    world.scheduler.schedule(
        world.tick + DAY_NIGHT_CHECK_TICKS,
        ScheduledTask::DayNightCheck { was_night: night },
    );
}

fn npc_in_combat(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::AggroList>(&oid)
        .is_some_and(|a| !a.0.is_empty())
}

fn despawn_by_oid(world: &mut World, oid: i32) {
    let region = world
        .objects
        .get_component::<crate::model::components::RegionCell>(&oid)
        .map(|r| r.0);
    if let Some(region) = region {
        crate::game_loop::death::despawn_npc(world, oid, region);
    }
}

/// Java `onDayNightChange`: day + alive → despawn (30 s retry while he is
/// fighting); otherwise if he is absent or dead, spawn him.
pub(crate) fn eilhalder_on_day_night_change(world: &mut World, night: bool) {
    let alive = find_by_npc_id(world, EILHALDER).and_then(|oid| {
        world
            .objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .map(|v| (oid, !v.dead))
    });
    match alive {
        Some((oid, true)) if !night => {
            if npc_in_combat(world, oid) {
                world.scheduler.schedule(
                    world.tick + EILHALDER_DESPAWN_RETRY_TICKS,
                    ScheduledTask::EilhalderDespawnRetry,
                );
            } else {
                despawn_by_oid(world, oid);
            }
        }
        Some((_, true)) => {}
        _ => {
            crate::model::npc::spawn_npc_at(
                world,
                EILHALDER,
                EILHALDER_SPAWN.0,
                EILHALDER_SPAWN.1,
                EILHALDER_SPAWN.2,
                0,
            );
        }
    }
}

/// The `"despawn"` retry: still fighting → try again in 30 s, else vanish.
pub(crate) fn handle_eilhalder_despawn_retry(world: &mut World) {
    let Some(oid) = find_by_npc_id(world, EILHALDER) else {
        return;
    };
    if npc_in_combat(world, oid) {
        world.scheduler.schedule(
            world.tick + EILHALDER_DESPAWN_RETRY_TICKS,
            ScheduledTask::EilhalderDespawnRetry,
        );
    } else {
        despawn_by_oid(world, oid);
    }
}
