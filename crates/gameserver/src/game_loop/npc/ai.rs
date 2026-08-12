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
use crate::model::movement::{self, MoveData};
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

fn think(world: &mut World, npc_oid: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    if is_dead(world, npc_oid) {
        return;
    }
    // A stunned/asleep/paralyzed mob does nothing at all — Java's `isDisabled()`
    // short-circuits `AttackableAI.onEvtThink`. A *rooted* one still thinks
    // (it can attack an adjacent target); the movement primitives refuse the
    // chase leg on their own.
    if abnormal::is_blocked_from_actions(world, npc_oid) {
        return;
    }
    // GM-controlled mobs run their own state machine (which itself reuses the
    // scan/attack/chase primitives below) rather than the wild AI.
    if let Some(group_id) = world
        .objects
        .get_component::<crate::model::mob_group::Controllable>(&npc_oid)
        .map(|c| c.group_id)
    {
        controllable_think(world, npc_oid, group_id);
        return;
    }
    // Only the Attackable subtree has this AI; the slice narrows to monsters —
    // plus town `Guard`s (Java `Guard extends Attackable`, so they run this same
    // AI; they're what hunts PKs) and stationed siege guards (`Defender`) while
    // their castle's siege runs, which use the same scan/attack/chase to defend
    // against attackers. Both facts are memoized on the `Npc` core — no
    // template lookup on the every-NPC-every-second path.
    if !npc.attackable_ai(world)
        && !(npc.is_defender(world) && siege::active_siege_guard_castle(world, npc_oid).is_some())
    {
        return;
    }
    // A servitor runs `SummonAI`, not `AttackableAI`: it trails its owner
    // instead of scanning for prey, and only fights what its owner points it
    // at. Once ordered, the ordinary attack think below drives it — "attack the
    // most-hated" is the right behaviour once the order has seeded the list.
    if world
        .objects
        .has_component::<crate::model::components::ServitorOf>(&npc_oid)
    {
        servitor::servitor_follow_tick(world, npc_oid);
        if world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| ai.intention == NpcIntention::Attack)
        {
            think_attack(world, npc_oid);
        }
        return;
    }
    let Some(ai) = world.objects.get_component::<NpcAi>(&npc_oid) else {
        return;
    };
    match ai.intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
        // `AttackableAI.onEvtThink`'s switch has no `AI_INTENTION_MOVE_TO`
        // case: a mob committed to a destination walk (today, a feared one)
        // thinks about nothing until it arrives. Without this arm the very
        // next think tick would re-issue a chase and cancel the flight.
        NpcIntention::MoveTo => {}
    }
}

/// How close a `Follow` member stays to its commander before it stops (Java's
/// `MobGroup` follow keeps ~offset spacing; a single range is enough here).
const FOLLOW_RANGE: f64 = 150.0;

/// Drive one GM-controlled mob per its group's [`MobGroupState`], reusing the
/// wild AI's scan/attack/chase (`think_active`/`think_attack`) for the combat
/// states and a plain walk for follow/return. Java's `ControllableMobAI` is a
/// parallel state machine; this collapses it onto the existing primitives.
fn controllable_think(world: &mut World, npc_oid: i32, group_id: i32) {
    use crate::model::mob_group::MobGroupState;
    let Some(state) = world.mob_groups.get(&group_id).map(|g| g.state) else {
        return;
    };
    match state {
        MobGroupState::Idle | MobGroupState::NoMove => {
            stop_npc(world, npc_oid);
            if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
                aggro.0.clear();
            }
        }
        MobGroupState::Random => {
            // The wild aggressive AI: same dispatch the non-controllable path runs.
            match world
                .objects
                .get_component::<NpcAi>(&npc_oid)
                .map(|ai| ai.intention)
            {
                Some(NpcIntention::Attack) => think_attack(world, npc_oid),
                _ => think_active(world, npc_oid),
            }
        }
        MobGroupState::Attack(target) | MobGroupState::Cast(target) => {
            seed_attack(world, npc_oid, target);
        }
        MobGroupState::AttackGroup(other) => {
            let victim = nearest_group_member(world, npc_oid, other);
            if let Some(v) = victim {
                seed_attack(world, npc_oid, v);
            } else {
                stop_npc(world, npc_oid);
            }
        }
        MobGroupState::Follow(commander) => {
            let Some((cx, cy, cz)) = pos_of(world, commander) else {
                return;
            };
            let dist = distance_2d(world, npc_oid, cx, cy);
            if dist > FOLLOW_RANGE && world.objects.get_component::<Movement>(&npc_oid).is_none() {
                move_npc_to(world, npc_oid, cx, cy, cz);
            } else if dist <= FOLLOW_RANGE {
                stop_npc(world, npc_oid);
            }
        }
        MobGroupState::Return(commander) => {
            if let Some((cx, cy, cz)) = pos_of(world, commander)
                && world.objects.get_component::<Movement>(&npc_oid).is_none()
            {
                move_npc_to(world, npc_oid, cx, cy, cz);
            }
        }
    }
}

/// Make the mob attack `target`: seed dominant hate and enter the attack loop
/// (reuses `think_attack`, so chase + swing are the wild AI's).
pub(crate) fn seed_attack(world: &mut World, npc_oid: i32, target: i32) {
    let target_alive = world
        .objects
        .get_component::<Vitals>(&target)
        .is_some_and(|v| !v.dead);
    if !target_alive {
        stop_npc(world, npc_oid);
        return;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.entry(target).or_default().hate = 1_000_000.0;
    }
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = u64::MAX; // commanded attacks don't time out
    }
    think_attack(world, npc_oid);
}

/// The nearest live member of `group_id` to `npc_oid` (for `//mobgroup_attackgrp`).
fn nearest_group_member(world: &World, npc_oid: i32, group_id: i32) -> Option<i32> {
    let (nx, ny, _) = pos_of(world, npc_oid)?;
    world.mob_groups.get(&group_id).and_then(|g| {
        g.members
            .iter()
            .filter(|&&m| {
                world
                    .objects
                    .get_component::<Vitals>(&m)
                    .is_some_and(|v| !v.dead)
            })
            .min_by_key(|&&m| {
                pos_of(world, m)
                    .map(|(x, y, _)| ((x - nx) as i64).pow(2) + ((y - ny) as i64).pow(2))
                    .unwrap_or(i64::MAX)
            })
            .copied()
    })
}

fn distance_2d(world: &World, oid: i32, x: i32, y: i32) -> f64 {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (((p.x - x) as f64).powi(2) + ((p.y - y) as f64).powi(2)).sqrt())
        .unwrap_or(f64::MAX)
}

/// Stop a mob dead (remove its move, broadcast `StopMove`) — the NPC half of
/// `AbstractAI.clientStopMoving(null)`.
pub(crate) fn stop_npc(world: &mut World, npc_oid: i32) {
    if !world.objects.has_component::<Movement>(&npc_oid) {
        return;
    }
    world.objects.remove_component::<Movement>(&npc_oid);
    if let (Some(pos), Some(region)) = (
        maybe_position(world, npc_oid),
        region_cell_of(world, npc_oid),
    ) {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }
}

/// `AttackableAI.setGlobalAggro(-25)`: the longer calm window Java drops a mob
/// into when it stops hating *everyone* — roughly 25 think seconds during which
/// [`think_active`] neither seeds hate from its aggro scan nor acts on hate it
/// is handed. Distinct from the −10 a fresh [`NpcAi`] carries out of spawn.
const CALM_GLOBAL_AGGRO: i32 = -25;

/// The tail Java runs when a mob stops hating everyone — `setGlobalAggro(-25)`
/// + `clearAggroList()` + `setWalking()` + `setIntention(ACTIVE)`. It appears
/// in `Attackable.setTarget(null)` (`Attackable.java` 1861-1881), which is what
/// [`on_forget_object`] ports, and in the three `Attackable.reduceHate`
/// branches (873-919) — which nothing on this chronicle reaches, so that caller
/// is left unwired on purpose: `AddHate` double-negates its way into *raising*
/// hate (see the note in `skills::effects`) and `TransferHate` (skill 489,
/// Shift Target) is off-chronicle here.
///
/// Without this a mob whose last hated player vanished re-seeds hate from the
/// very next scan tick and re-aggros instantly; Java stands it down for ~25 s.
pub(crate) fn go_calm(world: &mut World, npc_oid: i32) {
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.global_aggro = CALM_GLOBAL_AGGRO;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.clear();
    }
    // `setWalking()` + `setIntention(AI_INTENTION_ACTIVE)`.
    set_active(world, npc_oid);
}

