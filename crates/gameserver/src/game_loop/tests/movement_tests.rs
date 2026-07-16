use super::*;

/// A move click during a cast is rejected (ActionFailed, cast keeps going)
/// but saved as the next intention, and the move starts by itself once the
/// cast stops — Java `PlayerAI.onIntentionMoveTo`'s `saveNextIntention` +
/// `onEvtFinishCasting`.
#[test]
fn move_click_during_cast_is_queued_and_replayed_when_cast_stops() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&3001).unwrap().running = true;

    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Click to move mid-cast: rejected, cast intact, click remembered.
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(world.objects.has_component::<Casting>(&3001), "the cast is not aborted");
    assert!(!world.objects.has_component::<Movement>(&3001), "no move yet");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Move { .. })));

    // Launch (35 ticks) + finish (5 more, coolTime 0 frees the slot): the
    // queued click replays through the normal move pipeline.
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(!world.objects.has_component::<QueuedAction>(&3001), "queue consumed");
    let mv = world.objects.get_component::<Movement>(&3001).expect("move started at cast end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (500, 0));
    let a_packets = drain(&mut a_rx);
    assert!(a_packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION));
    let b_packets = drain(&mut b_rx);
    assert!(b_packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION));
}

/// A move click mid-swing waits out the swing (Java `onIntentionMoveTo`'s
/// `isAttackingNow` branch) and starts at swing end via `AttackFinish` —
/// which must fire even though the click dropped the attack intent.
#[test]
fn move_click_mid_swing_defers_to_swing_end() {
    use crate::model::components::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    world.forced_rolls.extend([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world.objects.get_component::<crate::model::components::AttackState>(&3001).unwrap().attack_end_tick;

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(a_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Movement>(&3001), "no move mid-swing");
    assert!(!world.objects.has_component::<Intent>(&3001), "move click ends the attack loop");
    assert!(matches!(world.objects.get_component::<QueuedAction>(&3001), Some(QueuedAction::Move { .. })));

    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let mv = world.objects.get_component::<Movement>(&3001).expect("move started at swing end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (500, 0));
}

/// `MoveBackwardToLocation` starts a move: `move_data` is set, a
/// `MoveToLocation` is sent to the mover (the client only starts walking
/// on the server's confirmation) and broadcast to other players, and
/// `movement::tick` interpolates the position over the precomputed tick
/// count before snapping to the destination and clearing `move_data` on
/// arrival.
#[test]
fn move_backward_to_location_interpolates_and_arrives() {
    let (mut world, ..) = test_world();
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 0, 0, 0);
    let mut bystander_rx = ingame_player(&mut world, 2, 4002, 500, 500, 0);
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);
    assert!(mover_rx.try_recv().is_err(), "exactly one packet to the mover");
    assert_eq!(bystander_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);

    let total_ticks = world.objects.get_component::<Movement>(&4001).unwrap().0.total_ticks;
    assert_eq!(total_ticks, 100, "distance 1000 / speed 100 * 10 ticks-per-sec");

    // Half way: linear interpolation.
    world.tick += total_ticks / 2;
    crate::model::movement::tick(&mut world);
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (500, 0, 0));
    assert!(world.objects.has_component::<Movement>(&4001));

    // Arrival: snapped exactly, move_data cleared, no StopMove needed.
    world.tick += total_ticks / 2;
    crate::model::movement::tick(&mut world);
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (1000, 0, 0));
    assert!(!world.objects.has_component::<Movement>(&4001));
}

/// Java's `MoveBackwardToLocation` early-returns with `StopMove` +
/// `ActionFailed` when the client's echoed origin equals its target
/// (used by the client as an explicit "stop" signal) — no movement state
/// is set.
#[test]
fn move_backward_to_location_same_origin_and_target_sends_stop_move() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 5001, 10, 20, 30);

    handle_move_backward_to_location(&mut world, 1, &move_body((100, 100, 100), (100, 100, 100), 1));

    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STOP_MOVE);
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(!world.objects.has_component::<Movement>(&5001));
}

/// A click past a geodata wall used to just clamp; now a clamp that
/// shortens the move by > 30 units defers the move to the path worker
/// (Java: `CellPathFinding.findPath` inline): nothing moves yet, a
/// `PathWait` marks the pending request, and no packet is sent.
#[test]
fn move_blocked_by_wall_defers_to_path_worker() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 8, 0); // cell 0
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    // Click to cell 20 (x = 328), on the far side of the wall at cell 10:
    // the clamp to cell 9 (x = 152) shortens 320 → 144, well over 30.
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 8, 0), (8, 8, 0), 1));

    assert!(!world.objects.has_component::<Movement>(&4001), "move deferred, not started");
    assert!(world.objects.has_component::<crate::model::components::PathWait>(&4001));
    assert!(mover_rx.try_recv().is_err(), "no packet until the path reply lands");
}

