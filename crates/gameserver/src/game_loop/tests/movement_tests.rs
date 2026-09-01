use super::*;
use crate::game_loop;
use crate::game_loop::client::actions;
use crate::game_loop::combat::death;
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
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&3001)
        .unwrap()
        .running = true;

    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(world.objects.has_component::<Casting>(&3001));
    drain(&mut a_rx);
    drain(&mut b_rx);

    // Click to move mid-cast: rejected, cast intact, click remembered.
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(a_rx.try_recv().is_err(), "nothing else while the cast runs");
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "the cast is not aborted"
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "no move yet"
    );
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Move { .. })
    ));

    // Launch (35 ticks) + finish (5 more, coolTime 0 frees the slot): the
    // queued click replays through the normal move pipeline.
    advance_ticks(&mut world, 45);
    assert!(!world.objects.has_component::<Casting>(&3001));
    assert!(
        !world.objects.has_component::<QueuedAction>(&3001),
        "queue consumed"
    );
    let mv = world
        .objects
        .get_component::<Movement>(&3001)
        .expect("move started at cast end");
    assert_eq!((mv.0.dest_x, mv.0.dest_y), (500, 0));
    let a_packets = drain(&mut a_rx);
    assert!(
        a_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION)
    );
    let b_packets = drain(&mut b_rx);
    assert!(
        b_packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION)
    );
}

/// A move click mid-swing waits out the swing (Java `onIntentionMoveTo`'s
/// `isAttackingNow` branch) and starts at swing end via `AttackFinish` —
/// which must fire even though the click dropped the attack intent.
#[test]
fn move_click_mid_swing_defers_to_swing_end() {
    use crate::game_loop;
    use crate::model::components::QueuedAction;

    let (mut world, ..) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 21;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    world.force_rolls([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    drain(&mut a_rx);
    let swing_end = world
        .objects
        .get_component::<model::components::AttackState>(&3001)
        .unwrap()
        .attack_end_tick;

    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(
        a_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "no move mid-swing"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "move click ends the attack loop"
    );
    assert!(matches!(
        world.objects.get_component::<QueuedAction>(&3001),
        Some(QueuedAction::Move { .. })
    ));

    let remaining = swing_end - world.tick;
    advance_ticks(&mut world, remaining);
    let mv = world
        .objects
        .get_component::<Movement>(&3001)
        .expect("move started at swing end");
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
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .run_spd = 100.0;
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .running = true;

    handle_move_backward_to_location(&mut world, 1, &move_body((1000, 0, 0), (0, 0, 0), 1));

    assert_eq!(
        mover_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );
    assert!(
        mover_rx.try_recv().is_err(),
        "exactly one packet to the mover"
    );
    assert_eq!(
        bystander_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );

    let total_ticks = world
        .objects
        .get_component::<Movement>(&4001)
        .unwrap()
        .0
        .total_ticks;
    assert_eq!(
        total_ticks, 100,
        "distance 1000 / speed 100 * 10 ticks-per-sec"
    );

    // Half way: linear interpolation.
    world.tick += total_ticks / 2;
    model::movement::tick(&mut world);
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (500, 0, 0));
    assert!(world.objects.has_component::<Movement>(&4001));

    // Arrival: snapped exactly, move_data cleared, no StopMove needed.
    world.tick += total_ticks / 2;
    model::movement::tick(&mut world);
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

    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((100, 100, 100), (100, 100, 100), 1),
    );

    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::STOP_MOVE
    );
    assert_eq!(
        rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
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
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .run_spd = 100.0;

    // Click to cell 20 (x = 328), on the far side of the wall at cell 10:
    // the clamp to cell 9 (x = 152) shortens 320 → 144, well over 30.
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 8, 0), (8, 8, 0), 1));

    assert!(
        !world.objects.has_component::<Movement>(&4001),
        "move deferred, not started"
    );
    assert!(
        world
            .objects
            .has_component::<model::components::PathWait>(&4001)
    );
    assert!(
        mover_rx.try_recv().is_err(),
        "no packet until the path reply lands"
    );
}