/// `AggroInfo.checkHate`, run across the aggro list before every most-hated
/// pick (Java runs it per-entry inside `Attackable.getMostHated`): hate
/// silently zeroes for an attacker who is dead, despawned, or no longer
/// inside the NPC's 3×3 surrounding regions. The entry survives — only its
/// weight drops — and this is what actually makes a mob forget a target that
/// left the neighbourhood; without it a hated player stays "most hated"
/// forever and the mob chases across the world.
fn check_hate(world: &mut World, npc_oid: i32) {
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let Some(aggro) = world.objects.get_component::<AggroList>(&npc_oid) else {
        return;
    };
    let hated: Vec<i32> = aggro
        .0
        .iter()
        .filter(|(_, info)| info.hate > 0.0)
        .map(|(&id, _)| id)
        .collect();
    let mut expired: Vec<i32> = Vec::new();
    for id in hated {
        let alive_nearby = world
            .objects
            .get_component::<Vitals>(&id)
            .is_some_and(|v| !v.dead)
            && world
                .objects
                .get_component::<RegionCell>(&id)
                .is_some_and(|r| regions_adjacent(region, r.0));
        if !alive_nearby {
            expired.push(id);
        }
    }
    if expired.is_empty() {
        return;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        for id in expired {
            if let Some(info) = aggro.0.get_mut(&id) {
                info.hate = 0.0;
            }
        }
    }
}

/// Java `CreatureAI.onEvtForgetObject` narrowed to an `Attackable` whose
/// *current target* is the departing object, i.e. `Attackable.setTarget(null)`
/// (`Attackable.java` 1861-1881): drop the target's aggro entry outright, and
/// if that emptied the list send the mob into the −25 calm window.
///
/// Fired from the visibility layer — the object leaving the NPC's 3×3 block or
/// the world — because that is where Java raises it (`World.switchRegion` /
/// `removeVisibleObject`), and doing it there rather than lazily at think time
/// matters twice over: it is an *edge*, so an object that was never nearby (a
/// script seeding a grudge across the map) can't trigger it; and it still fires
/// when the departure leaves the mob's region with no players in it, which
/// stops the AI thinking at all.
///
/// NPCs here hold no `TargetRef` — the aggro list *is* the target, and Java
/// only ever assigns one in `thinkAttack` from `getMostHated` — so the
/// most-hated stands in for `getTarget()`.
pub(crate) fn on_forget_object(world: &mut World, npc_oid: i32, object_id: i32) {
    let Some(aggro) = world.objects.get_component::<AggroList>(&npc_oid) else {
        return;
    };
    if aggro.most_hated() != Some(object_id) {
        return;
    }
    let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) else {
        return;
    };
    // `if (target != null) _aggroList.remove(target);`
    aggro.0.remove(&object_id);
    // `if (_aggroList.isEmpty())` — literally empty, as in Java: a zeroed entry
    // left behind for some other attacker keeps the mob out of the calm window.
    if aggro.0.is_empty() {
        go_calm(world, npc_oid);
    }
}

/// `AttackableAI.thinkActive`: tick `_globalAggro` toward 0, scan the aggro
/// range, pick the most hated, or drift back home.
fn think_active(world: &mut World, npc_oid: i32) {
    let (aggressive, aggro_range) = {
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .expect("caller checked")
            .npc_id;
        let ai = world
            .objects
            .get_component_mut::<NpcAi>(&npc_oid)
            .expect("caller checked");
        if ai.global_aggro != 0 {
            ai.global_aggro += if ai.global_aggro < 0 { 1 } else { -1 };
        }
        let t = world.data.npc_data.get(npc_id);
        // Java `npc.isAggressive()`: the explicit flag, not aggroRange —
        // nearly every passive mob in the datapack has an aggroRange too.
        //
        // `is_monster()` is the other half of the gate and it is load-bearing.
        // Java runs this scan for `isAggressive() || instanceof Guard`, but every
        // candidate then has to clear `isAggressiveTowards` → `isAutoAttackable`,
        // and `Player.isAutoAttackable` only returns true for an NPC attacker via
        // `attacker.isMonster()` — a `Guard` is an `Attackable`, not a `Monster`,
        // so it falls through the playable branches and returns false. The only
        // thing that makes a guard aggro a player is the `reputation < 0`
        // early-return in `isAggressiveTowards`, handled by `guard_aggro_scan`.
        // Town guards *are* `isAggressive="true"` in the datapack (all 186 of
        // them), so without this check every guard seeds hate on every lawful
        // player inside its 450-unit aggroRange and murders them.
        // `AttackableAI.autoAttackCondition`'s last gate before the
        // auto-attackable/line-of-sight test: `if (me.isChampion() &&
        // Config.CHAMPION_PASSIVE) return false`. With `ChampionPassive = True`
        // on this dist, a champion never seeds hate from the scan — it stands
        // where it spawned until something hits it, which is what stops a 10×-HP
        // mob from ambushing a passer-by.
        let champion_passive = world.cfg.champion.enable
            && world.cfg.champion.passive
            && world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_oid)
                .is_some_and(|n| n.champion);
        (
            t.map(|t| t.is_monster() && t.is_aggressive && t.aggro_range > 0)
                .unwrap_or(false)
                && !champion_passive
                // Java `Monster.isAggressive()`'s second term: a monster under
                // the `PASSIVE` flag (Veil 106, Requiem 1049) stops aggroing
                // whatever its template says (G34 S3).
                && !abnormal::is_pacified(world, npc_oid),
            t.map(|t| t.aggro_range).unwrap_or(0),
        )
    };
    let Some(region) = region_cell_of(world, npc_oid) else {
        return;
    };

    // `thinkActive` reads `_globalAggro` once, after the tick above, and wraps
    // *both* the aggro scan and the most-hated/attack decision in the same
    // `if (_globalAggro >= 0)`. While the counter is negative the mob is calm:
    // it neither seeds hate from proximity nor acts on hate it already carries
    // — the latter matters because hate can arrive without ever clearing the
    // counter (a faction call, a minion's master relaying, a script seeding the
    // list), and with only the scan gated a mob holding >10 hate would charge
    // straight out of its calm window. Java's `return` also sits *inside* that
    // block, so a calm mob falls through to the idle branches below and keeps
    // drifting/random-walking home rather than standing frozen over its list.
    let global_aggro = world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .map(|ai| ai.global_aggro)
        .unwrap_or(0);

    // Aggro-range scan (`isAggressiveTowards` narrowed: alive, in range,
    // geodata-visible; invisibility/silent-move/GM states don't exist).
    if aggressive && global_aggro >= 0 {
        let (nx, ny, nz) = {
            let pos = position(world, npc_oid);
            (pos.x, pos.y, pos.z)
        };
        let mut in_range = players_in_range_los(world, region, nx, ny, nz, aggro_range as f64);
        // Stealth / fake death (`isAggressiveTowards`).
        in_range.retain(|&pid| notices_target(world, npc_oid, pid));
        let mut newly_seen: Vec<i32> = Vec::new();
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            for player_oid in in_range {
                // `addDamageHate(t, 0, 0)` → first sight seeds 1 hate.
                let entry = aggro.0.entry(player_oid).or_default();
                if entry.hate == 0.0 {
                    entry.hate = 1.0;
                    newly_seen.push(player_oid);
                }
            }
        }
        // `onAggroRangeEnter` for the scripts that registered this monster
        // (the Primeval Isle Tyrannosaurus's curiosity pause).
        if !newly_seen.is_empty() {
            let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
            for player_oid in newly_seen {
                crate::game_loop::quests::notify_aggro_range_enter(
                    world, npc_oid, npc_id, player_oid,
                );
            }
        }
    }

    // Town guards hunt PKs (`isAggressiveTowards`, the `me instanceof Guard`
    // branch): a guard aggros a player with **negative reputation** inside a
    // *hardcoded* 500 units — Java uses the literal, not the template's
    // `aggroRange` (which is 450 on the stock guards), and does it regardless of
    // the `isAggressive` flag. A lawful player is ignored, which is what makes
    // this a PK-hunting rule rather than general aggression — and it is the
    // *only* way a guard aggros a player, since the generic scan above is
    // monster-only (see the `is_monster()` note there).
    if world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .is_some_and(|n| n.is_guard(world))
    {
        guard_aggro_scan(world, npc_oid, region);
    }

    // Siege guards (`Defender`) defend the castle: they aggro their employer's
    // enemies within aggro range regardless of the `isAggressive` flag (Java
    // `SiegeGuardAI` — the guard's own aggro scan). Reuses the hate → attack →
    // chase machinery below. `aggro_range` comes from the template (1000 for the
    // stock guards); the enemy filter (anyone but a defender of this castle) is
    // `attackable_siege_guard`.
    if aggro_range > 0
        && let Some(_castle) = siege::active_siege_guard_castle(world, npc_oid)
    {
        let (nx, ny, nz) = {
            let pos = position(world, npc_oid);
            (pos.x, pos.y, pos.z)
        };
        let mut in_range = players_in_range_los(world, region, nx, ny, nz, aggro_range as f64);
        // Keep only actual enemies (attackers / non-defenders).
        in_range.retain(|&pid| siege::attackable_siege_guard(world, npc_oid, pid));
        in_range.retain(|&pid| notices_target(world, npc_oid, pid));
        set_hate_for(world, npc_oid, in_range);
    }

    // Chose a target from the aggro list (`getMostHated`, after the
    // per-entry `checkHate` liveness/region test). Inside the calm gate, as in
    // Java — `return` included.
    if global_aggro >= 0 {
        check_hate(world, npc_oid);
        let hated = world
            .objects
            .get_component::<AggroList>(&npc_oid)
            .and_then(AggroList::most_hated);
        if let Some(target) = hated {
            let aggro_list = world
                .objects
                .get_component::<AggroList>(&npc_oid)
                .expect("checked");
            let aggro = aggro_list.0.get(&target).map(|a| a.hate).unwrap_or(0.0);
            if aggro + global_aggro as f64 > 0.0 {
                let became_running = {
                    let ai = world
                        .objects
                        .get_component_mut::<NpcAi>(&npc_oid)
                        .expect("checked");
                    ai.intention = NpcIntention::Attack;
                    ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
                    let speeds = world
                        .objects
                        .get_component_mut::<Speeds>(&npc_oid)
                        .expect("checked");
                    let flip = !speeds.running;
                    speeds.running = true;
                    flip
                };
                if became_running {
                    broadcast_near_region_in(
                        world,
                        region,
                        instance_of(world, npc_oid),
                        &server_packets::change_move_type(npc_oid, true),
                    );
                }
            }
            return;
        }
    }

    // No target: either return to the spawn anchor when drifted too far
    // (`Config.MAX_DRIFT_RANGE`), or — while inside that radius — take an
    // occasional random walk (`AttackableAI.thinkActive`'s two idle branches).
    let max_drift = world.cfg.npc.max_drift_range as f64;
    let (x, y, z, spawn, moving, can_move, random_walk) = {
        let npc = &world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .expect("npc");
        let pos = world
            .objects
            .get_component::<Position>(&npc_oid)
            .expect("caller checked");
        let t = npc.template(world);
        let can_move = t.map(|t| t.can_move).unwrap_or(false);
        // Java `isRandomWalkingEnabled()`: the template flag (minions/walking-
        // route targets that clear it at runtime aren't in the monster slice).
        let random_walk = t.map(|t| t.random_walk).unwrap_or(false);
        // `ai/others/Spawns/NoRandomActivity` can clear it per NPC.
        let random_walk = spawn_scripts::random_walk_enabled(world, npc_oid, random_walk);
        (
            pos.x,
            pos.y,
            pos.z,
            npc.spawn_loc,
            world.objects.has_component::<Movement>(&npc_oid),
            can_move,
            random_walk,
        )
    };
    if !can_move || moving {
        return;
    }
    let dist_sq = ((spawn.0 - x) as f64).powi(2) + ((spawn.1 - y) as f64).powi(2);
    if dist_sq > max_drift * max_drift {
        // Drifted out of range with nothing to chase: walk back home.
        move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
    } else if random_walk && world.roll(RANDOM_WALK_RATE) == 0 {
        random_walk_move(world, npc_oid, (x, y, z), spawn);
    }
}

