//! `AttackableAI` (G9 slice): the 1 s think tick over monsters in active
//! regions — aggro-range scans, chasing, swinging back, drift-return.
//!
//! Idle random walk and random social animations
//! (`RandomAnimationTaskManager`) are ported. Not ported yet (see PROGRESS):
//! guard aggro (karma players don't exist), clan/faction help calls, minions,
//! NPC skill casting (`AISkillScope` lists aren't parsed), the archer kite and
//! raid target-chaos moves, and Java's teleport-home on attack timeout
//! (walking home is used instead — no teleport plumbing for NPCs).

use std::collections::HashSet;

use rand::Rng;

use crate::model::components::{AttackState, Movement, Position, RegionCell, Speeds, Vitals};
use crate::model::movement::{self, MoveData};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::{regions_adjacent, World};

use super::combat::{self, ATTACK_TIMEOUT_TICKS};
use super::helpers::broadcast_near_region;

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
        if let ClientSession::InGame(s) = cs {
            if let Some(r) = world.objects.get_component::<RegionCell>(&s.player_object_id()) {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        active.insert((r.0 .0 + dx, r.0 .1 + dy));
                    }
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
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else { return };
    let Some(t) = npc.template(world) else { return };
    let attackable = t.attackable;
    let enabled = t.random_animation;
    let (min_s, max_s) = if attackable {
        (world.cfg.npc.min_monster_animation, world.cfg.npc.max_monster_animation)
    } else {
        (world.cfg.npc.min_npc_animation, world.cfg.npc.max_npc_animation)
    };
    if !enabled || max_s <= 0 {
        return;
    }

    let now = world.tick;
    // First visit: set the initial pending time and wait (Java `add()`).
    let Some(next) = world.objects.get_component::<NpcAi>(&npc_oid).and_then(|ai| ai.next_animation_tick) else {
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
    let idle = world.objects.get_component::<Vitals>(&npc_oid).is_some_and(|v| !v.dead)
        && world.objects.get_component::<NpcAi>(&npc_oid).is_some_and(|ai| ai.intention != NpcIntention::Attack)
        && !world.objects.has_component::<Movement>(&npc_oid);
    if idle {
        let throttled = world
            .objects
            .get_component::<NpcAi>(&npc_oid)
            .is_some_and(|ai| now.saturating_sub(ai.last_social_tick) <= SOCIAL_THROTTLE_TICKS);
        if !throttled {
            let action_id = world.rng.gen_range(2..=3); // Rnd.get(2, 3)
            if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
                ai.last_social_tick = now;
            }
            if let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) {
                broadcast_near_region(world, region, &server_packets::social_action(npc_oid, action_id));
            }
        }
    }
    let delay = animation_delay_ticks(world, min_s, max_s);
    if let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) {
        ai.next_animation_tick = Some(now + delay);
    }
}

/// `Rnd.get(min, max) * 1000` ms as ticks (inclusive of `max`).
fn animation_delay_ticks(world: &mut World, min_s: i32, max_s: i32) -> u64 {
    let secs = world.rng.gen_range(min_s.max(0)..=max_s.max(min_s).max(0));
    secs as u64 * TICKS_PER_SECOND
}

/// `ScheduledTask::NpcAttackReady` — the NPC's swing period elapsed. Re-run its
/// think immediately (Java `EVT_READY_TO_ACT`) so a fast attacker keeps swinging
/// at its weapon rate instead of stalling until the next 1 s AI tick.
pub(crate) fn on_npc_attack_ready(world: &mut World, npc_oid: i32) {
    think(world, npc_oid);
}