/// A clamp of ≤ 30 units starts the move directly with the clamped
/// destination (`GeoEngine.getValidLocation` in `Creature.moveToLocation`) —
/// no pathfinding round-trip.
#[test]
fn move_destination_is_clamped_by_geodata() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 120, 8, 0); // cell 7
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .run_spd = 100.0;

    // Click one cell into the wall (cell 10, x = 168): clamped to cell 9
    // (x = 152), only 16 units short of the request.
    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (120, 8, 0), 1));

    let md = world
        .objects
        .get_component::<Movement>(&4001)
        .map(|m| m.0.clone())
        .expect("move must start");
    assert_eq!(
        (md.dest_x, md.dest_y),
        (152, 8),
        "clamped to cell 9, before the wall"
    );
    let pkt = mover_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::MOVE_TO_LOCATION);
    let dest_x = i32::from_le_bytes(pkt[5..9].try_into().unwrap());
    assert_eq!(
        dest_x, 152,
        "MoveToLocation carries the clamped destination"
    );
}

/// Full pathfinding round-trip against a real path-worker thread: a click
/// across a walled-off area (with a gap further south) starts a
/// multi-segment route move once the worker replies, route advances
/// broadcast `MoveToLocation` per segment, and the mover arrives at the
/// exact requested destination on the far side of the wall.
#[test]
fn path_worker_round_trip_walks_around_wall() {
    use crate::geo::path::PathConfig;
    use crate::geo::{synthetic_region, wall_column_with_gap};
    use crate::model::components::PathWait;

    let (mut world, ..) = test_world();
    // Mid-region wall at cell x == 10 with a gap at y ∈ [1010, 1014) — far
    // from region edges so the search can't skirt through unloaded void.
    Arc::get_mut(&mut world.geo)
        .expect("geo Arc not shared yet")
        .set_region(20, 18, synthetic_region(wall_column_with_gap(1010..1014)));
    let (req_tx, req_rx) = std::sync::mpsc::channel();
    let (ev_tx, ev_rx) = std::sync::mpsc::channel();
    let worker = crate::geo::worker::spawn(
        world.geo.clone(),
        PathConfig::default(),
        req_rx,
        crate::geo::worker::PathEventTx(ev_tx),
    );
    world.path = req_tx;

    // Player at cell (0, 1000) = (8, 16008); click to cell (20, 1000).
    let mut mover_rx = ingame_player(&mut world, 1, 4001, 8, 16008, 0);
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .run_spd = 100.0;
    handle_move_backward_to_location(&mut world, 1, &move_body((328, 16008, 0), (8, 16008, 0), 1));
    assert!(world.objects.has_component::<PathWait>(&4001));

    // The reply normally lands via the unified event channel.
    let ev = match ev_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("worker reply")
    {
        crate::events::GameEvent::Path(ev) => ev,
        _ => unreachable!("the path worker only sends path events"),
    };
    handle_path_result(&mut world, ev);
    assert!(!world.objects.has_component::<PathWait>(&4001));

    let md = world
        .objects
        .get_component::<Movement>(&4001)
        .map(|m| m.0.clone())
        .expect("route move started");
    let path = md.geo_path.expect("move carries the geodata route");
    assert_eq!(path.index, 0);
    assert!(
        path.points.len() > 1,
        "walking around needs several segments"
    );
    assert_eq!((path.accurate_tx, path.accurate_ty), (328, 16008));
    assert_eq!(
        mover_rx.try_recv().unwrap()[0],
        server_packets::opcodes::MOVE_TO_LOCATION
    );

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
    assert!(
        !world.objects.has_component::<Movement>(&4001),
        "route must complete"
    );
    assert!(advances >= 1, "route advances broadcast MoveToLocation");
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (328, 16008),
        "arrived at the exact requested destination"
    );

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
    world
        .objects
        .get_component_mut::<Speeds>(&4001)
        .unwrap()
        .run_spd = 100.0;

    handle_move_backward_to_location(&mut world, 1, &move_body((168, 8, 0), (152, 8, 0), 1));

    assert!(
        !world.objects.has_component::<Movement>(&4001),
        "no movement into the wall"
    );
    assert_eq!(
        mover_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
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
    zones::revalidate_zone(&mut world, 4001, true);
    drain(&mut rx);

    // Climb: z 0 → 300 with matching client-z history — trusted, silent.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 300, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4001).unwrap().z,
        300
    );
    assert!(rx.try_recv().is_err(), "no correction for a trusted climb");

    // Drift: diffSq 270400 ∈ (250000, 360000), within move speed (600) —
    // server answers ValidateLocation and stays put.
    handle_validate_position(&mut world, 1, &validate_position_body(1520, 1000, 300, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4001).unwrap().x,
        1000,
        "server position kept on drift"
    );
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::VALIDATE_LOCATION);
    assert!(rx.try_recv().is_err());

    // Desync: 2000 units in one report — snap to the client, with z
    // pulled onto the geodata ground (server was above the client).
    handle_validate_position(&mut world, 1, &validate_position_body(3000, 1000, 0, 0));
    let pos = world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (3000, 1000, 0),
        "snapped, z on the geodata floor"
    );
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
    death::teleport_player(&mut world, 4001, 48765, 248461, -6160);
    // The "teleport finished" packet must reach the player, or the client
    // never leaves the loading screen.
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::EX
                && i16::from_le_bytes(p[1..3].try_into().unwrap())
                    == server_packets::opcodes::EX_TELEPORT_TO_LOCATION_ACTIVATE),
        "ExTeleportToLocationActivate sent to the teleporting player"
    );

    // The stale in-flight report from the old spot must not move the server.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 0, 0));
    let pos = *world.objects.get_component::<Position>(&4001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (48765, 248461, -6155),
        "teleport destination kept (z lifted by 5)"
    );
    assert!(
        rx.try_recv().is_err(),
        "no correction packet while teleporting"
    );

    // Appearing completes the teleport; afterwards reports count again.
    death::handle_appearing(&mut world, 1);
    assert!(
        !world
            .objects
            .get_component::<Player>(&4001)
            .unwrap()
            .teleporting
    );
    handle_validate_position(
        &mut world,
        1,
        &validate_position_body(48765, 248461, -6160, 0),
    );
    let c = world.objects.get_component::<ClientPos>(&4001).unwrap();
    assert_eq!(
        (c.x, c.y, c.z),
        (48765, 248461, -6160),
        "client pos tracked again"
    );
}

