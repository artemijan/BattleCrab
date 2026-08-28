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

use crate::game_loop::time::TICKS_PER_SECOND;
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

/// Core's lines (`NpcStringId`). The two intro lines fire **once**, on the
/// first hit of a life; `REMOVING_INTRUDERS` is a 1-in-100 taunt thereafter.
const A_NON_PERMITTED_TARGET_HAS_BEEN_DISCOVERED: i32 = 1_000_001;
const INTRUDER_REMOVAL_SYSTEM_INITIATED: i32 = 1_000_002;
const REMOVING_INTRUDERS: i32 = 1_000_003;
const A_FATAL_ERROR_HAS_OCCURRED: i32 = 1_000_004;
const SYSTEM_IS_BEING_SHUT_DOWN: i32 = 1_000_005;

/// `getRandom(100) == 0`.
const TAUNT_CHANCE: i32 = 100;

/// `startQuestTimer("spawn_minion", 60000, npc, null)`.
const MINION_RESPAWN_SECS: u64 = 60;
/// `startQuestTimer("despawn_minions", 20000, …)`.
const DESPAWN_DELAY_SECS: u64 = 20;

/// Core spawned: it is a stationary generator (Java `onSpawn` →
/// `setImmobilized(true)`), so it melees adjacent attackers but never chases —
/// then it places its minions.
pub(crate) fn on_core_spawned(world: &mut World, core_oid: i32) {
    world
        .objects
        .add_components(&core_oid, crate::model::components::Immobilized);
    // Java's spawn path restores `_firstAttacked` from `Core_Attacked`, so a
    // restart between the intro and the kill does not replay the intro lines.
    let first_attacked = crate::game_loop::global_vars::get_bool(world, CORE_ATTACKED_VAR, false);
    world
        .objects
        .add_components(&core_oid, CoreState { first_attacked });
    for (npc_id, x, y, z) in MINION_SPAWNS {
        crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, 0);
    }
}

/// Java's `_firstAttacked` — reset on death, so the intro plays once per life
/// rather than once per server run.
/// Java's `GlobalVariablesManager` key for `_firstAttacked` (`Core.onSave`).
pub(crate) const CORE_ATTACKED_VAR: &str = "Core_Attacked";

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct CoreState {
    pub first_attacked: bool,
}

/// `Core.onAttack` — the intro pair on the first hit, a rare taunt after.
pub(crate) fn on_core_attacked(world: &mut World, core_oid: i32) {
    let first = world
        .objects
        .get_component::<CoreState>(&core_oid)
        .is_some_and(|s| s.first_attacked);
    if first {
        // `if (getRandom(100) == 0)` — a rare line, not every swing.
        if world.roll(TAUNT_CHANCE) == 0 {
            crate::game_loop::npc::say::npc_say(world, core_oid, REMOVING_INTRUDERS);
        }
        return;
    }
    if world
        .objects
        .get_component::<CoreState>(&core_oid)
        .is_none()
    {
        world
            .objects
            .add_components(&core_oid, CoreState::default());
    }
    if let Some(s) = world.objects.get_component_mut::<CoreState>(&core_oid) {
        s.first_attacked = true;
    }
    // Java `onSave()` persists `_firstAttacked` as `Core_Attacked`, so a
    // restart mid-fight does not replay the intro. Written on the transition
    // rather than on a save timer — same value, no lost-window.
    crate::game_loop::global_vars::set(world, CORE_ATTACKED_VAR, true);
    // Both intro lines, in order.
    crate::game_loop::npc::say::npc_say(
        world,
        core_oid,
        A_NON_PERMITTED_TARGET_HAS_BEEN_DISCOVERED,
    );
    crate::game_loop::npc::say::npc_say(world, core_oid, INTRUDER_REMOVAL_SYSTEM_INITIATED);
}

/// Is this npc id one of Core's script-spawned minions?
pub(crate) fn is_core_minion(npc_id: i32) -> bool {
    MINION_SPAWNS.iter().any(|(id, ..)| *id == npc_id)
}

/// A Core minion died — respawn it in 60 s, **but only while Core is alive**.
/// Java guards on `getStatus(CORE) == ALIVE`, so minions killed after Core
/// stop coming back rather than repopulating an empty lair.
pub(crate) fn on_minion_killed(world: &mut World, npc_id: i32) {
    if crate::game_loop::grand_boss::status(world, CORE)
        != Some(crate::game_loop::grand_boss::ALIVE)
    {
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
    if crate::game_loop::grand_boss::status(world, CORE)
        != Some(crate::game_loop::grand_boss::ALIVE)
    {
        return;
    }
    if let Some((_, x, y, z)) = MINION_SPAWNS.iter().copied().find(|(id, ..)| *id == npc_id) {
        crate::game_loop::npc::spawn_npc_at(world, npc_id, x, y, z, 0);
    }
}

/// Core died: clear its minions after 20 s (Java's `despawn_minions` timer).
pub(crate) fn on_core_killed(world: &mut World) {
    // `_firstAttacked = false` — the intro plays again next life.
    crate::game_loop::global_vars::set(world, CORE_ATTACKED_VAR, false);
    let cores: Vec<i32> = world.npcs_with_id(CORE).to_vec();
    for oid in cores {
        if let Some(s) = world.objects.get_component_mut::<CoreState>(&oid) {
            s.first_attacked = false;
        }
    }
    world.scheduler.schedule(
        world.tick + DESPAWN_DELAY_SECS * TICKS_PER_SECOND,
        ScheduledTask::CoreDespawnMinions,
    );
}

/// The despawn timer firing — remove every living Core minion.
pub(crate) fn handle_despawn_minions(world: &mut World) {
    let doomed: Vec<i32> = MINION_SPAWNS
        .iter()
        .flat_map(|(id, ..)| world.npcs_with_id(*id).iter().copied())
        .collect();
    for oid in doomed {
        crate::game_loop::death::despawn_npc_by_oid(world, oid);
    }
}

/// Core's death lines, said before the minions are cleared.
pub(crate) fn say_death_lines(world: &mut World, core_oid: i32) {
    crate::game_loop::npc::say::npc_say(world, core_oid, A_FATAL_ERROR_HAS_OCCURRED);
    crate::game_loop::npc::say::npc_say(world, core_oid, SYSTEM_IS_BEING_SHUT_DOWN);
}
