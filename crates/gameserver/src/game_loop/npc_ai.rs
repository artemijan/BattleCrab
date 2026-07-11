//! `AttackableAI` (G9 slice): the 1 s think tick over monsters in active
//! regions — aggro-range scans, chasing, swinging back, drift-return.
//!
//! Not ported yet (see PROGRESS): random walk / `randomAnimation`, guard
//! aggro (karma players don't exist), clan/faction help calls, minions, NPC
//! skill casting (`AISkillScope` lists aren't parsed), the archer kite and
//! raid target-chaos moves, and Java's teleport-home on attack timeout
//! (walking home is used instead — no teleport plumbing for NPCs).

use std::collections::HashSet;

use crate::model::movement::{self, MoveData};
use crate::model::npc::NpcIntention;
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
            if let Some(p) = world.players.get(&s.player_object_id()) {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        active.insert((p.region.0 + dx, p.region.1 + dy));
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
    let Some(npc) = world.npcs.get(&npc_oid) else { return };
    if npc.dead {
        return;
    }
    let Some(t) = npc.template(world) else { return };
    // Only the Attackable subtree has this AI; the slice narrows further to
    // monsters (guards need the karma system to have anything to do).
    if !t.is_monster() {
        return;
    }
    match npc.intention {
        NpcIntention::Active => think_active(world, npc_oid),
        NpcIntention::Attack => think_attack(world, npc_oid),
    }
}