/// `TeleportWatchdogTimeout = 0` (Java's default and this dist) leaves the
/// feature off: nothing is armed, and a client that never sends `Appearing`
/// stays in the teleporting state indefinitely — retail behaviour, which is
/// exactly what the watchdog exists to opt out of.
#[test]
fn teleport_watchdog_off_by_default() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let _rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);
    assert_eq!(world.cfg.character.teleport_watchdog_timeout_ticks, 0);

    death::teleport_player(&mut world, 4001, 48765, 248461, -6160);
    assert!(
        world.teleport_watchdog_due.is_empty(),
        "nothing armed while the feature is disabled"
    );

    world.tick += 10_000; // ~16 minutes
    death::teleport_watchdog_tick(&mut world);
    assert!(
        world
            .objects
            .get_component::<Player>(&4001)
            .unwrap()
            .teleporting,
        "still waiting on the client — the server never steps in"
    );
}

/// Java `TeleportWatchdogTask`: with a timeout configured, a teleport whose
/// `Appearing` never arrives is completed server-side once the deadline
/// passes, instead of leaving the character decayed out of the world forever.
#[test]
fn teleport_watchdog_forces_completion_when_appearing_never_arrives() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    world.cfg.character.teleport_watchdog_timeout_ticks = 600; // 60 s
    let mut rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);

    death::teleport_player(&mut world, 4001, 48765, 248461, -6160);
    assert_eq!(
        world.teleport_watchdog_due.get(&4001),
        Some(&(world.tick + 600)),
        "armed for tick + timeout"
    );
    let _ = drain(&mut rx);

    // One tick short of the deadline: still the client's turn.
    world.tick += 599;
    death::teleport_watchdog_tick(&mut world);
    assert!(
        world
            .objects
            .get_component::<Player>(&4001)
            .unwrap()
            .teleporting,
        "watchdog must not fire early"
    );

    world.tick += 1;
    death::teleport_watchdog_tick(&mut world);
    assert!(
        !world
            .objects
            .get_component::<Player>(&4001)
            .unwrap()
            .teleporting,
        "watchdog completed the teleport (Java onTeleported)"
    );
    assert!(
        world.teleport_watchdog_due.is_empty(),
        "the fired entry is retired, not left to fire again"
    );
    // `onTeleported` → spawnMe + fresh UserInfo (0x32), same as the Appearing
    // path — the client is what would otherwise still be on a black screen.
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x32),
        "the player is told about themselves at the destination"
    );
}