/// `AttackableAI.thinkActive`'s random-walk branch: pick a point within
/// `MAX_DRIFT_RANGE` of the spawn anchor, geo-clamp the straight line to it,
/// and walk there — but only if the clamped spot is still within drift range.
fn random_walk_move(world: &mut World, npc_oid: i32, cur: (i32, i32, i32), spawn: (i32, i32, i32)) {
    let drift = world.cfg.npc.max_drift_range;
    // Java: deltaX ∈ [0, 2·drift); deltaY ∈ [deltaX, 2·drift] (Rnd.get(min,max)
    // is inclusive of max); then deltaY = √(deltaY² − deltaX²) so the offset
    // lands on a quarter arc of the drift circle around the spawn point.
    let delta_x = world.roll(drift * 2);
    let delta_y = delta_x + world.roll(drift * 2 - delta_x + 1);
    let delta_y = (((delta_y as f64).powi(2) - (delta_x as f64).powi(2)).max(0.0)).sqrt() as i32;
    let x1 = (delta_x + spawn.0) - drift;
    let y1 = (delta_y + spawn.1) - drift;
    let z1 = cur.2; // Java uses the NPC's current z, not the spawn z.

    let (vx, vy, vz) = world
        .geo
        .get_valid_location(cur.0, cur.1, cur.2, x1, y1, z1);
    // `Util.calculateDistance(spawn, moveLoc) <= MAX_DRIFT_RANGE`.
    let from_spawn_sq = ((vx - spawn.0) as f64).powi(2) + ((vy - spawn.1) as f64).powi(2);
    if from_spawn_sq <= (drift as f64) * (drift as f64) {
        move_npc_to(world, npc_oid, vx, vy, vz);
    }
}

