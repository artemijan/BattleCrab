//! `AttackableAI` (G9 slice): the 1 s think tick over monsters in active
//! regions — aggro-range scans, chasing, swinging back, drift-return.
//!
//! Idle random walk and random social animations
//! (`RandomAnimationTaskManager`), NPC skill casting (see
//! [`super::npc_cast`]), town-guard PK aggro, clan/faction help calls,
//! `thinkAttack`'s line-of-sight gate (a mob that cannot see its target
//! walks a geo-validated route instead of engaging), geodata-clamped
//! chasing, `checkHate` aggro decay and the teleport-home attack timeout are
//! ported. Not ported yet (see PROGRESS): minions' archer kite and raid
//! target-chaos moves.

use std::collections::HashSet;

use commons::util::rnd;

use crate::model::components::{AttackState, Movement, Position, RegionCell, Speeds, Vitals};
use crate::model::movement::{self, MoveData};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{World, regions_adjacent};

use super::combat::{self, ATTACK_TIMEOUT_TICKS};
use super::helpers::{broadcast_near_region_in, instance_of};

/// `AttackableThinkTaskManager.TASK_DELAY`: think once per second.
pub(crate) const NPC_THINK_PERIOD: u64 = 10;

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
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs
            && let Some(r) = world
                .objects
                .get_component::<RegionCell>(&s.player_object_id())
        {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    active.insert((r.0.0 + dx, r.0.1 + dy));
                }
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
    let Some(t) = npc.template(world) else { return };
    let attackable = t.attackable;
    let enabled =
        super::spawn_scripts::random_animation_enabled(world, npc_oid, t.random_animation);
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
            if let Some(region) = world
                .objects
                .get_component::<RegionCell>(&npc_oid)
                .map(|r| r.0)
            {
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

fn think(world: &mut World, npc_oid: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    if world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    // A stunned/asleep/paralyzed mob does nothing at all — Java's `isDisabled()`
    // short-circuits `AttackableAI.onEvtThink`. A *rooted* one still thinks
    // (it can attack an adjacent target); the movement primitives refuse the
    // chase leg on their own.
    if super::abnormal::is_blocked_from_actions(world, npc_oid) {
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
    let Some(t) = npc.template(world) else { return };
    // Only the Attackable subtree has this AI; the slice narrows to monsters —
    // plus town `Guard`s (Java `Guard extends Attackable`, so they run this same
    // AI; they're what hunts PKs) and stationed siege guards (`Defender`) while
    // their castle's siege runs, which use the same scan/attack/chase to defend
    // against attackers.
    if !t.is_monster()
        && !t.is_guard()
        && super::siege::active_siege_guard_castle(world, npc_oid).is_none()
    {
        return;
    }
    let _ = npc;
    // A servitor runs `SummonAI`, not `AttackableAI`: it trails its owner
    // instead of scanning for prey, and only fights what its owner points it
    // at. Once ordered, the ordinary attack think below drives it — "attack the
    // most-hated" is the right behaviour once the order has seeded the list.
    if world
        .objects
        .has_component::<crate::model::components::ServitorOf>(&npc_oid)
    {
        super::servitor::servitor_follow_tick(world, npc_oid);
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
            let Some((cx, cy, cz)) = position_of(world, commander) else {
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
            if let Some((cx, cy, cz)) = position_of(world, commander)
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
    let (nx, ny, _) = position_of(world, npc_oid)?;
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
                position_of(world, m)
                    .map(|(x, y, _)| ((x - nx) as i64).pow(2) + ((y - ny) as i64).pow(2))
                    .unwrap_or(i64::MAX)
            })
            .copied()
    })
}

fn position_of(world: &World, oid: i32) -> Option<(i32, i32, i32)> {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (p.x, p.y, p.z))
}

fn distance_2d(world: &World, oid: i32, x: i32, y: i32) -> f64 {
    world
        .objects
        .get_component::<Position>(&oid)
        .map(|p| (((p.x - x) as f64).powi(2) + ((p.y - y) as f64).powi(2)).sqrt())
        .unwrap_or(f64::MAX)
}

