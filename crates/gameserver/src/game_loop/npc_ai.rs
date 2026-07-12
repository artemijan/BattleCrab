//! `AttackableAI` (G9 slice): the 1 s think tick over monsters in active
//! regions — aggro-range scans, chasing, swinging back, drift-return.
//!
//! Not ported yet (see PROGRESS): random walk / `randomAnimation`, guard
//! aggro (karma players don't exist), clan/faction help calls, minions, NPC
//! skill casting (`AISkillScope` lists aren't parsed), the archer kite and
//! raid target-chaos moves, and Java's teleport-home on attack timeout
//! (walking home is used instead — no teleport plumbing for NPCs).

use std::collections::HashSet;

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
    }
}

fn think(world: &mut World, npc_oid: i32) {
    let Some(npc) = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid) else { return };
    if world.objects.get_component::<Vitals>(&npc_oid).is_none_or(|v| v.dead) {
        return;
    }
    let Some(t) = npc.template(world) else { return };
    // Only the Attackable subtree has this AI; the slice narrows further to
    // monsters (guards need the karma system to have anything to do).
    if !t.is_monster() {
        return;
    }
    let _ = npc;
    let Some(ai) = world.objects.get_component::<NpcAi>(&npc_oid) else { return };
    match ai.intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
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
        (t.map(|t| t.aggro_range > 0).unwrap_or(false), t.map(|t| t.aggro_range).unwrap_or(0))
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

    // No target: return to the spawn anchor when drifted too far
    // (`Config.MAX_DRIFT_RANGE`); random walk stays unported.
    let max_drift = world.cfg.npc.max_drift_range as f64;
    let (x, y, spawn, moving, can_move) = {
        let npc = &world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).expect("npc");
        let pos = world.objects.get_component::<Position>(&npc_oid).expect("caller checked");
        let can_move = npc.template(world).map(|t| t.can_move).unwrap_or(false);
        (pos.x, pos.y, npc.spawn_loc, world.objects.has_component::<Movement>(&npc_oid), can_move)
    };
    if can_move && !moving {
        let dist = (((spawn.0 - x) as f64).powi(2) + ((spawn.1 - y) as f64).powi(2)).sqrt();
        if dist > max_drift {
            move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
        }
    }
}

/// `AttackableAI.thinkAttack`: validate the hated target, time out, chase,
/// swing.
fn think_attack(world: &mut World, npc_oid: i32) {
    let now = world.tick;

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
        }),
    );
    broadcast_near_region(world, region, &server_packets::move_to_location(npc_oid, x, y, z, start.0, start.1, start.2));
}
