//! Castle doors, the artifact capture, battlefield despawn and the
//! end-of-siege ousting teleports.

use super::*;
/// Despawn every NPC spawned for this siege (Java `removeSiegeGuards` + the
/// control/flame towers — the latter unported yet).
pub(super) fn despawn_siege_npcs(world: &mut World, castle_id: i32) {
    let oids = world
        .sieges
        .get_mut(&castle_id)
        .map(|s| std::mem::take(&mut s.spawned_npcs))
        .unwrap_or_default();
    for oid in oids {
        crate::game_loop::death::despawn_npc_by_oid(world, oid);
    }
}

/// The object ids of a castle's doors — the doors standing inside its siege
/// zone (Java `Door.getCastle()` is region-based; the siege-zone polygon is the
/// port's proxy for the castle grounds).
fn castle_door_oids(world: &World, castle_id: i32) -> Vec<i32> {
    world
        .door_regions
        .values()
        .flatten()
        .copied()
        .filter(|&oid| {
            world
                .objects
                .get_component::<Position>(&oid)
                .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
                == Some(castle_id)
        })
        .collect()
}

/// Java `Castle.spawnDoor(isDoorWeak)`: revive breached doors to full (or half)
/// HP and close any open ones. Called at siege start/end (full) and on capture
/// (weak, 50%).
pub(super) fn spawn_castle_doors(world: &mut World, castle_id: i32, weak: bool) {
    for oid in castle_door_oids(world, castle_id) {
        let (door_id, dead) = match world.objects.get_component::<Door>(&oid) {
            Some(d) => (d.door_id, d.current_hp <= 0),
            None => continue,
        };
        if dead {
            let max_hp = world
                .data
                .door_data
                .get(door_id)
                .map(|t| t.hp_max)
                .unwrap_or(1);
            if let Some(d) = world.objects.get_component_mut::<Door>(&oid) {
                d.current_hp = if weak { (max_hp / 2).max(1) } else { max_hp };
            }
        }
        if world.geo.doors.is_open(door_id) {
            crate::game_loop::doors::close_door(world, oid);
        }
    }
}

/// Whether `door_oid` is a castle door standing in a siege zone whose siege is
/// in progress — i.e. currently attackable/breachable (Java `Door.isAttackable`
/// during a siege).
pub(crate) fn attackable_door(world: &World, door_oid: i32) -> bool {
    world.objects.has_component::<Door>(&door_oid)
        && crate::game_loop::pvp::active_siege_castle(world, door_oid).is_some()
}

/// Apply siege damage to a castle door; at 0 HP it's breached (opens). Returns
/// whether it broke on this hit. Driven by the melee path against a targeted
/// door (`combat::attack_door`).
pub(crate) fn damage_door(world: &mut World, door_oid: i32, damage: i32) -> bool {
    let breached = {
        let Some(d) = world.objects.get_component_mut::<Door>(&door_oid) else {
            return false;
        };
        if d.current_hp <= 0 {
            return false; // already breached
        }
        d.current_hp = (d.current_hp - damage).max(0);
        d.current_hp == 0
    };
    if breached {
        // `open_door` broadcasts the new state itself, HP included.
        crate::game_loop::doors::open_door(world, door_oid); // breach — the gate swings open
    } else {
        // Java `Door.reduceCurrentHp` → `broadcastStatusUpdate`: the gate's HP
        // and its 0..6 crack grade go out on **every** hit, which is what makes
        // a gate under attack visibly fall apart. Only the breach was announced
        // before, so a besieged gate looked untouched until it burst open.
        crate::game_loop::doors::broadcast_status(world, door_oid);
    }
    breached
}

/// The throne-room Holy Artifact capture (Java `Artefact.onAction` →
/// `Castle.setOwner` → `Siege.midVictory`): an attacker clan member touching the
/// artifact during an active siege takes the castle. No-op otherwise.
pub(crate) fn try_capture_artifact(world: &mut World, player_oid: i32, artifact_oid: i32) {
    let Some(pos) = maybe_position(world, artifact_oid) else {
        return;
    };
    let Some(castle_id) = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z) else {
        return;
    };
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    let clan_id = clan_of_or_zero(world, player_oid);
    if clan_id == 0 {
        return;
    }
    // Only a registered attacker can seize the castle.
    let is_attacker = world.sieges.get(&castle_id).is_some_and(|s| {
        s.clans
            .iter()
            .any(|c| c.clan_id == clan_id && c.kind == SiegeClanType::Attacker)
    });
    if is_attacker {
        capture(world, castle_id, clan_id);
    }
}

/// `Siege.teleportPlayer(NotOwner, TOWN)`: send every player standing in the
/// castle's siege zone who isn't in the owning clan (nor a GM) to their nearest
/// town.
pub(super) fn teleport_non_owners(world: &mut World, castle_id: i32) {
    let owner_clan_id = world
        .clans
        .values()
        .find(|c| c.castle_id == castle_id)
        .map(|c| c.id)
        .unwrap_or(0);
    // Collect first — teleporting mutates the world and re-runs visibility.
    let targets: Vec<i32> = world
        .in_game_player_oids()
        .filter(|&oid| {
            let Some(p) = world.objects.get_component::<Player>(&oid) else {
                return false;
            };
            // Owner clan stays; GMs (canOverrideCond CASTLE_CONDITIONS) stay.
            if (owner_clan_id != 0 && p.clan_id == owner_clan_id) || p.is_gm(&world.data) {
                return false;
            }
            world
                .objects
                .get_component::<Position>(&oid)
                .and_then(|pos| world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z))
                == Some(castle_id)
        })
        .collect();

    teleport_all_to_town(world, targets);
}

/// Java `Castle.oustAllPlayers()` → `getTeleZone().oustAllPlayers()`: every
/// player standing in the castle's `ResidenceTeleportZone` (the owner-restart
/// territory) is dropped on one of the zone's `<spawn>` points — the inner
/// castle. Used by the mass gatekeeper's `MASS_TELEPORT`.
pub(crate) fn oust_all_players(world: &mut World, castle_id: i32) {
    let Some(zone) = world.data.zone_data.residence_teleport_zone(castle_id) else {
        return;
    };
    if world
        .data
        .zone_data
        .residence_teleport_spawns(castle_id)
        .is_empty()
    {
        return;
    }
    let (min_z, max_z) = (zone.territory.min_z, zone.territory.max_z);
    // Collect first — teleporting mutates the world and re-runs visibility.
    let inside: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| {
            world
                .objects
                .get_component::<Position>(oid)
                .is_some_and(|p| {
                    p.z >= min_z && p.z <= max_z && zone.territory.contains_2d(p.x, p.y)
                })
        })
        .collect();

    for oid in inside {
        // `ZoneRespawn.getSpawnLoc()` picks a random point per player.
        let spawns: Vec<(i32, i32, i32)> = world
            .data
            .zone_data
            .residence_teleport_spawns(castle_id)
            .to_vec();
        let idx = world.roll(spawns.len() as i32) as usize;
        let (x, y, z) = spawns[idx];
        crate::game_loop::death::teleport_player(world, oid, x, y, z);
    }
}
