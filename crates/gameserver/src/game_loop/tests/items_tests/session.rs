//! What leaving the world stores: restart, logout and disconnect, and the
//! saved key mapping.

use super::*;

/// RequestRestart: the player is stored + removed, the client gets
/// `RestartResponse(true)`, drops back to `Authenticated`, and the reloaded
/// character list flows through the normal lobby path.
#[test]
fn restart_stores_player_and_returns_to_lobby() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&5001).unwrap();
        p.exp = 1234;
    }
    world
        .objects
        .get_component_mut::<Position>(&5001)
        .unwrap()
        .x = 777;

    handle_request_restart(&mut world, 1);

    // storeMe: the snapshot carries the live (not the loaded) state, and
    // is queued before the character-list reload.
    let save = expect_store_player(&mut db_rx);
    assert_eq!(
        (save.base.object_id, save.base.exp, save.base.x),
        (5001, 1234, 777)
    );
    match db_rx.try_recv() {
        Ok(db::DbCommand::LoadCharacters { client_id, account }) => {
            assert_eq!((client_id, account.as_str()), (1, "bob"));
        }
        _ => panic!("expected a LoadCharacters DB command after the store"),
    }

    // deleteMe + setConnectionState(AUTHENTICATED) + RestartResponse.TRUE.
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::Authenticated(_))
    ));
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 1, "RestartResponse.TRUE");

    // The reload result lands like any character-list load: InLobby +
    // CharSelectionInfo.
    on_characters_loaded(
        &mut world,
        1,
        "bob".into(),
        vec![dummy_char(5001, "P5001")],
        true,
    );
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::InLobby(_))
    ));
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::CHARACTER_SELECTION_INFO
    );
}

/// Logout: the player is stored + removed and the client gets `LeaveWorld`;
/// dropping the session is what closes the socket.
#[test]
fn logout_stores_player_and_sends_leave_world() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);

    handle_logout(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5002);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty(), "session dropped → socket closes");
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::LOG_OUT_OK
    );
}

/// An unexpected disconnect while in game persists the player too (Java
/// `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
#[test]
fn disconnect_stores_ingame_player() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _out_rx = ingame_player(&mut world, 1, 5003, 100, 200, 0);

    on_disconnect(&mut world, 1);

    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5003);
    assert_eq!(world.objects.count::<Player>(), 0);
    assert!(world.clients.is_empty());
}

/// **The client's key layout survives a relogin.** `RequestSaveKeyMapping`
/// stores the blob in a player variable (Java's `UI_KEY_MAPPING`), and
/// `RequestKeyMapping` replays it verbatim.
#[test]
fn the_saved_key_mapping_round_trips() {
    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // Nothing saved yet: Java's empty payload.
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_KEY_MAPPING, &[]),
    );
    let empty = drain(&mut rx)
        .into_iter()
        .find(|p| is_ex(p, server_packets::opcodes::EX_UI_SETTING))
        .expect("the UI setting packet");
    assert_eq!(
        i32::from_le_bytes([empty[3], empty[4], empty[5], empty[6]]),
        0,
        "no stored layout"
    );

    // Save three bytes, then ask for them back.
    let mut w = PacketWriter::new();
    w.write_i32(3);
    for b in [7u8, 0, 200] {
        w.write_u8(b);
    }
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_SAVE_KEY_MAPPING, &w.into_bytes()),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        ex_packet(cp::ex_opcodes::REQUEST_KEY_MAPPING, &[]),
    );
    let stored = drain(&mut rx)
        .into_iter()
        .find(|p| is_ex(p, server_packets::opcodes::EX_UI_SETTING))
        .expect("the UI setting packet");
    assert_eq!(
        i32::from_le_bytes([stored[3], stored[4], stored[5], stored[6]]),
        3,
        "three bytes come back"
    );
    assert_eq!(&stored[7..10], &[7, 0, 200], "…verbatim, high bytes intact");
}
