use super::*;

/// RequestShowMiniMap (0x6C): empty body, answered with `ShowMiniMap` —
/// map id 0 (base world map) plus the Seven Signs state byte.
#[test]
fn request_show_mini_map_opens_world_map() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SHOW_MINI_MAP]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::SHOW_MINI_MAP);
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 0, "base world map");
    assert_eq!(pkt[5], 0, "Seven Signs state");
    assert_eq!(pkt.len(), 6);
}

/// The world map's data requests: `RequestAllCastleInfo` (0xD0:0x39) and
/// `RequestAllFortressInfo` (0xD0:0x3A) are answered with the static
/// residence lists — 9 castles and 21 forts, all unowned.
#[test]
fn map_castle_and_fortress_info_requests_answered() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::EX_PACKET, 0x39, 0x00]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::EX);
    assert_eq!(i16::from_le_bytes(pkt[1..3].try_into().unwrap()), server_packets::opcodes::EX_SHOW_CASTLE_INFO);
    assert_eq!(i32::from_le_bytes(pkt[3..7].try_into().unwrap()), 9, "nine castles");
    assert!(a_rx.try_recv().is_err(), "no PartyMemberPosition when solo");

    on_packet(&mut world, 1, vec![cop::EX_PACKET, 0x3A, 0x00]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], server_packets::opcodes::EX);
    assert_eq!(i16::from_le_bytes(pkt[1..3].try_into().unwrap()), server_packets::opcodes::EX_SHOW_FORTRESS_INFO);
    assert_eq!(i32::from_le_bytes(pkt[3..7].try_into().unwrap()), 21, "twenty-one forts");
}

/// RequestSkillList (0x50): empty body, re-sends the `SkillList` packet
/// (`player.sendSkillList()`) — the client asks for this when it opens the
/// skills panel.
#[test]
fn request_skill_list_resends_skill_list() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // 4 known skills
    drain(&mut a_rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_SKILL_LIST]);
    let pkt = a_rx.try_recv().unwrap();
    assert_eq!(pkt[0], 0x5F, "SkillList opcode");
    assert_eq!(i32::from_le_bytes(pkt[1..5].try_into().unwrap()), 4, "all known skills listed");
}

/// `RequestStopMove` (`player.stopMove(getLocation())`): the in-flight move
/// and any pending path request are dropped, and `StopMove` is broadcast to
/// the mover (Player `broadcastPacket` includes self) at the current spot.
#[test]
fn request_stop_move_clears_movement_and_pending_path() {
    use crate::model::components::PathWait;
    use crate::model::movement::MoveData;

    let (mut world, ..) = test_world();
    let mut rx = ingame_player(&mut world, 1, 5001, 700, 800, 0);

    // Simulate an in-flight move plus a still-outstanding path request.
    world.objects.add_components(
        &5001,
        Movement(MoveData {
            start_x: 700,
            start_y: 800,
            start_z: 0,
            dest_x: 2000,
            dest_y: 800,
            dest_z: 0,
            start_tick: world.tick,
            total_ticks: 100,
            geo_path: None,
        }),
    );
    world.objects.add_components(&5001, PathWait { seq: 42 });

    handle_request_stop_move(&mut world, 1);

    assert!(!world.objects.has_component::<Movement>(&5001), "move data deleted");
    assert!(!world.objects.has_component::<PathWait>(&5001), "pending path dropped");
    assert_eq!(rx.try_recv().unwrap()[0], server_packets::opcodes::STOP_MOVE);
}

/// `ExSendSelectedQuestZoneID` stores the selected zone id on the player
/// (default -1 → the client's choice), read later by quest teleports.
#[test]
fn ex_send_selected_quest_zone_id_sets_field() {
    let (mut world, ..) = test_world();
    let _rx = ingame_player(&mut world, 1, 5001, 10, 20, 30);
    assert_eq!(world.objects.get_component::<Player>(&5001).unwrap().quest_zone_id, -1);

    handle_ex_send_selected_quest_zone_id(&mut world, 1, &int_body(7));

    assert_eq!(world.objects.get_component::<Player>(&5001).unwrap().quest_zone_id, 7);
}

