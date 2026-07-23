use super::*;

/// Registering a skill shortcut echoes `ShortCutRegister` + a `SkillList`
/// re-send (Java's quirk) and persists; deleting it re-sends the whole
/// (now empty) `ShortCutInit` and deletes the row.
#[test]
fn register_and_delete_shortcut_round_trip() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_short_cut_reg(
        &mut world,
        1,
        &shortcut_reg_body(2, 13, 1177, 1),
    );
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    assert_eq!(
        i32::from_le_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]),
        2,
        "SKILL type"
    );
    assert_eq!(packets[1][0], 0x5F, "SkillList re-send");
    let scs = player_shortcuts(&world, 3001);
    assert_eq!(scs.len(), 1);
    assert_eq!(
        (scs[0].slot, scs[0].page, scs[0].id, scs[0].level),
        (1, 1, 1177, 1)
    );
    // Memory-first: the shortcut lives in the Shortcuts component; no per-action
    // DB write (it persists on the next flush).
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "shortcut register does not touch the DB"
    );

    super::shortcuts::handle_request_short_cut_del(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(13);
        w.into_bytes()
    });
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_INIT);
    assert_eq!(
        i32::from_le_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]),
        0,
        "panel now empty"
    );
    assert!(player_shortcuts(&world, 3001).is_empty());
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "shortcut delete does not touch the DB"
    );
}

/// An ITEM shortcut referencing an object id not in the inventory isn't
/// stored or persisted — but the `ShortCutRegister` echo and `SkillList`
/// still go out, exactly like Java's unconditional replies.
#[test]
fn item_shortcut_without_item_not_stored_but_echoed() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_short_cut_reg(
        &mut world,
        1,
        &shortcut_reg_body(1, 0, 999_999, 0),
    );
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    assert!(player_shortcuts(&world, 3001).is_empty(), "not stored");
    assert!(drain_db(&mut db_rx).is_empty(), "not persisted");
}

/// `RequestMakeMacro` validation order and the no-recurring-macros
/// deviation: a SHORTCUT-type command is rejected with SM 810 and nothing
/// is stored (Java accepts it — that's the AFK macro-loop vector).
#[test]
fn make_macro_validations_and_recurring_rejection() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let macros_of = |world: &World| {
        world
            .objects
            .get_component::<Macros>(&3001)
            .unwrap()
            .entries
            .len()
    };

    // SHORTCUT command → invalid macro.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "loop", "d", &[(4, 0, 11, "")]),
    );
    assert_eq!(
        sm_id(&rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS
    );
    assert_eq!(macros_of(&world), 0);

    // Empty name.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "", "d", &[(1, 1177, 1, "")]),
    );
    assert_eq!(
        sm_id(&rx.try_recv().unwrap()),
        server_packets::sm_ids::ENTER_THE_NAME_OF_THE_MACRO
    );

    // Description over 32 chars.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "m", &"d".repeat(33), &[(1, 1177, 1, "")]),
    );
    assert_eq!(
        sm_id(&rx.try_recv().unwrap()),
        server_packets::sm_ids::MACRO_DESCRIPTIONS_MAY_CONTAIN_UP_TO_32_CHARACTERS
    );

    // Command strings over 255 chars total.
    let long = "x".repeat(256);
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "m", "d", &[(3, 0, 0, long.as_str())]),
    );
    assert_eq!(
        sm_id(&rx.try_recv().unwrap()),
        server_packets::sm_ids::INVALID_MACRO_REFER_TO_THE_HELP_FILE_FOR_INSTRUCTIONS
    );

    // Macro cap: with more than 48 stored, registration is refused.
    {
        let macros = world.objects.get_component_mut::<Macros>(&3001).unwrap();
        for i in 0..49 {
            macros.entries.push(Macro {
                id: 2000 + i,
                icon: 0,
                name: "m".into(),
                descr: String::new(),
                acronym: String::new(),
                commands: vec![],
            });
        }
    }
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "m", "d", &[(1, 1177, 1, "")]),
    );
    assert_eq!(
        sm_id(&rx.try_recv().unwrap()),
        server_packets::sm_ids::YOU_MAY_CREATE_UP_TO_48_MACROS
    );
    world
        .objects
        .get_component_mut::<Macros>(&3001)
        .unwrap()
        .entries
        .clear();
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "no rejected macro persisted"
    );

    // A valid macro: id 0 → allocated 1000, ADD echo, persisted.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "buffs", "d", &[(1, 1177, 1, ""), (3, 0, 0, "/sit")]),
    );
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::MACRO_LIST);
    assert_eq!(pkt[1], 1, "ADD");
    assert_eq!(i32::from_le_bytes([pkt[2], pkt[3], pkt[4], pkt[5]]), 1000);
    assert_eq!(macros_of(&world), 1);
    let stored = world
        .objects
        .get_component::<Macros>(&3001)
        .unwrap()
        .get(1000)
        .unwrap()
        .clone();
    assert_eq!(stored.commands.len(), 2);
    assert_eq!(stored.commands[1].cmd, "/sit");
    // Memory-first: the macro lives in the Macros component; no per-action write.
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "macro create does not touch the DB"
    );

    // Editing it (real id) → MODIFY echo, still one macro.
    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(1000, "buffs2", "d", &[(1, 1177, 1, "")]),
    );
    let pkt = rx.try_recv().unwrap();
    assert_eq!(pkt[1], 2, "MODIFY");
    assert_eq!(macros_of(&world), 1);
    assert_eq!(
        world
            .objects
            .get_component::<Macros>(&3001)
            .unwrap()
            .get(1000)
            .unwrap()
            .name,
        "buffs2"
    );
}

