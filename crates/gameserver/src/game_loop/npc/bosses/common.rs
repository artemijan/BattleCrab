use crate::game_loop::helpers::send_sm_bare_to_player;
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
    crate::game_loop::space::zones::players_in_zone(world, zone)
}

/// Is the object inside a boss zone? Falls **open** when the zone table isn't
/// loaded (minimal test worlds) — the dist always carries these zones, so the
/// gates keyed on this never misfire in production.
pub(crate) fn in_boss_zone(world: &World, zone_id: i32, oid: i32) -> bool {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::space::Position>(&oid)
    else {
        return false;
    };
    world
        .data
        .zone_data
        .by_id(zone_id)
        .is_none_or(|z| z.contains(pos.x, pos.y, pos.z))
}

/// The strict variant of [`in_boss_zone`]: a missing zone row counts as
/// **outside** — for the anti-exploit gates (Valakas' kill-from-outside rule)
/// where falling open would punish nobody but falling closed must not punish
/// everybody in a world that genuinely has the zone.
pub(crate) fn in_boss_zone_strict(world: &World, zone_id: i32, oid: i32) -> bool {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::space::Position>(&oid)
    else {
        return false;
    };
    world
        .data
        .zone_data
        .by_id(zone_id)
        .is_some_and(|z| z.contains(pos.x, pos.y, pos.z))
}