/// `AttackableAI.thinkAttack`: validate the hated target, time out, chase,
/// swing.
fn think_attack(world: &mut World, npc_oid: i32) {
    let now = world.tick;

    // `thinkAttack`'s very first line: `if ((npc == null) || npc.isCastingNow())
    // return;`. A mob mid-cast does nothing else — no faction call, no chase,
    // no swing — until the cast resolves. It went missing twice over, and each
    // time a different tail of the think ran anyway: the 1 s think landing
    // inside a 2 s cast fell through to the **swing** tail and the mob attacked
    // while casting, and it fell through to the **range** tail and re-issued
    // `chase()` every second, so the mob sprinted at its target with the cast
    // bar still up. Note `try_cast` above does refuse a second concurrent cast,
    // but it reports that as `false` = "no cast this think", which is exactly
    // what lets the caller carry on into both tails.
    if world.objects.has_component::<Casting>(&npc_oid) {
        return;
    }

    // Chase leash (`AttackableAI.thinkAttack` `AGGRO_DISTANCE_CHECK`): a monster
    // dragged farther than the configured range from its spawn drops all aggro,
    // heals to full and teleports home with its escort. On (2000/4000 units) on
    // this dist. Guards/defenders, route walkers and grand bosses are exempt.
    if world.cfg.npc.aggro_distance_check_enabled && npc_leash_return_home(world, npc_oid) {
        return;
    }

    check_hate(world, npc_oid);
    let target = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(AggroList::most_hated);
    let Some(target_oid) = target else {
        set_active(world, npc_oid);
        return;
    };

    // Target dead or gone → stop hating it (next think re-evaluates).
    let target_alive = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_some_and(|v| !v.dead);
    if !target_alive {
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            aggro.0.remove(&target_oid);
        }
        return;
    }

    // Attack timeout (`thinkAttack`): give up the hunt — back to the scan
    // loop at walking speed. Java does *not* clear the aggro list here (the
    // `checkHate` region test is what ultimately forgets a vanished target);
    // instead a monster still mid-combat — or one nobody is left watching —
    // teleports straight back to its spawn
    // (`npc.teleToLocation(npc.getSpawn(), false)`).
    if world
        .objects
        .get_component::<NpcAi>(&npc_oid)
        .is_some_and(|ai| ai.attack_timeout_tick < now)
    {
        set_active(world, npc_oid);
        let Some(npc) = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
        else {
            return;
        };
        let spawn = npc.spawn_loc;
        let is_monster = npc.template(world).is_some_and(|t| t.is_monster());
        // `npc.isInCombat()` = `AttackStanceTaskManager.hasAttackStanceTask`.
        // Being *attacked* re-arms the attack timeout, so at this point only
        // the mob's own recent swings can still hold the stance.
        let in_combat = combat::has_attack_stance(world, npc_oid);
        let players_visible = {
            let Some(region) = region_cell_of(world, npc_oid) else {
                return;
            };
            // Index-derived (≤9 cells); like the sweep it replaced, unattended
            // shops count — a mob doesn't teleport home over an offline store.
            world.players_visible_from(region).next().is_some()
        };
        if is_monster && instance_of(world, npc_oid) == 0 && (in_combat || !players_visible) {
            let heading = world
                .objects
                .get_component::<Position>(&npc_oid)
                .map(|p| p.heading)
                .unwrap_or(0);
            death::relocate_npc(world, npc_oid, spawn.0, spawn.1, spawn.2, heading);
        }
        return;
    }

    // Mid-swing. In Java this is **not** a `thinkAttack` gate: the guard lives
    // in `Creature.doAutoAttack` (`isAttackDisabled()` = `isAttackingNow() ||
    // isDisabled()`), so a mob whose swing is still winding down keeps
    // thinking — it calls its faction, walks, and above all *casts*; only the
    // next swing is refused.
    //
    // Returning here instead cost the cast ladder most of its rolls. The AI
    // thinks once a second, plus once at each swing's end
    // (`ScheduledTask::NpcAttackReady`), but with this gate on top every
    // periodic think inside the swing window died before the ladder — leaving
    // exactly one `hasSkillChance()` roll per swing. At Porta's (20213) 253
    // atk. spd. that is one roll per ~2 s against Java's one per second, and
    // since the roll is only ~11 %, opportunities came ~18 s apart while its
    // Stun (4073) cooled down in 6 — so the SHORT_RANGE rung always had Stun
    // ready and the GENERAL rung that holds Summon (4161) was never reached.
    // Measured over 300 s of melee: 11 stuns, 0 summons.
    let mid_swing = world
        .objects
        .get_component::<AttackState>(&npc_oid)
        .is_some_and(|st| st.attack_end_tick > now);

    // "Actor should be able to see target" (`thinkAttack`'s geodata gate): a
    // sight line cut by a wall or a tower floor means no faction call, no
    // cast, no swing and — crucially — no straight-line chase. Java issues
    // `moveTo(target)`, an ordinary geo-validated walk that clamps at the
    // last walkable cell and falls back to the path worker (the stairs
    // route), then returns. Without this gate a mob whose hated target
    // climbed to another level engages straight through the geometry.
    {
        let (Some(npos), Some(tpos)) = (
            maybe_position(world, npc_oid),
            maybe_position(world, target_oid),
        ) else {
            return;
        };
        if !world
            .geo
            .can_see_target(npos.x, npos.y, npos.z, tpos.x, tpos.y, tpos.z)
        {
            let can_move = npc_template(world, npc_oid).is_some_and(|t| t.can_move);
            if can_move {
                move_npc_to(world, npc_oid, tpos.x, tpos.y, tpos.z);
            }
            return;
        }
    }

    // Call the faction for help before anything else this think (Java runs the
    // block right after the geodata check, ahead of the cast ladder).
    faction_call(world, npc_oid, target_oid);

    // The three movement blocks Java runs *between* the faction call and the
    // cast ladder. Each one ends the think when it fires.
    if shuffle_off_a_stacked_mob(world, npc_oid, target_oid) {
        return;
    }
    if archer_backs_off(world, npc_oid, target_oid) {
        return;
    }
    // Raid/minion target chaos can swap the target out from under the rest of
    // this think, so it is re-read afterwards rather than reusing `target_oid`.
    if raid_target_chaos(world, npc_oid) {
        return;
    }

    // Cast before closing distance — Java's "Cast skills" block sits between
    // the target checks and the range/move tail, so a caster that launched a
    // spell this think neither chases nor swings.
    if super::cast::try_cast(world, npc_oid, target_oid) {
        return;
    }

    let Some(attacker) = combat::combatant(world, npc_oid) else {
        return;
    };
    let Some(victim) = combat::combatant(world, target_oid) else {
        return;
    };
    // `int range = npc.getPhysicalAttackRange() + combinedCollision; if
    // (getAiType() == ARCHER) range = 850 + combinedCollision;` — an archer
    // mob's *engagement* range is the flat bow range, not its template
    // `<attack range>` (40 on most of them). Without the override all 220
    // ARCHER templates on this dist walked into melee before shooting.
    let combined_collision = attacker.collision_radius + victim.collision_radius;
    let reach = if ai_type_of(world, npc_oid) == AiType::Archer {
        NPC_BOW_RANGE as f64 + combined_collision
    } else {
        attacker.atk_range as f64 + combined_collision
    };
    let dist_sq =
        ((victim.x - attacker.x) as f64).powi(2) + ((victim.y - attacker.y) as f64).powi(2);

    let can_move = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .expect("npc")
        .template(world)
        .map(|t| t.can_move)
        .unwrap_or(false);
    // Out of range: close, or — when the target has become unreachable — pick
    // another. `checkTarget` only fails for a dead target or an *immobilised*
    // mob that can't close (Java gates its range/LOS test on
    // `npc.isMovementDisabled()`), which is precisely the case that used to
    // leave a rooted mob standing still forever instead of switching to
    // whoever else was hitting it.
    //
    // Java then falls straight through to `doAutoAttack` with the new pick,
    // without re-testing the range — so does this.
    let mut target_oid = target_oid;
    if dist_sq > reach * reach {
        if can_move && check_target(world, npc_oid, target_oid) {
            chase(world, npc_oid, target_oid, reach);
            return;
        }
        match target_reconsider(world, npc_oid) {
            Some(t) => target_oid = t,
            None => return,
        }
    }

    // `Creature.doAutoAttack`'s `isAttackDisabled()` refusal — the swing that
    // is still running blocks the next one, and nothing else.
    if mid_swing {
        return;
    }

    // In reach: stop and swing.
    if world.objects.has_component::<Movement>(&npc_oid) {
        world.objects.remove_component::<Movement>(&npc_oid);
        let (Some(pos), Some(region)) = (
            maybe_position(world, npc_oid),
            region_cell_of(world, npc_oid),
        ) else {
            return;
        };
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }
    combat::do_auto_attack(world, npc_oid, target_oid);
}

/// `npc.getAiType()`, defaulting to `FIGHTER` for a template we can't read.
fn ai_type_of(world: &World, npc_oid: i32) -> AiType {
    npc_template(world, npc_oid)
        .map(|t| t.ai_type)
        .unwrap_or(AiType::Fighter)
}

/// `Creature.isMovementDisabled()` for a monster: the abnormal states that pin
/// it (root/stun/sleep/paralysis) *or* a template that cannot move at all.
fn movement_disabled(world: &World, npc_oid: i32) -> bool {
    abnormal::is_movement_disabled(world, npc_oid)
        || !npc_template(world, npc_oid).is_some_and(|t| t.can_move)
}

/// `thinkAttack`'s "In case many mobs are trying to hit from same place, move a
/// bit, circling around the target" block.
///
/// A 3-in-100 roll per think, and only when another `Attackable` is standing
/// inside this mob's own collision radius: step to a fresh spot roughly
/// `combinedCollision + Rnd(40)` off the *target* on each axis, sign chosen at
/// random, geo-validated. It is what stops a pack from stacking into one pixel
/// while they all beat on the same player. Returns whether the think ends here
/// — Java `return`s whenever it found a crowding neighbour, **even if the
/// chosen spot was rejected**.
fn shuffle_off_a_stacked_mob(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    if movement_disabled(world, npc_oid) || world.roll(100) > 3 {
        return false;
    }
    let (Some(me), Some(target)) = (
        combat::combatant(world, npc_oid),
        combat::combatant(world, target_oid),
    ) else {
        return false;
    };
    let collision = me.collision_radius;
    let combined = collision + target.collision_radius;

    let Some(region) = region_cell_of(world, npc_oid) else {
        return false;
    };
    let crowder = world
        .npcs_visible_from(region)
        .into_iter()
        .filter(|&other| other != npc_oid && other != target_oid)
        .filter(|&other| {
            world
                .objects
                .get_component::<Vitals>(&other)
                .is_some_and(|v| !v.dead)
        })
        .find(|&other| {
            world
                .objects
                .get_component::<Position>(&other)
                .is_some_and(|p| {
                    let (dx, dy) = ((p.x - me.x) as f64, (p.y - me.y) as f64);
                    dx * dx + dy * dy <= collision * collision
                })
        });
    if crowder.is_none() {
        return false;
    }

    // `newX = combinedCollision + Rnd.get(40)`, then added to or subtracted
    // from the *target's* coordinate on a coin flip — per axis, so the mob can
    // end up on any of the four diagonals around whoever it is hitting.
    let (dx_step, dy_step) = (
        combined as i32 + world.roll(40),
        combined as i32 + world.roll(40),
    );
    let (flip_x, flip_y) = (world.roll(2) == 0, world.roll(2) == 0);
    let new_x = if flip_x {
        target.x + dx_step
    } else {
        target.x - dx_step
    };
    let new_y = if flip_y {
        target.y + dy_step
    } else {
        target.y - dy_step
    };
    // `if (!npc.isInsideRadius2D(newX, newY, 0, collision))` — don't bother
    // shuffling onto the spot we already occupy.
    let (dx, dy) = ((new_x - me.x) as f64, (new_y - me.y) as f64);
    if dx * dx + dy * dy > collision * collision {
        let new_z = me.z + 30;
        let (vx, vy, vz) = world
            .geo
            .get_valid_location(me.x, me.y, me.z, new_x, new_y, new_z);
        move_npc_to(world, npc_oid, vx, vy, vz);
    }
    true
}

