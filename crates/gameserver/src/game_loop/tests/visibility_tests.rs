use super::*;

/// Leaving the world (logout here; restart/disconnect share the path)
/// broadcasts `DeleteObject` to everyone watching and drops their target
/// (Java `deleteMe` → `World.removeVisibleObject`).
#[test]
fn leave_world_sends_delete_object_to_watchers() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leaver_rx = ingame_player(&mut world, 1, 6301, 0, 0, 0);
    let mut near_rx = ingame_player(&mut world, 2, 6302, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 3, 6303, 10_000, 10_000, 0);
    world.objects.get_component_mut::<TargetRef>(&6302).unwrap().0 = Some(6301);

    handle_logout(&mut world, 1);

    let to_near = drain(&mut near_rx);
    assert_eq!(to_near[0][0], server_packets::opcodes::TARGET_UNSELECTED, "ring released before the delete");
    assert_eq!(delete_object_id(&to_near[1]), 6301);
    assert_eq!(world.objects.get_component::<TargetRef>(&6302).unwrap().0, None, "dangling target dropped");
    assert!(far_rx.try_recv().is_err());
}
