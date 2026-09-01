//! `ai/others/Spawns/*` — the two scripts that own spawn *templates* rather
//! than NPC ids: `DayNightSpawns` and `NoRandomActivity`.
//!
//! ## DayNightSpawns — the templates whose population follows the clock
//!
//! 47 templates on this dist carry `ai="DayNightSpawns"` with a `dayTime` and a
//! `nightTime` group, both `spawnByDefault="false"`: Devil's Isle, the
//! Interlude day/night map tiles, and the vampire/undead night spawns. Java
//! tracks the activated templates in the script and calls `manageSpawns` on
//! activation and on every `OnDayNightChange`; here the template list is a
//! constant of the loaded data, so the phase is applied straight over
//! `SpawnData` — `spawnAll`/`despawnAll` on the group that just came in or out.
//!
//! The clock itself is G33's (`game_time::is_night_at`), polled by the minute
//! beat in [`area::handle_day_night_check`].
//!
//! ## NoRandomActivity — the templates that pin their NPCs down
//!
//! One template on this dist (Rune's Chapel Guards) declares
//! `disableRandomWalk`. Java's `onSpawnNpc` clears that NPC's random-walk and
//! random-animation flags; both are read off the shared *template* here, so
//! the override rides a per-instance [`SpawnActivity`] component applied by
//! [`apply_spawn_ai`] at spawn time.

use crate::model::npc::Npc;
use crate::world::World;

/// `ai="DayNightSpawns"`.
const DAY_NIGHT_AI: &str = "DayNightSpawns";
/// Java's `DAY_GROUP_NAME` / `NIGHT_GROUP_NAME`.
const DAY_GROUP: &str = "dayTime";
const NIGHT_GROUP: &str = "nightTime";

/// Java `onSpawnActivate` — the templates activate at boot, so the half that
/// matches the current phase is placed and the other stays empty.
pub(crate) fn activate_at_boot(world: &mut World) {
    let night = crate::game_loop::upkeep::game_time::is_night_at(commons::util::now_millis());
    let placed = apply_phase(world, night, false);
    tracing::info!(
        "DayNightSpawns: {} phase active, {placed} NPCs placed.",
        if night { "night" } else { "day" }
    );
}

/// Java `onDayNightChange` → `manageSpawns(template, isNight)` for every
/// tracked template: the group for the phase that just started spawns, the
/// other despawns.
pub(crate) fn on_day_night_change(world: &mut World, night: bool) {
    let placed = apply_phase(world, night, true);
    tracing::info!(
        "DayNightSpawns: turned {}, {placed} NPCs placed.",
        if night { "night" } else { "day" }
    );
}

/// The shared half of `manageSpawns`. `despawn_stale` is false at boot, where
/// nothing of either group is standing yet.
fn apply_phase(world: &mut World, night: bool, despawn_stale: bool) -> usize {
    let mut placed = 0;
    for spawn_idx in 0..world.data.spawn_data.spawns.len() {
        if world.data.spawn_data.spawns[spawn_idx].ai.as_deref() != Some(DAY_NIGHT_AI) {
            continue;
        }
        for group_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups.len() {
            let wanted = match world.data.spawn_data.spawns[spawn_idx].groups[group_idx]
                .name
                .as_deref()
            {
                Some(NIGHT_GROUP) => night,
                Some(DAY_GROUP) => !night,
                // A template can hold other groups; Java's `manageSpawns`
                // leaves them alone.
                _ => continue,
            };
            if wanted {
                let fresh = spawn_group(world, spawn_idx, group_idx);
                placed += fresh.len();
                // Java's `Spawn.doSpawn` puts the NPC in the world *and* shows
                // it to everyone already standing there. At boot there is no
                // audience, so the placement is silent — but a phase change
                // happens under live players, and without this the night mobs
                // only appeared once a player left the region and came back.
                if despawn_stale {
                    for oid in fresh {
                        crate::game_loop::npc::introduce_npc(world, oid);
                    }
                }
            } else if despawn_stale {
                despawn_group(world, spawn_idx, group_idx);
            }
        }
    }
    placed
}