/// `thinkAttack`'s "Calculate Archer movement" block: an `ARCHER` mob that has
/// been closed to inside `60 + combinedCollision` backs off 300 units on each
/// axis, away from its target, on a 15-in-100 roll — but only if the geodata
/// says it can actually walk there (`canMoveToTarget`, not `canSeeTarget`).
///
/// This is the kiting that makes bow mobs feel different from melee ones.
/// Returns whether the think ends here; Java returns as soon as the mob is
/// inside the trigger distance, whether or not the retreat was walkable.
fn archer_backs_off(world: &mut World, npc_oid: i32, target_oid: i32) -> bool {
    if movement_disabled(world, npc_oid)
        || ai_type_of(world, npc_oid) != AiType::Archer
        || world.roll(100) >= 15
    {
        return false;
    }
    let (Some(me), Some(target)) = (
        combat::combatant(world, npc_oid),
        combat::combatant(world, target_oid),
    ) else {
        return false;
    };
    let combined = me.collision_radius + target.collision_radius;
    let (dx, dy) = ((target.x - me.x) as f64, (target.y - me.y) as f64);
    if dx * dx + dy * dy > (60.0 + combined) * (60.0 + combined) {
        return false;
    }

    // Straight away from the target on each axis, 300 units.
    let pos_x = if target.x < me.x {
        me.x + 300
    } else {
        me.x - 300
    };
    let pos_y = if target.y < me.y {
        me.y + 300
    } else {
        me.y - 300
    };
    let pos_z = me.z + 30;
    if world
        .geo
        .can_move_to_target(me.x, me.y, me.z, pos_x, pos_y, pos_z)
    {
        move_npc_to(world, npc_oid, pos_x, pos_y, pos_z);
    }
    true
}

/// `thinkAttack`'s "BOSS/Raid Minion Target Reconsider" block — the chaos
/// timer that makes a raid stop tunnelling its tank and lunge at someone else.
///
/// The chance climbs as the boss loses HP, on three different curves, and each
/// tier only starts rolling once `chaostime` has ticked past its config gate
/// (`RaidChaosTime`/`GrandChaosTime`/`MinionChaosTime`, all 10 on this dist —
/// i.e. ten thinks, ten seconds). A successful swap resets the counter and ends
/// the think. Returns whether the think ends here.
fn raid_target_chaos(world: &mut World, npc_oid: i32) -> bool {
    let Some(template) = npc_template(world, npc_oid) else {
        return false;
    };
    let (is_raid, is_grand) = (template.is_raid(), template.type_name == "GrandBoss");
    let is_minion = world.objects.has_component::<MinionOf>(&npc_oid);
    if !is_raid && !is_grand && !is_minion {
        return false;
    }

    let chaos_time = {
        let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) else {
            return false;
        };
        ai.chaos_time += 1;
        ai.chaos_time
    };
    let hp_fraction = hp_fraction(world, npc_oid).unwrap_or(1.0);

    let cfg = &world.cfg.npc;
    // Java's ladder: GrandBoss first only because `instanceof RaidBoss` is
    // checked first and a GrandBoss is not a RaidBoss; the three arms are
    // mutually exclusive.
    let change = if is_grand && chaos_time > cfg.grand_chaos_time {
        let chaos_rate = 100.0 - hp_fraction * 300.0;
        (chaos_rate <= 10.0 && world.roll(100) <= 10)
            || (chaos_rate > 10.0 && (world.roll(100) as f64) <= chaos_rate)
    } else if is_raid && chaos_time > cfg.raid_chaos_time {
        // `hasMinions() ? 200 : 100` — a boss with an escort shuffles sooner.
        let multiplier = if world
            .objects
            .get_component::<Minions>(&npc_oid)
            .is_some_and(|m| !m.0.is_empty())
        {
            200.0
        } else {
            100.0
        };
        (world.roll(100) as f64) <= 100.0 - hp_fraction * multiplier
    } else if is_minion && chaos_time > cfg.minion_chaos_time {
        (world.roll(100) as f64) <= 100.0 - hp_fraction * 200.0
    } else {
        return false;
    };
    if !change {
        return false;
    }

    // `targetReconsider(true)` — a *random* valid attacker rather than the
    // most hated one. That randomness is the whole mechanic.
    let Some(new_target) = target_reconsider_random(world, npc_oid) else {
        return false;
    };
    // Java `setTarget(target); chaostime = 0; return;` — the swap is expressed
    // here by making the new pick dominant in the aggro list, since an NPC's
    // "target" in this port *is* its most-hated entry.
    let top = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| {
            a.0.values()
                .map(|i| i.hate)
                .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
        })
        .unwrap_or(0.0);
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.entry(new_target).or_default().hate = top + 1.0;
    }
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.chaos_time = 0;
    }
    true
}

/// `AttackableAI.checkTarget` — is this still something worth walking to?
///
/// Alive, and (for an **immobilised** mob only) inside physical attack reach
/// with line of sight, and auto-attackable. The `isMovementDisabled()` gate is
/// load-bearing: a mob that *can* move is allowed to chase a target it cannot
/// currently see, which is what lets it walk around a corner after you.
fn check_target(world: &World, npc_oid: i32, target_oid: i32) -> bool {
    if is_dead(world, target_oid) {
        return false;
    }
    if movement_disabled(world, npc_oid) {
        let (Some(me), Some(target)) = (
            combat::combatant(world, npc_oid),
            combat::combatant(world, target_oid),
        ) else {
            return false;
        };
        let reach = me.atk_range as f64 + me.collision_radius + target.collision_radius;
        let (dx, dy) = ((target.x - me.x) as f64, (target.y - me.y) as f64);
        if dx * dx + dy * dy > reach * reach {
            return false;
        }
        if !world
            .geo
            .can_see_target(me.x, me.y, me.z, target.x, target.y, target.z)
        {
            return false;
        }
    }
    target::is_auto_attackable(world, npc_oid, target_oid)
}

/// `AttackableAI.targetReconsider(false)` — the most hated attacker that still
/// passes [`check_target`], falling back to the first valid creature inside the
/// aggro range when the mob is aggressive and its whole list has gone stale.
fn target_reconsider(world: &mut World, npc_oid: i32) -> Option<i32> {
    let candidates: Vec<(i32, f64)> = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .map(|a| a.0.iter().map(|(&oid, i)| (oid, i.hate)).collect())
        .unwrap_or_default();
    let best = candidates
        .iter()
        .filter(|&&(oid, _)| check_target(world, npc_oid, oid))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|&(oid, _)| oid);
    if best.is_some() {
        return best;
    }
    aggro_range_candidates(world, npc_oid)
        .into_iter()
        .find(|&oid| check_target(world, npc_oid, oid))
}

/// `AttackableAI.targetReconsider(true)` — any valid attacker at random, plus
/// (for an aggressive mob) anyone standing inside the aggro range.
fn target_reconsider_random(world: &mut World, npc_oid: i32) -> Option<i32> {
    let mut valid: Vec<i32> = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .map(|a| a.0.keys().copied().collect())
        .unwrap_or_default();
    valid.extend(aggro_range_candidates(world, npc_oid));
    valid.retain(|&oid| check_target(world, npc_oid, oid));
    if valid.is_empty() {
        return None;
    }
    Some(valid[world.roll(valid.len() as i32) as usize])
}

/// The "if npc is aggressive, add characters within aggro range too" leg of
/// both `targetReconsider` arms. Empty for a passive mob.
fn aggro_range_candidates(world: &mut World, npc_oid: i32) -> Vec<i32> {
    let Some(template) = npc_template(world, npc_oid) else {
        return Vec::new();
    };
    if !template.is_aggressive {
        return Vec::new();
    }
    let range = template.aggro_range as f64;
    let (Some(pos), Some(region)) = (
        maybe_position(world, npc_oid),
        region_cell_of(world, npc_oid),
    ) else {
        return Vec::new();
    };
    // Index-derived like the aggro scan, but deliberately without the LOS and
    // liveness filters the scan applies — this candidate list feeds
    // `target_reconsider`, which does its own checks.
    let range_sq = range * range;
    let mut out = Vec::new();
    for pid in world.players_visible_from(region) {
        let Some(ppos) = world.objects.get_component::<Position>(&pid) else {
            continue;
        };
        let (dx, dy, dz) = (
            (ppos.x - pos.x) as f64,
            (ppos.y - pos.y) as f64,
            (ppos.z - pos.z) as f64,
        );
        if dx * dx + dy * dy + dz * dz <= range_sq {
            out.push(pid);
        }
    }
    out
}

/// `Creature.setRunning()` for an NPC: flip the move type and tell everyone
/// watching (`ChangeMoveType`). Idempotent — Java guards every call site with
/// `if (!me.isRunning())` and so does this.
///
/// Every path that puts a monster into the attack loop has to come through
/// here. [`think_active`] already did it inline when it promoted its own
/// target, but the two paths that seed hate from *outside* the think — the
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