/// Stop a mob dead (remove its move, broadcast `StopMove`).
fn stop_npc(world: &mut World, npc_oid: i32) {
    if !world.objects.has_component::<Movement>(&npc_oid) {
        return;
    }
    world.objects.remove_component::<Movement>(&npc_oid);
    if let (Some(pos), Some(region)) = (
        world.objects.get_component::<Position>(&npc_oid).copied(),
        world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0),
    ) {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading),
        );
    }
}

/// `AggroInfo.checkHate`, run across the aggro list before every most-hated
/// pick (Java runs it per-entry inside `Attackable.getMostHated`): hate
/// silently zeroes for an attacker who is dead, despawned, or no longer
/// inside the NPC's 3×3 surrounding regions. The entry survives — only its
/// weight drops — and this is what actually makes a mob forget a target that
/// left the neighbourhood; without it a hated player stays "most hated"
/// forever and the mob chases across the world.
fn check_hate(world: &mut World, npc_oid: i32) {
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
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
        (
            t.map(|t| t.is_monster() && t.is_aggressive && t.aggro_range > 0)
                .unwrap_or(false),
            t.map(|t| t.aggro_range).unwrap_or(0),
        )
    };
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };

    // Aggro-range scan (`isAggressiveTowards` narrowed: alive, in range,
    // geodata-visible; invisibility/silent-move/GM states don't exist).
    if aggressive
        && world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| ai.global_aggro >= 0)
    {
        let (nx, ny, nz) = {
            let pos = world
                .objects
                .get_component::<Position>(&npc_oid)
                .expect("caller checked");
            (pos.x, pos.y, pos.z)
        };
        let mut in_range: Vec<i32> = Vec::new();
        {
            let crate::world::World { objects, geo, .. } = &mut *world;
            objects.for_each_mut::<(&crate::model::Player, &Position, &RegionCell, &Vitals)>(
                |(p, pos, r, v)| {
                    // 3D range (`World.forEachVisibleObjectInRange` uses
                    // `calculateDistance3D`): a player a floor above is out
                    // of a ground mob's aggro sphere even when horizontally
                    // on top of it.
                    if !v.dead
                        && regions_adjacent(region, r.0)
                        && (((pos.x - nx) as f64).powi(2)
                            + ((pos.y - ny) as f64).powi(2)
                            + ((pos.z - nz) as f64).powi(2))
                        .sqrt()
                            <= aggro_range as f64
                        && geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
                    {
                        in_range.push(p.object_id);
                    }
                },
            );
        }
        // Stealth / fake death (`isAggressiveTowards`): filtered after the
        // sweep because the sweep closure holds `objects` mutably and the flag
        // lookup needs it shared — the same shape the siege branch below uses.
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
            let npc_id = world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_oid)
                .map(|n| n.npc_id)
                .unwrap_or(0);
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
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_guard())
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
        && let Some(_castle) = super::siege::active_siege_guard_castle(world, npc_oid)
    {
        let (nx, ny, nz) = {
            let pos = world
                .objects
                .get_component::<Position>(&npc_oid)
                .expect("caller checked");
            (pos.x, pos.y, pos.z)
        };
        let mut in_range: Vec<i32> = Vec::new();
        {
            let crate::world::World { objects, geo, .. } = &mut *world;
            objects.for_each_mut::<(&crate::model::Player, &Position, &RegionCell, &Vitals)>(
                |(p, pos, r, v)| {
                    if !v.dead
                        && regions_adjacent(region, r.0)
                        && (((pos.x - nx) as f64).powi(2)
                            + ((pos.y - ny) as f64).powi(2)
                            + ((pos.z - nz) as f64).powi(2))
                        .sqrt()
                            <= aggro_range as f64
                        && geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
                    {
                        in_range.push(p.object_id);
                    }
                },
            );
        }
        // Keep only actual enemies (attackers / non-defenders).
        in_range.retain(|&pid| super::siege::attackable_siege_guard(world, npc_oid, pid));
        in_range.retain(|&pid| notices_target(world, npc_oid, pid));
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            for player_oid in in_range {
                let entry = aggro.0.entry(player_oid).or_default();
                if entry.hate == 0.0 {
                    entry.hate = 1.0;
                }
            }
        }
    }

    // Chose a target from the aggro list (`getMostHated`, after the
    // per-entry `checkHate` liveness/region test).
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
        let global_aggro = world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .map(|ai| ai.global_aggro)
            .unwrap_or(0);
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
        let random_walk = super::spawn_scripts::random_walk_enabled(world, npc_oid, random_walk);
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
    let dist = (((spawn.0 - x) as f64).powi(2) + ((spawn.1 - y) as f64).powi(2)).sqrt();
    if dist > max_drift {
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
    let from_spawn = (((vx - spawn.0) as f64).powi(2) + ((vy - spawn.1) as f64).powi(2)).sqrt();
    if from_spawn <= drift as f64 {
        move_npc_to(world, npc_oid, vx, vy, vz);
    }
}

