//! Castle siege lifecycle — Java `Siege.startSiege`/`endSiege`, the timed-event
//! skeleton: announce the start to everyone, set the in-progress flag, and
//! schedule the auto-end after the siege length; the auto-end announces the
//! finish and clears the flag.
//!
//! The battlefield itself — teleport, control/flame towers, castle doors, siege
//! guards, the siege-zone PvP, and the winner/ownership change — is a later
//! milestone (TODO(G24) at the call sites).

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

    // TODO(G24): updatePlayerSiegeStateFlags, teleport non-participants out,
    // spawn control/flame towers + castle doors + siege guards, and activate the
    // siege zone (Castle.getZone().setActive).
}

/// `Siege.endSiege` — announce the finish and clear the in-progress flag.
pub(crate) fn end_siege(world: &mut World, castle_id: i32) {
    match world.sieges.get_mut(&castle_id) {
        Some(siege) if siege.in_progress => siege.in_progress = false,
        _ => return, // unknown castle or not in progress
    }

    broadcast_sm(world, sm_ids::THE_S1_SIEGE_HAS_FINISHED, castle_id);

    // TODO(G24): determine the winner + change ownership, despawn towers/guards,
    // deactivate the siege zone, teleport participants out (Siege.endSiege).
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
