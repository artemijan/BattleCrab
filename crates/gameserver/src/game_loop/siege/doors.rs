//! Castle doors, the artifact engraving, battlefield despawn and the
//! end-of-siege ousting teleports.

use super::capture;
use super::teleport_all_to_town;
use crate::db::DbCommand;
use crate::game_loop::clans::clan_of_or_zero;
use crate::game_loop::space::position::maybe_position;
use crate::model::Player;
use crate::model::castle::CastleSide;
use crate::model::components::space::Position;
use crate::model::door::Door;
use crate::network::server_packets::{SmParam, sm_ids};
use crate::world::World;
/// Despawn every NPC spawned for this siege (Java `removeSiegeGuards` + the
/// control/flame towers — the latter unported yet).
pub(super) fn despawn_siege_npcs(world: &mut World, castle_id: i32) {
    let oids = world
        .sieges
        .get_mut(&castle_id)
        .map(|s| std::mem::take(&mut s.spawned_npcs))
        .unwrap_or_default();
    for oid in oids {
        crate::game_loop::npc::despawn_npc_by_oid(world, oid);
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
            crate::game_loop::npc::doors::close_door(world, oid);
        }
    }
}

/// Whether `door_oid` is a castle door standing in a siege zone whose siege is
/// in progress — i.e. currently attackable/breachable (Java `Door.isAttackable`
/// during a siege).
pub(crate) fn attackable_door(world: &World, door_oid: i32) -> bool {
    world.objects.has_component::<Door>(&door_oid)
        && crate::game_loop::combat::pvp::active_siege_castle(world, door_oid).is_some()
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
        crate::game_loop::npc::doors::open_door(world, door_oid); // breach — the gate swings open
    } else {
        // Java `Door.reduceCurrentHp` → `broadcastStatusUpdate`: the gate's HP
        // and its 0..6 crack grade go out on **every** hit, which is what makes
        // a gate under attack visibly fall apart. Only the breach was announced
        // before, so a besieged gate looked untouched until it burst open.
        crate::game_loop::npc::doors::broadcast_status(world, door_oid);
    }
    breached
}

/// `Castle.engrave(clan, target, side)`, reached from the `TakeCastle` effect
/// of the leader's Imprint skills: the castle the engraver stands in goes to
/// their clan, on `side`, if `artifact_oid` is one of that castle's Holy
/// Artifacts.
///
/// Like Java this checks the artifact and nothing else — that the engraver is
/// an attacking clan's leader is `PossessHolything`'s job, and the cast does
/// not start without it.
pub(crate) fn engrave(world: &mut World, player_oid: i32, artifact_oid: i32, side: CastleSide) {
    // `TakeCastle.instant`: `if (!effector.isPlayer()) return;`.
    if !world.objects.has_component::<Player>(&player_oid) {
        return;
    }
    // `CastleManager.getCastle(effector)`.
    let Some(castle_id) = maybe_position(world, player_oid)
        .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
    else {
        return;
    };
    // `if (!_artefacts.contains(target)) return;` — an artifact registers with
    // the castle whose grounds it spawns on.
    let on_this_castle = crate::game_loop::skills::cast::is_artefact(world, artifact_oid)
        && maybe_position(world, artifact_oid)
            .and_then(|p| world.data.zone_data.siege_castle_at(p.x, p.y, p.z))
            == Some(castle_id);
    let clan_id = clan_of_or_zero(world, player_oid);
    if !on_this_castle || clan_id == 0 {
        return;
    }

    // `setSide(side)`.
    if let Some(castle) = world.castle_mut(castle_id) {
        castle.side = side;
    }
    let _ = world.db.send(DbCommand::UpdateCastleSide {
        castle_id,
        side: side.as_db().to_string(),
    });
    // `setOwner(clan)` → `Siege.midVictory()`.
    capture(world, castle_id, clan_id);

    // `getSiege().announceToPlayer(CLAN_S1_HAS_SUCCEEDED_IN_S2, true)` — both
    // sides of the siege hear it.
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let castle_name = world
        .castle(castle_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let params = [SmParam::Text(clan_name), SmParam::Text(castle_name)];
    for oid in super::registered_online_members(world, castle_id) {
        crate::game_loop::helpers::send_sm_to_player(
            world,
            oid,
            sm_ids::CLAN_S1_HAS_SUCCEEDED_IN_S2,
            &params,
        );
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
            // Owner clan stays; GMs (`isGM()`) stay.
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
