use super::*;

#[test]
fn auth_then_load_reaches_lobby_with_char_list() {
    let (mut world, _db_tx, mut db_rx, mut link_rx) = test_world();
    let mut out_rx = connect(&mut world, 1);

    // AuthLogin → PlayerAuthRequest.
    let key = SessionKey::new(11, 12, 21, 22);
    handle_auth_login(&mut world, 1, &auth_login_body("Bob", key));
    assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&1));
    assert!(matches!(
        link_rx.try_recv().unwrap(),
        LoginLinkCommand::PlayerAuthRequest { .. }
    ));

    // PlayerAuthResponse(authed) → Authenticated + LOGIN_SUCCESS + LoadCharacters.
    handle_player_auth_response(&mut world, "bob".to_string(), true);
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::Authenticated(_))
    ));
    assert!(matches!(
        link_rx.try_recv().unwrap(),
        LoginLinkCommand::PlayerInGame { .. }
    ));
    assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_success());
    assert!(matches!(
        db_rx.try_recv().unwrap(),
        db::DbCommand::LoadCharacters { client_id: 1, .. }
    ));

    // DB returns the list → InLobby + CharSelectionInfo (opcode 0x09).
    on_characters_loaded(
        &mut world,
        1,
        "bob".to_string(),
        vec![dummy_char(0x10000000, "Hero")],
        true,
    );
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::InLobby(_))
    ));
    let sel = out_rx.try_recv().unwrap();
    assert_eq!(sel[0], server_packets::opcodes::CHARACTER_SELECTION_INFO);
}

#[test]
fn character_delete_marks_slot() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = connect(&mut world, 1);
    // Fast-forward to InLobby with one character.
    let ClientSession::Connecting(s) = world.clients.remove(&1).unwrap() else {
        unreachable!()
    };
    let s = s
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![dummy_char(555, "Hero")]);
    world.clients.insert(1, ClientSession::InLobby(s));

    let mut body = PacketWriter::new();
    body.write_i32(0); // slot 0
    handle_character_delete(&mut world, 1, &body.into_bytes());

    assert_eq!(
        out_rx.try_recv().unwrap(),
        server_packets::char_delete_success()
    );
    match db_rx.try_recv().unwrap() {
        db::DbCommand::MarkDelete {
            char_id,
            delete_time,
            ..
        } => {
            assert_eq!(char_id, 555);
            assert!(delete_time > commons::util::now_millis());
        }
        _ => panic!("expected MarkDelete"),
    }
}

#[test]
fn wrong_session_key_closes_connection() {
    let (mut world, _db_tx, _db_rx, mut link_rx) = test_world();
    let mut out_rx = connect(&mut world, 1);

    handle_auth_login(
        &mut world,
        1,
        &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)),
    );
    let _ = link_rx.try_recv(); // PlayerAuthRequest

    handle_player_auth_response(&mut world, "bob".to_string(), false);
    assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_fail(0, 1));
    assert!(world.clients.get(&1).is_none());
    assert!(!world.login.accounts_in_gameserver.contains_key("bob"));
    assert!(matches!(
        link_rx.try_recv().unwrap(),
        LoginLinkCommand::PlayerLogout { .. }
    ));
}

#[test]
fn duplicate_account_login_is_rejected() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    world
        .login
        .accounts_in_gameserver
        .insert("bob".to_string(), 99); // already on
    connect(&mut world, 1);
    handle_auth_login(
        &mut world,
        1,
        &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)),
    );
    assert!(world.clients.get(&1).is_none());
    assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&99));
}

/// Server-shutdown save-all: every online player is persisted (level/exp/
/// position) without being despawned, so a restart doesn't revert them to
/// their last logout — the bug where a character leveled up, the server was
/// restarted, and the level was lost (skills, saved eagerly, were not).
#[test]
fn shutdown_saves_all_online_players() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _o1 = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    let _o2 = ingame_player(&mut world, 2, 5002, 300, 400, 0);
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&5001)
            .unwrap();
        p.level = 7;
        p.exp = 9999;
    }

    super::net::save_all_players(&mut world);

    // A StorePlayer snapshot per online player (ECS iteration order isn't
    // fixed, so collect by object id).
    let mut snaps = std::collections::HashMap::new();
    for _ in 0..2 {
        let s = expect_store_player(&mut db_rx);
        snaps.insert(s.base.object_id, s);
    }
    assert_eq!(snaps.len(), 2, "both online players saved");
    assert_eq!(
        snaps[&5001].base.level, 7,
        "the leveled-up character's level is persisted"
    );
    assert_eq!(snaps[&5001].base.exp, 9999);
    assert!(snaps.contains_key(&5002));
    // Save-all does not despawn — the players are still in the world.
    assert_eq!(world.objects.count::<Player>(), 2);
}

/// A second select → enter-world round trip works on the restarted session
/// (the original relogin bug: the restart packet was ignored entirely).
#[test]
fn restart_then_reenter_world() {
    let (mut world, _db_tx, _db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    handle_request_restart(&mut world, 1);
    on_characters_loaded(
        &mut world,
        1,
        "bob".into(),
        vec![dummy_char(5001, "P5001")],
        true,
    );
    while out_rx.try_recv().is_ok() {} // RestartResponse + CharSelectionInfo

    let mut w = PacketWriter::new();
    w.write_i32(0); // slot
    handle_character_select(&mut world, 1, &w.into_bytes());
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::CHAR_SELECTED
    );
    handle_enter_world(&mut world, 1);
    assert!(
        world.objects.has_component::<crate::model::Player>(&5001),
        "player re-entered the world"
    );
    assert!(matches!(
        world.clients.get(&1),
        Some(ClientSession::InGame(_))
    ));
}