/// `AttackableAI.thinkAttack`: validate the hated target, time out, chase,
/// swing.
fn think_attack(world: &mut World, npc_oid: i32) {
    let now = world.tick;

    // Chase leash (`AttackableAI.thinkAttack` `AGGRO_DISTANCE_CHECK`): a monster
    // dragged farther than the configured range from its spawn drops all aggro
    // and walks home (healed to full when configured). Off by default on this
    // dist. Guards/defenders (not `isMonster`) and grand bosses are exempt,
    // matching Java.
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
            let Some(region) = world
                .objects
                .get_component::<RegionCell>(&npc_oid)
                .map(|r| r.0)
            else {
                return;
            };
            let mut any = false;
            world
                .objects
                .for_each_mut::<(&crate::model::Player, &RegionCell)>(|(_, r)| {
                    any |= regions_adjacent(region, r.0);
                });
            any
        };
        if is_monster && instance_of(world, npc_oid) == 0 && (in_combat || !players_visible) {
            let heading = world
                .objects
                .get_component::<Position>(&npc_oid)
                .map(|p| p.heading)
                .unwrap_or(0);
            super::death::relocate_npc(world, npc_oid, spawn.0, spawn.1, spawn.2, heading);
        }
        return;
    }

    // Busy swinging.
    if world
        .objects
        .get_component::<AttackState>(&npc_oid)
        .is_some_and(|st| st.attack_end_tick > now)
    {
        return;
    }

    // "Actor should be able to see target" (`thinkAttack`'s geodata gate): a
    // sight line cut by a wall or a tower floor means no faction call, no
    // cast, no swing and — crucially — no straight-line chase. Java issues
    // `moveTo(target)`, an ordinary geo-validated walk that clamps at the
    // last walkable cell and falls back to the path worker (the stairs
    // route), then returns. Without this gate a mob whose hated target
    // climbed to another level engages straight through the geometry.
    {
        let (Some(npos), Some(tpos)) = (
            world.objects.get_component::<Position>(&npc_oid).copied(),
            world
                .objects
                .get_component::<Position>(&target_oid)
                .copied(),
        ) else {
            return;
        };
        if !world
            .geo
            .can_see_target(npos.x, npos.y, npos.z, tpos.x, tpos.y, tpos.z)
        {
            let can_move = world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_oid)
                .and_then(|n| n.template(world))
                .is_some_and(|t| t.can_move);
            if can_move {
                move_npc_to(world, npc_oid, tpos.x, tpos.y, tpos.z);
            }
            return;
        }
    }

    // Call the faction for help before anything else this think (Java runs the
    // block right after the geodata check, ahead of the cast ladder).
    faction_call(world, npc_oid, target_oid);

    // Cast before closing distance — Java's "Cast skills" block sits between
    // the target checks and the range/move tail, so a caster that launched a
    // spell this think neither chases nor swings.
    if super::npc_cast::try_cast(world, npc_oid, target_oid) {
        return;
    }

    let Some(attacker) = combat::combatant(world, npc_oid) else {
        return;
    };
    let Some(victim) = combat::combatant(world, target_oid) else {
        return;
    };
    let reach = attacker.atk_range as f64 + attacker.collision_radius + victim.collision_radius;
    let dist = (((victim.x - attacker.x) as f64).powi(2)
        + ((victim.y - attacker.y) as f64).powi(2))
    .sqrt();

    let can_move = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .expect("npc")
        .template(world)
        .map(|t| t.can_move)
        .unwrap_or(false);
    if dist > reach {
        if can_move {
            chase(world, npc_oid, target_oid, reach);
        }
        return;
    }

    // In reach: stop and swing.
    if world.objects.has_component::<Movement>(&npc_oid) {
        world.objects.remove_component::<Movement>(&npc_oid);
        let (Some(pos), Some(region)) = (
            world.objects.get_component::<Position>(&npc_oid).copied(),
            world
                .objects
                .get_component::<RegionCell>(&npc_oid)
                .map(|r| r.0),
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
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
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
/// target, optionally heal to full, and walk it home. Returns whether the leash
/// fired (the caller then aborts the swing this think). Guards/defenders (not
/// `isMonster`) and grand bosses are exempt, and raids only leash when
/// `AggroDistanceCheckRaids` is set — matching Java.
fn npc_leash_return_home(world: &mut World, npc_oid: i32) -> bool {
    let (spawn, is_monster, is_grandboss, is_raid) = {
        let Some(npc) = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
        else {
            return false;
        };
        let spawn = npc.spawn_loc;
        let Some(t) = npc.template(world) else {
            return false;
        };
        (
            spawn,
            t.is_monster(),
            t.type_name == "GrandBoss",
            t.is_raid(),
        )
    };
    if !is_monster || is_grandboss {
        return false;
    }
    if is_raid && !world.cfg.npc.aggro_distance_check_raids {
        return false;
    }
    let range = (if is_raid {
        world.cfg.npc.aggro_distance_check_raid_range
    } else {
        world.cfg.npc.aggro_distance_check_range
    }) as f64;
    let restore = world.cfg.npc.aggro_distance_check_restore_life;
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else {
        return false;
    };
    let dist = (((spawn.0 - pos.x) as f64).powi(2) + ((spawn.1 - pos.y) as f64).powi(2)).sqrt();
    if dist <= range {
        return false;
    }
    if restore && let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid) {
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.clear();
    }
    set_active(world, npc_oid);
    move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
    true
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
    if super::abnormal::is_movement_disabled(world, npc_oid) {
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
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() else {
            return;
        };
        let Some(region) = world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0)
        else {
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
    let flags = super::abnormal::flags_of(world, target_oid);
    // `isAlikeDead()` — a fake-dead player is, for aggro purposes, a corpse.
    if flags & effect_flag::FAKE_DEATH != 0 {
        return false;
    }
    if flags & effect_flag::SILENT_MOVE != 0 {
        let is_raid = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_raid());
        if !is_raid {
            return false;
        }
    }
    true
}

/// Java `AttackableAI.isAggressiveTowards`, `me instanceof Guard` branch.
/// Guards seed hate on nearby **PKs** (reputation < 0) so the ordinary attack
/// loop takes over from there.
///
/// The 500 is Java's literal (`GUARD_ATTACK_RANGE` in spirit — the source has
/// the bare constant with a "TODO Make sure how guards behave towards players"
/// note beside it), deliberately not the template `aggroRange`.
const GUARD_AGGRO_RANGE: f64 = 500.0;

fn guard_aggro_scan(world: &mut World, npc_oid: i32, region: (i32, i32)) {
    let (nx, ny, nz) = {
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else {
            return;
        };
        (pos.x, pos.y, pos.z)
    };
    let mut pks: Vec<i32> = Vec::new();
    {
        let crate::world::World { objects, geo, .. } = &mut *world;
        objects.for_each_mut::<(&crate::model::Player, &Position, &RegionCell, &Vitals)>(
            |(p, pos, r, v)| {
                // `getReputation() < 0` is the whole test: a clean player walks
                // past a guard untouched no matter how close.
                if !v.dead
                    && p.reputation < 0
                    && regions_adjacent(region, r.0)
                    && (((pos.x - nx) as f64).powi(2)
                        + ((pos.y - ny) as f64).powi(2)
                        + ((pos.z - nz) as f64).powi(2))
                    .sqrt()
                        <= GUARD_AGGRO_RANGE
                    && geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
                {
                    pks.push(p.object_id);
                }
            },
        );
    }
    // Guards run the same `isAggressiveTowards` (Java `Guard extends
    // Attackable`), so stealth and fake death hide a PK from them too.
    pks.retain(|&pid| notices_target(world, npc_oid, pid));
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        for oid in pks {
            let entry = aggro.0.entry(oid).or_default();
            if entry.hate == 0.0 {
                entry.hate = 1.0;
            }
        }
    }
}