/// The full Q00258 loop against the real dist htmls: quest window on talk
/// (`ExNpcQuestHtmlMessage` for the `.htm`), accept event (`startQuest`:
/// cond 1 + STARTED persisted, accept sound, `.html` via plain
/// `NpcHtmlMessage`), pelts accumulating on kills (quest tab refresh +
/// "earned" SM), the 40-pelt cond bump (`ExShowQuestMark` + middle sound),
/// and the turn-in (reward roll, quest items destroyed with removed-type
/// `InventoryUpdate` + DB deletes, repeatable exit wiping the state).
#[test]
fn quest_q00258_accept_collect_turn_in() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 3;
    drain_db(&mut db_rx);

    // Talk: the single talk-quest short-circuits the chooser; CREATED at
    // level 3 → 30001-02.htm → the quest-window packet (FE:0x8E).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)), "quest window html");

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html")),
    );
    let pkts = drain(&mut rx);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.state, crate::model::quest::state::STARTED);
        assert_eq!(qs.cond(), 1);
    }
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::QUEST_LIST), "QuestList after accept");
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_accept".to_string()), "accept sound");
    assert!(
        pkts.iter().any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        ".html result uses the plain window"
    );
    // Memory-first: cond + state land in the Quests component (they persist on
    // the next flush, not per set).
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.cond(), 1, "cond set in memory");
        assert_eq!(qs.state, crate::model::quest::state::STARTED, "state Started in memory");
    }

    // First wolf kill: one pelt, earned-SM, quest-tab refresh, itemget sound.
    let wolf = NPC_OID + 1;
    add_test_npc(&mut world, wolf, 20120, "Monster", 5, 30, 0, 0);
    death::npc_do_die(&mut world, wolf, 3001);
    let pkts = drain(&mut rx);
    let inv_count = |world: &World| {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&3001)
            .unwrap()
            .count_of(702)
    };
    assert_eq!(inv_count(&world), 1);
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1), "earned SM");
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_QUEST_ITEM_LIST)), "quest tab refresh");
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_itemget".to_string()));

    // 38 more pelts, then the 40th kill flips cond 2 (+ mark + middle).
    super::items::add_inventory_item(&mut world, 3001, 702, 38).unwrap();
    let wolf2 = NPC_OID + 2;
    add_test_npc(&mut world, wolf2, 20442, "Monster", 5, 30, 0, 0);
    death::npc_do_die(&mut world, wolf2, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 40);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert_eq!(quests.0["Q00258_BringWolfPelts"].cond(), 2);
    }
    let mark = pkts.iter().find(|p| is_ex(p, server_packets::opcodes::EX_SHOW_QUEST_MARK)).expect("quest mark");
    assert_eq!(i32::from_le_bytes(mark[3..7].try_into().unwrap()), 258);
    assert_eq!(i32::from_le_bytes(mark[7..11].try_into().unwrap()), 2);
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()));

    // Turn-in: roll 0 → Cloth Cap; pelts destroyed; repeatable exit.
    drain_db(&mut db_rx);
    world.forced_rolls.push_back(0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts")));
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 0, "pelts destroyed on exit");
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(41),
        1,
        "Cloth Cap rewarded on roll 0"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00258_BringWolfPelts")
            .is_none(),
        "repeatable exit forgets the quest"
    );
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()));
    // The removal reaches the client as a removed-type InventoryUpdate.
    assert!(
        pkts.iter().any(|p| p[0] == 0x21 && i16::from_le_bytes([p[3], p[4]]) == 3),
        "InventoryUpdate with change type 3 (removed)"
    );
    // Memory-first: the pelts are gone from the Inventory component and the
    // quest from the Quests component (both asserted above); the flush reconcile
    // deletes their rows — no per-action DB write.

    // Re-talk: the quest is takeable again (CREATED intro window).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)), "repeatable re-offer");
}

/// Q00320's chance-drop path (forced `roll_f64`), the giveItemRandomly
/// limit semantics, the level/race start gates, and the rated adena reward.
#[test]
fn quest_q00320_chance_drops_and_adena_reward() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30359, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 2; // Dark Elf
    }
    drain_db(&mut db_rx);

    // Accept (talk creates the CREATED state, the event starts it).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture 30359-04.htm")),
    );
    drain(&mut rx);

    let skel = NPC_OID + 1;
    add_test_npc(&mut world, skel, 20517, "Monster", 5, 30, 0, 0);

    // Roll 0.999999 > 0.18 → no drop.
    world.forced_rolls.push_back(999_999);
    death::npc_do_die(&mut world, skel, 3001);
    let count_of = |world: &World, id: i32| {
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(id)
    };
    assert_eq!(count_of(&world, 809), 0, "18% roll failed");

    // Roll 0 → drop.
    let skel2 = NPC_OID + 2;
    add_test_npc(&mut world, skel2, 20517, "Monster", 5, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, skel2, 3001);
    assert_eq!(count_of(&world, 809), 1);
    drain(&mut rx);

    // 9 bones banked, the 10th caps the collection: cond 2 + middle sound.
    super::items::add_inventory_item(&mut world, 3001, 809, 8).unwrap();
    let skel3 = NPC_OID + 3;
    add_test_npc(&mut world, skel3, 20517, "Monster", 5, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, skel3, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 10);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert_eq!(quests.0["Q00320_BonesTellTheFuture"].cond(), 2);
    }
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()), "limit-reached sound");

    // Turn-in: 500 adena (rates ×1 in tests), bones destroyed, exit.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture")));
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 0);
    assert_eq!(count_of(&world, 57), 500, "500 adena at ×1 rates");
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1_ADENA));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00320_BonesTellTheFuture")
            .is_none()
    );
}