/// The client answering in time cancels the watchdog (Java
/// `setTeleporting(false)` → `_teleportWatchdog.cancel(false)`). Without the
/// cancel a stale entry would fire into the *next* teleport and complete it
/// early, spawning the character in before their client had loaded.
#[test]
fn appearing_cancels_the_watchdog_so_the_next_teleport_arms_fresh() {
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    world.cfg.character.teleport_watchdog_timeout_ticks = 600;
    let _rx = ingame_player(&mut world, 1, 4001, 1000, 1000, 0);

    death::teleport_player(&mut world, 4001, 48765, 248461, -6160);
    death::handle_appearing(&mut world, 1);
    assert!(
        world.teleport_watchdog_due.is_empty(),
        "completed teleport leaves no armed watchdog"
    );

    // A second teleport 30 s later gets its own full window, not the remains
    // of the first one's.
    world.tick += 300;
    death::teleport_player(&mut world, 4001, 1000, 1000, 0);
    assert_eq!(
        world.teleport_watchdog_due.get(&4001),
        Some(&(world.tick + 600))
    );
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
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "move click drops the walk-to-cast"
    );
    advance_world(&mut world, 60);
    assert!(!world.objects.has_component::<Casting>(&3001));
    let packets = drain(&mut a_rx);
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "the cast never fires"
    );
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
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "walk-to-cast started"
    );

    // Walk a couple of ticks, then switch to monster B — the client emits
    // a target cancel followed by the new select click.
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 61;
    let (npc, extra) = model::npc::Npc::for_test(npc_b, 40001, 300, 300, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(
        world.objects.has_component::<Intent>(&3001),
        "re-target must not drop the cast intent"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_b),
        "target switched to B"
    );
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(
        nvit(&world, npc_a).cur_hp < 5000.0,
        "nuke landed on monster A"
    );
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
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "walk-to-cast started"
    );

    // Walk a couple of ticks, then switch to monster B, far off to the side
    // (well beyond castRange 600 from the walking player).
    advance_world(&mut world, 2);
    let npc_b = NPC_OID + 63;
    let (npc, extra) = model::npc::Npc::for_test(npc_b, 40001, 700, 1500, 0, 5000, 30);
    world.npc_regions.entry(extra.1.0).or_default().push(npc_b);
    world.objects.spawn(npc_b, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_b, cs);
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    handle_action(&mut world, 1, &action_body(npc_b, 0));
    drain(&mut a_rx);

    assert!(
        world.objects.has_component::<Intent>(&3001),
        "re-target must not drop the cast intent"
    );
    assert_eq!(
        world.objects.get_component::<TargetRef>(&3001).unwrap().0,
        Some(npc_b),
        "target switched to B"
    );
    advance_world(&mut world, 60);
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "cast fires at the snapshotted target after the walk"
    );
    assert!(
        nvit(&world, npc_a).cur_hp < 5000.0,
        "nuke landed on monster A"
    );
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
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 500, 500, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    drain(&mut a_rx);

    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, false));
    assert!(
        world.objects.has_component::<Intent>(&3001),
        "walk-to-cast started"
    );

    // ~93 units to walk at run speed — in range well within 20 ticks.
    advance_world(&mut world, 20);
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "cast starts on arrival despite boundary rounding"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the walk-to-cast intent is consumed"
    );
    let packets = drain(&mut a_rx);
    assert!(
        packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "the cast fires"
    );
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

    world
        .objects
        .get_component_mut::<Vitals>(&npc_oid)
        .unwrap()
        .dead = true;
    advance_world(&mut world, 1);
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "dead target ends the walk-to-cast"
    );
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
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 0, 0, 0, 5000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(&mut a_rx);

    // Even if a 0 were queued, the random_walk gate short-circuits before it.
    world.force_rolls([0, 0, 0]);
    ai::npc_ai_tick(&mut world);

    assert!(
        !world.objects.has_component::<Movement>(&npc_oid),
        "a non-wandering mob never moves while idle"
    );
    let packets = drain(&mut a_rx);
    assert!(
        !packets
            .iter()
            .any(|p| p[0] == server_packets::opcodes::MOVE_TO_LOCATION),
        "no wander broadcast"
    );
}

