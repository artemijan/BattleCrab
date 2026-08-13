use crate::game_loop::helpers::{in_zone, send_sm_bare_to_player};
use crate::network::server_packets::sm_ids;
use crate::world::World;
/// Gather range for the leader's party (Java `isInsideRadius3D(npc, 1000)`).
const GATHER_RANGE: f64 = 1000.0;
pub(crate) fn maybe_distance_too_far(world: &World, player_object_id: i32) {
    send_sm_bare_to_player(
        world,
        player_object_id,
        sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED,
    );
}

pub fn near_leader(world: &World, leader: i32, member: i32) -> bool {
    if leader == member {
        return true;
    }
    crate::geo::distance::within_2d(world, leader, member, GATHER_RANGE)
}

/// Object ids of the online players standing in the lair zone.
pub fn players_in_lair_oids(world: &World, zone: i32) -> Vec<i32> {
    let Some(zone) = world.data.zone_data.by_id(zone) else {
        return Vec::new();
    };
    world
        .in_game_player_oids()
        .filter(|oid| in_zone(world, *oid, zone))
        .collect()
}
