//! Broadcast helpers over regions and onlookers.

use super::client_for_player;
use super::instance_of;
use super::region_cell_of;
use crate::world::World;
/// `World.forEachVisibleObject`: only players whose world region is in the
/// broadcaster's 3×3 surrounding-region block **and same instance** receive it.
pub(crate) fn broadcast_to_others(world: &World, from_object_id: i32, packet: &[u8]) {
    // The packet is copied into `Bytes` once and refcounted from there, instead
    // of `to_vec()`-ing it per recipient — a crowded region turned one
    // broadcast into dozens of allocations on the game thread.
    broadcast_to_others_shared(world, from_object_id, bytes::Bytes::copy_from_slice(packet));
}

/// [`broadcast_to_others`] for a payload already in `Bytes` —
/// `broadcast_including_self` shares one buffer between the self-send and the
/// onlookers instead of copying the packet twice.
fn broadcast_to_others_shared(world: &World, from_object_id: i32, shared: bytes::Bytes) {
    use crate::model::components::RegionCell;
    let Some(from) = world.objects.get_component::<RegionCell>(&from_object_id) else {
        return;
    };
    let from_region = from.0;
    let from_instance = instance_of(world, from_object_id);
    // The 3×3 block *is* the recipient set, so walk the region index rather
    // than every connected client. Indexed players without a session (the
    // unattended shops) simply resolve to no client and are skipped, which is
    // what the old session scan did by never seeing them.
    for other_id in world.players_visible_from(from_region) {
        if other_id == from_object_id {
            continue;
        }
        if instance_of(world, other_id) != from_instance {
            continue;
        }
        if let Some(cs) = world.clients.session_of_player(other_id) {
            cs.send(shared.clone());
        }
    }
}
/// Send `packet` to every in-game player in `instance` whose region cell is
/// adjacent to `region` — the broadcast shape for NPC-originated packets (Java
/// `Npc.broadcastPacket`; NPCs never hold a session, so there is no self/others
/// split), scoped to the source's instance so instanced content stays private
/// (G27). `broadcast_near_region` is this with the overworld (instance 0).
pub(crate) fn broadcast_near_region_in(
    world: &World,
    region: (i32, i32),
    instance: i32,
    packet: &[u8],
) {
    // One `Bytes` for the whole block; see `broadcast_to_others`.
    let shared = bytes::Bytes::copy_from_slice(packet);
    for oid in world.players_visible_from(region) {
        if instance_of(world, oid) != instance {
            continue;
        }
        if let Some(cs) = world.clients.session_of_player(oid) {
            cs.send(shared.clone());
        }
    }
}

/// [`broadcast_near_region_in`] fixed to the overworld (instance 0) — the shape
/// for NPC packets that only ever originate in the open world (boats, fishing,
/// cursed weapons, town social actions, …).
pub(crate) fn broadcast_near_region(world: &World, region: (i32, i32), packet: &[u8]) {
    broadcast_near_region_in(world, region, 0, packet);
}

/// Send `packet` to a player's own client (if still connected) and every
/// player that can see them — Java `Creature.broadcastPacket(packet)` with
/// `includeSelf == true`.
pub(crate) fn broadcast_including_self(world: &World, object_id: i32, packet: &[u8]) {
    // One `Bytes` for the mover and every onlooker alike.
    let shared = bytes::Bytes::copy_from_slice(packet);
    if let Some(client_id) = client_for_player(world, object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(shared.clone());
    }
    broadcast_to_others_shared(world, object_id, shared);
}

/// Broadcast to every player in the 3×3 region block around `object_id` —
/// the `region_cell_of` + [`broadcast_near_region`] pair that a couple dozen
/// call sites spelled out by hand. A no-op for an object with no region (left
/// the world mid-task).
pub(crate) fn broadcast_from(world: &World, object_id: i32, packet: &[u8]) {
    if let Some(region) = region_cell_of(world, object_id) {
        broadcast_near_region(world, region, packet);
    }
}
