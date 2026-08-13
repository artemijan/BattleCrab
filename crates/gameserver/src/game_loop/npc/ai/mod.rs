//! `AttackableAI` (G9 slice): the 1 s think tick over monsters in active
//! regions — aggro-range scans, chasing, swinging back, drift-return.
//!
//! The 1 s period ([`NPC_THINK_PERIOD`], Java's
//! `AttackableThinkTaskManager.TASK_DELAY`) is the *idle* cadence only. Java
//! also re-enters `onEvtThink()` off the 100 ms fast paths, and so does this
//! module: [`on_npc_attack_ready`] (`EVT_READY_TO_ACT`, the swing period
//! elapsing) and [`on_npc_arrived`] (`EVT_ARRIVED`, the chase closing). The
//! second one is what makes a mob swing the instant it reaches its target
//! instead of standing in range for up to a second.
//!
//! Idle random walk and random social animations
//! (`RandomAnimationTaskManager`), NPC skill casting (see
//! [`super::cast`]), town-guard PK aggro, clan/faction help calls,
//! `thinkAttack`'s line-of-sight gate (a mob that cannot see its target
//! walks a geo-validated route instead of engaging), geodata-clamped
//! chasing, `checkHate` aggro decay and the teleport-home attack timeout are
//! ported.
//!
//! `thinkAttack` is now walked end to end, in Java's order: the anti-stacking
//! shuffle, the `AIType.ARCHER` kite and its flat 850 bow range, the
//! raid/minion target-chaos block, and the `checkTarget` → `targetReconsider`
//! tail. Faction calls seed hate directly (Java's `EVT_AGGRESSION`, whose
//! `Summon` leg never applies to an `Attackable` recruit), run the
//! `setRunning()` that event carries, and dispatch the
//! `OnAttackableFactionCall` script event's two listeners on this dist —
//! Queen Ant's nurses and Orfen's minions — via `on_faction_call_script`.

use crate::game_loop::guard::{maybe_position, position};
use crate::game_loop::helpers::hp_fraction;
use crate::game_loop::helpers::hp_pair;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::is_raid_npc;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::pos_of;
use std::collections::HashSet;

use commons::util::rnd;

use crate::data::npc_data::AiType;
use crate::model::components::{
    AttackState, Casting, Movement, Position, RegionCell, Speeds, Vitals,
};
use crate::model::movement::MoveData;
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::network::server_packets;
use crate::world::{World, regions_adjacent};

use crate::game_loop::combat::{self, ATTACK_TIMEOUT_TICKS};
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::{broadcast_near_region_in, instance_of};
use crate::game_loop::minions::{MinionOf, Minions};
use crate::game_loop::walkers::WalkState;
use crate::game_loop::{abnormal, death, minions, pvp, servitor, siege, spawn_scripts, target};

/// `AttackableThinkTaskManager.TASK_DELAY`: think once per second.
pub(crate) const NPC_THINK_PERIOD: u64 = 10;

/// `thinkAttack`: "Base bow range for NPCs" — the flat engagement range an
/// `AIType.ARCHER` mob uses instead of its template `<attack range>`.
const NPC_BOW_RANGE: i32 = 850;

/// `AttackableAI.RANDOM_WALK_RATE`: an idle mob rolls a 1-in-30 chance each
/// think (≈ once every 30 s) to wander to a new spot near its spawn.
const RANDOM_WALK_RATE: i32 = 30;

/// 100 ms game ticks per second — animation intervals are configured in
/// seconds (`Min/MaxNpcAnimation`).
const TICKS_PER_SECOND: u64 = 10;

/// `Npc.MINIMUM_SOCIAL_INTERVAL` (6000 ms): floor between social broadcasts.
const SOCIAL_THROTTLE_TICKS: u64 = 60;

