//! NPC movement: the leash snap-back and the chase/geo-move walk paths
//! shared by every think.

use super::clear_aggro;
use super::set_active;
use super::stop_npc;
use crate::game_loop::abnormal;
use crate::game_loop::combat;
use crate::game_loop::death;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::broadcast_near_region_in;
use crate::game_loop::helpers::instance_of;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::minions;
use crate::game_loop::walkers::WalkState;
use crate::model::components::Movement;
use crate::model::components::Position;
use crate::model::components::Speeds;
use crate::model::components::Vitals;
use crate::model::movement::MoveData;
use crate::network::server_packets;
use crate::world::World;
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
pub(super) fn npc_leash_return_home(world: &mut World, npc_oid: i32) -> bool {
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
    clear_aggro(world, npc_oid);
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
pub(super) fn chase(world: &mut World, npc_oid: i32, target_oid: i32, reach: f64) {
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
    let heading = crate::model::movement::calculate_heading(dx, dy);
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