/// `AttackableAI.thinkActive`: tick `_globalAggro` toward 0, scan the aggro
/// range, pick the most hated, or drift back home.
fn think_active(world: &mut World, npc_oid: i32) {
    let (aggressive, aggro_range, region) = {
        let npc = world.npcs.get_mut(&npc_oid).expect("caller checked");
        if npc.global_aggro != 0 {
            npc.global_aggro += if npc.global_aggro < 0 { 1 } else { -1 };
        }
        let t = world.data.npc_data.get(npc.npc_id);
        (
            t.map(|t| t.aggro_range > 0).unwrap_or(false),
            t.map(|t| t.aggro_range).unwrap_or(0),
            npc.region,
        )
    };

    // Aggro-range scan (`isAggressiveTowards` narrowed: alive, in range,
    // geodata-visible; invisibility/silent-move/GM states don't exist).
    if aggressive && world.npcs[&npc_oid].global_aggro >= 0 {
        let (nx, ny, nz) = {
            let npc = &world.npcs[&npc_oid];
            (npc.x, npc.y, npc.z)
        };
        let in_range: Vec<i32> = world
            .players
            .values()
            .filter(|p| {
                !p.dead
                    && regions_adjacent(region, p.region)
                    && (((p.x - nx) as f64).powi(2) + ((p.y - ny) as f64).powi(2)).sqrt() <= aggro_range as f64
                    && world.geo.can_see_target(nx, ny, nz, p.x, p.y, p.z)
            })
            .map(|p| p.object_id)
            .collect();
        if let Some(npc) = world.npcs.get_mut(&npc_oid) {
            for player_oid in in_range {
                // `addDamageHate(t, 0, 0)` → first sight seeds 1 hate.
                let entry = npc.aggro.entry(player_oid).or_default();
                if entry.hate == 0.0 {
                    entry.hate = 1.0;
                }
            }
        }
    }

    // Chose a target from the aggro list (`getMostHated`).
    let hated = world.npcs[&npc_oid].most_hated();
    if let Some(target) = hated {
        let aggro = world.npcs[&npc_oid].aggro.get(&target).map(|a| a.hate).unwrap_or(0.0);
        if aggro + world.npcs[&npc_oid].global_aggro as f64 > 0.0 {
            let became_running = {
                let npc = world.npcs.get_mut(&npc_oid).expect("checked");
                let flip = !npc.running;
                npc.running = true;
                npc.intention = NpcIntention::Attack;
                npc.attack_timeout_tick = world.tick + ATTACK_TIMEOUT_TICKS;
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
        let npc = &world.npcs[&npc_oid];
        let can_move = npc.template(world).map(|t| t.can_move).unwrap_or(false);
        (npc.x, npc.y, npc.spawn_loc, npc.move_data.is_some(), can_move)
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

    let target = world.npcs[&npc_oid].most_hated();
    let Some(target_oid) = target else {
        set_active(world, npc_oid);
        return;
    };

    // Target dead or gone → stop hating it (next think re-evaluates).
    let target_alive = world.players.get(&target_oid).is_some_and(|p| !p.dead);
    if !target_alive {
        if let Some(npc) = world.npcs.get_mut(&npc_oid) {
            npc.aggro.remove(&target_oid);
        }
        return;
    }

    // Attack timeout: give up, forget everyone, walk home (Java teleports —
    // see the module note).
    if world.npcs[&npc_oid].attack_timeout_tick < now {
        let spawn = {
            let npc = world.npcs.get_mut(&npc_oid).expect("checked");
            npc.aggro.clear();
            npc.spawn_loc
        };
        set_active(world, npc_oid);
        move_npc_to(world, npc_oid, spawn.0, spawn.1, spawn.2);
        return;
    }

    // Busy swinging.
    if world.npcs[&npc_oid].attack_end_tick > now {
        return;
    }

    let Some(attacker) = combat::combatant(world, npc_oid) else { return };
    let Some(victim) = combat::combatant(world, target_oid) else { return };
    let reach = attacker.atk_range as f64 + attacker.collision_radius + victim.collision_radius;
    let dist = (((victim.x - attacker.x) as f64).powi(2) + ((victim.y - attacker.y) as f64).powi(2)).sqrt();

    let can_move = world.npcs[&npc_oid].template(world).map(|t| t.can_move).unwrap_or(false);
    if dist > reach {
        if can_move {
            chase(world, npc_oid, target_oid, reach);
        }
        return;
    }

    // In reach: stop and swing.
    if world.npcs.get(&npc_oid).is_some_and(|n| n.move_data.is_some()) {
        let (x, y, z, heading, region) = {
            let npc = world.npcs.get_mut(&npc_oid).expect("checked");
            npc.move_data = None;
            (npc.x, npc.y, npc.z, npc.heading, npc.region)
        };
        broadcast_near_region(world, region, &server_packets::stop_move(npc_oid, x, y, z, heading));
    }
    combat::do_auto_attack(world, npc_oid, target_oid);
}

/// Back to the scan loop: walking move type + Active intention (Java
/// `setIntention(AI_INTENTION_ACTIVE)` + `setWalking`).
fn set_active(world: &mut World, npc_oid: i32) {
    let (was_running, region) = {
        let Some(npc) = world.npcs.get_mut(&npc_oid) else { return };
        let was = npc.running;
        npc.running = false;
        npc.intention = NpcIntention::Active;
        (was, npc.region)
    };
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
        let npc = &world.npcs[&npc_oid];
        let t = npc.template(world);
        let speed = t.map(|t| if npc.running { t.base_run_spd } else { t.base_walk_spd }).unwrap_or(0.0);
        (speed, (npc.x, npc.y, npc.z), npc.region)
    };
    if speed <= 0.0 {
        return;
    }
    let distance = (((dest_x - start.0) as f64).powi(2) + ((dest_y - start.1) as f64).powi(2)).sqrt();
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    let start_tick = world.tick;
    if let Some(npc) = world.npcs.get_mut(&npc_oid) {
        npc.heading = heading;
        npc.move_data = Some(MoveData {
            start_x: start.0,
            start_y: start.1,
            start_z: start.2,
            dest_x,
            dest_y,
            dest_z,
            start_tick,
            total_ticks,
        });
    }
    broadcast_near_region(
        world,
        region,
        &server_packets::move_to_pawn(npc_oid, target_oid, reach as i32, start.0, start.1, start.2, target.x, target.y, target.z),
    );
}

/// A plain destination walk (return-home) with a `MoveToLocation` broadcast.
fn move_npc_to(world: &mut World, npc_oid: i32, x: i32, y: i32, z: i32) {
    let (speed, start, region) = {
        let Some(npc) = world.npcs.get(&npc_oid) else { return };
        let t = npc.template(world);
        let speed = t.map(|t| if npc.running { t.base_run_spd } else { t.base_walk_spd }).unwrap_or(0.0);
        (speed, (npc.x, npc.y, npc.z), npc.region)
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
    if let Some(npc) = world.npcs.get_mut(&npc_oid) {
        npc.heading = heading;
        npc.move_data = Some(MoveData {
            start_x: start.0,
            start_y: start.1,
            start_z: start.2,
            dest_x: x,
            dest_y: y,
            dest_z: z,
            start_tick,
            total_ticks,
        });
    }
    broadcast_near_region(world, region, &server_packets::move_to_location(npc_oid, x, y, z, start.0, start.1, start.2));
}
