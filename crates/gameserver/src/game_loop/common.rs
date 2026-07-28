use crate::game_loop::helpers::client_for_player;
use crate::network::server_packets;
use crate::network::server_packets::sm_ids;
use crate::world::World;

pub(crate) fn maybe_distance_too_far(world: &World, player_object_id: i32) {
    if let Some(client_id) = client_for_player(world, player_object_id)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(server_packets::system_message_with(
            sm_ids::DISTANCE_TOO_FAR_CASTING_CANCELLED,
            &[],
        ));
    }
}
