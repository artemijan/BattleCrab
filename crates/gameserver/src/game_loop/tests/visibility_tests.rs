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

/// A clan leader coming into view sends the observer a `RelationChanged` with
/// the `RELATION_LEADER` (0x80) crown bit — even with no siege — because
/// `CharInfo` carries no is-leader field (Java `Player.sendInfo`).
#[test]
fn clan_leader_crown_relation_sent_on_entering_view() {
    use crate::model::Player;
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let _leader_rx = ingame_player(&mut world, 1, 6401, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&6401).unwrap();
        p.clan_id = 7;
        p.clan_leader = true;
    }
    let mut obs_rx = ingame_player(&mut world, 2, 6402, 200, 0, 0);
    // The observer's knownlist add exchanges CharInfo + RelationChanged.
    super::visibility::on_enter_world(&world, 2, 6402);
    let saw_crown = drain(&mut obs_rx).iter().any(|p| {
        p[0] == server_packets::opcodes::RELATION_CHANGED
            && i32::from_le_bytes(p[2..6].try_into().unwrap()) == 6401
            && i32::from_le_bytes(p[6..10].try_into().unwrap()) & 0x80 != 0
    });
    assert!(saw_crown, "leader entering view sends RelationChanged with the 0x80 crown bit");
}