/// **The client reports it walked into a wall.** `CannotMoveAnymore` (0x47)
/// drops the server-side walk, plants the player where the client says it
/// stopped, and tells everyone (including the mover — Java's
/// `broadcastPacket` on `StopMove` from `clientStopMoving`) to stop the
/// animation. A pending cast intention is dropped with it.
#[test]
fn a_client_stuck_report_stops_the_walk_where_it_says() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    for oid in [3001, 3002] {
        let sp = world.objects.get_component_mut::<Speeds>(&oid).unwrap();
        sp.run_spd = 100.0;
        sp.running = true;
    }

    handle_move_backward_to_location(&mut world, 1, &move_body((5000, 0, 0), (0, 0, 0), 1));
    assert!(world.objects.has_component::<Movement>(&3001), "walking");
    world.objects.add_components(
        &3001,
        Intent(model::PlayerIntent::Cast {
            skill_id: 1177,
            ctrl: false,
            shift: false,
            target_object_id: 3002,
        }),
    );
    drain(&mut a_rx);
    drain(&mut b_rx);

    // The client got 300 units along and hit geometry.
    let mut w = PacketWriter::new();
    for v in [300, 0, 0, 16384] {
        w.write_i32(v);
    }
    let mut body = vec![cp::opcodes::CANNOT_MOVE_ANYMORE];
    body.extend_from_slice(&w.into_bytes());
    on_packet(&mut world, 1, body);

    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "the walk is dropped, not resumed toward 5000"
    );
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "the queued cast intention goes with it"
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z, pos.heading),
        (300, 0, 0, 16384),
        "the player lands where the client reported"
    );

    // Both the mover and the onlooker see the stop.
    for (who, rx) in [("mover", &mut a_rx), ("onlooker", &mut b_rx)] {
        let stop = drain(rx)
            .into_iter()
            .find(|p| p[0] == server_packets::opcodes::STOP_MOVE)
            .unwrap_or_else(|| panic!("{who} is told to stop"));
        assert_eq!(
            i32::from_le_bytes([stop[1], stop[2], stop[3], stop[4]]),
            3001
        );
        assert_eq!(
            i32::from_le_bytes([stop[5], stop[6], stop[7], stop[8]]),
            300,
            "…at the reported spot"
        );
    }

    // The walk really is over: ticking does not creep toward the old target.
    advance_ticks(&mut world, 5);
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(pos.x, 300, "no drift after the stop");
}

// ---------------------------------------------------------------------------
// Sitting (G29 sweep) — `sitDown`/`standUp` + the SitStand player action
// ---------------------------------------------------------------------------

fn sit_action_body(action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(action_id);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_u8(0);
    w.into_bytes()
}

/// **Sitting is two-phase, and the phases are different predicates.** Sitting
/// down flips the flag at once but blocks actions for the 2.5 s animation;
/// standing up broadcasts first and only clears the flag when the animation
/// ends. A port that collapsed either into one step would let a player act
/// mid-animation, or stand instantly.
#[test]
fn sitting_down_and_standing_up_each_take_an_animation() {
    let (mut world, ..) = cast_test_world();
    // `RequestActionUse` dispatches through `ActionData.xml`'s handler table,
    // and the fixture world ships an empty one — without the row the packet
    // finds no handler and the assertions below would pass against a player
    // who never sat down.
    world
        .data
        .action_data
        .insert_row_for_test(actions::action::SIT_STAND, "SitStand", 0);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    drain(&mut rx);
    let seated = |w: &World| crate::game_loop::character::sit_stand::is_sitting(w, 3001);
    let blocked = |w: &World| abnormal::is_blocked_from_actions(w, 3001);

    actions::handle_request_action_use(&mut world, 1, &sit_action_body(0));
    assert!(seated(&world), "seated the instant the toggle is used");
    assert!(blocked(&world), "…and blocked while the animation plays");
    let wt = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::CHANGE_WAIT_TYPE)
        .expect("ChangeWaitType broadcast");
    assert_eq!(
        i32::from_le_bytes([wt[5], wt[6], wt[7], wt[8]]),
        0,
        "WT_SITTING"
    );

    advance_ticks(&mut world, 26); // past 2.5 s
    assert!(seated(&world), "still seated after the animation");
    assert!(!blocked(&world), "but free to act again");

    // Stand: the flag survives until the stand animation finishes.
    actions::handle_request_action_use(&mut world, 1, &sit_action_body(0));
    assert!(
        seated(&world),
        "standing up is not instant — the flag holds through the animation"
    );
    advance_ticks(&mut world, 26);
    assert!(!seated(&world), "on their feet");
}

/// **The seated regen bonus is the point of sitting**: `MoveType::Sitting`
/// wins over standing/running, and it is the largest multiplier.
#[test]
fn sitting_selects_the_seated_regen_multiplier() {
    use crate::model::stats::MoveType;
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    assert_eq!(
        crate::game_loop::stats::regen::move_type_of(&world, 3001),
        MoveType::Standing
    );
    crate::game_loop::character::sit_stand::sit_down(&mut world, 3001);
    assert_eq!(
        crate::game_loop::stats::regen::move_type_of(&world, 3001),
        MoveType::Sitting,
        "the seated branch wins"
    );
    assert!(
        crate::game_loop::stats::regen::movement_regen_multiplier(MoveType::Sitting)
            > crate::game_loop::stats::regen::movement_regen_multiplier(MoveType::Standing),
        "and it regenerates faster than standing"
    );
}

