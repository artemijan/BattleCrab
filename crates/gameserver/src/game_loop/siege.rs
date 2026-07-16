//! Castle siege lifecycle — Java `Siege.startSiege`/`endSiege`, the timed-event
//! skeleton: announce the start to everyone, set the in-progress flag, and
//! schedule the auto-end after the siege length; the auto-end announces the
//! finish and clears the flag.
//!
//! The battlefield itself — teleport, control/flame towers, castle doors, siege
//! guards, the siege-zone PvP, and the winner/ownership change — is a later
//! milestone (TODO(G24) at the call sites).

use crate::model::components::Position;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::ms_to_ticks;

/// `SiegeManager.getSiegeLength()` — `SiegeLength = 120` (minutes) in Siege.ini.
const SIEGE_LENGTH_MIN: i32 = 120;

/// `Siege.startSiege` (lifecycle slice). Called only with a registered attacker
/// (the admin path guards that).
pub(crate) fn start_siege(world: &mut World, castle_id: i32) {
    match world.sieges.get_mut(&castle_id) {
        Some(siege) if !siege.in_progress => siege.in_progress = true,
        _ => return, // unknown castle or already in progress
    }

    // "The <castle> siege has started." + the siege sound, to everyone.
    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_STARTED, castle_id);
    broadcast_to_all(world, &server_packets::play_sound("systemmsg_eu.17"));

    // Auto-end after the siege length (Java `ScheduleEndSiegeTask`).
    let fire_at = world.tick + ms_to_ticks(SIEGE_LENGTH_MIN * 60 * 1000);
    world.scheduler.schedule(fire_at, ScheduledTask::SiegeEnd { castle_id });

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield of everyone but
    // the owning clan. TODO(G24): attackers/defenders re-enter through their
    // siege HQ / flags (unported), so for now they're simply evicted too.
    teleport_non_owners(world, castle_id);

    // TODO(G24): updatePlayerSiegeStateFlags, spawn control/flame towers +
    // castle doors + siege guards (Castle.getZone().setActive is modelled by the
    // in-progress flag the siege-zone PvP check reads).
}

/// `Siege.endSiege` — announce the finish and clear the in-progress flag.
pub(crate) fn end_siege(world: &mut World, castle_id: i32) {
    match world.sieges.get_mut(&castle_id) {
        Some(siege) if siege.in_progress => siege.in_progress = false,
        _ => return, // unknown castle or not in progress
    }

    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_FINISHED, castle_id);

    // `teleportPlayer(NotOwner, TOWN)` — clear the battlefield at the end too.
    teleport_non_owners(world, castle_id);

    // TODO(G24): determine the winner + change ownership, despawn towers/guards
    // (Siege.endSiege).
}

/// `Siege.teleportPlayer(NotOwner, TOWN)`: send every player standing in the
/// castle's siege zone who isn't in the owning clan (nor a GM) to their nearest
/// town.
fn teleport_non_owners(world: &mut World, castle_id: i32) {
    let owner_clan_id = world.clans.values().find(|c| c.castle_id == castle_id).map(|c| c.id).unwrap_or(0);
    // Collect first — teleporting mutates the world and re-runs visibility.
    let targets: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .filter(|&oid| {
            let Some(p) = world.objects.get_component::<Player>(&oid) else { return false };
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

    for oid in targets {
        let Some(pos) = world.objects.get_component::<Position>(&oid).copied() else { continue };
        let race = world
            .objects
            .get_component::<Player>(&oid)
            .and_then(|p| crate::enums::Race::from_ordinal(p.race))
            .unwrap_or(crate::enums::Race::Human);
        if let Some((x, y, z)) = world.data.map_region.town_respawn(pos.x, pos.y, pos.z, race, 0) {
            super::death::teleport_player(world, oid, x, y, z);
        }
    }
}

/// Broadcast `SystemMessage(id, castleName = castle_id)` to every online player.
fn broadcast_sm(world: &World, message_id: i16, castle_id: i32) {
    let pkt = server_packets::system_message_with(message_id, &[SmParam::CastleName(castle_id)]);
    broadcast_to_all(world, &pkt);
}

/// Java `Broadcast.toAllOnlinePlayers`.
fn broadcast_to_all(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.to_vec());
        }
    }
}