/// `AttackableAI.thinkAttack`'s AggroDistanceCheck leash body: if `npc_oid` is
/// a leashable monster now beyond its configured range from spawn, forget every
/// target, heal to full when `AggroDistanceCheckRestoreLife` is set, and send it
/// — plus its whole escort — back to the spawn point. Returns whether the leash
/// fired (the caller then aborts the swing this think).
///
/// **Deliberate deviation from Java:** Java issues `AI_INTENTION_MOVE_TO` and
/// lets the mob *walk* home (only the AI-less branch teleports), which leaves it
/// jogging across the map for tens of seconds, re-aggroable and re-pullable the
/// whole way — the exact drag-train the leash exists to stop. The operator asked
/// for the snap-back behaviour, so this port teleports instead
/// (`Npc.teleToLocation(spawn, true)`, the same relocate the attack-timeout path
/// already uses). Everything else in the block is Java's.
fn npc_leash_return_home(world: &mut World, npc_oid: i32) -> bool {
    let Some(spawn) = leash_home_point(world, npc_oid) else {
        return false;
    };
    leash_send_home(world, npc_oid, spawn);
    // "Minions should return as well" — Java walks the leader's escort back to
    // the *leader's* spawn point, not each minion's own.
    for minion_oid in minions::live_pack(world, npc_oid) {
        leash_send_home(world, minion_oid, spawn);
    }
    true
}

/// The leash gate: `Some(spawn point)` when this NPC is over its leash radius
/// and every exemption in Java's condition lets it through. Guards/defenders
/// (not `isMonster`), route walkers (`isWalker`) and grand bosses are exempt;
/// raids only leash under `AggroDistanceCheckRaids`, instanced monsters only
/// under `AggroDistanceCheckInstances`.
fn leash_home_point(world: &World, npc_oid: i32) -> Option<(i32, i32, i32)> {
    let npc = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)?;
    let spawn = npc.spawn_loc;
    let chase_range = npc.chase_range;
    let t = npc.template(world)?;
    if !t.is_monster() || t.type_name == "GrandBoss" {
        return None;
    }
    let is_raid = t.is_raid();
    // `!npc.isWalker()` — a route NPC's "home" is wherever its route has it.
    if world.objects.has_component::<WalkState>(&npc_oid) {
        return None;
    }
    if is_raid && !world.cfg.npc.aggro_distance_check_raids {
        return None;
    }
    if instance_of(world, npc_oid) != 0 && !world.cfg.npc.aggro_distance_check_instances {
        return None;
    }
    // `spawn.getChaseRange() > 0 ? max(MAX_DRIFT_RANGE, chaseRange) : …`
    let range = if chase_range > 0 {
        chase_range.max(world.cfg.npc.max_drift_range)
    } else if is_raid {
        world.cfg.npc.aggro_distance_check_raid_range
    } else {
        world.cfg.npc.aggro_distance_check_range
    } as f64;
    let pos = world.objects.get_component::<Position>(&npc_oid)?;
    let dist_sq = ((spawn.0 - pos.x) as f64).powi(2) + ((spawn.1 - pos.y) as f64).powi(2);
    (dist_sq > range * range).then_some(spawn)
}

/// One leashed mob's trip home: full HP/MP (when configured), an emptied aggro
/// list — the port's stand-in for Java's `clearAggroList()` *and*
/// `getAttackByList().clear()`, which share one structure here — back to the
/// `ACTIVE` scan loop at walking speed, and a teleport onto the spawn point.
fn leash_send_home(world: &mut World, npc_oid: i32, spawn: (i32, i32, i32)) {
    if world.cfg.npc.aggro_distance_check_restore_life
        && let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid)
    {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.clear();
    }
    set_active(world, npc_oid);
    // Drop the in-flight chase before relocating, or the movement sweep keeps
    // interpolating from the old position and drags the mob straight back out.
    stop_npc(world, npc_oid);
    let heading = world
        .objects
        .get_component::<Position>(&npc_oid)
        .map(|p| p.heading)
        .unwrap_or(0);
    death::relocate_npc(world, npc_oid, spawn.0, spawn.1, spawn.2, heading);
}

/// `moveToPawn` for a chasing NPC: walk to the edge of attack reach,
/// re-pathed every think (1 s). Java funnels this through the very same
/// `Creature.moveToLocation` geodata block as any other walk — the chase
/// destination is clamped to the last walkable cell and re-routed through the
/// path worker when the straight line is cut — and `AbstractAI.moveToPawn`
/// then broadcasts `MoveToPawn` only when the move is *not* on a geodata
/// route (a routed move announces ordinary `MoveToLocation` segments).
/// Skipping this block was how an aggroed mob glided vertically through
/// tower floors to a target on another level.
fn chase(world: &mut World, npc_oid: i32, target_oid: i32, reach: f64) {
    let Some(mover) = combat::combatant(world, npc_oid) else {
        return;
    };
    let Some(target) = combat::combatant(world, target_oid) else {
        return;
    };
    let Some((dest_x, dest_y, dest_z, _heading)) = combat::pawn_destination(&mover, &target, reach)
    else {
        return;
    };
    npc_geo_move(
        world,
        npc_oid,
        (dest_x, dest_y, dest_z),
        Some(PawnRef {
            target_oid,
            offset: reach as i32,
            target_pos: (target.x, target.y, target.z),
        }),
    );
}

/// The pawn a chase move is aimed at — carried down to the broadcast so a
/// direct (non-routed) move announces `MoveToPawn` the way Java's
/// `AbstractAI.moveToPawn` does.
struct PawnRef {
    target_oid: i32,
    offset: i32,
    target_pos: (i32, i32, i32),
}

/// A plain destination walk (return-home) with a `MoveToLocation` broadcast.
pub(crate) fn move_npc_to(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32) {
    npc_geo_move(world, npc_oid, (x, y, z), None);
}

/// The NPC half of `Creature.moveToLocation`, shared by every NPC walk —
/// chase, return-home, random walk (Java shares the method between players
/// and mobs; the player half lives in `position.rs`).
fn npc_geo_move(world: &mut World, npc_oid: i32, dest: (i32, i32, i32), pawn: Option<PawnRef>) {
    // `Creature.moveToLocation` bails on `isMovementDisabled()` — a rooted mob
    // stays put (and a stunned one never gets here; `think` already returned).
    if abnormal::is_movement_disabled(world, npc_oid) {
        return;
    }
    let (speed, start, region) = {
        let Some(speed) = world
            .objects
            .get_component::<Speeds>(&npc_oid)
            .map(Speeds::move_speed)
        else {
            return;
        };
        let Some(pos) = maybe_position(world, npc_oid) else {
            return;
        };
        let Some(region) = region_cell_of(world, npc_oid) else {
            return;
        };
        (speed, (pos.x, pos.y, pos.z), region)
    };
    if speed <= 0.0 {
        return;
    }

    // GEODATA MOVEMENT CHECKS AND PATHFINDING — the NPC half of
    // `Creature.moveToLocation`, which Java shares between players and mobs.
    let (mut x, mut y, mut z) = dest;
    let (original_x, original_y, original_z) = (x, y, z);
    let original_distance = {
        let dx = (x - start.0) as f64;
        let dy = (y - start.1) as f64;
        (dx * dx + dy * dy).sqrt()
    };
    // Deliberate divergence: Java also skips the clamp for a monster whose
    // destination differs by more than 100 z ("Monsters can move on ledges",
    // Creature.java) — and because the skipped clamp is also what arms the
    // pathfinding fallback, a Mobius monster chasing across a big z gap
    // moves in a straight unchecked 3D line, i.e. glides through tower
    // floors. That exception is not ported: a cross-floor chase here clamps
    // like any other walk and falls back to the path worker (stairs), which
    // is the retail-faithful outcome the rest of Java's design (LOS-gated
    // aggro and engagement) clearly intends.
    if world.path_finding > 0
        && original_distance <= 3000.0
        && !(start.2 - z > 300 && original_distance < 300.0)
    {
        let (vx, vy, vz) = world
            .geo
            .get_valid_location(start.0, start.1, start.2, x, y, z);
        x = vx;
        y = vy;
        // `if (!isPlayer()) z = destiny.getZ()` — unlike a player (who keeps
        // the z its client asked for), an NPC takes the geodata's corrected z.
        z = vz;
    }

    let dx = (x - start.0) as f64;
    let dy = (y - start.1) as f64;
    let distance = (dx * dx + dy * dy).sqrt();

    // The clamp cut the move short — the direct line is blocked, so ask the
    // path worker for a route to the *original* destination. `playable: false`
    // is Java's cheaper single-pass filter for AI movers. The move starts when
    // the reply lands in `handle_path_result`.
    if world.path_finding > 0 && (original_distance - distance) > 30.0 {
        // One outstanding request at a time: the AI re-issues a chase every
        // think (1 s), which would otherwise flood the worker with duplicates
        // for the same mob.
        if world
            .objects
            .has_component::<crate::model::components::PathWait>(&npc_oid)
        {
            return;
        }
        let seq = world.next_path_seq();
        world
            .objects
            .add_components(&npc_oid, crate::model::components::PathWait { seq });
        let _ = world.path.send(crate::geo::worker::PathRequest {
            seq,
            // NPCs have no client; every client-facing send on the reply path
            // is gated on the mover being a player, so this is never read.
            client_id: 0,
            object_id: npc_oid,
            from: start,
            to: (original_x, original_y, original_z),
            playable: false,
        });
        return;
    }

    if distance < 1.0 {
        return;
    }
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    let heading = movement::calculate_heading(dx, dy);
    let start_tick = world.tick;
    if let Some(pos) = world.objects.get_component_mut::<Position>(&npc_oid) {
        pos.heading = heading;
    }
    world.objects.add_components(
        &npc_oid,
        Movement(MoveData {
            start_x: start.0,
            start_y: start.1,
            start_z: start.2,
            dest_x: x,
            dest_y: y,
            dest_z: z,
            start_tick,
            total_ticks,
            geo_path: None,
        }),
    );
    // `AbstractAI.moveToPawn`: a chase that ended up as a plain direct move
    // announces `MoveToPawn`; everything else (including any routed move —
    // handled on the path worker's reply in `start_move`) announces
    // `MoveToLocation`.
    let pkt = match &pawn {
        Some(p) => server_packets::move_to_pawn(
            npc_oid,
            p.target_oid,
            p.offset,
            start.0,
            start.1,
            start.2,
            p.target_pos.0,
            p.target_pos.1,
            p.target_pos.2,
        ),
        None => server_packets::move_to_location(npc_oid, x, y, z, start.0, start.1, start.2),
    };
    broadcast_near_region_in(world, region, instance_of(world, npc_oid), &pkt);
}