/// One AI pass over every living monster in an active region (Java gates
/// `onEvtThink` on `WorldRegion.areNeighborsActive()`; regions are "active"
/// exactly while a player's 3×3 block covers them, which is the same test as
/// `regions_adjacent` against each player).
pub(crate) fn npc_ai_tick(world: &mut World) {
    // Active-region set: every cell adjacent to a player-occupied cell.
    let mut active: HashSet<(i32, i32)> = HashSet::new();
    for cell in world.occupied_player_cells() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                active.insert((cell.0 + dx, cell.1 + dy));
            }
        }
    }
    if active.is_empty() {
        return;
    }

    let candidates: Vec<i32> = active
        .iter()
        .filter_map(|region| world.npc_regions.get(region))
        .flatten()
        .copied()
        .collect();
    for npc_oid in candidates {
        think(world, npc_oid);
        // Idle social animations run for every NPC in an active region, not
        // just the monster AI subtree (Java's `RandomAnimationTaskManager` is
        // independent of `AttackableAI`).
        random_animation_think(world, npc_oid);
    }
}

/// `RandomAnimationTaskManager.run`: while an NPC stands idle in an active
/// region, occasionally broadcast a `SocialAction` (idle animation 2 or 3),
/// then reschedule the next attempt a random 5–60 s out.
///
/// Timing is drawn from `world.rng` directly (not `world.roll`) so it never
/// disturbs the shared forced-roll queue combat tests depend on.
fn random_animation_think(world: &mut World, npc_oid: i32) {
    // `hasRandomAnimation`: template flag + a positive Max*Animation bound.
    // (Java also excludes `AIType.CORPSE`; that enum isn't modelled, but such
    // NPCs — chests — carry `randomAnimation="false"` in the datapack anyway.)
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    // Both flags are memoized on the core at spawn — no template lookup here.
    let attackable = npc.attackable(world);
    let enabled =
        spawn_scripts::random_animation_enabled(world, npc_oid, npc.random_animation(world));
    let (min_s, max_s) = if attackable {
        (
            world.cfg.npc.min_monster_animation,
            world.cfg.npc.max_monster_animation,
        )
    } else {
        (
            world.cfg.npc.min_npc_animation,
            world.cfg.npc.max_npc_animation,
        )
    };
    if !enabled || max_s <= 0 {
        return;
    }

    let now = world.tick;
    // First visit: set the initial pending time and wait (Java `add()`).
    let Some(next) = world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .and_then(|ai| ai.next_animation_tick)
    else {
        let delay = animation_delay_ticks(world, min_s, max_s);
        if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
            ai.next_animation_tick = Some(now + delay);
        }
        return;
    };
    if now <= next {
        return;
    }

    // Due: play an animation if idle (alive, not in combat, not moving),
    // honouring the 6 s social throttle; then reschedule regardless.
    let idle = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_some_and(|v| !v.dead)
        && world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| ai.intention != NpcIntention::Attack)
        && !world.objects.has_component::<Movement>(&npc_oid);
    if idle {
        let throttled = world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| now.saturating_sub(ai.last_social_tick) <= SOCIAL_THROTTLE_TICKS);
        if !throttled {
            let action_id = rnd::get_range(2, 3); // Rnd.get(2, 3)
            if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
                ai.last_social_tick = now;
            }
            if let Some(region) = region_cell_of(world, npc_oid) {
                broadcast_near_region_in(
                    world,
                    region,
                    instance_of(world, npc_oid),
                    &server_packets::social_action(npc_oid, action_id),
                );
            }
        }
    }
    let delay = animation_delay_ticks(world, min_s, max_s);
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.next_animation_tick = Some(now + delay);
    }
}

/// `Rnd.get(min, max) * 1000` ms as ticks (inclusive of `max`).
fn animation_delay_ticks(_world: &mut World, min_s: i32, max_s: i32) -> u64 {
    let secs = rnd::get_range(min_s.max(0), max_s.max(min_s).max(0));
    secs as u64 * TICKS_PER_SECOND
}

/// `ScheduledTask::NpcAttackReady` — the NPC's swing period elapsed. Re-run its
/// think immediately (Java `EVT_READY_TO_ACT`) so a fast attacker keeps swinging
/// at its weapon rate instead of stalling until the next 1 s AI tick.
pub(crate) fn on_npc_attack_ready(world: &mut World, npc_oid: i32) {
    think(world, npc_oid);
}