/// The quest UI's Abandon button (`RequestQuestAbort` 0x63): repeatable
/// exit without the finish sound — state forgotten, quest items destroyed.
#[test]
fn quest_abort_wipes_state_and_items() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 3;

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html")),
    );
    super::items::add_inventory_item(&mut world, 3001, 702, 5).unwrap();
    drain(&mut rx);
    drain_db(&mut db_rx);

    let mut w = PacketWriter::new();
    w.write_i32(258);
    on_packet(&mut world, 1, {
        let mut v = vec![cop::REQUEST_QUEST_ABORT];
        v.extend(w.into_bytes());
        v
    });

    let pkts = drain(&mut rx);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Quests>(&3001)
            .unwrap()
            .0
            .get("Q00258_BringWolfPelts")
            .is_none(),
        "abort forgets the quest"
    );
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().count_of(702),
        0,
        "quest items destroyed"
    );
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::QUEST_LIST), "QuestList refresh");
    assert!(!sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()), "no finish sound on abort");
    // Memory-first: the quest is forgotten in the Quests component (asserted
    // above); the flush reconcile drops its rows — no per-action DB write.
}

/// Quest-timer groundwork: a synthetic script starts a 500 ms timer via an
/// event bypass; it fires once through the scheduler (seq match) and a
/// cancelled one stays silent (seq bumped).
#[test]
fn quest_timer_fires_once_and_cancels() {
    struct TimerTestScript;
    impl quests::QuestScript for TimerTestScript {
        fn id(&self) -> i32 {
            -2
        }
        fn name(&self) -> &'static str {
            "TimerTest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[30001]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_event(&self, ctx: &mut quests::QuestCtx, event: &str) -> Option<String> {
            match event {
                "start" => ctx.start_quest_timer("tick", 500),
                "cancel" => ctx.cancel_quest_timer("tick"),
                _ => {}
            }
            None
        }
        fn on_timer(&self, ctx: &mut quests::QuestCtx, name: &str) {
            if name == "tick" {
                ctx.play_sound("timer_fired");
            }
        }
    }

    let (mut world, _db_rx, _link_rx) = quest_test_world();
    world.quests = std::sync::Arc::new(quests::QuestRegistry::new(vec![std::sync::Arc::new(TimerTestScript)]));
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")));
    drain(&mut rx);
    advance_ticks(&mut world, 5);
    let pkts = drain(&mut rx);
    assert!(sound_names(&pkts).contains(&"timer_fired".to_string()), "timer fired at 500 ms");
    advance_ticks(&mut world, 10);
    assert!(drain(&mut rx).is_empty(), "non-repeating: fires once");

    // Start then cancel: the stale seq no-ops.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest start")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TimerTest cancel")));
    drain(&mut rx);
    advance_ticks(&mut world, 10);
    assert!(sound_names(&drain(&mut rx)).is_empty(), "cancelled timer never fires");
}

/// A purchase debits adena, adds the items, and answers with the
/// InventoryUpdate/inven-weight/sell-refresh/SM-4358 tail; the guards
/// (wrong quantity, empty purse, no merchant target) refuse cleanly.
#[test]
fn request_buy_item_purchases_and_guards() {
    let (mut world, _db_rx, mut rx) = shop_world();

    // 1 Cloth Cap (100) + 5 potions (50) = 150 adena.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1), (1061, 5)]));
    assert_eq!(adena_of(&world, 3001), 850);
    assert_eq!(count_of_item(&world, 3001, 41), 1);
    assert_eq!(count_of_item(&world, 3001, 1061), 5);
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == 0x21), "InventoryUpdate");
    assert!(pkts.iter().any(|p| is_ex(p, 0x166)), "ExUserInfoInvenWeight");
    let sell_done = pkts.iter().find(|p| is_ex(p, crate::network::trade::EX_BUY_SELL_LIST)).expect("sell refresh");
    assert_eq!(*sell_done.last().unwrap(), 1, "done flag");
    assert!(sm_ids_of(&pkts).contains(&server_packets::sm_ids::EXCHANGE_IS_SUCCESSFUL));

    // Non-stackable quantity > 1: SM 1036, nothing purchased.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 2)]));
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_HAVE_EXCEEDED_THE_QUANTITY_THAT_CAN_BE_INPUTTED));
    assert_eq!(adena_of(&world, 3001), 850);

    // Too expensive: SM 279.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 100)]));
    assert!(sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA));
    assert_eq!(adena_of(&world, 3001), 850);

    // Off-list item: dropped, no charge.
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(702, 1)]));
    assert!(drain(&mut rx).is_empty());
    assert_eq!(adena_of(&world, 3001), 850);

    // No merchant targeted: ActionFailed.
    handle_request_target_canceld(&mut world, 1, &target_canceld_body(true));
    drain(&mut rx);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(1061, 1)]));
    let pkts = drain(&mut rx);
    assert!(pkts.iter().any(|p| p[0] == server_packets::opcodes::ACTION_FAIL));
    assert_eq!(adena_of(&world, 3001), 850);
}