/// A clamp of ≤ 30 units starts the move directly with the clamped
/// destination (`GeoEngine.getValidLocation` in `Creature.moveToLocation`) —
/// no pathfinding round-trip.
#[test]
fn move_destination_is_clamped_by_geodata() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 120, 8, 0); // cell 7
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    // Click one cell into the wall (cell 10, x = 168): clamped to cell 9
    // (x = 152), only 16 units short of the request.
    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (120, 8, 0), 1));

    let md = world.objects.get_component::<Movement>(&4001).map(|m| m.0.clone()).expect("move must start");
    assert_eq!((md.dest_x, md.dest_y), (152, 8), "clamped to cell 9, before the wall");
    let pkt = mover_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::MOVE_TO_LOCATION);
    let dest_x = i32::from_le_bytes(pkt[5..9].try_into().unwrap());
    assert_eq!(dest_x, 152, "MoveToLocation carries the clamped destination");
}

/// Full pathfinding round-trip against a real path-worker thread: a click
/// across a walled-off area (with a gap further south) starts a
/// multi-segment route move once the worker replies, route advances
/// broadcast `MoveToLocation` per segment, and the mover arrives at the
/// exact requested destination on the far side of the wall.
#[test]
fn path_worker_round_trip_walks_around_wall() {
    use crate::geo::path::PathConfig;
    use crate::geo::{synthetic_region, NSWE_ALL, NSWE_EAST};
    use crate::model::components::PathWait;

    let (mut world, ..) = test_world();
    // Mid-region wall at cell x == 10 with a gap at y ∈ [1010, 1014) — far
    // from region edges so the search can't skirt through unloaded void.
    std::sync::Arc::get_mut(&mut world.geo).expect("geo Arc not shared yet").set_region(
        20,
        18,
        synthetic_region(|x, y| {
            let in_gap = (1010..1014).contains(&y);
            if x == 10 && !in_gap {
                (200, 0)
            } else if x == 9 && !in_gap {
                (0, NSWE_ALL & !NSWE_EAST)
            } else {
                (0, NSWE_ALL)
            }
        }),
    );
    let (req_tx, req_rx) = std::sync::mpsc::channel();
    let (ev_tx, ev_rx) = std::sync::mpsc::channel();
    let worker = crate::geo::worker::spawn(world.geo.clone(), PathConfig::default(), req_rx, ev_tx);
    world.path = req_tx;

    // Player at cell (0, 1000) = (8, 16008); click to cell (20, 1000).
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 16008, 0);
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 16008, 0), (8, 16008, 0), 1));
    assert!(world.objects.has_component::<PathWait>(&4001));

    // The reply normally lands via `drain_path` on a later tick.
    let ev = ev_rx.recv_timeout(std::time::Duration::from_secs(10)).expect("worker reply");
    handle_path_result(&mut world, ev);
    assert!(!world.objects.has_component::<PathWait>(&4001));

    let md = world.objects.get_component::<Movement>(&4001).map(|m| m.0.clone()).expect("route move started");
    let path = md.geo_path.expect("move carries the geodata route");
    assert_eq!(path.index, 0);
    assert!(path.points.len() > 1, "walking around needs several segments");
    assert_eq!((path.accurate_tx, path.accurate_ty), (328, 16008));
    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::MOVE_TO_LOCATION);

    // Walk the whole route: each segment completion advances to the next
    // point and broadcasts another MoveToLocation; the last one arrives.
    let mut advances = 0;
    for _ in 0..10_000 {
        if !world.objects.has_component::<Movement>(&4001) {
            break;
        }
        world.tick += 1;
        visibility::movement_tick(&mut world);
        // Count segment-advance MoveToLocation broadcasts; ignore other
        // legitimate mid-walk packets (e.g. ExSetCompassZoneCode when the
        // route crosses into a different compass zone code).
        while let Ok(pkt) = mover_rx.try_recv() {
            if pkt[0] == server_packets::opcodes::MOVE_TO_LOCATION {
                advances += 1;
            }
        }
    }
    assert!(!world.objects.has_component::<Movement>(&4001), "route must complete");
    assert!(advances >= 1, "route advances broadcast MoveToLocation");
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y), (328, 16008), "arrived at the exact requested destination");

    drop(world);
    worker.join().unwrap();
}