/// Deleting a macro removes it, cascade-deletes the panel slots holding it
/// (each re-sending `ShortCutInit`, like Java), and echoes the DELETE
/// `SendMacroList`.
#[test]
fn delete_macro_cascades_panel_slots() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    super::shortcuts::handle_request_make_macro(
        &mut world,
        1,
        &make_macro_body(0, "m", "d", &[(3, 0, 0, "/loc")]),
    );
    super::shortcuts::handle_request_short_cut_reg(
        &mut world,
        1,
        &shortcut_reg_body(4, 5, 1000, 0),
    );
    drain(&mut rx);
    drain_db(&mut db_rx);

    super::shortcuts::handle_request_delete_macro(&mut world, 1, &{
        let mut w = PacketWriter::new();
        w.write_i32(1000);
        w.into_bytes()
    });
    assert!(world
        .objects
        .get_component::<Macros>(&3001)
        .unwrap()
        .entries
        .is_empty());
    assert!(
        player_shortcuts(&world, 3001).is_empty(),
        "macro slot cascade-deleted"
    );
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 2);
    assert_eq!(
        packets[0][0],
        server_packets::opcodes::SHORT_CUT_INIT,
        "cascade re-sends the panel"
    );
    assert_eq!(packets[1][0], server_packets::opcodes::MACRO_LIST);
    assert_eq!(packets[1][1], 0, "DELETE");
    // Memory-first: the macro removal + shortcut cascade are in-memory (asserted
    // above); nothing is written per action.
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "macro delete cascade does not touch the DB"
    );
}

/// A skill upgrade rewrites the SKILL slots holding it: new level in the
/// component, a `ShortCutRegister` echo, and a row upsert
/// (`ShortCuts.updateShortCuts`, called from skill learn and level-up
/// grants).
#[test]
fn skill_upgrade_updates_matching_shortcuts() {
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    super::shortcuts::handle_request_short_cut_reg(
        &mut world,
        1,
        &shortcut_reg_body(2, 0, 1177, 1),
    );
    super::shortcuts::handle_request_short_cut_reg(&mut world, 1, &shortcut_reg_body(3, 1, 2, 0)); // an ACTION, untouched
    drain(&mut rx);
    drain_db(&mut db_rx);

    super::shortcuts::update_skill_shortcuts(&mut world, 3001, 1177, 2);
    let scs = player_shortcuts(&world, 3001);
    assert_eq!(scs.iter().find(|sc| sc.id == 1177).unwrap().level, 2);
    assert_eq!(
        scs.iter()
            .find(|sc| sc.kind == ShortcutType::Action)
            .unwrap()
            .level,
        0
    );
    let packets = drain(&mut rx);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][0], server_packets::opcodes::SHORT_CUT_REGISTER);
    // Memory-first: the level bump is in the Shortcuts component; no per-action write.
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "shortcut level bump does not touch the DB"
    );

    // No matching slot → no traffic.
    super::shortcuts::update_skill_shortcuts(&mut world, 3001, 9999, 1);
    assert!(drain(&mut rx).is_empty());
    assert!(drain_db(&mut db_rx).is_empty());
}