/// **You cannot sit-tank.** Java's `PlayerStatus.reduceHp` stands a seated
/// victim up on any hit — and takes their shop down with them.
#[test]
fn taking_a_hit_stands_a_seated_player_up() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let _rx2 = ingame_caster(&mut world, 2, 3002, 60, 0);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&3001) {
        v.cur_hp = 500.0;
    }
    crate::game_loop::character::sit_stand::sit_down(&mut world, 3001);
    advance_ticks(&mut world, 26);
    assert!(crate::game_loop::character::sit_stand::is_sitting(
        &world, 3001
    ));

    combat::player_receive_damage(&mut world, 3001, 3002, 50.0);
    // The stand-up is scheduled, as any other is.
    advance_ticks(&mut world, 26);
    assert!(
        !crate::game_loop::character::sit_stand::is_sitting(&world, 3001),
        "a hit puts them back on their feet"
    );
}

/// A shopkeeper stays seated while their store is open — Java's `standUp`
/// refuses outright for `isInStoreMode()`, which is what keeps a vendor sitting
/// behind their wares.
#[test]
fn a_shopkeeper_cannot_stand_while_the_store_is_open() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    crate::game_loop::character::sit_stand::sit_down(&mut world, 3001);
    advance_ticks(&mut world, 26);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .store_type = 1;

    crate::game_loop::character::sit_stand::stand_up(&mut world, 3001);
    advance_ticks(&mut world, 26);
    assert!(
        crate::game_loop::character::sit_stand::is_sitting(&world, 3001),
        "the store keeps them seated"
    );

    // Close the store and they can get up.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .store_type = 0;
    crate::game_loop::character::sit_stand::stand_up(&mut world, 3001);
    advance_ticks(&mut world, 26);
    assert!(!crate::game_loop::character::sit_stand::is_sitting(
        &world, 3001
    ));
}

/// **The seated *state* is what refuses actions, not the sit animation.** The
/// `SitBlock` that `sitDown` raises covers 2.5 s and then lapses while the
/// character stays in the chair — so every gate that reads only
/// `hasBlockActions()` goes quiet after two and a half seconds. Java refuses on
/// the seat itself, in three separate places: `Player.useMagic`'s
/// `_waitTypeSitting` branch (SM 31), `PlayableAI.onIntentionAttack`'s
/// `isSitting()` early return (silent — no packet at all), and
/// `CreatureAI.onIntentionMoveTo`'s `AI_INTENTION_REST` branch
/// (`clientActionFailed`). Without them a player who sat down mid-fight kept
/// casting, swinging and running from the chair.
#[test]
fn a_seated_player_cannot_cast_attack_or_move_once_the_animation_lapses() {
    use crate::game_loop;
    use crate::model::components::{Intent, Movement};

    let (mut world, ..) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let mut b_rx = ingame_caster(&mut world, 2, 3002, 100, 0);
    let npc_oid = NPC_OID + 22;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    crate::game_loop::character::sit_stand::sit_down(&mut world, 3001);
    advance_ticks(&mut world, 26); // past the 2.5 s sit animation
    assert!(crate::game_loop::character::sit_stand::is_sitting(
        &world, 3001
    ));
    assert!(
        !abnormal::is_blocked_from_actions(&world, 3001),
        "the animation block has lapsed — only the seated state is left to say no"
    );
    drain(&mut rx);
    drain(&mut b_rx);

    // Cast: SM 31, and nothing starts.
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        has_system_message(
            &drain(&mut rx),
            server_packets::sm_ids::YOU_CANNOT_USE_ACTIONS_AND_SKILLS_WHILE_THE_CHARACTER_IS_SITTING
        ),
        "told to get up first"
    );
    assert!(
        !world.objects.has_component::<Casting>(&3001),
        "no cast from the chair"
    );

    // Attack: refused silently — Java's `onIntentionAttack` just returns.
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    assert!(
        !world.objects.has_component::<Intent>(&3001),
        "no attack intent from the chair"
    );

    // Move: ActionFailed, no movement, no drift.
    drain(&mut rx);
    handle_move_backward_to_location(&mut world, 1, &move_body((500, 0, 0), (0, 0, 0), 1));
    assert_eq!(
        drain(&mut rx).first().map(|p| p[0]),
        Some(server_packets::opcodes::ACTION_FAIL),
        "the click is answered with ActionFailed and nothing else"
    );
    assert!(
        !world.objects.has_component::<Movement>(&3001),
        "no move started"
    );
    advance_world(&mut world, 5);
    assert_eq!(
        world.objects.get_component::<Position>(&3001).unwrap().x,
        0,
        "and no drift"
    );

    // Positive control: on their feet again, the very same clicks work.
    crate::game_loop::character::sit_stand::stand_up(&mut world, 3001);
    advance_ticks(&mut world, 26);
    assert!(!crate::game_loop::character::sit_stand::is_sitting(
        &world, 3001
    ));
    drain(&mut rx);
    handle_action(&mut world, 1, &action_body(3002, 0));
    handle_request_magic_skill_use(&mut world, 1, &magic_skill_use_body(1177, true));
    assert!(
        world.objects.has_component::<Casting>(&3001),
        "standing, the cast goes through"
    );
}

