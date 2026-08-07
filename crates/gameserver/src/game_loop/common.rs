use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::network::server_packets::sm_ids;
use crate::world::World;

pub(crate) fn maybe_distance_too_far(world: &World, player_object_id: i32) {
    send_sm_bare_to_player(
        world,
        player_object_id,
        sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED,
    );
}