/// `CtrlEvent.EVT_ARRIVED` → `CreatureAI.onEvtArrived` — an NPC reached its
/// destination this movement tick (100 ms), so it thinks *immediately* instead
/// of waiting out the rest of the 1 s AI period.
///
/// This is what makes a mob swing the moment it closes: `moveToPawn` shortens
/// the chase destination to `range - 5` (`Creature.moveToLocation`'s offset
/// branch), so arrival *is* "in attack range", and Java's `onEvtArrived` ends
/// with `onEvtThink()` → `thinkAttack()` → `doAutoAttack(target)`. Without this
/// hook the mob stands in range for up to a full second before its first hit.
///
/// The caller has already applied the `AI_INTENTION_MOVE_TO` → `ACTIVE` reset
/// that `onEvtArrived` does ahead of the think.
pub(crate) fn on_npc_arrived(world: &mut World, npc_oid: i32) {
    // `AbstractAI.notifyEvent`: "happens e.g. from stopmove but we don't
    // process it if we're casting" — a mob that arrived mid-cast defers to the
    // cast finishing.
    if world.objects.has_component::<Casting>(&npc_oid) {
        return;
    }
    // `AttackableAI.onEvtThink` bails unless the actor's region and its
    // neighbors are active; the 1 s tick pre-filters on that, so an
    // out-of-band think has to test it itself.
    if !region_active(world, npc_oid) {
        return;
    }
    think(world, npc_oid);
}

/// `WorldRegion.areNeighborsActive()`: a region is active exactly while some
/// player's 3×3 block covers it, which is the same test `npc_ai_tick` builds
/// its active set from.
fn region_active(world: &World, npc_oid: i32) -> bool {
    let Some(region) = region_cell_of(world, npc_oid) else {
        return false;
    };
    // Adjacency is symmetric, so "some player's 3×3 block covers me" is the
    // same question as "is any player inside my own 3×3 block" — one index
    // lookup over nine cells instead of a scan of every connected client. This
    // runs per NPC arrival on the 100 ms movement tick.
    world.in_game_players_visible_from(region).next().is_some()
}

mod faction;
mod hate;
mod intentions;
mod movement;
mod perception;
mod tactics;
mod think;

pub(crate) use faction::*;
pub(crate) use hate::*;
pub(crate) use intentions::*;
pub(crate) use movement::*;
pub(crate) use perception::*;
use tactics::*;
pub(crate) use think::*;
/// faction call (`AttackableAI.onEvtAggression`, which calls `setRunning` in as
/// many words) and the minion assist (whose recruits reach `setRunning` via
/// their next `thinkActive`) — set the intention directly and skipped it. A
/// recruit that never flips chases at its **walk** speed, which for most mobs
/// is a fraction of its run speed: the pack answers the call for help and then
/// trails in one at a time, far behind whoever pulled it.
pub(crate) fn set_running(world: &mut World, npc_oid: i32) {
    let flipped = match world.objects.get_component_mut::<Speeds>(&npc_oid) {
        Some(speeds) if !speeds.running => {
            speeds.running = true;
            true
        }
        _ => false,
    };
    if !flipped {
        return;
    }
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    broadcast_near_region_in(
        world,
        region,
        instance_of(world, npc_oid),
        &server_packets::change_move_type(npc_oid, true),
    );
}

/// Back to the scan loop: walking move type + Active intention (Java
/// `setIntention(AI_INTENTION_ACTIVE)` + `setWalking`). `pub(crate)` so the
/// `DeleteHate`/`DeleteHateOfMe` skill effects (`skills/effects.rs`) can
/// disengage a target's AI the same way Java's handlers do.
pub(crate) fn set_active(world: &mut World, npc_oid: i32) {
    let was_running = {
        let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) else {
            return;
        };
        ai.intention = NpcIntention::Active;
        let Some(speeds) = world.objects.get_component_mut::<Speeds>(&npc_oid) else {
            return;
        };
        let was = speeds.running;
        speeds.running = false;
        was
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    if was_running {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::change_move_type(npc_oid, false),
        );
    }
}