/// **Sitting down must not cancel the combat stance — only the swing.** Java's
/// `sitDown` calls `breakAttack()` → `abortAttack()`, which ends the current
/// swing and nothing else; the 15 s stance lives in a separate
/// `AttackStanceTaskManager` map that sitting never touches, so it expires on
/// its own schedule and broadcasts `AutoAttackStop`.
///
/// The port used to drop the whole `AttackState` component here, and that
/// component carries `stance_until_tick` — so a player who sat down while in
/// stance left the stance sweep for good: `AutoAttackStop` never went out (the
/// client held the sword drawn forever) and `refresh_attack_stance`'s `if let
/// Some` write silently no-opped from then on, so no later fight could put them
/// back in stance either.
#[test]
fn sitting_down_keeps_the_combat_stance_ticking_toward_its_own_expiry() {
    let (mut world, ..) = combat_test_world();
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 23;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 30, 0, 0, 100_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);

    handle_action(&mut world, 1, &action_body(npc_oid, 0));
    world.force_rolls([0, 99, 10]);
    handle_attack_request(&mut world, 1, &attack_request_body(npc_oid));
    assert!(
        combat::has_attack_stance(&world, 3001),
        "swinging draws the sword"
    );

    // Sit mid-fight: the swing is cut, the stance is not.
    world.objects.remove_component::<Casting>(&3001);
    if let Some(st) = world
        .objects
        .get_component_mut::<model::components::AttackState>(&3001)
    {
        st.attack_end_tick = 0; // let the sit request past `isAttackingNow()`
    }
    crate::game_loop::character::sit_stand::sit_down(&mut world, 3001);
    assert!(crate::game_loop::character::sit_stand::is_sitting(
        &world, 3001
    ));
    assert!(
        combat::has_attack_stance(&world, 3001),
        "sitting does not sheathe the sword — Java's breakAttack only ends the swing"
    );
    drain(&mut rx);

    // …and it still runs out on its own, 15 s after the last swing. Advance
    // with the timer-only helper and drive the stance sweep by hand: under the
    // full world tick the monster swings back, and every hit it lands re-arms
    // the stance (and stands its victim up) — which is precisely the state this
    // assertion must not be measuring.
    advance_ticks(&mut world, combat::COMBAT_STANCE_TICKS + 1);
    combat::stance_tick(&mut world);
    assert!(
        !combat::has_attack_stance(&world, 3001),
        "the stance expires on schedule instead of hanging forever"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::AUTO_ATTACK_STOP),
        "and the client is told to sheathe it"
    );
}

/// "Demonic mode" (`//instant_move`): the armed GM's next click teleports
/// instead of walking, and the latch is spent — the click after it walks
/// normally again (Java `MoveBackwardToLocation`'s `case DEMONIC`, which ends
/// with `setTeleMode(NORMAL)`).
#[test]
fn demonic_tele_mode_teleports_the_click_and_disarms() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4101, 1000, 1000, 0);
    world
        .objects
        .get_component_mut::<Player>(&4101)
        .unwrap()
        .tele_mode = AdminTeleportType::Demonic;
    drain(&mut rx);

    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((1500, 1000, 0), (1000, 1000, 0), 1),
    );

    let pos = *world.objects.get_component::<Position>(&4101).unwrap();
    assert_eq!((pos.x, pos.y), (1500, 1000), "warped to the click");
    assert!(
        !world.objects.has_component::<Movement>(&4101),
        "a teleport, not a walk"
    );
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "TeleportToLocation went out"
    );
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "Java answers the consumed click with ActionFailed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&4101)
            .unwrap()
            .tele_mode,
        AdminTeleportType::Normal,
        "one-shot latch"
    );

    // Disarmed: the next click is an ordinary walk.
    world
        .objects
        .get_component_mut::<Player>(&4101)
        .unwrap()
        .teleporting = false;
    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((1700, 1000, 0), (1500, 1000, 0), 1),
    );
    assert!(
        world.objects.has_component::<Movement>(&4101),
        "back to walking"
    );
}