/// Standing right at the wall, a click into it clamps the whole path away
/// (distance < 1) — Java cancels the movement with `ActionFailed`.
#[test]
fn move_into_wall_from_adjacent_cell_is_cancelled() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 152, 8, 0); // cell 9
    world.objects.get_component_mut::<Speeds>(&4001).unwrap().run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (152, 8, 0), 1));

    assert!(!world.objects.has_component::<Movement>(&4001), "no movement into the wall");
    assert_eq!(mover_rx.try_recv().unwrap()[0], server_packets::opcodes::ACTION_FAIL);
    assert!(mover_rx.try_recv().is_err());
}

/// `ValidatePosition` reconciliation, one branch at a time: a plausible
/// climb (|dz| 200..1500, near the last reported client z) adopts the
/// client z; moderate 2D drift is answered with `ValidateLocation` and
/// the server keeps its position; a desync beyond one second of movement
/// snaps the server to the client, geodata-correcting z downwards.
#[test]
fn validate_position_reconciles_client_and_server() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&4001).unwrap();
        speeds.run_spd = 600.0;
        speeds.running = true;
    }
    // The enter-world revalidate pushes the initial compass code; do it here
    // so the reconciliation branches below are packet-exact.
    super::zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    // Climb: z 0 → 300 with matching client-z history — trusted, silent.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 300, 0));
    assert_eq!(world.objects.get_component::<Position>(&4001).unwrap().z, 300);
    assert!(rx.try_recv().is_err(), "no correction for a trusted climb");

    // Drift: diffSq 270400 ∈ (250000, 360000), within move speed (600) —
    // server answers ValidateLocation and stays put.
    handle_validate_position(&mut world, 1, &validate_position_body(1520, 1000, 300, 0));
    assert_eq!(world.objects.get_component::<Position>(&4001).unwrap().x, 1000, "server position kept on drift");
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::VALIDATE_LOCATION);
    assert!(rx.try_recv().is_err());

    // Desync: 2000 units in one report — snap to the client, with z
    // pulled onto the geodata ground (server was above the client).
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, 0, 0));
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (3000, 1000, 0), "snapped, z on the geodata floor");
    let c = world.objects.get_component::<ClientPos>(&4001).unwrap();
    assert_eq!((c.x, c.y, c.z), (3000, 1000, 0));
}

/// `ValidatePosition` is ignored while teleporting (Java's `isTeleporting()`
/// bail): during a far teleport the client keeps reporting its OLD position
/// until the destination region loads — without the bail the desync snap
/// reverts the teleport and the client hangs on the loading screen
/// (gatekeeper → Elven Ruins regression).
#[test]
fn validate_position_ignored_while_teleporting() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);
    super::death::teleport_player(&mut world, 4001, 48765, 248461, -6160);
    // The "teleport finished" packet must reach the player, or the client
    // never leaves the loading screen.
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::EX
            && i16::from_le_bytes(p[1..3].try_into().unwrap())
                == server_packets::opcodes::EX_TELEPORT_TO_LOCATION_ACTIVATE),
        "ExTeleportToLocationActivate sent to the teleporting player"
    );

    // The stale in-flight report from the old spot must not move the server.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 0, 0));
    let pos = *world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (48765, 248461, -6155), "teleport destination kept (z lifted by 5)");
    assert!(rx.try_recv().is_err(), "no correction packet while teleporting");

    // Appearing completes the teleport; afterwards reports count again.
    super::death::handle_appearing(&mut world, 1);
    assert!(!world.objects.get_component::<Player>(&4001).unwrap().teleporting);
    handle_validate_position(
        &mut world,
        1,
        &validate_position_body(48765, 248461, -6160, 0),
    );
    let c = world.objects.get_component::<ClientPos>(&4001).unwrap();
    assert_eq!((c.x, c.y, c.z), (48765, 248461, -6160), "client pos tracked again");
}

/// A move click while walking to cast abandons the cast intention (Java: the
/// new MOVE_TO intention replaces CAST) — the player never casts.
#[test]
fn move_click_cancels_walk_to_cast() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 10;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001));

    handle_move_backward_to_location(&mut world, 1, &move_body((0, 300, 0), (0, 0, 0), 1));
    assert!(!world.objects.has_component::<Intent>(&3001), "move click drops the walk-to-cast");
    advance_world(&mut world, 60);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let packets = drain(&mut a_rx);
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "the cast never fires");
}