/// Q00303 Collect Arrowheads: accept → 40%-chance drops to the 10-arrowhead
/// cap (cond 2) → turn-in pays 500 adena and exits repeatably.
#[test]
fn quest_q00303_collect_arrowheads_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(963, "Orcish Arrowhead", true)]);
    let mut t = crate::data::npc_data::default_template(20361);
    t.type_name = "Monster".into();
    t.level = 11;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30029, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;
    drain_db(&mut db_rx);

    // Accept.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads 30029-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, "Q00303_CollectArrowheads"), Some(1));
    drain(&mut rx);

    // Kill 10 marksmen with the 40% roll forced to hit each time.
    let mob = NPC_OID + 1;
    for i in 0..10 {
        add_test_npc(&mut world, mob + i, 20361, "Monster", 11, 30, 0, 0);
        world.forced_rolls.push_back(0); // roll_f64 → 0.0 ≤ 0.4
        death::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 963), 10);
    assert_eq!(quest_cond(&world, 3001, "Q00303_CollectArrowheads"), Some(2));
    drain(&mut rx);

    // Turn-in.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 500);
    assert_eq!(item_count(&world, 3001, 963), 0, "quest items removed on exit");
    assert!(quest_cond(&world, 3001, "Q00303_CollectArrowheads").is_none(), "repeatable exit");
}

/// Q00316 Destroy Plague Carriers: the first hit on Varool Foulclaw makes
/// him shout (`on_attack` + script value), his fang drops at most once, and
/// the turn-in pays the fang/wererat ladder.
#[test]
fn quest_q00316_on_attack_say_and_limited_fang() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1042, "Wererat Fang", true), (1043, "Varool Foulclaw Fang", true)]);
    for id in [27020, 20040] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30155, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 1; // Elf
    }
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers 30155-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, "Q00316_DestroyPlagueCarriers"), Some(1));
    drain(&mut rx);

    // First hit on Varool: exactly one NpcSay; further hits stay quiet.
    let varool = NPC_OID + 1;
    add_test_npc(&mut world, varool, 27020, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0);
    let pkts = drain(&mut rx);
    let says: Vec<_> = pkts.iter().filter(|p| p[0] == server_packets::opcodes::NPC_SAY).collect();
    assert_eq!(says.len(), 1, "one shout on the first hit");
    assert_eq!(i32::from_le_bytes(says[0][13..17].try_into().unwrap()), 31603, "WHY_DO_YOU_OPPRESS_US_SO");
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0);
    assert!(
        !drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "script value keeps him quiet"
    );

    // His fang drops once (chance 10/7 ≥ 1 → guaranteed), never twice.
    death::npc_do_die(&mut world, varool, 3001);
    assert_eq!(item_count(&world, 3001, 1043), 1);
    let varool2 = NPC_OID + 2;
    add_test_npc(&mut world, varool2, 27020, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, varool2, 3001);
    assert_eq!(item_count(&world, 3001, 1043), 1, "only one Varool fang ever");

    // Wererats drop fangs freely (chance 2.0 → always).
    for i in 0..10 {
        let rat = NPC_OID + 3 + i;
        add_test_npc(&mut world, rat, 20040, "Monster", 20, 30, 0, 0);
        death::npc_do_die(&mut world, rat, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1042), 10);
    drain(&mut rx);

    // Turn-in: 10×5 + 1×1000 + 5000 bonus.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 50 + 1000 + 5000);
    assert_eq!(item_count(&world, 3001, 1042), 0);
    assert_eq!(item_count(&world, 3001, 1043), 0);
}

/// Q00109 In Search of the Nest: the three-NPC cond 1→2→3 chain ends in a
/// one-time completion — the quest survives as COMPLETED and answers with
/// the already-completed page.
#[test]
fn quest_q00109_multi_cond_one_time() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(14858, "Scout's Note", true)]);
    let (pierce, corpse, kahman) = (NPC_OID, NPC_OID + 1, NPC_OID + 2);
    add_test_npc(&mut world, pierce, 31553, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, corpse, 32015, "Folk", 5, 120, 0, 0);
    add_test_npc(&mut world, kahman, 31554, "Folk", 5, 140, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 81;
    drain_db(&mut db_rx);

    let q = "Q00109_InSearchOfTheNest";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-0.htm")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1));

    // The corpse: cond 2 + the note.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{corpse}_Quest {q} 32015-2.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 14858), 1);

    // Back to Pierce: cond 3, note taken.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q} 31553-3.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(item_count(&world, 3001, 14858), 0);

    // Kahman pays out; one-time exit keeps the COMPLETED state.
    let (adena, exp) = (
        item_count(&world, 3001, 57),
        world.objects.get_component::<Player>(&3001).unwrap().exp,
    );
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kahman}_Quest {q} 31554-2.html")));
    assert_eq!(item_count(&world, 3001, 57), adena + 161500);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().exp > exp);
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
    }

    // Talking to Pierce again answers the already-completed page.
    drain(&mut rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pierce}_Quest {q}")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("html");
    assert!(
        html.contains("already completed") || html.contains("already been completed"),
        "already-completed message, got: {html}"
    );
}