/// "Charge mode" (`//teleto charge`): the click slides the GM there with the
/// charge flourish (skill 30012) and — unlike the other two modes — leaves the
/// latch armed, because Java's `case CHARGE` never calls `setTeleMode(NORMAL)`.
#[test]
fn charge_tele_mode_slides_with_the_flourish_and_stays_armed() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4102, 1000, 1000, 0);
    world
        .objects
        .get_component_mut::<Player>(&4102)
        .unwrap()
        .tele_mode = AdminTeleportType::Charge;
    drain(&mut rx);

    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((1600, 1000, 0), (1000, 1000, 0), 1),
    );

    let pos = *world.objects.get_component::<Position>(&4102).unwrap();
    assert_eq!((pos.x, pos.y), (1600, 1000), "slid to the click");
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::FLY_TO_LOCATION),
        "the charge fly"
    );
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "skill 30012's cast"
    );
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_LAUNCHED),
        "…and its launch"
    );
    assert!(
        !out.iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "a slide, not a teleport — no loading screen"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&4102)
            .unwrap()
            .tele_mode,
        AdminTeleportType::Charge,
        "charge mode is sticky until //teleto end"
    );
}

/// "Sayune mode" (`//teleto sayune`): a silent slide, latch spent. Java's own
/// `ExFlyMove`/`ExFlyMoveBroadcast` are Ertheia opcodes with no Interlude
/// counterpart, so the port substitutes the plain `FlyToLocation` slide — what
/// must hold either way is that the GM ends up at the click without a loading
/// screen and the mode disarms.
#[test]
fn sayune_tele_mode_slides_and_disarms() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4103, 1000, 1000, 0);
    world
        .objects
        .get_component_mut::<Player>(&4103)
        .unwrap()
        .tele_mode = AdminTeleportType::Sayune;
    drain(&mut rx);

    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((1400, 1000, 0), (1000, 1000, 0), 1),
    );

    let pos = *world.objects.get_component::<Position>(&4103).unwrap();
    assert_eq!((pos.x, pos.y), (1400, 1000));
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::FLY_TO_LOCATION)
    );
    assert!(
        !out.iter()
            .any(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE),
        "no charge flourish on a sayune hop"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&4103)
            .unwrap()
            .tele_mode,
        AdminTeleportType::Normal,
        "one-shot latch"
    );
}

/// Java arms `blinkActive` inside the `FlyToLocation` constructor and
/// `ValidatePosition` spends it on its out-of-sync branch: right after a slide
/// the client is still reporting the pre-slide spot, and adopting that report
/// would undo the slide. The first stale report is swallowed; a later one is
/// honoured as usual.
#[test]
fn a_slide_survives_the_clients_stale_position_report() {
    use crate::enums::AdminTeleportType;
    let (mut world, ..) = test_world();
    install_wall_region(&mut world);
    let mut rx = ingame_player(&mut world, 1, 4104, 1000, 1000, 0);
    {
        let speeds = world.objects.get_component_mut::<Speeds>(&4104).unwrap();
        speeds.run_spd = 120.0;
        speeds.running = true;
    }
    world
        .objects
        .get_component_mut::<Player>(&4104)
        .unwrap()
        .tele_mode = AdminTeleportType::Sayune;
    handle_move_backward_to_location(
        &mut world,
        1,
        &move_body((1400, 1000, 0), (1000, 1000, 0), 1),
    );
    assert!(
        world
            .objects
            .get_component::<Player>(&4104)
            .unwrap()
            .blink_active,
        "the fly armed the blink guard"
    );
    drain(&mut rx);

    // The in-flight report from the old spot: swallowed, position kept.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 0, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4104).unwrap().x,
        1400,
        "the slide stands"
    );
    assert!(
        !world
            .objects
            .get_component::<Player>(&4104)
            .unwrap()
            .blink_active,
        "guard spent"
    );

    // Guard spent: a genuine desync snaps the server to the client again.
    handle_validate_position(&mut world, 1, &validate_position_body(1000, 1000, 0, 0));
    assert_eq!(
        world.objects.get_component::<Position>(&4104).unwrap().x,
        1000,
        "ordinary out-of-sync handling is back"
    );
}