/// `AttackableAI.isAggressiveTowards`'s playable-state gates — whether this NPC
/// notices `target_oid` at all.
///
/// Two effect flags hide a player from an aggro scan, and Java checks them on
/// adjacent lines of the same method:
///
/// - **`SILENT_MOVE`** (Silent Move 221, Stealth 411, Dance of Shadows 366):
///   `!me.isRaid() && !me.canSeeThroughSilentMove() && target.isSilentMovingAffected()`.
///   Raid bosses see through stealth; `canSeeThroughSilentMove` is always false
///   on this dist (`setSeeThroughSilentMove` has no callers in the whole Java
///   tree), so only the raid exemption is ported.
/// - **`FAKE_DEATH`** via `isAlikeDead()`, which `Player` overrides to include
///   it — the very first check in the method.
///
/// Java's third gate here, `player.isRecentFakeDeath()` (a grace window after
/// standing up), is inert on this dist: `PlayerFakeDeathUpProtection = 0`.
pub(crate) fn notices_target(world: &World, npc_oid: i32, target_oid: i32) -> bool {
    use crate::game_loop::abnormal;
    use crate::model::skill::effect_flag;
    // `//invis`: an invisible GM is never noticed — Java's `AttackableAI`
    // drops invisible targets and `OnCreatureSee` never fires for them
    // (no raid exemption, unlike SILENT_MOVE below).
    if world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&target_oid)
        .is_some_and(|f| f.hidden)
    {
        return false;
    }
    let flags = abnormal::flags_of(world, target_oid);
    // `isAlikeDead()` — a fake-dead player is, for aggro purposes, a corpse.
    if flags & effect_flag::FAKE_DEATH != 0 {
        return false;
    }
    if flags & effect_flag::SILENT_MOVE != 0 {
        let is_raid = is_raid_npc(world, npc_oid);
        if !is_raid {
            return false;
        }
    }
    true
}

/// The shared body of the AI proximity scans: live players within `range`
/// (3D — `World.forEachVisibleObjectInRange` uses `calculateDistance3D`, so a
/// player a floor above is outside a ground mob's aggro sphere) of (nx,ny,nz)
/// with geodata line of sight, drawn from the `player_regions` index (≤9
/// cells) instead of a full player-table sweep. Like Java's knownlist it
/// includes unattended shops — they are `Player` objects in the region index.
fn players_in_range_los(
    world: &World,
    region: (i32, i32),
    nx: i32,
    ny: i32,
    nz: i32,
    range: f64,
) -> Vec<i32> {
    let range_sq = range * range;
    let mut out = Vec::new();
    for pid in world.players_visible_from(region) {
        let (Some(pos), Some(v)) = (
            world.objects.get_component::<Position>(&pid),
            world.objects.get_component::<Vitals>(&pid),
        ) else {
            continue;
        };
        if !v.dead
            && ((pos.x - nx) as f64).powi(2)
                + ((pos.y - ny) as f64).powi(2)
                + ((pos.z - nz) as f64).powi(2)
                <= range_sq
            && world.geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
        {
            out.push(pid);
        }
    }
    out
}

/// Java `AttackableAI.isAggressiveTowards`, `me instanceof Guard` branch.
/// Guards seed hate on nearby **PKs** (reputation < 0) so the ordinary attack
/// loop takes over from there.
///
/// The 500 is Java's literal (`GUARD_ATTACK_RANGE` in spirit — the source has
/// the bare constant with a "Make sure how guards behave towards players"
/// note beside it), deliberately not the template `aggroRange`.
const GUARD_AGGRO_RANGE: f64 = 500.0;

fn set_hate_for(world: &mut World, npc_oid: i32, in_range: Vec<i32>) {
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        for player_oid in in_range {
            let entry = aggro.0.entry(player_oid).or_default();
            if entry.hate == 0.0 {
                entry.hate = 1.0;
            }
        }
    }
}
fn guard_aggro_scan(world: &mut World, npc_oid: i32, region: (i32, i32)) {
    let (nx, ny, nz) = {
        let Some(pos) = maybe_position(world, npc_oid) else {
            return;
        };
        (pos.x, pos.y, pos.z)
    };
    let mut pks = players_in_range_los(world, region, nx, ny, nz, GUARD_AGGRO_RANGE);
    // `getReputation() < 0` is the whole test: a clean player walks past a
    // guard untouched no matter how close.
    pks.retain(|&pid| {
        world
            .objects
            .get_component::<crate::model::Player>(&pid)
            .is_some_and(|p| p.reputation < 0)
    });
    // Guards run the same `isAggressiveTowards` (Java `Guard extends
    // Attackable`), so stealth and fake death hide a PK from them too.
    pks.retain(|&pid| notices_target(world, npc_oid, pid));
    set_hate_for(world, npc_oid, pks);
}

/// Java `AttackableAI.thinkAttack`'s faction block: an engaged NPC drags its
/// nearby clan-mates into the fight.
///
/// The gate that is easy to drop: **only if the target actually attacked *this*
/// NPC.** Java checks `getAttackByList`; the port's proxy is a non-zero `damage`
/// entry in the aggro list. Without it, walking up to one mob of a faction and
/// hitting *nothing* would still pull the whole camp. The rest of the scan lives
/// in [`faction_recruits`].
///
/// This runs from the think tick, so it never fires for a mob that dies before
/// its first think — [`faction_call_on_kill`] is the site that covers that.
///
/// Java routes the recruit through `EVT_AGGRESSION` (whose `Summon`-aware leg
/// never applies — a faction recruit is always an `Attackable`) and fires the
/// `OnAttackableFactionCall` script event; the port seeds hate directly and
/// dispatches the event's two listeners via [`on_faction_call_script`].
fn faction_call(world: &mut World, npc_oid: i32, target_oid: i32) {
    let Some((npc_id, help_range, collision)) = npc_id_of(world, npc_oid).and_then(|id| {
        world
            .data
            .npc_data
            .get(id)
            .map(|t| (id, t.clan_help_range, t.collision_radius))
    }) else {
        return;
    };
    if help_range <= 0
        || world
            .data
            .npc_data
            .get(npc_id)
            .is_none_or(|t| t.clans.is_empty())
    {
        return;
    }

    // Gate 1: this NPC must actually have been attacked by the target.
    let was_attacked = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&target_oid))
        .is_some_and(|info| info.damage > 0.0);
    if !was_attacked {
        return;
    }

    let hate = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&target_oid))
        .map(|i| i.hate)
        .unwrap_or(1.0);
    let target_is_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid);
    let Some(target_pos) = maybe_position(world, target_oid) else {
        return;
    };

    // `thinkAttack` widens the scan by the caller's collision radius and honours
    // `ignoreClanNpcIds`; `doDie` (see [`faction_call_on_kill`]) does neither.
    let recruits = faction_recruits(
        world,
        npc_oid,
        help_range as f64 + collision,
        target_pos.z,
        true,
    );

    for other in recruits {
        // Java: a *playable* target gets `EVT_AGGRESSION … 1` (a nudge — the
        // recruit picks its own target), anything else inherits the caller's
        // full hate. Either way the recruit switches to the attack loop.
        let added = if target_is_player { 1.0 } else { hate };
        recruit_to_attack(world, other, target_oid, added);
        let attacker = pvp::acting_player(world, target_oid);
        on_faction_call_script(world, other, npc_oid, attacker);
    }
}