/// `SpawnGroup.spawnAll` — place every `<npc>` line's `count`, and report the
/// object ids so the caller can show them to nearby players.
fn spawn_group(world: &mut World, spawn_idx: usize, group_idx: usize) -> Vec<i32> {
    let mut placed = Vec::new();
    let lines = world.data.spawn_data.spawns[spawn_idx].groups[group_idx]
        .npcs
        .len();
    for npc_idx in 0..lines {
        let count = world.data.spawn_data.spawns[spawn_idx].groups[group_idx].npcs[npc_idx].count;
        for _ in 0..count {
            if let Some(oid) =
                crate::game_loop::npc::spawn_one(world, spawn_idx, group_idx, npc_idx)
            {
                placed.push(oid);
            }
        }
    }
    placed
}

/// `SpawnGroup.despawnAll` — every NPC still standing from this group's lines
/// goes, including corpses waiting to decay. Their pending respawns are left
/// to [`respawn_is_in_phase`], which refuses to place an out-of-phase group.
fn despawn_group(world: &mut World, spawn_idx: usize, group_idx: usize) {
    let mut victims: Vec<(i32, (i32, i32))> = Vec::new();
    world
        .objects
        .for_each_mut::<(&Npc, &crate::model::components::RegionCell)>(|(npc, region)| {
            if npc.spawn_ref.0 == spawn_idx && npc.spawn_ref.1 == group_idx {
                victims.push((npc.object_id, region.0));
            }
        });
    for (oid, region) in victims {
        crate::game_loop::npc::despawn_npc(world, oid, region);
    }
}

/// The respawn guard: a mob killed just before its phase ended must not walk
/// back out of the ground during the other half of the day (Java's
/// `despawnAll` stops the spawns outright; here the scheduled respawn task
/// survives the despawn, so it is filtered when it fires).
pub(crate) fn respawn_is_in_phase(world: &World, spawn_idx: usize, group_idx: usize) -> bool {
    let Some(template) = world.data.spawn_data.spawns.get(spawn_idx) else {
        return true;
    };
    if template.ai.as_deref() != Some(DAY_NIGHT_AI) {
        return true;
    }
    let night = crate::game_loop::upkeep::game_time::is_night_at(commons::util::now_millis());
    match template
        .groups
        .get(group_idx)
        .and_then(|g| g.name.as_deref())
    {
        Some(NIGHT_GROUP) => night,
        Some(DAY_GROUP) => !night,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// NoRandomActivity
// ---------------------------------------------------------------------------

/// `ai="NoRandomActivity"`.
const NO_RANDOM_ACTIVITY_AI: &str = "NoRandomActivity";

/// Java `NoRandomActivity.onSpawnNpc`: the per-NPC `setRandomWalking` /
/// `setRandomAnimation` overrides its template would otherwise decide. Present
/// only on NPCs whose spawn template asks for it, so the common case pays
/// nothing.
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct SpawnActivity {
    pub random_walk: bool,
    pub random_animation: bool,
}

/// Apply the spawn template's `ai=` script to a freshly placed NPC. Called
/// from the spawn path (Java's `onSpawnNpc` notification); only
/// `NoRandomActivity` acts on individual NPCs.
pub(crate) fn apply_spawn_ai(world: &mut World, npc_oid: i32, spawn_idx: usize) {
    let Some(template) = world.data.spawn_data.spawns.get(spawn_idx) else {
        return;
    };
    if template.ai.as_deref() != Some(NO_RANDOM_ACTIVITY_AI) {
        return;
    }
    let flag = |name: &str| {
        template
            .parameters
            .get(name)
            .map(|v| v == "true")
            .unwrap_or(false)
    };
    let activity = SpawnActivity {
        random_walk: !flag("disableRandomWalk"),
        random_animation: !flag("disableRandomAnimation"),
    };
    world.objects.add_components(&npc_oid, activity);
}

/// `Npc.isRandomWalkingEnabled()` — the template flag unless this NPC's spawn
/// template overrode it.
pub(crate) fn random_walk_enabled(world: &World, npc_oid: i32, template_flag: bool) -> bool {
    world
        .objects
        .get_component::<SpawnActivity>(&npc_oid)
        .map_or(template_flag, |a| a.random_walk)
}

/// `Npc.hasRandomAnimation()`'s per-NPC half.
pub(crate) fn random_animation_enabled(world: &World, npc_oid: i32, template_flag: bool) -> bool {
    world
        .objects
        .get_component::<SpawnActivity>(&npc_oid)
        .map_or(template_flag, |a| a.random_animation)
}