fn think(world: &mut World, npc_oid: i32) {
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else { return };
    if world.objects.get_component::<Vitals>(&npc_oid).is_none_or(|v| v.dead) {
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
    if let Some(group_id) = world.objects.get_component::<crate::model::mob_group::Controllable>(&npc_oid).map(|c| c.group_id) {
        controllable_think(world, npc_oid, group_id);
        return;
    }
    let Some(t) = npc.template(world) else { return };
    // Only the Attackable subtree has this AI; the slice narrows to monsters —
    // plus stationed siege guards (`Defender`) while their castle's siege runs,
    // which use the same scan/attack/chase to defend against attackers.
    if !t.is_monster() && super::siege::active_siege_guard_castle(world, npc_oid).is_none() {
        return;
    }
    let _ = npc;
    let Some(ai) = world.objects.get_component::<NpcAi>(&npc_oid) else { return };
    match ai.intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
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
            match world.objects.get_component::<NpcAi>(&npc_oid).map(|ai| ai.intention) {
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
            let Some((cx, cy, cz)) = position_of(world, commander) else { return };
            let dist = distance_2d(world, npc_oid, cx, cy);
            if dist > FOLLOW_RANGE && world.objects.get_component::<Movement>(&npc_oid).is_none() {
                move_npc_to(world, npc_oid, cx, cy, cz);
            } else if dist <= FOLLOW_RANGE {
                stop_npc(world, npc_oid);
            }
        }
        MobGroupState::Return(commander) => {
            if let Some((cx, cy, cz)) = position_of(world, commander) {
                if world.objects.get_component::<Movement>(&npc_oid).is_none() {
                    move_npc_to(world, npc_oid, cx, cy, cz);
                }
            }
        }
    }
}

/// Make the mob attack `target`: seed dominant hate and enter the attack loop
/// (reuses `think_attack`, so chase + swing are the wild AI's).
fn seed_attack(world: &mut World, npc_oid: i32, target: i32) {
    let target_alive = world.objects.get_component::<Vitals>(&target).is_some_and(|v| !v.dead);
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
            .filter(|&&m| world.objects.get_component::<Vitals>(&m).is_some_and(|v| !v.dead))
            .min_by_key(|&&m| {
                position_of(world, m)
                    .map(|(x, y, _)| ((x - nx) as i64).pow(2) + ((y - ny) as i64).pow(2))
                    .unwrap_or(i64::MAX)
            })
            .copied()
    })
}

fn position_of(world: &World, oid: i32) -> Option<(i32, i32, i32)> {
    world.objects.get_component::<Position>(&oid).map(|p| (p.x, p.y, p.z))
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
        world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0),
    ) {
        broadcast_near_region(world, region, &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading));
    }
}