/// Selecting another target mid-walk must NOT drop the cast: Java's
/// `RequestTargetCanceld` (which the client also sends on a target switch)
/// never touches the AI intention, and `thinkCast` casts at the intention's
/// snapshotted target even after a re-target.
#[test]
fn retarget_mid_walk_keeps_cast_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_a = NPC_OID + 60;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_a, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // Walk a couple of ticks, then switch to monster B — the client emits
    // a target cancel followed by the new select click.
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 61;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_b, 40001, 300, 300, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(world.objects.has_component::<Intent>(&3001), "re-target must not drop the cast intent");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_b), "target switched to B");
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(nvit(&world, npc_a).cur_hp < 5000.0, "nuke landed on monster A");
    assert_eq!(nvit(&world, npc_b).cur_hp, 5000.0, "monster B untouched");
}

/// Same as `retarget_mid_walk_keeps_cast_intent`, but the new target is far
/// away (out of the skill's cast range) — the reported live repro: the switch
/// must still not drop the walk-to-cast, and the nuke still lands on A.
#[test]
fn retarget_mid_walk_to_far_target_keeps_cast_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_a = NPC_OID + 62;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_a, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // Walk a couple of ticks, then switch to monster B, far off to the side
    // (well beyond castRange 600 from the walking player).
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 63;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_b, 40001, 700, 1500, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(world.objects.has_component::<Intent>(&3001), "re-target must not drop the cast intent");
    assert_eq!(world.objects.get_component::<TargetRef>(&3001).unwrap().0, Some(npc_b), "target switched to B");
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(nvit(&world, npc_a).cur_hp < 5000.0, "nuke landed on monster A");
    assert_eq!(nvit(&world, npc_b).cur_hp, 5000.0, "monster B untouched");
}

/// Off-axis approach where the reach-boundary point rounds to integer
/// coordinates just *outside* reach: from (0,0) to a monster at (500,500)
/// (distance ~707.1, reach 619) the exact-boundary destination rounds to
/// (62,62), which is ~619.4 from the target. Without Java `moveToLocation`'s
/// "move a bit closer" inset (`distance -= (offset - 5)`) the chase wedges
/// in an arrive/re-path loop there and the cast never fires.
#[test]
fn walk_to_cast_boundary_rounding_still_casts() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 64;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 500, 500, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001), "walk-to-cast started");

    // ~93 units to walk at run speed — in range well within 20 ticks.
    advance_world(&mut world, 20);
    assert!(world.objects.has_component::<Casting>(&3001), "cast starts on arrival despite boundary rounding");
    assert!(!world.objects.has_component::<Intent>(&3001), "the walk-to-cast intent is consumed");
    let packets = drain(&mut a_rx);
    assert!(packets.iter().any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE), "the cast fires");
}

/// The walk-to-cast target dying mid-walk drops the intention on the next
/// think (`checkTargetLost`).
#[test]
fn walk_to_cast_target_death_drops_intent() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 11;
    spawn_targeted_monster(&mut world, &mut a_rx, npc_oid, 700);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(world.objects.has_component::<Intent>(&3001));

    world.objects.get_component_mut::<Vitals>(&npc_oid).unwrap().dead = true;
    advance_world(&mut world, 1);
    assert!(!world.objects.has_component::<Intent>(&3001), "dead target ends the walk-to-cast");
    assert!(!world.objects.has_component::<Casting>(&3001));
}

/// A monster with random walk disabled stays put when idle: the roll is never
/// even reached, so it never starts a wander.
#[test]
fn idle_monster_without_random_walk_stays_put() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // 40001 already has random_walk = false in the base test template.
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 9;
    let (npc, extra) = crate::model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world.npc_regions.entry(extra.1 .0).or_default().push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = crate::model::npc::npc_combat_stats(world.data.npc_data.get(40001).unwrap(), &world.data.stat_bonus);
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Even if a 0 were queued, the random_walk gate short-circuits before it.
    world.forced_rolls.extend([0, 0, 0]);
    npc_ai::npc_ai_tick(&mut world);

    assert!(!world.objects.has_component::<Movement>(&npc_oid), "a non-wandering mob never moves while idle");
    let packets = drain(&mut a_rx);
    assert!(!packets.iter().any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION), "no wander broadcast");
}