/// `Player.canLogout` refuses a restart while the player is in combat stance:
/// the client gets `RestartResponse.FALSE` + `ActionFailed`, the player stays
/// in the world, the session stays `InGame`, and nothing is persisted.
#[test]
fn restart_blocked_while_in_combat_stance() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5001, 100, 200, 0);
    // In stance until 15 s from now (AttackStanceTaskManager.addAttackStanceTask).
    world
        .objects
        .get_component_mut::<crate::model::components::AttackState>(&5001)
        .unwrap()
        .stance_until_tick = world.tick + 1;

    handle_request_restart(&mut world, 1);

    assert_eq!(
        world.objects.count::<Player>(),
        1,
        "player stays in the world"
    );
    assert!(
        matches!(world.clients.get(&1), Some(ClientSession::InGame(_))),
        "still in game"
    );
    assert!(db_rx.try_recv().is_err(), "no store/reload while refused");
    let pkt = out_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::RESTART_RESPONSE);
    assert_eq!(pkt[1], 0, "RestartResponse.FALSE");
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
}

/// `Player.canLogout` refuses a logout while in combat stance: `ActionFailed`
/// only, no `LeaveWorld`, and the player stays in the world.
#[test]
fn logout_blocked_while_in_combat_stance() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let mut out_rx = ingame_player(&mut world, 1, 5002, 100, 200, 0);
    world
        .objects
        .get_component_mut::<crate::model::components::AttackState>(&5002)
        .unwrap()
        .stance_until_tick = world.tick + 1;

    handle_logout(&mut world, 1);

    assert_eq!(
        world.objects.count::<Player>(),
        1,
        "player stays in the world"
    );
    assert!(
        matches!(world.clients.get(&1), Some(ClientSession::InGame(_))),
        "still in game"
    );
    assert!(db_rx.try_recv().is_err(), "no store while refused");
    assert_eq!(
        out_rx.try_recv().unwrap()[0],
        server_packets::opcodes::ACTION_FAIL
    );
    assert!(out_rx.try_recv().is_err(), "no LeaveWorld");
}

/// The staggered periodic autosave (Java `PlayerAutoSaveTaskManager`): a due
/// player is flushed once and rescheduled one interval out, and at most one
/// player is flushed per sweep (SQL-flood throttle). The player stays in the
/// world — this is a live save, not logout.
#[test]
fn autosave_flushes_one_due_player_and_reschedules() {
    let (mut world, _db_tx, mut db_rx, _link_rx) = test_world();
    let _a = ingame_player(&mut world, 1, 5001, 10, 20, 0);
    let _b = ingame_player(&mut world, 2, 5002, 30, 40, 0);
    world.cfg.character.character_data_store_interval_ticks = 100;
    // Both due at the current tick.
    world.player_autosave_due.insert(5001, world.tick);
    world.player_autosave_due.insert(5002, world.tick);

    super::autosave_tick(&mut world);

    // Exactly one StorePlayer this sweep (the lowest object id), and both players
    // are still in the world.
    let save = expect_store_player(&mut db_rx);
    assert_eq!(
        save.base.object_id, 5001,
        "lowest due object id flushed first"
    );
    assert!(
        db_rx.try_recv().is_err(),
        "only one player flushed per sweep"
    );
    assert_eq!(
        world.objects.count::<Player>(),
        2,
        "autosave does not despawn"
    );
    // 5001 rescheduled one interval out; 5002 still due.
    assert_eq!(world.player_autosave_due[&5001], world.tick + 100);
    assert_eq!(world.player_autosave_due[&5002], world.tick);

    // Next sweep flushes the other player; a third finds nothing due.
    super::autosave_tick(&mut world);
    assert_eq!(expect_store_player(&mut db_rx).base.object_id, 5002);
    super::autosave_tick(&mut world);
    assert!(
        db_rx.try_recv().is_err(),
        "nothing due after both rescheduled"
    );
}

/// Entering the world exchanges `CharInfo` with players in the surrounding
/// regions (Java `spawnMe` → `World.addVisibleObject`) and with no one
/// beyond them.
#[test]
fn enter_world_exchanges_char_info_with_nearby_players_only() {
    let (mut world, ..) = test_world();
    let mut near_rx = ingame_player(&mut world, 1, 6001, 500, 500, 0);
    let mut far_rx = ingame_player(&mut world, 2, 6002, 10_000, 10_000, 0);
    let mut new_rx = entering_player(&mut world, 3, 6003, 0, 0, 0);

    handle_enter_world(&mut world, 3);

    // The nearby player learns about the newcomer — CharInfo then the paired
    // RelationChanged (Java `sendInfo`), and nothing more.
    let pkt = near_rx.try_recv().expect("nearby player must get CharInfo");
    assert_eq!(char_info_object_id(&pkt), 6003);
    let rc = near_rx
        .try_recv()
        .expect("nearby player must get the paired RelationChanged");
    assert_eq!(rc[0], server_packets::opcodes::RELATION_CHANGED);
    assert_eq!(i32::from_le_bytes(rc[2..6].try_into().unwrap()), 6003);
    assert!(near_rx.try_recv().is_err());
    // …the far one (regions (4,4) vs (0,0)) hears nothing…
    assert!(
        far_rx.try_recv().is_err(),
        "far player must not get CharInfo"
    );
    // …and the newcomer's burst ends with the nearby player's CharInfo only.
    let to_newcomer = drain(&mut new_rx);
    let char_infos: Vec<i32> = to_newcomer
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::CHAR_INFO)
        .map(|p| char_info_object_id(p))
        .collect();
    assert_eq!(char_infos, vec![6001]);
}