/// `AttackableAI.thinkActive`: tick `_globalAggro` toward 0, scan the aggro
/// range, pick the most hated, or drift back home.
fn think_active(world: &mut World, npc_oid: i32) {
    let (aggressive, aggro_range) = {
        let npc_id = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("caller checked").npc_id;
        let ai = world.objects.get_component_mut::<NpcAi>(&npc_oid).expect("caller checked");
        if ai.global_aggro != 0 {
            ai.global_aggro += if ai.global_aggro < 0 { 1 } else { -1 };
        }
        let t = world.data.npc_data.get(npc_id);
        // Java `npc.isAggressive()`: the explicit flag, not aggroRange —
        // nearly every passive mob in the datapack has an aggroRange too.
        (t.map(|t| t.is_aggressive && t.aggro_range > 0).unwrap_or(false), t.map(|t| t.aggro_range).unwrap_or(0))
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };

    // Aggro-range scan (`isAggressiveTowards` narrowed: alive, in range,
    // geodata-visible; invisibility/silent-move/GM states don't exist).
    if aggressive && world.objects.get_component::<NpcAi>(&npc_oid).is_some_and(|ai| ai.global_aggro >= 0) {
        let (nx, ny, nz) = {
            let pos = world.objects.get_component::<Position>(&npc_oid).expect("caller checked");
            (pos.x, pos.y, pos.z)
        };
        let mut in_range: Vec<i32> = Vec::new();
        {
            let crate::world::World { objects, geo, .. } = &mut *world;
            objects.for_each_mut::<(&crate::model::Player, &Position, &RegionCell, &Vitals)>(|(p, pos, r, v)| {
                if !v.dead
                    && regions_adjacent(region, r.0)
                    && (((pos.x - nx) as f64).powi(2) + ((pos.y - ny) as f64).powi(2)).sqrt() <= aggro_range as f64
                    && geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
                {
                    in_range.push(p.object_id);
                }
            });
        }
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            for player_oid in in_range {
                // `addDamageHate(t, 0, 0)` → first sight seeds 1 hate.
                let entry = aggro.0.entry(player_oid).or_default();
                if entry.hate == 0.0 {
                    entry.hate = 1.0;
                }
            }
        }
    }

    // Siege guards (`Defender`) defend the castle: they aggro their employer's
    // enemies within aggro range regardless of the `isAggressive` flag (Java
    // `SiegeGuardAI` — the guard's own aggro scan). Reuses the hate → attack →
    // chase machinery below. `aggro_range` comes from the template (1000 for the
    // stock guards); the enemy filter (anyone but a defender of this castle) is
    // `attackable_siege_guard`.
    if aggro_range > 0 {
        if let Some(_castle) = super::siege::active_siege_guard_castle(world, npc_oid) {
            let (nx, ny, nz) = {
                let pos = world.objects.get_component::<Position>(&npc_oid).expect("caller checked");
                (pos.x, pos.y, pos.z)
            };
            let mut in_range: Vec<i32> = Vec::new();
            {
                let crate::world::World { objects, geo, .. } = &mut *world;
                objects.for_each_mut::<(&crate::model::Player, &Position, &RegionCell, &Vitals)>(|(p, pos, r, v)| {
                    if !v.dead
                        && regions_adjacent(region, r.0)
                        && (((pos.x - nx) as f64).powi(2) + ((pos.y - ny) as f64).powi(2)).sqrt() <= aggro_range as f64
                        && geo.can_see_target(nx, ny, nz, pos.x, pos.y, pos.z)
                    {
                        in_range.push(p.object_id);
                    }
                });
            }
            // Keep only actual enemies (attackers / non-defenders).
            in_range.retain(|&pid| super::siege::attackable_siege_guard(world, npc_oid, pid));
            if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
                for player_oid in in_range {
                    let entry = aggro.0.entry(player_oid).or_default();
                    if entry.hate == 0.0 {
                        entry.hate = 1.0;
                    }
                }
            }
        }
    }

    // Chose a target from the aggro list (`getMostHated`).
    let hated = world.objects.get_component::<AggroList>(&npc_oid).and_then(AggroList::most_hated);
    if let Some(target) = hated {
        let aggro_list = world.objects.get_component::<AggroList>(&npc_oid).expect("checked");
        let aggro = aggro_list.0.get(&target).map(|a| a.hate).unwrap_or(0.0);
        let global_aggro = world.objects.get_component::<NpcAi>(&npc_oid).map(|ai| ai.global_aggro).unwrap_or(0);
        if aggro + global_aggro as f64 > 0.0 {
            let became_running = {
                let ai = world.objects.get_component_mut::<NpcAi>(&npc_oid).expect("checked");
                ai.intention = NpcIntention::Attack;
                ai.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
                let speeds = world.objects.get_component_mut::<Speeds>(&npc_oid).expect("checked");
                let flip = !speeds.running;
                speeds.running = true;
                flip
            };
            if became_running {
                broadcast_near_region(world, region, &server_packets::change_move_type(npc_oid, true));
            }
        }
        return;
    }

    // No target: either return to the spawn anchor when drifted too far
    // (`Config.MAX_DRIFT_RANGE`), or — while inside that radius — take an
    // occasional random walk (`AttackableAI.thinkActive`'s two idle branches).
    let max_drift = world.cfg.npc.max_drift_range as f64;
    let (x, y, z, spawn, moving, can_move, random_walk) = {
        let npc = &world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("npc");
        let pos = world.objects.get_component::<Position>(&npc_oid).expect("caller checked");
        let t = npc.template(world);
        let can_move = t.map(|t| t.can_move).unwrap_or(false);
        // Java `isRandomWalkingEnabled()`: the template flag (minions/walking-
        // route targets that clear it at runtime aren't in the monster slice).
        let random_walk = t.map(|t| t.random_walk).unwrap_or(false);
        (pos.x, pos.y, pos.z, npc.spawn_loc, world.objects.has_component::<Movement>(&npc_oid), can_move, random_walk)
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

    let (vx, vy, vz) = world.geo.get_valid_location(cur.0, cur.1, cur.2, x1, y1, z1);
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

    let target = world.objects.get_component::<AggroList>(&npc_oid).and_then(AggroList::most_hated);
    let Some(target_oid) = target else {
        set_active(world, npc_oid);
        return;
    };

    // Target dead or gone → stop hating it (next think re-evaluates).
    let target_alive = world.objects.get_component::<Vitals>(&target_oid).is_some_and(|v| !v.dead);
    if !target_alive {
        if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
            aggro.0.remove(&target_oid);
        }
        return;
    }

    // Attack timeout: give up, forget everyone, walk home (Java teleports —
    // see the module note).
    if world.objects.get_component::<NpcAi>(&npc_oid).is_some_and(|ai| ai.attack_timeout_tick < now) {
        let spawn = {
            if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
                aggro.0.clear();
            }
            world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("checked").spawn_loc
        };
        set_active(world, npc_oid);
        move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
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

    let Some(attacker) = combat::combatant(world, npc_oid) else { return };
    let Some(victim) = combat::combatant(world, target_oid) else { return };
    let reach = attacker.atk_range as f64 + attacker.collision_radius + victim.collision_radius;
    let dist = (((victim.x - attacker.x) as f64).powi(2) + ((victim.y - attacker.y) as f64).powi(2)).sqrt();

    let can_move = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("npc").template(world).map(|t| t.can_move).unwrap_or(false);
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
            world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0),
        ) else {
            return;
        };
        broadcast_near_region(world, region, &server_packets::stop_move(npc_oid, pos.x, pos.y, pos.z, pos.heading));
    }
    combat::do_auto_attack(world, npc_oid, target_oid);
}