/// OrcChange1: an eligible Orc Fighter with the Mark of Raider becomes an
/// Orc Raider — proof consumed, 15 coupons paid, class persisted; the
/// category gates refuse a player who already transferred.
#[test]
fn orc_change1_first_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1592, "Mark of Raider", true), (8869, "Shadow Coupon (D)", false)]);
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[44, 45]);
    world.data.categories.insert_for_test("MAGE_GROUP", &[49]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[45]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30500, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 3; // Orc
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
    }
    super::items::add_inventory_item(&mut world, 3001, 1592, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // The named bypass shows the fighter class list.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("class list");
    assert!(html.contains("45") || !html.is_empty());

    // Transfer to Orc Raider (45).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")));
    {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        assert_eq!(p.class_id, 45);
        assert_eq!(p.base_class_id, 45);
    }
    assert_eq!(item_count(&world, 3001, 1592), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "shadow coupons");
    // The change persisted immediately.
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(c, db::DbCommand::StorePlayer { save } if save.base.class_id == 45)),
        "StorePlayer with the new class"
    );
    // A UserInfo re-broadcast reached the player.
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x32), "UserInfo after transfer");

    // Now in SECOND_CLASS_GROUP: another transfer attempt is refused.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("refusal page");
    assert!(html.contains("class transfer") || !html.is_empty());
    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 45, "unchanged");
}

/// TeleportWithCharm: the bare `Quest` click consumes the token and
/// teleports; without a token it shows the "come back with one" page.
#[test]
fn teleport_with_charm_consumes_token() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1659, "Gatekeeper Token", false)]);
    add_test_npc(&mut world, NPC_OID, 30540, "Teleporter", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);

    // No token: the explain page.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("no-token page");
    assert!(html.contains("Token") || html.contains("token"), "got: {html}");

    // With a token: teleport + consumption.
    super::items::add_inventory_item(&mut world, 3001, 1659, 1);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    assert_eq!(item_count(&world, 3001, 1659), 0, "token consumed");
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (-80826, 149775, -3038), "destination z lifted by 5 (teleToLocation)");
    assert!(world.objects.get_component::<Player>(&3001).unwrap().teleporting);
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x22),
        "TeleportToLocation sent"
    );
}

/// TeleportToRaceTrack: a gatekeeper's free "Monster Race Track" button
/// sends the player to the arena and records the origin in `MONSTER_RETURN`;
/// the Race Manager reads it back and returns them, clearing the variable.
/// (Destination z is lifted by 5 by `teleToLocation`, as in the charm test.)
#[test]
fn teleport_to_race_track_round_trips_via_monster_return() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Trisha (Dion gatekeeper) and the Race Manager at the arena.
    add_test_npc(&mut world, NPC_OID, 30059, "Teleporter", 70, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30995, "RaceManager", 70, 12661, 181687, -3540);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // Outbound: the gatekeeper's button.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TeleportToRaceTrack")));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (12661, 181687, -3535), "at the race track");
    assert_eq!(
        world.objects.get_component::<crate::model::components::PlayerVariables>(&3001).unwrap().get_int("MONSTER_RETURN", -1),
        30059,
        "origin gatekeeper remembered"
    );
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x22), "TeleportToLocation sent");

    // Inbound: the manager sends them back to Trisha's town, not the default.
    world.objects.get_component_mut::<Player>(&3001).unwrap().teleporting = false;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest TeleportToRaceTrack", NPC_OID + 1)),
    );
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (15670, 142983, -2700), "returned to Dion");
    assert_eq!(
        world.objects.get_component::<crate::model::components::PlayerVariables>(&3001).unwrap().get_int("MONSTER_RETURN", -1),
        -1,
        "return point consumed"
    );
}

/// The Race Manager with no stored origin falls back to Trisha (Dion) —
/// Java's `TELEPORTER_LOCATIONS.get(30059)` branch.
#[test]
fn race_manager_without_monster_return_falls_back_to_dion() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    // Within interaction range of the player — the bypass is distance-gated.
    add_test_npc(&mut world, NPC_OID, 30995, "RaceManager", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest TeleportToRaceTrack")));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!((pos.x, pos.y, pos.z), (15670, 142983, -2700), "default return is Dion");
}

