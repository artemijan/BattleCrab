//! Instances (G27) slice 1: id allocation + cross-instance invisibility.

use super::*;

use crate::model::components::InstanceId;
use crate::network::server_packets::opcodes;

#[test]
fn instance_manager_allocates_unique_ids() {
    let (mut world, _tx, _db, _l) = test_world();
    let a = world.instances.create(0);
    let b = world.instances.create(0);
    assert_ne!(a, b, "distinct ids");
    assert!(a >= 1 && b >= 1, "0 is reserved for the overworld");
    assert!(world.instances.contains(a) && world.instances.contains(b));

    world.instances.destroy(a);
    assert!(!world.instances.contains(a), "destroyed");
    assert!(world.instances.contains(b), "the other survives");
}

fn saw_char_info(packets: &[Vec<u8>]) -> bool {
    packets
        .iter()
        .any(|p| p.first() == Some(&opcodes::CHAR_INFO))
}

#[test]
fn players_share_visibility_only_within_an_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    // Two players standing on the exact same spot.
    let mut rx_a = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    let _rx_b = ingame_player(&mut world, 2, 200, 1000, 1000, 0);

    // Same instance (both overworld) → they see each other on enter.
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        saw_char_info(&drain(&mut rx_a)),
        "same instance: CharInfo exchanged"
    );

    // Move A into a private instance → no cross-instance CharInfo.
    world.objects.add_components(&100, InstanceId(7));
    crate::game_loop::visibility::on_enter_world(&world, 1, 100);
    assert!(
        !saw_char_info(&drain(&mut rx_a)),
        "different instances: invisible to each other"
    );
}

#[test]
fn broadcast_is_scoped_to_the_instance() {
    let (mut world, _tx, _db, _l) = test_world();
    let _rx_a = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    let mut rx_b = ingame_player(&mut world, 2, 200, 1000, 1000, 0);
    world.objects.add_components(&100, InstanceId(7)); // A alone in instance 7

    crate::game_loop::helpers::broadcast_to_others(&world, 100, &[0xAB, 0xCD]);

    assert!(
        drain(&mut rx_b).is_empty(),
        "an overworld player receives nothing from an instanced broadcaster"
    );
}
