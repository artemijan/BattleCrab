//! Small send/broadcast/range helpers shared by the packet handlers.

use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// The client id of the in-game session linked to a `Player`, or `None` if
/// they've disconnected since the task was scheduled (dead-id ⇒ no-op, per
/// the scheduler's contract).
pub(crate) fn client_for_player(world: &World, player_object_id: i32) -> Option<u32> {
    world.clients.iter().find_map(|(&cid, cs)| match cs {
        ClientSession::InGame(s) if s.player_object_id() == player_object_id => Some(cid),
        _ => None,
    })
}

/// Send `packet` to every in-game player that can see `from_object_id`,
/// excluding the broadcaster — Java `Creature.broadcastPacket(packet)` via
/// `World.forEachVisibleObject`: only players whose world region is in the
/// broadcaster's 3×3 surrounding-region block receive it.
pub(crate) fn broadcast_to_others(world: &World, from_object_id: i32, packet: &[u8]) {
    let Some(from) = world.players.get(&from_object_id) else { return };
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let other_id = s.player_object_id();
            if other_id == from_object_id {
                continue;
            }
            let Some(other) = world.players.get(&other_id) else { continue };
            if crate::world::regions_adjacent(from.region, other.region) {
                cs.send(packet.to_vec());
            }
        }
    }
}

/// Send `packet` to every in-game player whose region cell is adjacent to
/// `region` — the broadcast shape for NPC-originated packets (Java
/// `Npc.broadcastPacket`; NPCs never hold a session, so there is no
/// self/others split).
pub(crate) fn broadcast_near_region(world: &World, region: (i32, i32), packet: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(s) = cs {
            let Some(p) = world.players.get(&s.player_object_id()) else { continue };
            if crate::world::regions_adjacent(region, p.region) {
                cs.send(packet.to_vec());
            }
        }
    }
}

/// Round a millisecond duration up to whole 100 ms ticks.
pub(crate) fn ms_to_ticks(ms: i32) -> u64 {
    (ms.max(0) as u64).div_ceil(100)
}

/// Send a `SystemMessage` + `ActionFailed` to one client — the standard
/// "request rejected" reply shape all over `Player.useMagic` /
/// `SkillCaster.checkUseConditions`.
pub(crate) fn send_sm_and_action_failed(world: &World, client_id: u32, message_id: i16, params: &[server_packets::SmParam]) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::system_message_with(message_id, params));
        cs.send(server_packets::action_failed());
    }
}

/// Send `packet` to a player's own client (if still connected) and every
/// player that can see them — Java `Creature.broadcastPacket(packet)` with
/// `includeSelf == true`.
pub(crate) fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    if let Some(client_id) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(packet.to_vec());
        }
    }
    broadcast_to_others(world, object_id, packet);
}