/// `RequestSellItem` (0x37) sells inventory items to the targeted merchant for
/// reference-price/2 adena each.
#[test]
fn request_sell_item_pays_adena() {
    let (mut world, _db_rx, mut rx) = shop_world();
    world.data.item_data.insert_for_test(crate::data::item_data::ItemTemplate {
        item_id: 5000,
        name: "Trophy".into(),
        kind: crate::data::item_data::ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: true,
        type1: 4,
        type2: 5,
        is_quest_item: false,
        price: 200, // sells for 100 each
        handler: crate::data::item_data::ItemHandler::None,
        crystal_type: crate::data::item_data::CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(), etc_item_type: crate::data::item_data::EtcItemType::Other, enchant_enabled: false, enchant_limit: 0, is_magic_weapon: false,
    });
    super::items::add_inventory_item(&mut world, 3001, 5000, 10).expect("trophies");
    let oid = world.objects.get_component::<crate::model::inventory::Inventory>(&3001).unwrap().items().iter().find(|it| it.item_id == 5000).unwrap().object_id;
    drain(&mut rx);

    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_SELL_ITEM);
    w.write_i32(3); // list id
    w.write_i32(1); // one line
    w.write_i32(oid);
    w.write_i32(5000);
    w.write_i64(4);
    on_packet(&mut world, 1, w.into_bytes());

    assert_eq!(count_of_item(&world, 3001, 5000), 6, "4 sold");
    assert_eq!(adena_of(&world, 3001), 1000 + 400, "paid 4 × (200/2)");
    assert!(drain(&mut rx).iter().any(|p| p[0] == 0x21), "InventoryUpdate sent");
}

/// Register a Newbie Guide (30598, Talking Island / Human) as a live NPC,
/// with the `<race>HUMAN</race>` its dist template declares.
fn add_newbie_guide(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(30598);
    t.type_name = "Folk".into();
    t.name = "Newbie Guide".into();
    t.race = Some(0); // HUMAN
    world.data.npc_data.insert_for_test(t);
    add_test_npc(world, NPC_OID, 30598, "Folk", 70, 0, 0, 0);
}

/// `NewbieGuide.onFirstTalk`: an `addFirstTalkId` script owns the whole chat
/// window. Without the first-talk route the guide has no
/// `data/html/default/30598.htm`, so it degrades to `npcdefault.htm`'s lone
/// "Quest" button — the four-entry menu below is the regression guard.
#[test]
fn newbie_guide_first_talk_replaces_the_default_chat_window() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    // First click targets, second interacts (Java `Player.doInteract`).
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("guide window");
    assert!(html.contains("Ask for an advice"), "advice entry: {html}");
    assert!(html.contains("Quest NpcLocationInfo"), "npc-location entry: {html}");
    assert!(html.contains("Link default/SupportMagic.htm"), "support-magic entry: {html}");
    assert!(html.contains("action=\"bypass -h Quest\">Quest"), "quest entry: {html}");
    assert!(!html.contains("I have nothing to say"), "not the npcdefault fallback: {html}");
}

/// The race gate: a guide only advises its own race (`npc.getRace() !=
/// player.getRace()` → `-no.htm`).
#[test]
fn newbie_guide_turns_away_other_races() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<crate::model::Player>(&3001).unwrap().race = 1; // ELF
    drain(&mut rx);

    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    handle_action(&mut world, 1, &action_body(NPC_OID, 0));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("refusal window");
    assert!(!html.contains("Ask for an advice"), "menu withheld: {html}");
}

/// The advice pages: `Quest NewbieGuide <n>` picks `<npcId>-<n><m|f>.htm`,
/// `f` for the fighter class this test's player carries. Event `0` returns
/// to the menu.
#[test]
fn newbie_guide_advice_pages_follow_the_class_suffix() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NewbieGuide 1"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("advice page");
    assert!(html.contains("What should I do now?"), "30598-1f.htm: {html}");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NewbieGuide 0"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("menu");
    assert!(html.contains("Ask for an advice"), "back to the menu: {html}");
}

/// `NpcLocationInfo`: the bare bypass opens the profession list, a page name
/// navigates, and a whitelisted npc id drops a radar marker on its spawn.
#[test]
fn npc_location_info_marks_the_requested_npc_on_the_radar() {
    let (mut world, ..) = quest_test_world();
    add_newbie_guide(&mut world);
    // Gatekeeper Roxxy — a whitelisted target, spawned so the lookup lands.
    add_test_npc(&mut world, NPC_OID + 1, 30006, "Teleporter", 70, 500, 600, 700);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("profession list");
    assert!(html.contains("Teleporter"), "30598.htm: {html}");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo 30598-1.htm"));
    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("teleporter page");
    assert!(html.contains("Gatekeeper Roxxy"), "30598-1.htm: {html}");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo 30006"));
    let pkts = drain(&mut rx);
    let html = pkts.iter().find_map(|p| decode_npc_html(p)).expect("MoveToLoc page");
    assert!(html.contains("direction of the arrow"), "MoveToLoc.htm: {html}");
    assert!(pkts.iter().any(|p| p[0] == 0xF1), "RadarControl sent");

    // Off-whitelist id: Java returns null, so nothing is sent.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Quest NpcLocationInfo 99999"));
    assert!(drain(&mut rx).is_empty(), "no window for an unlisted npc");
}