/// Java `AttackableAI.thinkAttack`'s faction block: an engaged NPC drags its
/// nearby clan-mates into the fight.
///
/// Three gates are easy to drop and each one matters:
/// 1. **Only if the target actually attacked *this* NPC.** Java checks
///    `getAttackByList`; the port's proxy is a non-zero `damage` entry in the
///    aggro list. Without it, walking up to one mob of a faction and hitting
///    *nothing* would still pull the whole camp.
/// 2. **Only idle/active clan-mates answer.** One already attacking or casting
///    is left alone, so a fight doesn't continually re-target everyone in it.
/// 3. **`ignoreNpcId`** — 82 templates on this dist refuse calls from specific
///    faction-mates.
///
/// `TODO(G21)`: Java fires `EVT_AGGRESSION` (a `Summon`-aware event) and an
/// `OnAttackableFactionCall` script hook; the port seeds hate directly and has
/// no script listeners yet.
fn faction_call(world: &mut World, npc_oid: i32, target_oid: i32) {
    let Some((npc_id, help_range, collision)) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
        .and_then(|id| {
            world
                .data
                .npc_data
                .get(id)
                .map(|t| (id, t.clan_help_range, t.collision_radius))
        })
    else {
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

    let range = help_range as f64 + collision;
    let (Some(pos), Some(region)) = (
        world.objects.get_component::<Position>(&npc_oid).copied(),
        world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0),
    ) else {
        return;
    };
    let hate = world
        .objects
        .get_component::<AggroList>(&npc_oid)
        .and_then(|a| a.0.get(&target_oid))
        .map(|i| i.hate)
        .unwrap_or(1.0);
    let target_is_player = world
        .objects
        .has_component::<crate::model::Player>(&target_oid);
    let Some(target_pos) = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied()
    else {
        return;
    };

    // Candidate clan-mates: NPCs in this and the neighbouring regions.
    let nearby: Vec<i32> = (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter_map(|(dx, dy)| world.npc_regions.get(&(region.0 + dx, region.1 + dy)))
        .flatten()
        .copied()
        .filter(|&other| other != npc_oid)
        .collect();

    let mut recruits: Vec<i32> = Vec::new();
    for other in nearby {
        // Alive, and within the faction range in 2D with Java's ±600 z band.
        let Some(opos) = world.objects.get_component::<Position>(&other).copied() else {
            continue;
        };
        if world
            .objects
            .get_component::<Vitals>(&other)
            .is_none_or(|v| v.dead)
        {
            continue;
        }
        // 3D range around the *caller* (`forEachVisibleObjectInRange`), plus
        // Java's explicit ±600 z band against the *target* — a helper on
        // another tower level never answers a call about a target it could
        // only reach by crossing floors.
        let dist = (((opos.x - pos.x) as f64).powi(2)
            + ((opos.y - pos.y) as f64).powi(2)
            + ((opos.z - pos.z) as f64).powi(2))
        .sqrt();
        if dist > range || (opos.z - target_pos.z).abs() > 600 {
            continue;
        }
        // Gate 2: only the uncommitted answer.
        if world
            .objects
            .get_component::<NpcAi>(&other)
            .is_none_or(|ai| ai.intention == NpcIntention::Attack)
        {
            continue;
        }
        // Gate 3: same faction, and not on either side's ignore list.
        let Some(other_id) = world
            .objects
            .get_component::<crate::model::npc::Npc>(&other)
            .map(|n| n.npc_id)
        else {
            continue;
        };
        let (Some(mine), Some(theirs)) = (
            world.data.npc_data.get(npc_id),
            world.data.npc_data.get(other_id),
        ) else {
            continue;
        };
        if !mine.shares_clan_with(theirs) || theirs.ignore_clan_npc_ids.contains(&npc_id) {
            continue;
        }
        recruits.push(other);
    }

    for other in recruits {
        // Java: a *playable* target gets `EVT_AGGRESSION … 1` (a nudge — the
        // recruit picks its own target), anything else inherits the caller's
        // full hate. Either way the recruit switches to the attack loop.
        let added = if target_is_player { 1.0 } else { hate };
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&other) {
            let entry = aggro.0.entry(target_oid).or_default();
            entry.hate += added;
        }
        if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&other) {
            ai.intention = NpcIntention::Attack;
            ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
        }
    }
}