/// Java `Creature.doDie`'s "Clan help range aggro on kill" block: the clan-mates
/// of a monster that just *died* aggro its killer.
///
/// This is the second of Java's two faction-call sites and is not a nicety — it
/// is the only one that fires when a player one-shots a mob. [`faction_call`]
/// runs from the AI think tick, so a monster killed before its first think in
/// `AI_INTENTION_ATTACK` never gets to call anyone; without this block a
/// high-level character farming low-level `[G]` mobs (Cave Blade Spiders, say)
/// would never pull the pack, which reads as group aggro being broken.
///
/// Java's version deliberately differs from `thinkAttack`'s in three ways, all
/// mirrored here: the killer must be a **non-GM playable**, the scan range is
/// the bare `clanHelpRange` (no collision radius added), and
/// `ignoreClanNpcIds` is *not* consulted.
///
/// As with [`faction_call`], the recruit is seeded directly (Java's
/// `EVT_AGGRESSION`) and the script event's listeners run via
/// [`on_faction_call_script`].
pub(crate) fn faction_call_on_kill(world: &mut World, npc_oid: i32, killer_oid: i32) {
    // `killer.isPlayable()` — a player or a summon, not another NPC.
    let killer_is_playable = world
        .objects
        .has_component::<crate::model::Player>(&killer_oid)
        || world
            .objects
            .has_component::<crate::model::components::ServitorOf>(&killer_oid);
    if !killer_is_playable {
        return;
    }
    // `!killer.getActingPlayer().isGM()` — a GM cleaning up a spawn is ignored.
    let actor = pvp::acting_player(world, killer_oid);
    if world
        .objects
        .get_component::<crate::model::Player>(&actor)
        .is_some_and(|p| p.is_gm(&world.data))
    {
        return;
    }

    let Some(help_range) = npc_id_of(world, npc_oid)
        .and_then(|id| world.data.npc_data.get(id))
        .filter(|t| !t.clans.is_empty())
        .map(|t| t.clan_help_range)
    else {
        return;
    };
    if help_range <= 0 {
        return;
    }

    let Some(killer_pos) = maybe_position(world, killer_oid) else {
        return;
    };

    // Java: `notifyEvent(EVT_AGGRESSION, killer, 1)` — hate on the *killer*
    // object (a summon aggroes the pack on itself, exactly as in Java).
    for other in faction_recruits(world, npc_oid, help_range as f64, killer_pos.z, false) {
        recruit_to_attack(world, other, killer_oid, 1.0);
        let attacker = pvp::acting_player(world, killer_oid);
        on_faction_call_script(world, other, npc_oid, attacker);
    }
}

/// The recruit scan shared by Java's two faction-call sites.
///
/// Returns the clan-mates of `caller_oid` that would answer a call about a
/// target at `target_z`. Three gates are easy to drop and each one matters:
/// alive-and-in-range, **only idle/active clan-mates answer** (one already
/// attacking is left alone, so a fight doesn't continually re-target everyone
/// in it), and same-faction. `honor_ignore_list` covers `ignoreClanNpcIds` —
/// 82 templates on this dist refuse calls from specific faction-mates — which
/// only `thinkAttack` consults.
fn faction_recruits(
    world: &World,
    caller_oid: i32,
    range: f64,
    target_z: i32,
    honor_ignore_list: bool,
) -> Vec<i32> {
    let (Some(caller_id), Some(pos), Some(region)) = (
        npc_id_of(world, caller_oid),
        maybe_position(world, caller_oid),
        region_cell_of(world, caller_oid),
    ) else {
        return Vec::new();
    };

    // Candidate clan-mates: NPCs in this and the neighbouring regions.
    let nearby: Vec<i32> = world
        .npcs_visible_from(region)
        .into_iter()
        .filter(|&other| other != caller_oid)
        .collect();

    let mut recruits: Vec<i32> = Vec::new();
    for other in nearby {
        let Some(opos) = maybe_position(world, other) else {
            continue;
        };
        if is_dead(world, other) {
            continue;
        }
        // 3D range around the *caller* (`forEachVisibleObjectInRange`), plus
        // Java's explicit ±600 z band against the *target* — a helper on
        // another tower level never answers a call about a target it could
        // only reach by crossing floors.
        let dist_sq = ((opos.x - pos.x) as f64).powi(2)
            + ((opos.y - pos.y) as f64).powi(2)
            + ((opos.z - pos.z) as f64).powi(2);
        if dist_sq > range * range || (opos.z - target_z).abs() > 600 {
            continue;
        }
        // Only the uncommitted answer.
        if world
            .objects
            .get_component::<NpcAi>(&other)
            .is_none_or(|ai| ai.intention == NpcIntention::Attack)
        {
            continue;
        }
        // Same faction, and not on the recruit's ignore list.
        let Some(other_id) = npc_id_of(world, other) else {
            continue;
        };
        let (Some(mine), Some(theirs)) = (
            world.data.npc_data.get(caller_id),
            world.data.npc_data.get(other_id),
        ) else {
            continue;
        };
        if !mine.shares_clan_with(theirs)
            || (honor_ignore_list && theirs.ignore_clan_npc_ids.contains(&caller_id))
        {
            continue;
        }
        recruits.push(other);
    }
    recruits
}

/// The port's `OnAttackableFactionCall`. Java fires the script event at each
/// recruit from exactly its two faction-call sites (`AttackableAI.thinkAttack`
/// and `Creature.doDie`); on this dist only two scripts listen — Queen Ant
/// (`addFactionCallId(NURSE)`) and Orfen (`registerMobs`) — so the dispatch is
/// a direct match on the recruit's npc id rather than a listener registry.
/// Every listener starts by bailing while the recruit is mid-cast.
fn on_faction_call_script(world: &mut World, recruit_oid: i32, caller_oid: i32, attacker_oid: i32) {
    /// Queen Ant's healer minion; heals the hurt caller with Recovery (4020,1).
    const NURSE: i32 = 29003;
    /// Orfen's melee minion; 1-in-20 to open with Blow (4067,4) on the attacker.
    const RAIKEL_LEOS: i32 = 29016;
    /// Orfen Heal (4516,1) at a half-dead caller: 9-in-10 for Orfen herself,
    /// 1-in-10 for anyone else (never for a fellow Riba Iren).
    const ORFEN_HEAL: (i32, i32) = (4516, 1);
    const QA_HEAL: (i32, i32) = (4020, 1);
    const BLOW: (i32, i32) = (4067, 4);

    let recruit_id = npc_id_of(world, recruit_oid);
    let riba = crate::game_loop::orfen::RIBA_IREN;
    let Some(recruit_id) = recruit_id else { return };
    if !(recruit_id == NURSE || recruit_id == RAIKEL_LEOS || recruit_id == riba) {
        return;
    }
    if world.objects.has_component::<Casting>(&recruit_oid) {
        return;
    }
    let caller_hp = hp_pair(world, caller_oid);
    let cast = |world: &mut World, target: i32, (id, lvl): (i32, i32)| {
        crate::game_loop::npc::cast::cast_skill(world, recruit_oid, target, id, lvl);
    };
    match recruit_id {
        NURSE => {
            // `caller.getCurrentHp() < caller.getMaxHp()` — any wound at all.
            if caller_hp.is_some_and(|(cur, max)| cur < max) {
                cast(world, caller_oid, QA_HEAL);
            }
        }
        RAIKEL_LEOS => {
            if world.roll(20) == 0 {
                cast(world, attacker_oid, BLOW);
            }
        }
        id if id == riba => {
            let caller_id = npc_id_of(world, caller_oid);
            let chance = if caller_id == Some(crate::game_loop::orfen::ORFEN) {
                9
            } else {
                1
            };
            if caller_id != Some(riba)
                && caller_hp.is_some_and(|(cur, max)| cur < max / 2.0)
                && world.roll(10) < chance
            {
                cast(world, caller_oid, ORFEN_HEAL);
            }
        }
        _ => {}
    }
}

/// Test hook.
#[cfg(test)]
pub(crate) fn on_faction_call_script_for_test(
    world: &mut World,
    recruit_oid: i32,
    caller_oid: i32,
    attacker_oid: i32,
) {
    on_faction_call_script(world, recruit_oid, caller_oid, attacker_oid);
}

/// A faction-mate answering a call: seed hate on the target and switch it into
/// the attack loop.
fn recruit_to_attack(world: &mut World, recruit_oid: i32, target_oid: i32, hate: f64) {
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&recruit_oid) {
        let entry = aggro.0.entry(target_oid).or_default();
        entry.hate += hate;
    }
    // `onEvtAggression`: run **before** switching to the attack intention.
    set_running(world, recruit_oid);
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&recruit_oid) {
        ai.intention = NpcIntention::Attack;
        ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
    }
}