/// DwarfWarehouseChange1: a Dwarven Fighter with the Ring of Raven becomes a
/// Scavenger. Mirrors the OrcChange1 test, but on the shared
/// `DwarfChange1` implementation.
#[test]
fn dwarf_warehouse_change1_first_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1642, "Ring of Raven", true), (8869, "Shadow Coupon (D)", false)]);
    world.data.categories.insert_for_test("BOUNTY_HUNTER_GROUP", &[53, 54]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[54]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30498, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 4; // Dwarf
        p.class_id = 53;
        p.base_class_id = 53;
    }
    super::items::add_inventory_item(&mut world, 3001, 1642, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfWarehouseChange1 54")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 54, "now a Scavenger");
    assert_eq!(p.base_class_id, 54, "on the base slot the base class moves too");
    assert_eq!(item_count(&world, 3001, 1642), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "shadow coupons paid");
}

/// The level gate: 19 is refused, the proof is kept, and nothing is paid.
#[test]
fn dwarf_change1_refuses_below_level_20() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1635, "Final Pass Certificate", true), (8869, "Shadow Coupon (D)", false)]);
    world.data.categories.insert_for_test("WARSMITH_GROUP", &[53, 56]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[56]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30499, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.race = 4;
        p.class_id = 53;
        p.base_class_id = 53;
    }
    super::items::add_inventory_item(&mut world, 3001, 1635, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange1 56")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 53, "still a Dwarven Fighter at 19");
    assert_eq!(item_count(&world, 3001, 1635), 1, "the proof is NOT consumed on a refusal");
    assert_eq!(item_count(&world, 3001, 8869), 0, "and nothing is paid");
}

/// Without the proof item the transfer is refused even at level 20.
#[test]
fn dwarf_change1_refuses_without_the_proof_item() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1635, "Final Pass Certificate", true), (8869, "Shadow Coupon (D)", false)]);
    world.data.categories.insert_for_test("WARSMITH_GROUP", &[53, 56]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[56]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30499, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 4;
        p.class_id = 53;
        p.base_class_id = 53;
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange1 56")),
    );

    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 53, "no proof, no transfer");
}

/// Every html page the two scripts can return must exist in the dist, or a
/// player hits a blank window at the moment of their class change.
#[test]
fn dwarf_change1_html_pages_exist_in_dist() {
    // Village-master pages live under data/scripts/, not data/html/.
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/");
    for (dir, npcs, extra) in [
        ("village_master/DwarfBlacksmithChange1", [30499, 30504, 30595, 32093], "30499-12.htm"),
        ("village_master/DwarfWarehouseChange1", [30498, 30503, 30594, 32092], "30498-12.htm"),
    ] {
        for npc in npcs {
            // -01/-05 from onTalk, -06/-07 refusals, -08..-11 the level/proof
            // matrix, -10 the success page.
            for suffix in ["01", "05", "06", "07", "08", "09", "10", "11"] {
                let path = format!("{DIST}{dir}/{npc}-{suffix}.htm");
                assert!(
                    std::path::Path::new(&path).exists(),
                    "missing dist page {dir}/{npc}-{suffix}.htm"
                );
            }
        }
        // Only the *first* NPC of each set ships a `-12` page; Java hard-codes
        // that one id for the fourth-class refusal regardless of who you are
        // talking to, which is why the port does the same.
        assert!(
            std::path::Path::new(&format!("{DIST}{dir}/{extra}")).exists(),
            "missing fourth-class page {dir}/{extra}"
        );
    }
}