/// Back to the scan loop: walking move type + Active intention (Java
/// `setIntention(AI_INTENTION_ACTIVE)` + `setWalking`).
fn set_active(world: &mut World, npc_oid: i32) {
    let was_running = {
        let Some(ai) = world.objects.get_component_mut::<NpcAi>(&npc_oid) else { return };
        ai.intention = NpcIntention::Active;
        let Some(speeds) = world.objects.get_component_mut::<Speeds>(&npc_oid) else { return };
        let was = speeds.running;
        speeds.running = false;
        was
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };
    if was_running {
        broadcast_near_region(world, region, &server_packets::change_move_type(npc_oid, false));
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
        let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else {
            return false;
        };
        let spawn = npc.spawn_loc;
        let Some(t) = npc.template(world) else { return false };
        (spawn, t.is_monster(), t.type_name == "GrandBoss", t.is_raid())
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
    if restore {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid) {
            v.cur_hp = v.max_hp as f64;
            v.cur_mp = v.max_mp as f64;
        }
    }
    if let Some(aggro) = world.objects.get_component_mut::<AggroList>(&npc_oid) {
        aggro.0.clear();
    }
    set_active(world, npc_oid);
    move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
    true
}

/// `moveToPawn` for a chasing NPC: walk to the edge of attack reach,
/// re-pathed every think (1 s), broadcasting `MoveToPawn`.
fn chase(world: &mut World, npc_oid: i32, target_oid: i32, reach: f64) {
    let Some(mover) = combat::combatant(world, npc_oid) else { return };
    let Some(target) = combat::combatant(world, target_oid) else { return };
    let Some((dest_x, dest_y, dest_z, heading)) = combat::pawn_destination(&mover, &target, reach) else { return };

    let (speed, start, region) = {
        let speed = world.objects.get_component::<Speeds>(&npc_oid).map(Speeds::move_speed).unwrap_or(0.0);
        let pos = world.objects.get_component::<Position>(&npc_oid).expect("checked");
        let region = world.objects.get_component::<RegionCell>(&npc_oid).expect("checked").0;
        (speed, (pos.x, pos.y, pos.z), region)
    };
    if speed <= 0.0 {
        return;
    }
    let distance = (((dest_x - start.0) as f64).powi(2) + ((dest_y - start.1) as f64).powi(2)).sqrt();
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
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
            dest_x,
            dest_y,
            dest_z,
            start_tick,
            total_ticks,
            geo_path: None,
        }),
    );
    broadcast_near_region(
        world,
        region,
        &server_packets::move_to_pawn(npc_oid, target_oid, reach as i32, start.0, start.1, start.2, target.x, target.y, target.z),
    );
}

/// A plain destination walk (return-home) with a `MoveToLocation` broadcast.
fn move_npc_to(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32) {
    // `Creature.moveToLocation` bails on `isMovementDisabled()` — a rooted mob
    // stays put (and a stunned one never gets here; `think` already returned).
    if super::abnormal::is_movement_disabled(world, npc_oid) {
        return;
    }
    let (speed, start, region) = {
        let Some(speed) = world.objects.get_component::<Speeds>(&npc_oid).map(Speeds::move_speed) else { return };
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() else { return };
        let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) else { return };
        (speed, (pos.x, pos.y, pos.z), region)
    };
    if speed <= 0.0 {
        return;
    }
    let dx = (x - start.0) as f64;
    let dy = (y - start.1) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
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
    broadcast_near_region(world, region, &server_packets::move_to_location(npc_oid, x, y, z, start.0, start.1, start.2));
}