/// `from_char` restores the panel and macros; ITEM shortcuts whose object
/// id left the inventory are pruned (`ShortCuts.restoreMe`'s verification),
/// so they never reach the bundle and the next flush's reconcile drops their
/// rows (`stale_item_shortcuts` identifies them).
#[test]
fn from_char_restores_and_prunes_shortcuts() {
    let (world, ..) = test_world();
    let mut chr = dummy_char(3001, "P");
    chr.items = vec![crate::character::ItemRow {
        object_id: 500,
        item_id: 57,
        count: 10,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    }];
    let sc = |slot: i32, kind: ShortcutType, id: i32| Shortcut {
        slot,
        page: 0,
        kind,
        id,
        level: 1,
        character_type: 1,
        shared_reuse_group: -1,
    };
    chr.shortcuts = vec![
        sc(0, ShortcutType::Item, 500),
        sc(1, ShortcutType::Item, 999),
        sc(2, ShortcutType::Skill, 1177),
    ];
    chr.macros = vec![Macro {
        id: 1005,
        icon: 1,
        name: "m".into(),
        descr: String::new(),
        acronym: String::new(),
        commands: vec![MacroCmd {
            entry: 0,
            kind: MacroType::Text,
            d1: 0,
            d2: 0,
            cmd: "/loc".into(),
        }],
    }];

    let bundle = Player::from_char(&world.data, &chr);
    let restored: Vec<_> = bundle.shortcuts.iter().copied().collect();
    assert_eq!(restored.len(), 2, "stale ITEM shortcut pruned");
    assert!(restored
        .iter()
        .any(|s| s.kind == ShortcutType::Item && s.id == 500));
    assert!(restored
        .iter()
        .any(|s| s.kind == ShortcutType::Skill && s.id == 1177));
    assert_eq!(Player::stale_item_shortcuts(&chr), vec![(1, 0)]);
    assert_eq!(bundle.macros.entries.len(), 1);
    assert_eq!(bundle.macros.entries[0].commands[0].cmd, "/loc");
}

/// The enter-world burst carries the real `ShortCutInit` and the macro LIST
/// packets in Java's order (macros before `ItemList`, panel after it).
#[test]
fn enter_world_sends_macros_and_shortcut_panel() {
    let (mut world, ..) = test_world();
    let mut chr = dummy_char(3001, "P");
    chr.shortcuts = vec![Shortcut {
        slot: 0,
        page: 0,
        kind: ShortcutType::Action,
        id: 2,
        level: 0,
        character_type: 1,
        shared_reuse_group: -1,
    }];
    chr.macros = vec![Macro {
        id: 1000,
        icon: 0,
        name: "m".into(),
        descr: String::new(),
        acronym: String::new(),
        commands: vec![],
    }];
    let bundle = Player::from_char(&world.data, &chr);
    let (out_tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let s = Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())
        .into_authenticated("bob".into(), SessionKey::new(1, 2, 3, 4))
        .into_lobby(vec![])
        .into_entering(bundle);
    world.clients.insert(1, ClientSession::Entering(s));

    handle_enter_world(&mut world, 1);
    let packets = drain(&mut rx);
    let pos = |op: u8| {
        packets
            .iter()
            .position(|p| p[0] == op)
            .unwrap_or_else(|| panic!("opcode 0x{op:02x} missing"))
    };
    let macro_pos = pos(server_packets::opcodes::MACRO_LIST);
    let item_list_pos = pos(0x11);
    let shortcut_pos = pos(server_packets::opcodes::SHORT_CUT_INIT);
    assert!(macro_pos < item_list_pos, "macros before ItemList");
    assert!(item_list_pos < shortcut_pos, "ShortCutInit after ItemList");
    let sc_pkt = &packets[shortcut_pos];
    assert_eq!(
        i32::from_le_bytes([sc_pkt[1], sc_pkt[2], sc_pkt[3], sc_pkt[4]]),
        1,
        "one shortcut"
    );
    let m_pkt = &packets[macro_pos];
    assert_eq!(m_pkt[6], 1, "one macro in the LIST burst");
}