/// ElfHumanFighterChange1: a Human Fighter with the Medallion of Warrior
/// becomes a Warrior. The same NPCs serve Elves, so the elf branch is checked
/// too — a bad `from_class` match would let a Human take an Elven class.
#[test]
fn elf_human_fighter_change1_transfers_by_race() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1145, "Medallion of Warrior", true), (8869, "Coupon", false)]);
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[0, 1, 18, 19]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[1, 19]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30066, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 0; // Human
        p.class_id = 0; // Fighter
        p.base_class_id = 0;
    }
    super::items::add_inventory_item(&mut world, 3001, 1145, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // A Human Fighter may not take the Elven Knight (19) branch.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange1 19")),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().class_id,
        0,
        "a Human must not take an Elf class from the same NPC"
    );
    assert_eq!(item_count(&world, 3001, 1145), 1, "and nothing was consumed");

    // Warrior (1) is the Human branch.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange1 1")),
    );
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 1, "now a Warrior");
    assert_eq!(p.base_class_id, 1);
    assert_eq!(item_count(&world, 3001, 1145), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// ElfHumanWizardChange1: an Elven Mage becomes an Oracle.
#[test]
fn elf_human_wizard_change1_elf_branch() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1235, "Leaf of Oracle", true), (8869, "Coupon", false)]);
    world.data.categories.insert_for_test("MAGE_GROUP", &[10, 25, 29]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[29]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30037, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 1; // Elf
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
    }
    super::items::add_inventory_item(&mut world, 3001, 1235, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanWizardChange1 29")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 29, "now an Oracle");
    assert_eq!(item_count(&world, 3001, 1235), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// Every page the two scripts can return must exist in the dist. The matrix is
/// table-driven (four consecutive pages per target), so an off-by-one in the
/// table would silently serve the wrong — or a missing — page.
#[test]
fn elf_human_change1_html_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/");
    // (dir, npcs, per-target first pages, talk/refusal pages, fourth-class page)
    let sets: [(&str, &[i32], &[u32], &[u32], &str); 2] = [
        (
            "village_master/ElfHumanFighterChange1",
            &[30066, 30288, 30373, 32094],
            &[21, 25, 29, 33, 37],
            &[1, 11, 18, 19, 20],
            "30066-41.htm",
        ),
        (
            "village_master/ElfHumanWizardChange1",
            &[30037, 30070, 30289, 32095, 32098],
            &[18, 22, 26, 30],
            &[1, 8, 15, 16, 17],
            "30037-34.htm",
        ),
    ];
    for (dir, npcs, firsts, fixed, fourth) in sets {
        for npc in npcs {
            for page in fixed {
                let path = format!("{DIST}{dir}/{npc}-{page:02}.htm");
                assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{page:02}.htm");
            }
            for first in firsts {
                for p in *first..=(*first + 3) {
                    let path = format!("{DIST}{dir}/{npc}-{p}.htm");
                    assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{p}.htm");
                }
            }
        }
        assert!(
            std::path::Path::new(&format!("{DIST}{dir}/{fourth}")).exists(),
            "missing fourth-class page {dir}/{fourth}"
        );
    }
}

/// DarkElfChange1: a Dark Fighter with the Gaze of Abyss becomes a Palus
/// Knight. Note the bypass event is the CLASSES **row index** (0), not the
/// class id — the opposite convention to the other Change1 scripts.
#[test]
fn dark_elf_change1_transfers_by_row_index() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1244, "Gaze of Abyss", true), (8869, "Coupon", false)]);
    world.data.categories.insert_for_test("FIRST_CLASS_GROUP", &[32, 35, 39, 42]);
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 2; // Dark Elf
        p.class_id = 31; // Dark Fighter
        p.base_class_id = 31;
    }
    super::items::add_inventory_item(&mut world, 3001, 1244, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1 0")));

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 32, "row 0 is Palus Knight");
    assert_eq!(item_count(&world, 3001, 1244), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// A Dark Mage cannot take a Dark Fighter row, even though the same NPC
/// serves both — Java checks the source class per row.
#[test]
fn dark_elf_change1_rejects_the_wrong_source_class() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1244, "Gaze of Abyss", true), (8869, "Coupon", false)]);
    world.data.categories.insert_for_test("FIRST_CLASS_GROUP", &[32, 35, 39, 42]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 2;
        p.class_id = 38; // Dark MAGE asking for the fighter row
        p.base_class_id = 38;
    }
    super::items::add_inventory_item(&mut world, 3001, 1244, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1 0")));

    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 38, "unchanged");
    assert_eq!(item_count(&world, 3001, 1244), 1, "and nothing consumed");
}

/// A character standing on a subclass is refused outright — Java's
/// `if (player.isSubClassActive()) return getNoQuestMsg(player);`.
#[test]
fn dark_elf_change1_refuses_while_a_subclass_is_active() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1244, "Gaze of Abyss", true)]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 2;
        p.class_id = 31;
        p.base_class_id = 31;
        p.class_index = 1; // on a subclass
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1")));

    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("a reply");
    assert!(!html.contains("30290-01"), "the class list must not be offered on a subclass");
}

/// Every page DarkElfChange1 can return exists — and note these are `.html`,
/// not `.htm` like its siblings.
#[test]
fn dark_elf_change1_html_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/village_master/DarkElfChange1/");
    for npc in [30290, 30297, 30462] {
        for page in [1, 8, 31, 32, 33] {
            let path = format!("{DIST}{npc}-{page:02}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{page:02}.html");
        }
        for page in 15..=30 {
            let path = format!("{DIST}{npc}-{page}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{page}.html");
        }
    }
}
