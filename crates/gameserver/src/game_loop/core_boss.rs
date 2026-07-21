//! Core (`ai/bosses/Core`) — the second grand-boss script.
//!
//! Core's own mechanics are its **script-spawned minions**: they are not in the
//! NPC minion table, they respawn on a 60 s timer while Core lives, and they
//! are cleared 20 s after it dies.
//!
//! # The 19-that-are-3
//!
//! Java builds its spawn table as
//! `Map<Integer, Location> MINNION_SPAWNS` and calls `put` **19 times** — ten
//! Death Knights, five Doom Wraiths, four Susceptors, each at a different
//! location. But the map is keyed by **npc id**, so every put overwrites the
//! last: only **three** entries survive, one per minion type, at the last
//! location listed for each.
//!
//! Core therefore spawns **3 minions, not 19**. That is plainly not what the
//! author meant, and it is what the server does — porting the 19 locations as a
//! list would hand Core six times the adds and a completely different fight.
//! Ported as it behaves, pinned by a test.

use crate::scheduler::ScheduledTask;
use crate::world::World;

pub const CORE: i32 = 29006;
const DEATH_KNIGHT: i32 = 29007;
const DOOM_WRAITH: i32 = 29008;
const SUSCEPTOR: i32 = 29011;

/// The three surviving entries of Java's overwritten map — the **last**
/// location listed for each type.
const MINION_SPAWNS: [(i32, i32, i32, i32); 3] = [
    (DEATH_KNIGHT, 17726, 110391, -6648),
    (DOOM_WRAITH, 17993, 111458, -6584),
    (SUSCEPTOR, 17849, 109388, -6480),
];

const TICKS_PER_SECOND: u64 = 10;
/// `startQuestTimer("spawn_minion", 60000, npc, null)`.
const MINION_RESPAWN_SECS: u64 = 60;
/// `startQuestTimer("despawn_minions", 20000, …)`.
const DESPAWN_DELAY_SECS: u64 = 20;

/// Core spawned: place its minions.
pub(crate) fn on_core_spawned(world: &mut World) {
    for (npc_id, x, y, z) in MINION_SPAWNS {
        crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, 0);
    }
}

/// Is this npc id one of Core's script-spawned minions?
pub(crate) fn is_core_minion(npc_id: i32) -> bool {
    MINION_SPAWNS.iter().any(|(id, ..)| *id == npc_id)
}

/// A Core minion died — respawn it in 60 s, **but only while Core is alive**.
/// Java guards on `getStatus(CORE) == ALIVE`, so minions killed after Core
/// stop coming back rather than repopulating an empty lair.
pub(crate) fn on_minion_killed(world: &mut World, npc_id: i32) {
    if crate::game_loop::grand_boss::status(world, CORE) != Some(crate::game_loop::grand_boss::ALIVE) {
        return;
    }
    world.scheduler.schedule(
        world.tick + MINION_RESPAWN_SECS * TICKS_PER_SECOND,
        ScheduledTask::CoreMinionRespawn { npc_id },
    );
}

/// The 60 s timer firing.
pub(crate) fn handle_minion_respawn(world: &mut World, npc_id: i32) {
    // Core died while the timer ran: don't repopulate.
    if crate::game_loop::grand_boss::status(world, CORE) != Some(crate::game_loop::grand_boss::ALIVE) {
        return;
    }
    if let Some((_, x, y, z)) = MINION_SPAWNS.iter().copied().find(|(id, ..)| *id == npc_id) {
        crate::model::npc::spawn_npc_at(world, npc_id, x, y, z, 0);
    }
}

/// Core died: clear its minions after 20 s (Java's `despawn_minions` timer).
pub(crate) fn on_core_killed(world: &mut World) {
    world
        .scheduler
        .schedule(world.tick + DESPAWN_DELAY_SECS * TICKS_PER_SECOND, ScheduledTask::CoreDespawnMinions);
}

/// The despawn timer firing — remove every living Core minion.
pub(crate) fn handle_despawn_minions(world: &mut World) {
    let mut doomed = Vec::new();
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
        if is_core_minion(n.npc_id) {
            doomed.push(n.object_id);
        }
    });
    for oid in doomed {
        if let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&oid).map(|r| r.0) {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
    }
}
