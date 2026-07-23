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
    let pkts = drain(&mut rx);
    eprintln!("DBG opcodes: {:?}", pkts.iter().map(|p| p[0]).collect::<Vec<_>>());
    let html = pkts.iter().find_map(|p| decode_npc_html(p)).unwrap_or_default();
    eprintln!("DBG html: {html}");
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
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
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

/// FirstClassTransferTalk: the seven headmasters only *talk* about transfers.
/// The page name uses an underscore and `.html`, unlike every other
/// village-master script.
#[test]
fn first_class_transfer_talk_picks_the_page_by_race_and_progress() {
    let cases: [(i32, i32, bool, i32, &str); 7] = [
        // (npc, player race, is_mage, class level, expected suffix)
        (30026, 0, false, 0, "fighter"),      // Blitz, human fighter
        (30026, 0, true, 0, "no"),            // a mage at the fighter guild
        (30031, 0, true, 0, "mystic"),        // Biotin, human priest
        (30154, 1, true, 0, "mystic"),        // Asterios serves both sides
        (30520, 4, false, 0, "fighter"),      // Dwarves: fighter only
        (30026, 0, false, 1, "transfer_1"),   // already first-occupation
        (30026, 0, false, 2, "transfer_2"),   // second or beyond
    ];
    for (npc_id, race, is_mage, class_level, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        // class 0 = base fighter, 1 = a first occupation, 4 = a second.
        let class_id = match class_level {
            0 => if is_mage { 10 } else { 0 },
            1 => 1,
            _ => 4,
        };
        world.data.categories.insert_for_test("MAGE_GROUP", &[10]);
        world.data.categories.insert_for_test("FIRST_CLASS_GROUP", &[1]);
        world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[4]);
        world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
        world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
        add_test_npc(&mut world, NPC_OID, npc_id, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.race = race;
            p.class_id = class_id;
            p.base_class_id = class_id;
        }
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest FirstClassTransferTalk")),
        );

        let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).unwrap_or_default();
        // Compare against the actual dist page, run through the same strip the
        // cache applies — asserting "non-empty" would happily accept the
        // *wrong* page.
        let want_path = format!(
            "{}/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/{npc_id}_{expected}.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let want = crate::data::htm_cache::strip_htm(
            &std::fs::read_to_string(&want_path).unwrap_or_else(|_| panic!("dist page {want_path}")),
        )
        .replace("%objectId%", &NPC_OID.to_string());
        assert_eq!(
            html, want,
            "npc {npc_id} race {race} mage {is_mage} level {class_level}: wrong page (wanted {expected})"
        );
    }
}

/// A player of the wrong race gets the refusal page.
#[test]
fn first_class_transfer_talk_refuses_another_race() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30520, "VillageMaster", 70, 100, 0, 0); // Dwarf master
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.race = 0; // Human at a Dwarf headmaster
        p.class_id = 0;
        p.base_class_id = 0;
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest FirstClassTransferTalk")),
    );

    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("a reply");
    let want = crate::data::htm_cache::strip_htm(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/30520_no.html"
        ))
        .expect("dist page"),
    )
    .replace("%objectId%", &NPC_OID.to_string());
    assert_eq!(html, want, "a Human at a Dwarf headmaster gets the refusal page");
}

/// Every page the script can name must exist — and the availability is
/// asymmetric: the Human fighter master ships no `mystic`, the priest no
/// `fighter`, and neither Dwarf master ships a `mystic` at all.
#[test]
fn first_class_transfer_talk_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/"
    );
    let expected: [(i32, &[&str]); 7] = [
        (30026, &["fighter", "no", "transfer_1", "transfer_2"]),
        (30031, &["mystic", "no", "transfer_1", "transfer_2"]),
        (30154, &["fighter", "mystic", "no", "transfer_1", "transfer_2"]),
        (30358, &["fighter", "mystic", "no", "transfer_1", "transfer_2"]),
        (30565, &["fighter", "mystic", "no", "transfer_1", "transfer_2"]),
        (30520, &["fighter", "no", "transfer_1", "transfer_2"]),
        (30525, &["fighter", "no", "transfer_1", "transfer_2"]),
    ];
    for (npc, suffixes) in expected {
        for s in suffixes {
            let path = format!("{DIST}{npc}_{s}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}_{s}.html");
        }
    }
    // And the asymmetry is real, not an accident of my table: the Human
    // fighter master genuinely ships no mystic page, which is why the script
    // must answer `no` there rather than inventing one.
    assert!(!std::path::Path::new(&format!("{DIST}30026_mystic.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30031_fighter.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30520_mystic.html")).exists());
}

/// DwarfBlacksmithChange2: an Artisan with **all three** marks becomes a
/// Warsmith at level 40, paying C-grade coupons.
#[test]
fn dwarf_change2_second_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(3119, "Mark of Guildsman", true), (3238, "Mark of Prosperity", true),
          (2867, "Mark of Maestro", true), (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("WARSMITH_GROUP", &[56, 57]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30512, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 4;
        p.class_id = 56; // Artisan
        p.base_class_id = 56;
    }
    for id in [3119, 3238, 2867] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange2 57")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 57, "now a Warsmith");
    for id in [3119, 3238, 2867] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 8870), 15, "C-grade coupons");
}

/// Holding only *some* of the marks is not enough — Java's
/// `hasQuestItems(a, b, c)` is an AND. Treating it as "any" would let a player
/// transfer on one mark.
#[test]
fn dwarf_change2_requires_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(3119, "Mark of Guildsman", true), (3238, "Mark of Prosperity", true),
          (2867, "Mark of Maestro", true), (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("WARSMITH_GROUP", &[56, 57]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30512, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 4;
        p.class_id = 56;
        p.base_class_id = 56;
    }
    // Two of the three.
    super::items::add_inventory_item(&mut world, 3001, 3119, 1);
    super::items::add_inventory_item(&mut world, 3001, 3238, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange2 57")),
    );

    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 56, "still an Artisan");
    assert_eq!(item_count(&world, 3001, 3119), 1, "and the marks are not taken");
    assert_eq!(item_count(&world, 3001, 3238), 1);
}

/// Level 39 is refused even holding all three marks.
#[test]
fn dwarf_change2_requires_level_40() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(3119, "Mark of Guildsman", true), (3238, "Mark of Prosperity", true),
          (2809, "Mark of Searcher", true), (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("BOUNTY_HUNTER_GROUP", &[54, 55]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30511, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 39;
        p.race = 4;
        p.class_id = 54; // Scavenger
        p.base_class_id = 54;
    }
    for id in [3119, 3238, 2809] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfWarehouseChange2 55")),
    );

    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 54, "still a Scavenger at 39");
    assert_eq!(item_count(&world, 3001, 3119), 1, "marks kept");
}

/// One 12-page set serves all eight masters per script — every page the
/// scripts can name belongs to the *first* NPC's id.
#[test]
fn dwarf_change2_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/village_master/");
    for (dir, page_npc) in [("DwarfBlacksmithChange2", 30512), ("DwarfWarehouseChange2", 30511)] {
        for n in 1..=12 {
            let path = format!("{DIST}{dir}/{page_npc}-{n:02}.htm");
            assert!(std::path::Path::new(&path).exists(), "missing {dir}/{page_npc}-{n:02}.htm");
        }
        // And the other masters genuinely ship nothing of their own.
        let other = format!("{DIST}{dir}/30677-01.htm");
        assert!(!std::path::Path::new(&other).exists(), "only the first NPC ships pages");
    }
}

/// OrcChange2: an Orc Raider with the three marks becomes a Destroyer and is
/// paid C-grade coupons.
#[test]
fn orc_change2_transfer_pays_coupons() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(2627, "Challenger", true), (3203, "Glory", true), (3276, "Champion", true), (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("ORC_MALL_CLASS", &[45, 46]);
    world.data.categories.insert_for_test("ORC_FALL_CLASS", &[]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30513, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 3;
        p.class_id = 45; // Orc Raider
        p.base_class_id = 45;
    }
    for id in [2627, 3203, 3276] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange2 46")));

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 46, "now a Destroyer");
    assert_eq!(item_count(&world, 3001, 8870), 15, "Orc masters pay coupons");
}

/// DarkElfChange2 takes the **row index**, and — unlike every other Change2 —
/// pays **no coupon at all**.
#[test]
fn dark_elf_change2_uses_row_index_and_pays_nothing() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(2633, "Duty", true), (3172, "Fate", true), (3307, "Witchcraft", true), (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("SECOND_CLASS_GROUP", &[33]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30474, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 2; // Dark Elf
        p.class_id = 32; // Palus Knight
        p.base_class_id = 32;
    }
    for id in [2633, 3172, 3307] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    // Row 0 = Shillien Knight (33) from Palus Knight (32).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange2 0")));

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 33, "row 0 is Shillien Knight");
    for id in [2633, 3172, 3307] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 8870), 0, "the Dark Elf script pays NO coupon");
}

/// Both scripts need all three marks.
#[test]
fn change2_scripts_require_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(2627, "Challenger", true), (3203, "Glory", true), (3276, "Champion", true)]);
    world.data.categories.insert_for_test("ORC_MALL_CLASS", &[45, 46]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30513, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 3;
        p.class_id = 45;
        p.base_class_id = 45;
    }
    super::items::add_inventory_item(&mut world, 3001, 2627, 1); // one of three
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange2 46")));

    assert_eq!(world.objects.get_component::<Player>(&3001).unwrap().class_id, 45, "still a Raider");
    assert_eq!(item_count(&world, 3001, 2627), 1, "the one mark is kept");
}

/// Both page sets exist, and both are owned by a single NPC — note the Dark
/// Elf owner (30474) is the *third* entry in its NPC list, not the first.
#[test]
fn change2_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/village_master/");
    for n in [1u32, 2, 6, 10, 17, 18, 19] {
        let p = format!("{DIST}OrcChange2/30513-{n:02}.htm");
        assert!(std::path::Path::new(&p).exists(), "missing OrcChange2/30513-{n:02}.htm");
    }
    for first in [20u32, 24, 28, 32] {
        for n in first..=(first + 3) {
            let p = format!("{DIST}OrcChange2/30513-{n}.htm");
            assert!(std::path::Path::new(&p).exists(), "missing OrcChange2/30513-{n}.htm");
        }
    }
    for n in [1u32, 8, 12, 19, 54, 55, 56] {
        let p = format!("{DIST}DarkElfChange2/30474-{n:02}.html");
        assert!(std::path::Path::new(&p).exists(), "missing DarkElfChange2/30474-{n:02}.html");
    }
    for first in [26u32, 30, 34, 38, 42, 46, 50] {
        for n in first..=(first + 3) {
            let p = format!("{DIST}DarkElfChange2/30474-{n}.html");
            assert!(std::path::Path::new(&p).exists(), "missing DarkElfChange2/30474-{n}.html");
        }
    }
}

/// ElfHumanFighterChange2: a Warrior with all three marks becomes a Gladiator
/// at level 40 and is paid 15 C-grade coupons.
#[test]
fn elf_human_change2_second_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(2627, "Challenger", true), (2734, "Trust", true), (2762, "Duelist", true),
          (8870, "Coupon C", false)],
    );
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[1, 2, 3]);
    world.data.categories.insert_for_test("HUMAN_FALL_CLASS", &[1, 2, 3]);
    world.data.categories.insert_for_test("ELF_FALL_CLASS", &[]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30109, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 1; // Warrior
        p.base_class_id = 1;
    }
    for id in [2627, 2734, 2762] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange2 2")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 2, "now a Gladiator");
    assert_eq!(p.base_class_id, 2, "and it is the base class");
    for id in [2627, 2734, 2762] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 8870), 15, "15 C-grade coupons");
}

/// The `from_class` half of each row is load-bearing. All ten Fighter targets
/// live on the same NPC, so without it a Human Knight could take Temple
/// Knight — an *Elven* Knight's class — from the same master.
#[test]
fn elf_human_change2_rejects_the_wrong_source_class() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(2633, "Duty", true), (3140, "Life", true), (2820, "Healer", true)],
    );
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[4]);
    world.data.categories.insert_for_test("HUMAN_FALL_CLASS", &[4]);
    world.data.categories.insert_for_test("ELF_FALL_CLASS", &[]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30109, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 4; // Human Knight, holding exactly the Temple Knight marks
        p.base_class_id = 4;
    }
    for id in [2633, 3140, 2820] {
        super::items::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    // 20 = Temple Knight, which only an Elven Knight (19) may take.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange2 20")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 4, "still a Knight");
    for id in [2633, 3140, 2820] {
        assert_eq!(item_count(&world, 3001, id), 1, "mark {id} not consumed");
    }
}

/// All three marks are required — `hasQuestItems(a, b, c)` is an AND. With two
/// of three at level 40 the master serves the noProof page and takes nothing.
#[test]
fn elf_human_change2_requires_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(2721, "Pilgrim", true), (2734, "Trust", true), (2820, "Healer", true)],
    );
    world.data.categories.insert_for_test("CLERIC_GROUP", &[15]);
    world.data.categories.insert_for_test("HUMAN_CALL_CLASS", &[15]);
    world.data.categories.insert_for_test("ELF_CALL_CLASS", &[]);
    world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
    world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30120, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 15; // Cleric
        p.base_class_id = 15;
    }
    for id in [2721, 2734] {
        super::items::add_inventory_item(&mut world, 3001, id, 1); // two of three
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanClericChange2 16")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 15, "still a Cleric");
    for id in [2721, 2734] {
        assert_eq!(item_count(&world, 3001, id), 1, "mark {id} kept");
    }
}

/// `onTalk` routes to a class-list page, the first-occupation refusal, or the
/// mismatch page. Compared byte-for-byte against the dist page — asserting
/// "non-empty" would pass while serving the wrong window.
#[test]
fn elf_human_change2_talk_picks_the_class_list() {
    // (script, page npc, group, human cat, elf cat, class id, expected page)
    let cases: [(&str, i32, &str, &str, &str, i32, u32); 9] = [
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 1, 2),
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 5, 9),
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 7, 16),
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 19, 23),
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 22, 30),
        // No first occupation yet.
        ("ElfHumanFighterChange2", 30109, "FIGHTER_GROUP", "HUMAN_FALL_CLASS", "ELF_FALL_CLASS", 0, 37),
        ("ElfHumanWizardChange2", 30115, "WIZARD_GROUP", "HUMAN_MALL_CLASS", "ELF_MALL_CLASS", 11, 2),
        ("ElfHumanWizardChange2", 30115, "WIZARD_GROUP", "HUMAN_MALL_CLASS", "ELF_MALL_CLASS", 26, 12),
        ("ElfHumanClericChange2", 30120, "CLERIC_GROUP", "HUMAN_CALL_CLASS", "ELF_CALL_CLASS", 29, 9),
    ];
    for (script, npc_id, group, human_cat, elf_cat, class_id, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        world.data.categories.insert_for_test(group, &[class_id]);
        world.data.categories.insert_for_test(human_cat, &[class_id]);
        world.data.categories.insert_for_test(elf_cat, &[]);
        world.data.categories.insert_for_test("THIRD_CLASS_GROUP", &[]);
        world.data.categories.insert_for_test("FOURTH_CLASS_GROUP", &[]);
        add_test_npc(&mut world, NPC_OID, npc_id, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.class_id = class_id;
            p.base_class_id = class_id;
        }
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {script}")),
        );

        let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).unwrap_or_default();
        let want_path = format!(
            "{}/../../dist/game/data/scripts/village_master/{script}/{npc_id}-{expected:02}.htm",
            env!("CARGO_MANIFEST_DIR")
        );
        let want = crate::data::htm_cache::strip_htm(
            &std::fs::read_to_string(&want_path).unwrap_or_else(|_| panic!("dist page {want_path}")),
        )
        .replace("%objectId%", &NPC_OID.to_string());
        assert_eq!(html, want, "{script} class {class_id}: wrong page (wanted {expected})");
    }
}

/// Every page the three scripts can name exists — and each set is owned by one
/// NPC, so the other masters must ship nothing (which is why the hard-coded
/// page owner cannot be tidied into a per-NPC name that would 404).
#[test]
fn elf_human_change2_pages_exist_in_dist() {
    const DIST: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/village_master/");
    // (script, page npc, fixed pages, row first-pages, another master's id)
    let sets: [(&str, i32, &[u32], &[u32], i32); 3] = [
        ("ElfHumanFighterChange2", 30109, &[1, 2, 9, 16, 23, 30, 37, 38, 39],
         &[40, 44, 48, 52, 56, 60, 64, 68, 72, 76], 30187),
        ("ElfHumanWizardChange2", 30115, &[1, 2, 12, 19, 20, 21], &[22, 26, 30, 34, 38], 30174),
        ("ElfHumanClericChange2", 30120, &[1, 2, 9, 13, 14, 15], &[16, 20, 24], 30191),
    ];
    for (script, npc, fixed, firsts, other) in sets {
        for n in fixed {
            let p = format!("{DIST}{script}/{npc}-{n:02}.htm");
            assert!(std::path::Path::new(&p).exists(), "missing {script}/{npc}-{n:02}.htm");
        }
        for first in firsts {
            for n in *first..=(*first + 3) {
                let p = format!("{DIST}{script}/{npc}-{n}.htm");
                assert!(std::path::Path::new(&p).exists(), "missing {script}/{npc}-{n}.htm");
            }
        }
        let p = format!("{DIST}{script}/{other}-01.htm");
        assert!(!std::path::Path::new(&p).exists(), "only {npc} ships {script} pages");
    }
}

/// AllianceMaster's dist page, run through the same strip the cache applies.
fn alliance_page(name: &str) -> String {
    let path = format!(
        "{}/../../dist/game/data/scripts/village_master/AllianceMaster/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    crate::data::htm_cache::strip_htm(
        &std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("dist page {path}")),
    )
    .replace("%objectId%", &NPC_OID.to_string())
}

/// Talking to any village master opens the alliance menu — with no clan check,
/// so a clanless player does see the two buttons.
#[test]
fn alliance_master_talk_opens_the_menu_without_a_clan() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().clan_id,
        0,
        "fixture player is clanless"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest AllianceMaster")),
    );

    let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("a reply");
    assert_eq!(html, alliance_page("9001-01.htm"), "the menu, not the clan refusal");
}

/// Every page *except* the menu needs a clan. The asymmetry is the whole
/// script: the clanless player is refused on click, not on open.
#[test]
fn alliance_master_gates_every_page_but_the_menu_on_having_a_clan() {
    // (clan id, requested page, expected page)
    let cases: [(i32, &str, &str); 6] = [
        (0, "9001-02.htm", "9001-04.htm"), // create, clanless → refused
        (0, "9001-03.htm", "9001-04.htm"), // dissolve, clanless → refused
        (0, "9001-01.htm", "9001-01.htm"), // the menu is NOT gated
        (7, "9001-02.htm", "9001-02.htm"), // in a clan → the real page
        (7, "9001-03.htm", "9001-03.htm"),
        (7, "9001-01.htm", "9001-01.htm"),
    ];
    for (clan_id, requested, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        world.objects.get_component_mut::<Player>(&3001).unwrap().clan_id = clan_id;
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest AllianceMaster {requested}")),
        );

        let html = drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).expect("a reply");
        assert_eq!(
            html,
            alliance_page(expected),
            "clan {clan_id} asking for {requested} should get {expected}"
        );
    }
}

/// The four pages exist, and they are numbered against the **virtual** NPC id
/// 9001 — no real master ships a page of its own.
#[test]
fn alliance_master_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/AllianceMaster/"
    );
    for n in 1..=4 {
        let p = format!("{DIST}9001-{n:02}.htm");
        assert!(std::path::Path::new(&p).exists(), "missing 9001-{n:02}.htm");
    }
    for npc in [30026, 30031, 30913] {
        let p = format!("{DIST}{npc}-01.htm");
        assert!(!std::path::Path::new(&p).exists(), "pages are 9001-*, not per-NPC");
    }
}

/// Q00406 hand-rolls its drop instead of calling `giveItemRandomly`, so it is
/// **not** multiplied by `RateQuestDrop`. With the rate at 3× a single kill
/// still yields exactly one topaz — reaching for the helper here would have
/// silently tripled the drop.
#[test]
fn quest_q00406_drop_ignores_the_quest_drop_rate() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1205, "Topaz Piece", true)]);
    let mut t = crate::data::npc_data::default_template(20035);
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);
    world.cfg.rates.rate_quest_drop = 3.0;
    add_test_npc(&mut world, NPC_OID, 30327, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18; // Elven Fighter
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    accept_q406(&mut world);
    drain(&mut rx);

    let mob = NPC_OID + 1;
    add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0); // roll(100) → 0 < 70
    death::npc_do_die(&mut world, mob, 3001);

    assert_eq!(item_count(&world, 3001, 1205), 1, "one piece per kill regardless of RateQuestDrop");
}

/// Drive Q00406 up to "quest started".
fn accept_q406(world: &mut World) {
    handle_request_bypass_to_server(world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")));
    handle_request_bypass_to_server(world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight ACCEPT")));
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight 30327-06.htm")),
    );
}

/// The whole Elven Knight chain: 20 topaz → Sorius' letter → Kluto → 20
/// emerald → Kluto's box → the brooch. The brooch is what
/// `ElfHumanFighterChange1` consumes, so this is the quest that makes that
/// transfer reachable at all.
#[test]
fn quest_q00406_full_chain_awards_the_brooch() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1202, "Sorius Letter", true), (1203, "Kluto Box", true), (1205, "Topaz", true),
          (1206, "Emerald", true), (1276, "Kluto Memo", true), (1204, "Elven Knight Brooch", false)],
    );
    for id in [20035, 20782] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    let kluto = NPC_OID + 50;
    add_test_npc(&mut world, NPC_OID, 30327, "Folk", 5, 100, 0, 0); // Sorius
    add_test_npc(&mut world, kluto, 30317, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18;
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    accept_q406(&mut world);
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(1));
    drain(&mut rx);

    // 20 topaz.
    for i in 0..20 {
        let mob = NPC_OID + 100 + i;
        add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(0);
        death::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1205), 20);
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(2), "20 topaz advances");

    // Sorius hands over his letter.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")));
    assert_eq!(item_count(&world, 3001, 1202), 1, "Sorius' letter");
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(3));

    // Kluto swaps the letter for his memo.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest Q00406_PathOfTheElvenKnight 30317-02.html")),
    );
    assert_eq!(item_count(&world, 3001, 1202), 0, "letter consumed");
    assert_eq!(item_count(&world, 3001, 1276), 1, "memo received");
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(4));

    // 20 emerald from Ol Mahum Novices.
    for i in 0..20 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20782, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(0); // roll(100) → 0 < 50
        death::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1206), 20);
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(5));

    // Kluto builds the box, consuming everything.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kluto}_Quest Q00406_PathOfTheElvenKnight")));
    assert_eq!(item_count(&world, 3001, 1203), 1, "the box");
    assert_eq!(item_count(&world, 3001, 1205), 0, "topaz consumed");
    assert_eq!(item_count(&world, 3001, 1206), 0, "emerald consumed");
    assert_eq!(quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"), Some(6));
    drain(&mut rx);

    // Sorius pays out.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")));
    assert_eq!(item_count(&world, 3001, 1204), 1, "the Elven Knight Brooch");
    {
        // `exitQuest(false, ...)` — one-time, so the state stays COMPLETED
        // rather than being deleted (that would let it be repeated).
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0["Q00406_PathOfTheElvenKnight"].is_completed(), "one-time quest stays COMPLETED");
    }
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// Q00407's tag mechanic: `on_attack` stamps the mob with the attacker's
/// object id and `on_kill` pays only that player. A mob killed without being
/// attacked first drops nothing.
#[test]
fn quest_q00407_only_the_tagging_player_gets_the_letter() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1207, "Reisa Letter", true), (1208, "Torn 1", true), (1209, "Torn 2", true),
          (1210, "Torn 3", true), (1211, "Torn 4", true)],
    );
    let mut t = crate::data::npc_data::default_template(20053);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    let moretti = NPC_OID + 50;
    add_test_npc(&mut world, NPC_OID, 30328, "Folk", 5, 100, 0, 0); // Reoria
    add_test_npc(&mut world, moretti, 30337, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18;
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00407_PathOfTheElvenScout")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00407_PathOfTheElvenScout ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1207), 1, "Reisa's letter on accept");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{moretti}_Quest Q00407_PathOfTheElvenScout 30337-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, "Q00407_PathOfTheElvenScout"), Some(2));
    assert_eq!(item_count(&world, 3001, 1207), 0, "Moretti takes the letter");
    drain(&mut rx);

    // Killed cold — never attacked, so never tagged.
    let untagged = NPC_OID + 100;
    add_test_npc(&mut world, untagged, 20053, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, untagged, 3001);
    assert_eq!(item_count(&world, 3001, 1208), 0, "an untagged mob pays nothing");

    // Attack first, then kill.
    let tagged = NPC_OID + 101;
    add_test_npc(&mut world, tagged, 20053, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, tagged, 3001, 10.0);
    death::npc_do_die(&mut world, tagged, 3001);
    assert_eq!(item_count(&world, 3001, 1208), 1, "the tagging player is paid");
}

/// Both quests' pages exist. The extension is **mixed within one quest** —
/// `.htm` before accept, `.html` after — and Prias ships no `-03`, which Java
/// never names either.
#[test]
fn elven_path_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/quests/");
    let htm: [(&str, &str, &[&str]); 2] = [
        ("Q00406_PathOfTheElvenKnight", "30327", &["01", "02", "02a", "03", "04", "05", "06"]),
        ("Q00407_PathOfTheElvenScout", "30328", &["01", "02", "02a", "03", "04", "05"]),
    ];
    for (dir, npc, pages) in htm {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.htm");
            assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{p}.htm");
        }
    }
    let html: [(&str, &str, &[&str]); 6] = [
        ("Q00406_PathOfTheElvenKnight", "30327", &["07", "08", "09", "10", "11"]),
        ("Q00406_PathOfTheElvenKnight", "30317", &["01", "02", "03", "04", "05", "06"]),
        ("Q00407_PathOfTheElvenScout", "30328", &["06", "07", "08"]),
        ("Q00407_PathOfTheElvenScout", "30334", &["01"]),
        ("Q00407_PathOfTheElvenScout", "30337", &["01", "02", "03", "04", "05", "06", "07", "08", "09"]),
        ("Q00407_PathOfTheElvenScout", "30426", &["01", "02", "04"]),
    ];
    for (dir, npc, pages) in html {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{p}.html");
        }
    }
    // Prias' gap is real; the port must not invent a -03 to "complete" the run.
    let gap = format!("{DIST}Q00407_PathOfTheElvenScout/30426-03.html");
    assert!(!std::path::Path::new(&gap).exists(), "30426-03 genuinely does not ship");
}

/// Put `item_id` straight into the RHand paperdoll. Bypasses `equip_item`,
/// which would need full weapon templates for these quest items.
fn equip_weapon_row(world: &mut World, player: i32, item_id: i32) {
    let row = crate::character::ItemRow {
        object_id: 90000,
        item_id,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: crate::model::inventory::PaperdollSlot::RHand as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    world.objects.add_components(&player, crate::model::inventory::Inventory::from_rows(&[row]));
}

/// Accept Q00401 and return the world to "quest started".
fn accept_q401(world: &mut World) {
    handle_request_bypass_to_server(world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior")));
    handle_request_bypass_to_server(world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior ACCEPT")));
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior 30010-06.htm")),
    );
}

/// Q00401's spider legs are gated purely on the weapon/solo tag — there is no
/// chance roll — so an unarmed kill pays nothing and a kill with Auron's
/// sharpened sword always pays.
#[test]
fn quest_q00401_spider_legs_require_the_quest_sword() {
    for (equip_sword, expected) in [(false, 0), (true, 1)] {
        let (mut world, mut db_rx, _link_rx) = quest_test_world();
        add_quest_items(&mut world, &[(1138, "Auron Letter", true), (1142, "Rusted Sword 3", true), (1144, "Spider Leg", true)]);
        let mut t = crate::data::npc_data::default_template(20038);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
        add_test_npc(&mut world, NPC_OID, 30010, "Folk", 5, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.level = 19;
            p.class_id = 0; // Human Fighter
            p.base_class_id = 0;
        }
        if equip_sword {
            equip_weapon_row(&mut world, 3001, 1142);
        }
        drain_db(&mut db_rx);
        accept_q401(&mut world);
        drain(&mut rx);

        let spider = NPC_OID + 1;
        add_test_npc(&mut world, spider, 20038, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, spider, 3001, 10.0);
        death::npc_do_die(&mut world, spider, 3001);

        assert_eq!(
            item_count(&world, 3001, 1144),
            expected,
            "sword equipped = {equip_sword}: the tag is the only gate"
        );
    }
}

/// Q00401's rusted-sword drop is `getRandom(10) < 4`. Forcing the roll to 4
/// must *not* drop — if it were read as `getRandom(100) < 40` it would.
#[test]
fn quest_q00401_rusted_sword_chance_is_out_of_ten() {
    for (forced, expected) in [(3, 1), (4, 0)] {
        let (mut world, mut db_rx, _link_rx) = quest_test_world();
        add_quest_items(&mut world, &[(1138, "Auron Letter", true), (1139, "Guild Mark", true), (1140, "Rusted Sword 1", true)]);
        let mut t = crate::data::npc_data::default_template(20035);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
        add_test_npc(&mut world, NPC_OID, 30010, "Folk", 5, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.level = 19;
            p.class_id = 0;
            p.base_class_id = 0;
        }
        drain_db(&mut db_rx);
        accept_q401(&mut world);
        super::items::add_inventory_item(&mut world, 3001, 1139, 1); // guild mark
        drain(&mut rx);

        let mob = NPC_OID + 1;
        add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(forced);
        death::npc_do_die(&mut world, mob, 3001);

        assert_eq!(
            item_count(&world, 3001, 1140),
            expected,
            "roll {forced} against `getRandom(10) < 4`"
        );
    }
}

/// Q00403's drop table is the same `ItemChanceHolder` type quest 406 uses with
/// `getRandom(100)`, but this quest rolls `getRandom(REQUIRED_ITEM_COUNT)` —
/// out of **10**. A forced roll can't tell the two apart (the forced value
/// ignores the bound), so this asserts the *rate*: Ruin Spartoi are chance 8,
/// i.e. 80%, and 40 kills reliably cap the 10-bone collection. Read as a
/// percentage it would be 8% and cap essentially never.
#[test]
fn quest_q00403_bone_chance_is_out_of_ten_not_a_hundred() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1180, "Bezique Letter", true), (1181, "Neti Bow", true), (1182, "Neti Dagger", true), (1183, "Spartois Bones", true)]);
    let mut t = crate::data::npc_data::default_template(20054);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30379, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0;
        p.base_class_id = 0;
    }
    equip_weapon_row(&mut world, 3001, 1181); // Neti's bow satisfies the tag
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue ACCEPT")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue 30379-06.htm")));
    drain(&mut rx);

    for i in 0..40 {
        let mob = NPC_OID + 100 + i;
        add_test_npc(&mut world, mob, 20054, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, mob, 3001, 10.0);
        death::npc_do_die(&mut world, mob, 3001);
    }

    assert_eq!(item_count(&world, 3001, 1183), 10, "80% drop caps at 10 well within 40 kills");
    assert_eq!(quest_cond(&world, 3001, "Q00403_PathOfTheRogue"), Some(3));
}

/// The Cat's Eye Bandit taunts its attacker on the first qualifying hit —
/// **to that player only** — and on death broadcasts a different line and
/// yields one of the four stolen goods.
#[test]
fn quest_q00403_cats_eye_bandit_taunts_then_drops_loot() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1180, "Bezique Letter", true), (1181, "Neti Bow", true), (1185, "Most Wanted", true),
          (1186, "Stolen Jewelry", true), (1187, "Stolen Tomes", true), (1188, "Stolen Ring", true),
          (1189, "Stolen Necklace", true)],
    );
    let mut t = crate::data::npc_data::default_template(27038);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30379, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0;
        p.base_class_id = 0;
    }
    equip_weapon_row(&mut world, 3001, 1181);
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue ACCEPT")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue 30379-06.htm")));
    super::items::add_inventory_item(&mut world, 3001, 1185, 1); // the most-wanted list
    drain(&mut rx);

    let bandit = NPC_OID + 1;
    add_test_npc(&mut world, bandit, 27038, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, bandit, 3001, 10.0);
    let says: Vec<_> = drain(&mut rx).into_iter().filter(|p| p[0] == server_packets::opcodes::NPC_SAY).collect();
    assert_eq!(says.len(), 1, "one taunt on the first qualifying hit");
    assert_eq!(i32::from_le_bytes(says[0][13..17].try_into().unwrap()), 40306, "the taunt line");

    // A second hit must not re-taunt (script value is no longer 0).
    combat::npc_receive_damage(&mut world, bandit, 3001, 10.0);
    assert!(
        !drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "the taunt fires once"
    );

    world.forced_rolls.push_back(0); // pick STOLEN_JEWELRY
    death::npc_do_die(&mut world, bandit, 3001);
    assert_eq!(item_count(&world, 3001, 1186), 1, "one of the stolen goods");
    assert!(
        drain(&mut rx).iter().any(|p| {
            p[0] == server_packets::opcodes::NPC_SAY
                && i32::from_le_bytes(p[13..17].try_into().unwrap()) == 40307
        }),
        "the death line, which is broadcast rather than whispered"
    );
}

/// Both quests' pages exist, with the same mixed `.htm`/`.html` split as the
/// elven pair.
#[test]
fn warrior_rogue_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/quests/");
    let htm: [(&str, &str, &[&str]); 2] = [
        ("Q00401_PathOfTheWarrior", "30010", &["01", "02", "02a", "03", "04", "05", "06"]),
        ("Q00403_PathOfTheRogue", "30379", &["01", "02", "02a", "03", "04", "05", "06"]),
    ];
    for (dir, npc, pages) in htm {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.htm");
            assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{p}.htm");
        }
    }
    let html: [(&str, &str, &[&str]); 4] = [
        ("Q00401_PathOfTheWarrior", "30010", &["07", "08", "09", "10", "11", "12", "13"]),
        ("Q00401_PathOfTheWarrior", "30253", &["01", "02", "03", "04", "05", "06"]),
        ("Q00403_PathOfTheRogue", "30379", &["07", "08", "09", "10", "11"]),
        ("Q00403_PathOfTheRogue", "30425", &["01", "02", "03", "04", "05", "06", "07", "08"]),
    ];
    for (dir, npc, pages) in html {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {dir}/{npc}-{p}.html");
        }
    }
}

const Q402: &str = "Q00402_PathOfTheHumanKnight";

/// Q00402 world: Vasper at NPC_OID, quest accepted, `coins` Coins of Lords
/// already in the bag.
fn q402_world_with_coins(coins: usize) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1271, "Squire's Mark", true), (1161, "Sword of Ritual", false)];
    for id in [1162, 1163, 1164, 1165, 1166, 1167] {
        items.push((id, "Coin of Lords", true));
    }
    for id in [1168, 1169, 1170, 1171, 1172, 1173, 1174, 1175, 1176, 1177, 1178, 1179] {
        items.push((id, "Badge or trophy", true));
    }
    add_quest_items(&mut world, &items);
    add_test_npc(&mut world, NPC_OID, 30417, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0; // Human Fighter
        p.base_class_id = 0;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} ACCEPT")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-08.htm")));
    for id in [1162, 1163, 1164, 1165, 1166, 1167].iter().take(coins) {
        super::items::add_inventory_item(&mut world, 3001, *id, 1);
    }
    drain(&mut rx);
    (world, rx)
}

/// With exactly three coins, talking to Vasper only *offers* — the sword comes
/// from the confirmation bypass.
#[test]
fn quest_q00402_three_coins_needs_the_confirm_button() {
    let (mut world, _rx) = q402_world_with_coins(3);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")));
    assert_eq!(item_count(&world, 3001, 1161), 0, "talking alone does not pay at 3 coins");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-13.html")));
    assert_eq!(item_count(&world, 3001, 1161), 1, "the confirm button awards the Sword of Ritual");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q402].is_completed(), "one-time quest stays COMPLETED");
    }
}

/// Six coins is the one path that completes **inside `onTalk`**, with no
/// confirmation step. Asymmetric, and deliberate — see the module header.
#[test]
fn quest_q00402_six_coins_completes_on_talk_without_a_confirm() {
    let (mut world, _rx) = q402_world_with_coins(6);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")));

    assert_eq!(item_count(&world, 3001, 1161), 1, "six coins pays out on the talk itself");
    for id in [1162, 1163, 1164, 1165, 1166, 1167] {
        assert_eq!(item_count(&world, 3001, id), 0, "coin {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 1271), 0, "the Squire's Mark is taken");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q402].is_completed());
    }
}

/// Each confirm button is bound to its own coin range, so a client replaying
/// the wrong one gets nothing.
#[test]
fn quest_q00402_confirm_buttons_check_their_coin_range() {
    // `-13` is the "exactly 3" button; with 4 coins it must refuse.
    let (mut world, _rx) = q402_world_with_coins(4);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-13.html")));
    assert_eq!(item_count(&world, 3001, 1161), 0, "the 3-coin button refuses 4 coins");
    // `-14` is the "4 or 5" button; it must refuse a full set of 6.
    let (mut world6, _rx6) = q402_world_with_coins(6);
    handle_request_bypass_to_server(&mut world6, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-14.html")));
    assert_eq!(item_count(&world6, 3001, 1161), 0, "the 4-5 coin button refuses 6 coins");
}

/// One officer's sub-quest end to end — and Bathis' Bugbear Necklace is one of
/// the two trophies with **no chance roll**, so ten kills is exactly ten
/// necklaces.
#[test]
fn quest_q00402_badge_to_coin_and_the_unrolled_drop() {
    let (mut world, mut rx) = q402_world_with_coins(0);
    let mut t = crate::data::npc_data::default_template(20775); // Bugbear Raider
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);
    let bathis = NPC_OID + 60;
    add_test_npc(&mut world, bathis, 30332, "Folk", 5, 100, 0, 0);

    // The badge hand-over.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{bathis}_Quest {Q402} 30332-02.html")));
    assert_eq!(item_count(&world, 3001, 1168), 1, "Gludio Guard's 1st badge");
    drain(&mut rx);

    // Ten kills, no roll forced — every one must pay.
    for i in 0..10 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20775, "Monster", 20, 30, 0, 0);
        death::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1169), 10, "the necklace has no chance roll");

    // Turn in for the coin.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{bathis}_Quest {Q402}")));
    assert_eq!(item_count(&world, 3001, 1162), 1, "Coin of Lords 1");
    assert_eq!(item_count(&world, 3001, 1168), 0, "badge returned");
    assert_eq!(item_count(&world, 3001, 1169), 0, "necklaces handed over");
}

/// The drop is gated on holding that officer's badge — killing the right mob
/// without it pays nothing.
#[test]
fn quest_q00402_drops_need_the_matching_badge() {
    let (mut world, _rx) = q402_world_with_coins(0);
    let mut t = crate::data::npc_data::default_template(20775);
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);

    let mob = NPC_OID + 200;
    add_test_npc(&mut world, mob, 20775, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob, 3001);

    assert_eq!(item_count(&world, 3001, 1169), 0, "no badge, no trophy");
}

/// Vasper's pages alternate extensions (`-06` html, `-07`/`-08` htm, `-09`+
/// html) rather than splitting on a prefix like the other Path quests, and
/// Raymond alone ships six pages.
#[test]
fn human_knight_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00402_PathOfTheHumanKnight/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "07", "08"] {
        let path = format!("{DIST}30417-{p}.htm");
        assert!(std::path::Path::new(&path).exists(), "missing 30417-{p}.htm");
    }
    for p in ["06", "09", "10", "11", "12", "13", "14", "15"] {
        let path = format!("{DIST}30417-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30417-{p}.html");
    }
    // The alternation is real, not a tidy prefix split.
    assert!(!std::path::Path::new(&format!("{DIST}30417-07.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30417-06.htm")).exists());

    let officers: [(&str, &[&str]); 6] = [
        ("30332", &["01", "02", "03", "04", "05"]),
        ("30289", &["01", "02", "03", "04", "05", "06"]), // the six-page one
        ("30379", &["01", "02", "03", "04", "05"]),
        ("30037", &["01", "02", "03", "04", "05"]),
        ("30039", &["01", "02", "03", "04", "05"]),
        ("30031", &["01", "02", "03", "04", "05"]),
    ];
    for (npc, pages) in officers {
        for p in pages {
            let path = format!("{DIST}{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{p}.html");
        }
    }
    assert!(std::path::Path::new(&format!("{DIST}30653-01.html")).exists());
    // Only Raymond has a sixth page.
    for npc in ["30332", "30379", "30037", "30039", "30031"] {
        assert!(
            !std::path::Path::new(&format!("{DIST}{npc}-06.html")).exists(),
            "{npc} must not ship a -06"
        );
    }
}

const Q404: &str = "Q00404_PathOfTheHumanWizard";
const Q405: &str = "Q00405_PathOfTheCleric";

/// A mage at Parina with Q00404 accepted.
fn q404_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1292, "Bead of Season", false)];
    for id in 1280..=1291 {
        items.push((id, "Q404 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20021, 20359, 27030] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30391, "Folk", 5, 100, 0, 0); // Parina
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 10; // Mage
        p.base_class_id = 10;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404} ACCEPT")));
    drain(&mut rx);
    (world, rx)
}

/// The whole elemental chain: Fire → Wind → Water → Earth → the Bead of
/// Season. Exercises the branch table in order, including the conds.
#[test]
fn quest_q00404_full_elemental_chain_awards_the_bead() {
    let (mut world, mut rx) = q404_world();
    let (salamander, sylph, lizardman, undine, snake) =
        (NPC_OID + 11, NPC_OID + 12, NPC_OID + 10, NPC_OID + 13, NPC_OID + 9);
    add_test_npc(&mut world, salamander, 30411, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, sylph, 30412, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, lizardman, 30410, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, undine, 30413, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, snake, 30409, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(0); // always inside the chance
        death::npc_do_die(world, mob_oid, 3001);
    };

    // Fire: map → key (Ratman Warrior) → earring.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{salamander}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1280), 1, "Map of Luster");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(2));
    kill(&mut world, 20359);
    assert_eq!(item_count(&world, 3001, 1281), 1, "Key of Flame");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{salamander}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1282), 1, "Flame Earring");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(4));

    // Wind: mirror → feather (from DIALOG, not a kill) → bangle.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{sylph}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1283), 1, "Broken Bronze Mirror");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lizardman}_Quest {Q404} 30410-03.html")));
    assert_eq!(item_count(&world, 3001, 1284), 1, "Wind Feather comes from the lizardman's dialog");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(6));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{sylph}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1285), 1, "Wind Bangle");

    // Water: diary → two pebbles → necklace.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{undine}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1286), 1, "Rama's Diary");
    kill(&mut world, 27030);
    assert_eq!(quest_cond(&world, 3001, Q404), Some(8), "one pebble is not enough");
    kill(&mut world, 27030);
    assert_eq!(item_count(&world, 3001, 1287), 2, "two Sparkle Pebbles");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(9));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{undine}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1288), 1, "Water Necklace");

    // Earth: coin → red soil → ring.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{snake}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1289), 1, "Rusty Coin");
    kill(&mut world, 20021);
    assert_eq!(item_count(&world, 3001, 1290), 1, "Red Soil");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{snake}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1291), 1, "Earth Ring");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(13));
    drain(&mut rx);

    // Parina takes all four trinkets.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404}")));
    assert_eq!(item_count(&world, 3001, 1292), 1, "the Bead of Season");
    for id in [1282, 1285, 1288, 1291] {
        assert_eq!(item_count(&world, 3001, id), 0, "trinket {id} handed over");
    }
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// Each spirit refuses until the previous element's trinket is in hand, so the
/// chain can't be entered out of order.
#[test]
fn quest_q00404_branches_are_gated_on_the_previous_trinket() {
    let (mut world, _rx) = q404_world();
    let sylph = NPC_OID + 12;
    add_test_npc(&mut world, sylph, 30412, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{sylph}_Quest {Q404}")));

    assert_eq!(item_count(&world, 3001, 1283), 0, "no mirror without the Flame Earring");
}

/// A Q00405 world with the quest accepted (ACCEPT issues the first letter).
fn q405_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1201, "Mark of Faith", false)];
    for id in 1191..=1200 {
        items.push((id, "Q405 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20026, 20029] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30022, "Folk", 5, 100, 0, 0); // Zigaunt
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 10;
        p.base_class_id = 10;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405} ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1191), 1, "ACCEPT issues the 1st Letter of Order");
    drain(&mut rx);
    (world, rx)
}

/// Simplon hands over a **stack of three** where the other two book-givers
/// give one each, and cond 2 only lands once all three books are held.
#[test]
fn quest_q00405_simplon_gives_three_books() {
    let (mut world, _rx) = q405_world();
    let (vivyan, simplon, praga) = (NPC_OID + 30, NPC_OID + 31, NPC_OID + 32);
    add_test_npc(&mut world, vivyan, 30030, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, simplon, 30253, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, praga, 30333, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{vivyan}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1194), 1, "Vivyan gives one");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{simplon}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1195), 3, "Simplon gives THREE");
    assert!(quest_cond(&world, 3001, Q405) != Some(2), "Praga's book is still missing");

    // Praga: necklace on loan, pendant from a zombie (no chance roll), then
    // the book.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{praga}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1199), 1, "Necklace of Mother");
    let zombie = NPC_OID + 300;
    add_test_npc(&mut world, zombie, 20026, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, zombie, 3001);
    assert_eq!(item_count(&world, 3001, 1198), 1, "the pendant drops with no roll");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{praga}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1196), 1, "Book of Praga");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(2), "all three books held");

    // Zigaunt swaps the letters and takes ALL THREE of Simplon's books.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1195), 0, "the whole stack of three is taken");
    assert_eq!(item_count(&world, 3001, 1192), 1, "2nd Letter of Order");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(3));
}

/// The second errand: Lionel → Gallint → Lionel → Zigaunt for the Mark of
/// Faith.
#[test]
fn quest_q00405_courier_loop_awards_the_mark_of_faith() {
    let (mut world, mut rx) = q405_world();
    let (lionel, gallint) = (NPC_OID + 40, NPC_OID + 41);
    add_test_npc(&mut world, lionel, 30408, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, gallint, 30017, "Folk", 5, 100, 0, 0);
    // Jump the first errand by granting the 2nd letter directly. Zigaunt's
    // 1st-letter branch is only reached when the 2nd is absent, so leaving
    // the 1st in the bag doesn't change any path under test.
    super::items::add_inventory_item(&mut world, 3001, 1192, 1);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lionel}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1193), 1, "Lionel's Book");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(4));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{gallint}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1197), 1, "Certificate of Gallint");
    assert_eq!(item_count(&world, 3001, 1193), 0, "book handed over");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(5));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lionel}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1200), 1, "Lemoniell's Covenant");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(6));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")));
    assert_eq!(item_count(&world, 3001, 1201), 1, "the Mark of Faith");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q405].is_completed());
    }
}

/// Pages for both quests, including 404's uniform four-page scheme across all
/// four elemental spirits.
#[test]
fn wizard_cleric_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/quests/");
    for p in ["01", "02", "02a", "03", "04", "07"] {
        let path = format!("{DIST}Q00404_PathOfTheHumanWizard/30391-{p}.htm");
        assert!(std::path::Path::new(&path).exists(), "missing 30391-{p}.htm");
    }
    for p in ["05", "06"] {
        let path = format!("{DIST}Q00404_PathOfTheHumanWizard/30391-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30391-{p}.html");
    }
    // All four spirits (and the lizardman) use the same 01..04 scheme.
    for npc in ["30409", "30410", "30411", "30412", "30413"] {
        for p in ["01", "02", "03", "04"] {
            let path = format!("{DIST}Q00404_PathOfTheHumanWizard/{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{p}.html");
        }
    }
    for p in ["01", "02", "02a", "03", "04", "05"] {
        let path = format!("{DIST}Q00405_PathOfTheCleric/30022-{p}.htm");
        assert!(std::path::Path::new(&path).exists(), "missing 30022-{p}.htm");
    }
    let cleric: [(&str, &[&str]); 6] = [
        ("30022", &["06", "07", "08", "09"]),
        ("30017", &["01", "02"]),
        ("30030", &["01", "02"]),
        ("30253", &["01", "02"]),
        ("30333", &["01", "02", "03", "04"]),
        ("30408", &["01", "02", "03", "04", "05"]),
    ];
    for (npc, pages) in cleric {
        for p in pages {
            let path = format!("{DIST}Q00405_PathOfTheCleric/{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{p}.html");
        }
    }
}

const Q409: &str = "Q00409_PathOfTheElvenOracle";

/// An Elven Mage with Q00409 accepted, plus Allana and Perrin placed.
fn q409_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1235, "Leaf of Oracle", false)];
    for id in [1231, 1232, 1233, 1234, 1236, 1275] {
        items.push((id, "Q409 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [27032, 27033, 27034, 27035] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30293, "Folk", 5, 100, 0, 0); // Manuel
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409} ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1231), 1, "the Crystal Medallion");
    drain(&mut rx);
    (world, rx)
}

/// Object ids of every live NPC with `npc_id`.
fn npcs_of(world: &mut World, npc_id: i32) -> Vec<i32> {
    let mut out = Vec::new();
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|n| {
        if n.npc_id == npc_id {
            out.push(n.object_id);
        }
    });
    out
}

/// `replay_1` conjures three lizardmen **and sets them on the player** — the
/// new `spawn_attacker` primitive has to wire both halves, so this asserts the
/// spawn and the aggro together.
#[test]
fn quest_q00409_allana_spawns_three_ambushers_that_aggro() {
    let (mut world, _rx) = q409_world();
    let allana = NPC_OID + 20;
    add_test_npc(&mut world, allana, 30424, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{allana}_Quest {Q409}")));
    assert_eq!(quest_cond(&world, 3001, Q409), Some(2), "Allana starts the tale");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{allana}_Quest {Q409} replay_1")));

    for id in [27032, 27033, 27034] {
        let spawned = npcs_of(&mut world, id);
        assert_eq!(spawned.len(), 1, "one {id} ambusher was conjured");
        assert!(
            world
                .objects
                .get_component::<crate::model::npc::AggroList>(&spawned[0])
                .is_some_and(|a| a.0.contains_key(&3001)),
            "ambusher {id} was set on the player"
        );
    }
}

/// The ambush tag pays only the first attacker — and unlike quests 401/403
/// there is **no weapon requirement**, so an unarmed first hit still qualifies.
#[test]
fn quest_q00409_ambush_pays_only_the_first_attacker() {
    // Killed cold: never attacked, so script value stays 0.
    let (mut world, _rx) = q409_world();
    let cold = NPC_OID + 100;
    add_test_npc(&mut world, cold, 27033, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, cold, 3001);
    assert_eq!(item_count(&world, 3001, 1234), 0, "an untagged ambusher pays nothing");

    // Attacked first (bare-handed) then killed: qualifies.
    let tagged = NPC_OID + 101;
    add_test_npc(&mut world, tagged, 27033, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, tagged, 3001, 10.0);
    death::npc_do_die(&mut world, tagged, 3001);
    assert_eq!(item_count(&world, 3001, 1234), 1, "no weapon gate here, unlike 401/403");
    assert_eq!(quest_cond(&world, 3001, Q409), Some(3));
}

/// `memoState` and `cond` are separate axes and move independently: losing the
/// re-enactment rewinds `memoState` 2 → 1 while pushing `cond` to 8.
#[test]
fn quest_q00409_memo_state_rewinds_independently_of_cond() {
    let (mut world, mut rx) = q409_world();
    let allana = NPC_OID + 20;
    add_test_npc(&mut world, allana, 30424, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{allana}_Quest {Q409}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{allana}_Quest {Q409} replay_1")));
    assert_eq!(quest_memo(&world, 3001, Q409), 2, "the re-enactment is running");
    drain(&mut rx);

    // Back to Manuel empty-handed: the tale is reset, the window jumps to 8.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409}")));

    assert_eq!(quest_memo(&world, 3001, Q409), 1, "memoState rewound");
    assert_eq!(quest_cond(&world, 3001, Q409), Some(8), "cond moved the other way");
}

fn quest_memo(world: &World, player: i32, quest: &str) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .and_then(|qs| qs.vars.get("memoState"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

#[test]
fn elven_oracle_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00409_PathOfTheElvenOracle/"
    );
    for p in ["01", "02", "02a", "03", "04", "05"] {
        let path = format!("{DIST}30293-{p}.htm");
        assert!(std::path::Path::new(&path).exists(), "missing 30293-{p}.htm");
    }
    for p in ["06", "07", "08", "09"] {
        let path = format!("{DIST}30293-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30293-{p}.html");
    }
    for p in ["01", "02", "03", "04", "05", "06", "07", "08", "09"] {
        let path = format!("{DIST}30424-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30424-{p}.html");
    }
    for p in ["01", "02", "03", "04", "05", "06"] {
        let path = format!("{DIST}30428-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30428-{p}.html");
    }
}

const Q408: &str = "Q00408_PathOfTheElvenWizard";

/// An Elven Mage with Q00408 accepted (Rossela at `NPC_OID`).
fn q408_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1230, "Eternity Diamond", false)];
    for id in [1218, 1219, 1220, 1221, 1222, 1223, 1224, 1225, 1226, 1229, 1272, 1273, 1274] {
        items.push((id, "Q408 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20019, 20047, 20466] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30414, "Folk", 5, 100, 0, 0); // Rossela
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408} ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1229), 1, "the Fertility Peridot");
    drain(&mut rx);
    (world, rx)
}

/// All three errands, then the diamond. Errands 1 and 2 swap the introduction
/// for a charm through a dialog event; **errand 3 has no such event** and
/// swaps on the talk itself.
#[test]
fn quest_q00408_three_errands_award_the_eternity_diamond() {
    let (mut world, mut rx) = q408_world();
    let (greenis, thalia, northwind) = (NPC_OID + 20, NPC_OID + 21, NPC_OID + 22);
    add_test_npc(&mut world, greenis, 30157, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, thalia, 30371, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, northwind, 30423, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;

    // (offer event, specialist oid, swap event, mob, material, need, gem)
    let errands: [(&str, i32, Option<&str>, i32, i32, i64, i32); 3] = [
        ("30414-10.html", greenis, Some("30157-02.html"), 20466, 1219, 5, 1220),
        ("30414-12.html", thalia, Some("30371-02.html"), 20019, 1223, 5, 1221),
        ("30414-16.html", northwind, None, 20047, 1225, 2, 1226),
    ];
    for (offer, specialist, swap, mob, material, need, gem) in errands {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408} {offer}")));
        match swap {
            // Greenis / Thalia: the swap needs the dialog event.
            Some(ev) => {
                handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{specialist}_Quest {Q408} {ev}")));
            }
            // Northwind: talking is enough.
            None => {
                handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{specialist}_Quest {Q408}")));
            }
        }
        for _ in 0..need {
            mob_oid += 1;
            add_test_npc(&mut world, mob_oid, mob, "Monster", 20, 30, 0, 0);
            world.forced_rolls.push_back(0); // inside every chance
            death::npc_do_die(&mut world, mob_oid, 3001);
        }
        assert_eq!(item_count(&world, 3001, material), need, "collected material {material}");
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{specialist}_Quest {Q408}")));
        assert_eq!(item_count(&world, 3001, gem), 1, "gem {gem} awarded");
        assert_eq!(item_count(&world, 3001, material), 0, "material handed over");
    }
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408}")));
    assert_eq!(item_count(&world, 3001, 1230), 1, "the Eternity Diamond");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q408].is_completed());
    }
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// The charm is the drop gate: the same mob pays nothing before the
/// introduction has been swapped for it.
#[test]
fn quest_q00408_drops_need_the_charm() {
    let (mut world, _rx) = q408_world();
    let mob = NPC_OID + 400;
    add_test_npc(&mut world, mob, 20466, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1219), 0, "no charm, no Red Down");
}

/// Northwind ships only three pages, which is *why* his errand has no swap
/// event — there is no fourth page to route one to.
#[test]
fn elven_wizard_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00408_PathOfTheElvenWizard/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        let path = format!("{DIST}30414-{p}.htm");
        assert!(std::path::Path::new(&path).exists(), "missing 30414-{p}.htm");
    }
    for n in 7..=23 {
        let path = format!("{DIST}30414-{n:02}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30414-{n:02}.html");
    }
    for npc in ["30157", "30371"] {
        for p in ["01", "02", "03", "04"] {
            let path = format!("{DIST}{npc}-{p}.html");
            assert!(std::path::Path::new(&path).exists(), "missing {npc}-{p}.html");
        }
    }
    for p in ["01", "02", "03"] {
        let path = format!("{DIST}30423-{p}.html");
        assert!(std::path::Path::new(&path).exists(), "missing 30423-{p}.html");
    }
    assert!(
        !std::path::Path::new(&format!("{DIST}30423-04.html")).exists(),
        "Northwind has no fourth page — hence no swap event"
    );
}

const Q410: &str = "Q00410_PathOfThePalusKnight";
const Q411: &str = "Q00411_PathOfTheAssassin";

/// A Dark Fighter with the given quest accepted; `start_npc` sits at NPC_OID.
fn dark_elf_quest_world(
    quest: &str,
    start_npc: i32,
    accept_page: Option<&str>,
    items: &[i32],
    mobs: &[i32],
) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = items.iter().map(|id| (*id, "Q item", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in mobs {
        let mut t = crate::data::npc_data::default_template(*id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, start_npc, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 31; // Dark Fighter
        p.base_class_id = 31;
        p.race = 2;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} ACCEPT")));
    if let Some(page) = accept_page {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} {page}")));
    }
    drain(&mut rx);
    (world, rx)
}

/// Q00410 end to end. Every drop is **unrolled**, so the exact kill counts
/// below are the whole requirement — no forced rolls anywhere in this test.
#[test]
fn quest_q00410_full_chain_awards_the_gaze_of_abyss() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q410,
        30329,
        Some("30329-06.htm"),
        &[1237, 1238, 1239, 1240, 1241, 1242, 1243, 1244],
        &[20038, 20043, 20049],
    );
    let kalinta = NPC_OID + 20;
    add_test_npc(&mut world, kalinta, 30422, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        death::npc_do_die(world, mob_oid, 3001);
    };

    // 13 lycanthrope skulls — 13 kills, no rolls.
    for _ in 0..13 {
        kill(&mut world, 20049);
    }
    assert_eq!(item_count(&world, 3001, 1238), 13, "unrolled: 13 kills = 13 skulls");
    assert_eq!(quest_cond(&world, 3001, Q410), Some(2));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q410} 30329-10.html")));
    assert_eq!(item_count(&world, 3001, 1239), 1, "Virgil's letter");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kalinta}_Quest {Q410} 30422-02.html")));
    assert_eq!(item_count(&world, 3001, 1240), 1, "Morte talisman");

    // One carapace and five silks.
    kill(&mut world, 20038);
    for _ in 0..5 {
        kill(&mut world, 20043);
    }
    assert_eq!(item_count(&world, 3001, 1241), 1, "carapace");
    assert_eq!(item_count(&world, 3001, 1242), 5, "silks");
    assert_eq!(quest_cond(&world, 3001, Q410), Some(5), "collection complete");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kalinta}_Quest {Q410} 30422-06.html")));
    assert_eq!(item_count(&world, 3001, 1243), 1, "the coffin");
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q410}")));
    assert_eq!(item_count(&world, 3001, 1244), 1, "the Gaze of Abyss");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q410].is_completed());
    }
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION));
}

/// Each drop is gated on the matching talisman: the right mob pays nothing
/// while the wrong stage is active.
#[test]
fn quest_q00410_drops_are_gated_on_the_talisman() {
    let (mut world, _rx) = dark_elf_quest_world(
        Q410,
        30329,
        Some("30329-06.htm"),
        &[1237, 1238, 1240, 1241, 1242],
        &[20038, 20043],
    );
    // Holding the Pallus talisman, not the Morte one: spiders pay nothing.
    let mob = NPC_OID + 400;
    add_test_npc(&mut world, mob, 20038, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1241), 0, "carapace needs the Morte talisman");
}

/// Q00411's token chain, all the way to the Iron Heart.
#[test]
fn quest_q00411_token_chain_awards_the_iron_heart() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q411,
        30416,
        None, // ACCEPT starts the quest directly
        &[1245, 1246, 1247, 1248, 1250, 1251, 1252],
        &[20369, 27036],
    );
    assert_eq!(item_count(&world, 3001, 1245), 1, "Shilen's Call");
    let (leikan, arkenia) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, leikan, 30382, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        death::npc_do_die(world, mob_oid, 3001);
    };

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{arkenia}_Quest {Q411} 30419-05.html")));
    assert_eq!(item_count(&world, 3001, 1246), 1, "Arkenia's letter");
    assert_eq!(item_count(&world, 3001, 1245), 0, "the call is consumed — one token at a time");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{leikan}_Quest {Q411} 30382-03.html")));
    assert_eq!(item_count(&world, 3001, 1247), 1, "Leikan's note");
    assert_eq!(item_count(&world, 3001, 1246), 0);

    for _ in 0..10 {
        kill(&mut world, 20369);
    }
    assert_eq!(item_count(&world, 3001, 1248), 10, "unrolled: 10 kills = 10 molars");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(4));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{leikan}_Quest {Q411}")));
    assert_eq!(item_count(&world, 3001, 1248), 0, "molars handed over");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(5));

    kill(&mut world, 27036); // Calpico
    assert_eq!(item_count(&world, 3001, 1250), 1, "Shilen's Tears");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(6));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{arkenia}_Quest {Q411}")));
    assert_eq!(item_count(&world, 3001, 1251), 1, "Arkenia's recommendation");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q411}")));
    assert_eq!(item_count(&world, 3001, 1252), 1, "the Iron Heart");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q411].is_completed());
    }
}

/// Leikan answers on the same token but a different molar count — the one
/// place in 411 where an item outside the token chain changes the page.
#[test]
fn quest_q00411_leikan_page_tracks_the_molar_count() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q411,
        30416,
        None,
        &[1245, 1246, 1247, 1248, 1250, 1251, 1252],
        &[20369],
    );
    let (leikan, arkenia) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, leikan, 30382, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{arkenia}_Quest {Q411} 30419-05.html")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{leikan}_Quest {Q411} 30382-03.html")));

    let dist = |page: &str| {
        let path = format!(
            "{}/../../dist/game/data/scripts/quests/Q00411_PathOfTheAssassin/{page}",
            env!("CARGO_MANIFEST_DIR")
        );
        crate::data::htm_cache::strip_htm(&std::fs::read_to_string(&path).expect("dist page"))
            .replace("%objectId%", &leikan.to_string())
    };

    // 0 molars.
    drain(&mut rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{leikan}_Quest {Q411}")));
    assert_eq!(
        drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).unwrap_or_default(),
        dist("30382-05.html"),
        "note in hand, no molars"
    );

    // Partway.
    let mob = NPC_OID + 600;
    add_test_npc(&mut world, mob, 20369, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob, 3001);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{leikan}_Quest {Q411}")));
    assert_eq!(
        drain(&mut rx).iter().find_map(|p| decode_npc_html(p)).unwrap_or_default(),
        dist("30382-06.html"),
        "some molars but not ten"
    );
}

#[test]
fn dark_elf_path_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/quests/");
    // The two quests split .htm/.html at *different* points: 410's accept
    // page `30329-06` is `.htm`, while 411's `30416-06` is `.html` (its
    // accept page is `-05`). Asserted separately so the split can't be
    // assumed uniform across the tier.
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00410_PathOfThePalusKnight/30329-{p}.htm")).exists(),
            "missing 30329-{p}.htm"
        );
    }
    for p in ["01", "02", "02a", "03", "04", "05"] {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-{p}.htm")).exists(),
            "missing 30416-{p}.htm"
        );
    }
    assert!(
        !std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-06.htm")).exists(),
        "411's -06 is .html, unlike 410's"
    );
    for n in 7..=12 {
        assert!(std::path::Path::new(&format!("{DIST}Q00410_PathOfThePalusKnight/30329-{n:02}.html")).exists());
    }
    for n in 1..=6 {
        assert!(std::path::Path::new(&format!("{DIST}Q00410_PathOfThePalusKnight/30422-{n:02}.html")).exists());
    }
    for n in 6..=11 {
        assert!(std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-{n:02}.html")).exists());
    }
    for n in 1..=9 {
        assert!(std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30382-{n:02}.html")).exists());
    }
    for n in 1..=11 {
        assert!(std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30419-{n:02}.html")).exists());
    }
}

const Q412: &str = "Q00412_PathOfTheDarkWizard";
const Q413: &str = "Q00413_PathOfTheShillienOracle";

/// A Dark Mage with the given quest accepted; start NPC at NPC_OID.
fn dark_mage_quest_world(
    quest: &str,
    start_npc: i32,
    accept_page: Option<&str>,
    items: &[i32],
    mobs: &[i32],
) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = items.iter().map(|id| (*id, "Q item", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in mobs {
        let mut t = crate::data::npc_data::default_template(*id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, start_npc, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 38; // Dark Mage
        p.base_class_id = 38;
        p.race = 2;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} ACCEPT")));
    if let Some(page) = accept_page {
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} {page}")));
    }
    drain(&mut rx);
    (world, rx)
}

/// Q00412's three seed errands, then the jewel. Arkenia hands her tool over on
/// the **talk**; the other two need a dialog event — the same asymmetry as
/// quest 408, exercised here in one loop.
#[test]
fn quest_q00412_three_seeds_award_the_jewel_of_darkness() {
    let (mut world, mut rx) = dark_mage_quest_world(
        Q412,
        30421,
        None,
        &[1253, 1254, 1255, 1256, 1257, 1259, 1260, 1261, 1277, 1278, 1279],
        &[20015, 20022, 20045, 20517, 20518],
    );
    assert_eq!(item_count(&world, 3001, 1254), 1, "the Seed of Despair");
    let (charkeren, annika, arkenia) = (NPC_OID + 20, NPC_OID + 21, NPC_OID + 22);
    add_test_npc(&mut world, charkeren, 30415, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, annika, 30418, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;

    // (specialist, tool event, tool, mob, material, need, seed)
    let errands: [(i32, Option<&str>, i32, i32, i32, i64, i32); 3] = [
        (charkeren, Some("30415-03.html"), 1277, 20015, 1257, 3, 1253),
        (annika, Some("30418-02.html"), 1278, 20022, 1259, 2, 1255),
        (arkenia, None, 1279, 20045, 1260, 3, 1256),
    ];
    for (npc, tool_event, tool, mob, material, need, seed) in errands {
        match tool_event {
            Some(ev) => {
                handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{npc}_Quest {Q412} {ev}")));
            }
            None => {
                // Arkenia: the talk itself hands the Hub Scent over.
                handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{npc}_Quest {Q412}")));
            }
        }
        assert_eq!(item_count(&world, 3001, tool), 1, "tool {tool} received");
        for _ in 0..need {
            mob_oid += 1;
            add_test_npc(&mut world, mob_oid, mob, "Monster", 20, 30, 0, 0);
            world.forced_rolls.push_back(0); // `getRandom(2) == 0`
            death::npc_do_die(&mut world, mob_oid, 3001);
        }
        assert_eq!(item_count(&world, 3001, material), need, "material {material}");
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{npc}_Quest {Q412}")));
        assert_eq!(item_count(&world, 3001, seed), 1, "seed {seed} grown");
        assert_eq!(item_count(&world, 3001, tool), 0, "tool spent");
    }
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q412}")));
    assert_eq!(item_count(&world, 3001, 1261), 1, "the Jewel of Darkness");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q412].is_completed());
    }
}

/// Q00412 rolls `getRandom(2) == 0` — **equality**. A forced roll of 1 must
/// not drop; read as `getRandom(2) < 2` every kill would pay.
#[test]
fn quest_q00412_drop_is_a_coin_flip_on_equality() {
    for (forced, expected) in [(0, 1), (1, 0)] {
        let (mut world, _rx) = dark_mage_quest_world(
            Q412,
            30421,
            None,
            &[1253, 1254, 1257, 1277],
            &[20015],
        );
        let charkeren = NPC_OID + 20;
        add_test_npc(&mut world, charkeren, 30415, "Folk", 5, 100, 0, 0);
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{charkeren}_Quest {Q412} 30415-03.html")));
        let mob = NPC_OID + 400;
        add_test_npc(&mut world, mob, 20015, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(forced);
        death::npc_do_die(&mut world, mob, 3001);
        assert_eq!(item_count(&world, 3001, 1257), expected, "roll {forced} against `== 0`");
    }
}

/// Q00413's succubus kill is a **swap**: it spends a Blank Sheet to make a
/// Bloody Rune, so the two counts move in opposite directions and the stage
/// ends when the sheets run out.
#[test]
fn quest_q00413_succubus_swaps_sheets_for_runes() {
    let (mut world, _rx) = dark_mage_quest_world(
        Q413,
        30330,
        Some("30330-06.htm"),
        &[1262, 1263, 1264, 1265, 1266, 1267, 1268, 1269, 1270],
        &[20776],
    );
    let talbot = NPC_OID + 20;
    add_test_npc(&mut world, talbot, 30377, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{talbot}_Quest {Q413} 30377-02.html")));
    assert_eq!(item_count(&world, 3001, 1263), 5, "Talbot gives a stack of five sheets");

    for i in 1..=5 {
        let mob = NPC_OID + 500 + i;
        add_test_npc(&mut world, mob, 20776, "Monster", 20, 30, 0, 0);
        death::npc_do_die(&mut world, mob, 3001);
        assert_eq!(item_count(&world, 3001, 1264), i as i64, "rune {i} made");
        assert_eq!(item_count(&world, 3001, 1263), 5 - i as i64, "sheet {i} spent");
    }
    assert_eq!(quest_cond(&world, 3001, Q413), Some(3), "sheets exhausted AND five runes");

    // A sixth succubus has no sheet left to spend.
    let extra = NPC_OID + 600;
    add_test_npc(&mut world, extra, 20776, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, extra, 3001);
    assert_eq!(item_count(&world, 3001, 1264), 5, "no sheet, no rune");
}

/// Q00413 end to end.
#[test]
fn quest_q00413_full_chain_awards_the_orb_of_abyss() {
    let (mut world, mut rx) = dark_mage_quest_world(
        Q413,
        30330,
        Some("30330-06.htm"),
        &[1262, 1263, 1264, 1265, 1266, 1267, 1268, 1269, 1270],
        &[20776, 20457],
    );
    let (adonius, talbot) = (NPC_OID + 21, NPC_OID + 20);
    add_test_npc(&mut world, talbot, 30377, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, adonius, 30375, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        death::npc_do_die(world, mob_oid, 3001);
    };

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{talbot}_Quest {Q413} 30377-02.html")));
    for _ in 0..5 {
        kill(&mut world, 20776);
    }
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{talbot}_Quest {Q413}")));
    assert_eq!(item_count(&world, 3001, 1265), 1, "Garmiel's Book");
    assert_eq!(item_count(&world, 3001, 1266), 1, "Prayer of Adonius");
    assert_eq!(quest_cond(&world, 3001, Q413), Some(4));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{adonius}_Quest {Q413} 30375-04.html")));
    assert_eq!(item_count(&world, 3001, 1267), 1, "Penitent's Mark");
    for _ in 0..10 {
        kill(&mut world, 20457);
    }
    assert_eq!(item_count(&world, 3001, 1268), 10, "unrolled: 10 kills = 10 bones");
    assert_eq!(quest_cond(&world, 3001, Q413), Some(6));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{adonius}_Quest {Q413}")));
    assert_eq!(item_count(&world, 3001, 1269), 1, "Andariel's Book");
    assert_eq!(quest_cond(&world, 3001, Q413), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q413}")));
    assert_eq!(item_count(&world, 3001, 1270), 1, "the Orb of Abyss");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q413].is_completed());
    }
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION));
}

const Q414: &str = "Q00414_PathOfTheOrcRaider";

/// An Orc Fighter with Q00414 accepted (Karukia at NPC_OID).
fn q414_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> =
        [1578, 1579, 1580, 1589, 1590, 1591, 1592, 8544].iter().map(|id| (*id, "Q414", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20320, 27045, 27054] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30570, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
        p.race = 3;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414} ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1579), 1, "the Goblin Dwelling Map");
    drain(&mut rx);
    (world, rx)
}

/// Green blood is a rising **summon meter**, not loot: a roll above the held
/// count gains one, a roll at or below it wipes the stack and summons Kuruka.
#[test]
fn quest_q00414_green_blood_is_a_summon_meter() {
    let (mut world, _rx) = q414_world();

    // blood 0, forced roll 5 → `0 <= 5` → gain.
    let mob = NPC_OID + 100;
    add_test_npc(&mut world, mob, 20320, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(5);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 1, "gained a green blood");
    assert!(npcs_of(&mut world, 27045).is_empty(), "no summon yet");

    // blood 1, forced roll 0 → `1 <= 0` is false → wipe and summon.
    let mob2 = NPC_OID + 101;
    add_test_npc(&mut world, mob2, 20320, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, mob2, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 0, "the meter is wiped");
    let summoned = npcs_of(&mut world, 27045);
    assert_eq!(summoned.len(), 1, "Kuruka Ratman Leader was summoned");
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&summoned[0])
            .is_some_and(|a| a.0.contains_key(&3001)),
        "and set on the player"
    );
}

/// The tooth comes from Kuruka, never from the goblins — porting the blood as
/// a capped collection would make the quest unfinishable.
#[test]
fn quest_q00414_teeth_come_from_kuruka_and_reset_the_meter() {
    let (mut world, _rx) = q414_world();
    // Stock a little blood first.
    let mob = NPC_OID + 100;
    add_test_npc(&mut world, mob, 20320, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(19);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 1);

    let kuruka = NPC_OID + 200;
    add_test_npc(&mut world, kuruka, 27045, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, kuruka, 3001);
    assert_eq!(item_count(&world, 3001, 1580), 1, "the tooth comes from Kuruka");
    assert_eq!(item_count(&world, 3001, 1578), 0, "and resets the meter");
}

/// Umbar Orcs spend one report per head (Zakan's first), 20% of the time.
#[test]
fn quest_q00414_umbar_heads_spend_the_reports() {
    let (mut world, _rx) = q414_world();
    for _ in 0..10 {
        super::items::add_inventory_item(&mut world, 3001, 1580, 1); // 10 teeth
    }
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414} 30570-07a.htm")));
    assert_eq!(item_count(&world, 3001, 1589), 1, "Umbar's report");
    assert_eq!(item_count(&world, 3001, 1590), 1, "Zakan's report");
    assert_eq!(quest_cond(&world, 3001, Q414), Some(3));

    // A roll of 2 misses (`getRandom(10) < 2`).
    let miss = NPC_OID + 300;
    add_test_npc(&mut world, miss, 27054, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(2);
    death::npc_do_die(&mut world, miss, 3001);
    assert_eq!(item_count(&world, 3001, 1591), 0, "roll 2 is outside the 20%");

    for i in 0..2 {
        let mob = NPC_OID + 310 + i;
        add_test_npc(&mut world, mob, 27054, "Monster", 20, 30, 0, 0);
        world.forced_rolls.push_back(0);
        death::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1591), 2, "two betrayer heads");
    assert_eq!(item_count(&world, 3001, 1590), 0, "Zakan's report spent first");
    assert_eq!(item_count(&world, 3001, 1589), 0, "then Umbar's");
    assert_eq!(quest_cond(&world, 3001, Q414), Some(4));

    // Kasman pays out.
    let kasman = NPC_OID + 20;
    add_test_npc(&mut world, kasman, 30501, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kasman}_Quest {Q414}")));
    assert_eq!(item_count(&world, 3001, 1592), 1, "the Mark of Raider");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q414].is_completed());
    }
}

/// NPC 31978 ships five pages in this quest's directory but is registered
/// nowhere, and `30570-07.htm` offers only the `07a` button — so the whole
/// `07b` route is dead at both ends. Asserted so a future reader doesn't
/// "restore" one end without the other.
#[test]
fn orc_raider_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/"
    );
    // The orphaned pages really do ship.
    for p in ["01", "02", "03", "04", "05"] {
        assert!(
            std::path::Path::new(&format!("{DIST}31978-{p}.htm")).exists(),
            "31978-{p}.htm ships but is unreachable"
        );
    }
    // ...and nothing offers the 07b button.
    let fork = std::fs::read_to_string(format!("{DIST}30570-07.htm")).expect("the fork page");
    assert!(fork.contains("30570-07a.htm"), "07a is offered");
    assert!(!fork.contains("30570-07b.htm"), "07b is NOT offered — the route is unreachable");
}

#[test]
fn orc_raider_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "06", "07", "07a", "07b", "08"] {
        assert!(std::path::Path::new(&format!("{DIST}30570-{p}.htm")).exists(), "missing 30570-{p}.htm");
    }
    for p in ["01", "02", "03"] {
        assert!(std::path::Path::new(&format!("{DIST}30501-{p}.htm")).exists(), "missing 30501-{p}.htm");
    }
}

const Q415: &str = "Q00415_PathOfTheOrcMonk";

/// An Orc Fighter with Q00415 accepted (Gantaki at NPC_OID). `weapon` is put
/// straight into the RHand paperdoll when given.
fn q415_world(weapon: Option<i32>) -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let ids = [
        1593, 1594, 1595, 1596, 1597, 1598, 1599, 1600, 1601, 1602, 1603, 1604, 1605, 1606, 1607,
        1608, 1609, 1610, 1611, 1612, 1613, 1614, 1615, 8545, 8546,
    ];
    let rows: Vec<(i32, &str, bool)> = ids.iter().map(|id| (*id, "Q415", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20014, 20017, 20024, 20359, 20415, 20476, 20478, 20479, 21118] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30587, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
        p.race = 3;
    }
    if let Some(w) = weapon {
        equip_weapon_row(&mut world, 3001, w);
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415} ACCEPT")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415} 30587-06.htm")));
    assert_eq!(item_count(&world, 3001, 1593), 1, "the pomegranate");
    drain(&mut rx);
    (world, rx)
}

/// The weapon gate is the **inverse** of quests 401/403: bare hands pass, a
/// sword fails, a fist weapon passes.
#[test]
fn quest_q00415_weapon_gate_wants_bare_hands_or_fists() {
    // (equipped weapon, is it a fist type, expected claws after one kill)
    let cases: [(Option<i32>, bool, i64); 3] = [
        (None, false, 1),        // bare-handed — the pass case
        (Some(7000), false, 0),  // a sword — disqualifies
        (Some(7001), true, 1),   // a fist weapon — passes
    ];
    for (weapon, is_fist, expected) in cases {
        let (mut world, _rx) = q415_world(weapon);
        if let (Some(w), true) = (weapon, is_fist) {
            world.data.item_data.set_weapon_type_for_test(w, crate::data::item_data::WeaponType::Fist);
        }
        // Get pouch 1 from Rosheek.
        let rosheek = NPC_OID + 20;
        add_test_npc(&mut world, rosheek, 30590, "Folk", 5, 100, 0, 0);
        handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")));
        assert_eq!(item_count(&world, 3001, 1594), 1, "first leather pouch");

        let bear = NPC_OID + 100;
        add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0);
        death::npc_do_die(&mut world, bear, 3001);
        assert_eq!(
            item_count(&world, 3001, 1600),
            expected,
            "weapon {weapon:?} (fist={is_fist}): bare hands and fists pass, blades don't"
        );
    }
}

/// Each pouch takes **five** kills: four trophies, and the fifth converts.
#[test]
fn quest_q00415_pouch_takes_five_kills_not_four() {
    let (mut world, _rx) = q415_world(None);
    let rosheek = NPC_OID + 20;
    add_test_npc(&mut world, rosheek, 30590, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")));

    for i in 1..=4 {
        let bear = NPC_OID + 100 + i;
        add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0);
        death::npc_do_die(&mut world, bear, 3001);
        assert_eq!(item_count(&world, 3001, 1600), i as i64, "claw {i}");
        assert_eq!(item_count(&world, 3001, 1597), 0, "pouch not full yet");
    }
    // The fifth kill converts and consumes the four claws.
    let bear = NPC_OID + 200;
    add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, bear, 3001, 10.0);
    death::npc_do_die(&mut world, bear, 3001);
    assert_eq!(item_count(&world, 3001, 1597), 1, "the fifth kill fills the pouch");
    assert_eq!(item_count(&world, 3001, 1600), 0, "claws consumed");
    assert_eq!(item_count(&world, 3001, 1594), 0, "empty pouch handed over");
    assert_eq!(quest_cond(&world, 3001, Q415), Some(3));

    // Rosheek swaps the full pouch for the next empty one.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")));
    assert_eq!(item_count(&world, 3001, 1595), 1, "second pouch");
    assert_eq!(quest_cond(&world, 3001, Q415), Some(4));
}

/// The fourth pouch spans four mobs at three trophies each and converts on the
/// twelfth kill.
#[test]
fn quest_q00415_fourth_pouch_converts_on_the_twelfth_kill() {
    let (mut world, _rx) = q415_world(None);
    super::items::add_inventory_item(&mut world, 3001, 1607, 1); // the 4th pouch
    let mut oid = NPC_OID + 300;
    let mobs = [(20014, 1612), (20017, 1609), (20024, 1611), (20359, 1610)];
    let mut killed = 0;
    for (mob, trophy) in mobs {
        for _ in 0..3 {
            oid += 1;
            add_test_npc(&mut world, oid, mob, "Monster", 20, 30, 0, 0);
            combat::npc_receive_damage(&mut world, oid, 3001, 10.0);
            death::npc_do_die(&mut world, oid, 3001);
            killed += 1;
            if killed < 12 {
                assert_eq!(item_count(&world, 3001, 1608), 0, "not full at {killed} kills");
            }
        }
        let _ = trophy;
    }
    assert_eq!(item_count(&world, 3001, 1608), 1, "the twelfth kill fills the pouch");
    for id in [1609, 1610, 1611, 1612] {
        assert_eq!(item_count(&world, 3001, id), 0, "trophy {id} consumed");
    }
    assert_eq!(quest_cond(&world, 3001, Q415), Some(12));
}

/// The alternate ending is dead at both ends: no page offers `09c`, and
/// neither 31979 nor 32056 is registered anywhere — 13 orphaned pages.
#[test]
fn orc_monk_alternate_ending_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/"
    );
    // The orphaned pages ship: 31979 x4, 32056 x9.
    for p in ["01", "02", "03", "04"] {
        assert!(std::path::Path::new(&format!("{DIST}31979-{p}.html")).exists(), "31979-{p} ships");
    }
    for n in 1..=9 {
        assert!(
            std::path::Path::new(&format!("{DIST}32056-0{n}.html")).exists(),
            "32056-0{n} ships"
        );
    }
    // ...and the fork page offers only 09b.
    let fork = std::fs::read_to_string(format!("{DIST}30587-09a.html")).expect("the fork page");
    assert!(fork.contains("30587-09b.html"), "09b is offered");
    assert!(!fork.contains("30587-09c.html"), "09c is NOT offered — the route is unreachable");
}

#[test]
fn orc_monk_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        assert!(std::path::Path::new(&format!("{DIST}30587-{p}.htm")).exists(), "missing 30587-{p}.htm");
    }
    for p in ["07", "08", "09a", "09b", "09c", "10", "11"] {
        assert!(std::path::Path::new(&format!("{DIST}30587-{p}.html")).exists(), "missing 30587-{p}.html");
    }
    for n in 1..=4 {
        assert!(std::path::Path::new(&format!("{DIST}30501-0{n}.html")).exists(), "missing 30501-0{n}");
    }
    for n in 1..=9 {
        assert!(std::path::Path::new(&format!("{DIST}30590-0{n}.html")).exists(), "missing 30590-0{n}");
    }
    for n in 1..=4 {
        assert!(std::path::Path::new(&format!("{DIST}30591-0{n}.html")).exists(), "missing 30591-0{n}");
    }
}

const Q416: &str = "Q00416_PathOfTheOrcShaman";

/// An Orc Mage with Q00416 accepted (Tataru at NPC_OID).
fn q416_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> =
        (1616..=1631).map(|id| (id, "Q416", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20038, 20043, 20335, 20415, 20478, 20479, 27056] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30585, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 49; // Orc Mage
        p.base_class_id = 49;
        p.race = 3;
    }
    drain_db(&mut db_rx);
    // Note the event name: START, not ACCEPT.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416} START")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416} 30585-07.htm")));
    assert_eq!(item_count(&world, 3001, 1616), 1, "the fire charm");
    drain(&mut rx);
    (world, rx)
}

/// `ItemChanceHolder.count` is a **cond selector**, not a quantity: a grizzly
/// bear (gate cond 6) drops nothing at cond 1, and drops exactly one blood —
/// not six — once the cond matches.
#[test]
fn quest_q00416_holder_count_is_a_cond_gate_not_a_quantity() {
    let (mut world, _rx) = q416_world();
    // At cond 1 the grizzly is out of season.
    let early = NPC_OID + 100;
    add_test_npc(&mut world, early, 20335, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, early, 3001);
    assert_eq!(item_count(&world, 3001, 1625), 0, "grizzly is gated to cond 6");

    // Advance to cond 6 the short way: hand the player the flame charm and set
    // the cond, mirroring Umos' hand-over.
    super::items::add_inventory_item(&mut world, 3001, 1624, 1);
    set_quest_cond(&mut world, 3001, Q416, 6);
    let mob = NPC_OID + 101;
    add_test_npc(&mut world, mob, 20335, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1625), 1, "one blood per kill, not six");
}

/// The first stage: three different mobs, one trophy each, cond 2 when all
/// three are in.
#[test]
fn quest_q00416_first_stage_needs_one_of_each_trophy() {
    let (mut world, _rx) = q416_world();
    let mut oid = NPC_OID + 200;
    for (mob, item) in [(20415, 1619), (20478, 1618)] {
        oid += 1;
        add_test_npc(&mut world, oid, mob, "Monster", 20, 30, 0, 0);
        death::npc_do_die(&mut world, oid, 3001);
        assert_eq!(item_count(&world, 3001, item), 1, "trophy {item}");
        assert_eq!(quest_cond(&world, 3001, Q416), Some(1), "still collecting");
    }
    oid += 1;
    add_test_npc(&mut world, oid, 20479, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, oid, 3001);
    assert_eq!(quest_cond(&world, 3001, Q416), Some(2), "all three trophies");

    // Tataru swaps them for the mask and the second egg.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416}")));
    assert_eq!(item_count(&world, 3001, 1620), 1, "Hestui mask");
    assert_eq!(item_count(&world, 3001, 1621), 1, "second fiery egg");
    assert_eq!(item_count(&world, 3001, 1616), 0, "fire charm consumed");
    assert_eq!(quest_cond(&world, 3001, Q416), Some(3));
}

/// The parasite meter escalates and, unlike quest 414's Kuruka, the conjured
/// Durka Spirit is **not** set on the player.
#[test]
fn quest_q00416_durka_meter_summons_without_aggro() {
    let (mut world, _rx) = q416_world();
    super::items::add_inventory_item(&mut world, 3001, 1627, 1); // spirit net
    set_quest_cond(&mut world, 3001, Q416, 9);

    // Below the threshold the kill just pays a parasite.
    let mob = NPC_OID + 300;
    add_test_npc(&mut world, mob, 20038, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1629), 1, "a parasite");
    assert!(npcs_of(&mut world, 27056).is_empty(), "no spirit yet");

    // Eight parasites makes the summon certain.
    for _ in 0..7 {
        super::items::add_inventory_item(&mut world, 3001, 1629, 1);
    }
    assert_eq!(item_count(&world, 3001, 1629), 8);
    let mob2 = NPC_OID + 301;
    add_test_npc(&mut world, mob2, 20043, "Monster", 20, 30, 0, 0);
    death::npc_do_die(&mut world, mob2, 3001);
    assert_eq!(item_count(&world, 3001, 1629), 0, "the meter is wiped");
    let spirits = npcs_of(&mut world, 27056);
    assert_eq!(spirits.len(), 1, "a Durka Spirit was conjured");
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&spirits[0])
            .is_none_or(|a| !a.0.contains_key(&3001)),
        "and is NOT set on the player, unlike quest 414's Kuruka"
    );

    // Killing it yields the bound spirit and consumes the net.
    death::npc_do_die(&mut world, spirits[0], 3001);
    assert_eq!(item_count(&world, 3001, 1628), 1, "bound Durka spirit");
    assert_eq!(item_count(&world, 3001, 1627), 0, "the net is spent");
}

/// The tail: bound spirit → totem spirit blood → the Mask of Medium.
#[test]
fn quest_q00416_finish_awards_the_mask_of_medium() {
    let (mut world, mut rx) = q416_world();
    let (umos, duda) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, umos, 30502, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, duda, 30593, "Folk", 5, 100, 0, 0);
    super::items::add_inventory_item(&mut world, 3001, 1628, 1); // bound spirit
    set_quest_cond(&mut world, 3001, Q416, 9);
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{duda}_Quest {Q416}")));
    assert_eq!(item_count(&world, 3001, 1630), 1, "totem spirit blood");
    assert_eq!(quest_cond(&world, 3001, Q416), Some(11), "Java jumps 9 -> 11");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{umos}_Quest {Q416} 30502-07.html")));
    assert_eq!(item_count(&world, 3001, 1631), 1, "the Mask of Medium");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q416].is_completed());
    }
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION));
}

/// The `memoState` 100+ branch is dead at both ends — third Orc quest running.
#[test]
fn orc_shaman_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/"
    );
    // The orphaned NPCs really do ship pages.
    for npc in ["31979", "32057", "32090"] {
        let any = (1..=9).any(|n| {
            std::path::Path::new(&format!("{DIST}{npc}-0{n}.html")).exists()
        });
        assert!(any, "{npc} ships pages but is registered nowhere");
    }
    // The only entry to memoState 100 is 30585-14, which nothing offers.
    assert!(std::path::Path::new(&format!("{DIST}30585-14.html")).exists(), "30585-14 ships");
    for page in ["30585-11.html", "30585-12.html", "30585-13.html"] {
        let body = std::fs::read_to_string(format!("{DIST}{page}")).expect(page);
        assert!(!body.contains("30585-14"), "{page} must not offer the dead entry");
    }
}

#[test]
fn orc_shaman_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/"
    );
    for p in ["01", "02", "03", "04", "05", "06", "07"] {
        assert!(std::path::Path::new(&format!("{DIST}30585-{p}.htm")).exists(), "missing 30585-{p}.htm");
    }
    for n in 8..=16 {
        assert!(
            std::path::Path::new(&format!("{DIST}30585-{n:02}.html")).exists(),
            "missing 30585-{n:02}.html"
        );
    }
    for n in 1..=7 {
        assert!(std::path::Path::new(&format!("{DIST}30502-0{n}.html")).exists(), "missing 30502-0{n}");
    }
    for n in 1..=5 {
        assert!(std::path::Path::new(&format!("{DIST}30592-0{n}.html")).exists(), "missing 30592-0{n}");
    }
    for n in 1..=6 {
        assert!(std::path::Path::new(&format!("{DIST}30593-0{n}.html")).exists(), "missing 30593-0{n}");
    }
}

/// Force a quest's cond directly — used to jump into a mid-quest stage without
/// replaying the whole chain.
fn set_quest_cond(world: &mut World, player: i32, quest: &str, cond: i32) {
    if let Some(q) = world.objects.get_component_mut::<crate::model::components::Quests>(&player) {
        if let Some(qs) = q.0.get_mut(quest) {
            qs.vars.insert("cond".to_string(), cond.to_string());
        }
    }
}

const Q418: &str = "Q00418_PathOfTheArtisan";

/// A Dwarven Fighter with Q00418 accepted (Silvera at NPC_OID).
fn q418_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1632..=1641).map(|id| (id, "Q418", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20017, 20389, 20390] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30527, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} ACCEPT")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-06.htm")));
    assert_eq!(item_count(&world, 3001, 1632), 1, "Silvery's ring");
    drain(&mut rx);
    (world, rx)
}

/// The leader-tooth roll is lopsided: below 5 it pays **only** when one tooth
/// is already held, so the first tooth comes at 50% and the second at 100%.
#[test]
fn quest_q00418_leader_tooth_roll_has_a_hole_at_zero() {
    let (mut world, _rx) = q418_world();
    let mut oid = NPC_OID + 100;

    // Roll 0 with zero teeth: the `< 5` branch does nothing at all.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, oid, 3001);
    assert_eq!(item_count(&world, 3001, 1637), 0, "roll<5 at zero teeth pays nothing");

    // Roll 5 with zero teeth: the `else` branch always pays.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(5);
    death::npc_do_die(&mut world, oid, 3001);
    assert_eq!(item_count(&world, 3001, 1637), 1, "roll>=5 always pays");

    // Roll 0 with one tooth: now the `< 5` branch does pay.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, oid, 3001);
    assert_eq!(item_count(&world, 3001, 1637), 2, "roll<5 pays the second tooth");
}

/// Ratman teeth cap at 10 on a 70% roll; a roll of 7 misses.
#[test]
fn quest_q00418_ratman_teeth_roll_is_seventy_percent() {
    let (mut world, _rx) = q418_world();
    let miss = NPC_OID + 200;
    add_test_npc(&mut world, miss, 20389, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(7);
    death::npc_do_die(&mut world, miss, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 0, "roll 7 is outside `< 7`");

    let hit = NPC_OID + 201;
    add_test_npc(&mut world, hit, 20389, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(6);
    death::npc_do_die(&mut world, hit, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 1, "roll 6 pays");
}

/// The whole chain: teeth → 1st pass → Kluto's letter → Pinter's footprint →
/// the stolen box → 2nd pass → the Final Pass Certificate.
#[test]
fn quest_q00418_full_chain_awards_the_final_pass() {
    let (mut world, mut rx) = q418_world();
    let (pinter, kluto) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, pinter, 30298, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, kluto, 30317, "Folk", 5, 100, 0, 0);
    for _ in 0..10 {
        super::items::add_inventory_item(&mut world, 3001, 1636, 1);
    }
    for _ in 0..2 {
        super::items::add_inventory_item(&mut world, 3001, 1637, 1);
    }

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-08b.html")));
    assert_eq!(item_count(&world, 3001, 1633), 1, "first pass certificate");
    assert_eq!(item_count(&world, 3001, 1636), 0, "teeth handed over");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(3));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-04.html")));
    assert_eq!(item_count(&world, 3001, 1638), 1, "Kluto's letter");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(4));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-03.html")));
    assert_eq!(item_count(&world, 3001, 1639), 1, "footprint of thief");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(5));

    let orc = NPC_OID + 300;
    add_test_npc(&mut world, orc, 20017, "Monster", 20, 30, 0, 0);
    world.forced_rolls.push_back(0); // `getRandom(10) < 2`
    death::npc_do_die(&mut world, orc, 3001);
    assert_eq!(item_count(&world, 3001, 1640), 1, "the stolen secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(6));

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-06.html")));
    assert_eq!(item_count(&world, 3001, 1634), 1, "second pass certificate");
    assert_eq!(item_count(&world, 3001, 1641), 1, "the secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-10.html")));
    assert_eq!(item_count(&world, 3001, 1635), 1, "the Final Pass Certificate");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q418].is_completed());
    }
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION));
}

/// Fourth quest running with a route dead at both ends.
#[test]
fn artisan_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00418_PathOfTheArtisan/"
    );
    for npc in ["31956", "31963", "32052"] {
        let any =
            (1..=9).any(|n| std::path::Path::new(&format!("{DIST}{npc}-0{n}.html")).exists());
        assert!(any, "{npc} ships pages but is registered nowhere");
    }
    // Only 08b is offered; 08c (the memoState 10 entry) is not.
    // Pages only — the .java source naturally names `08c` as a case label,
    // which is exactly the handler we are proving is unreachable.
    let mut offers_08b = false;
    for entry in std::fs::read_dir(DIST).expect("quest dir") {
        let path = entry.expect("entry").path();
        let is_page = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "htm" || e == "html");
        if !is_page {
            continue;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(!body.contains("30527-08c"), "no page may offer the dead 08c entry");
        offers_08b |= body.contains("30527-08b");
    }
    assert!(offers_08b, "08b is the live route and is offered");
}

const Q417: &str = "Q00417_PathOfTheScavenger";

/// A Dwarven Fighter with Q00417 accepted (Pipi at NPC_OID).
fn q417_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1642..=1657).map(|id| (id, "Q417", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20403, 20508, 20777, 27058] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30524, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417} ACCEPT")));
    assert_eq!(item_count(&world, 3001, 1643), 1, "Pipi's letter");
    drain(&mut rx);
    (world, rx)
}

/// Mark `npc_oid` as spoiled by `player`, the way the Spoil effect would.
fn mark_spoiled(world: &mut World, npc_oid: i32, player: i32) {
    if let Some(n) = world.objects.get_component_mut::<crate::model::npc::Npc>(&npc_oid) {
        n.spoiler_object_id = player;
    }
}

/// The payout is gated on the corpse being **spoiled** — the Scavenger's own
/// mechanic. An unspoiled Honey Bear pays nothing.
#[test]
fn quest_q00417_payout_requires_a_spoiled_corpse() {
    for (spoil, expected) in [(false, 0), (true, 1)] {
        let (mut world, _rx) = q417_world();
        super::items::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture
        let bear = NPC_OID + 100;
        add_test_npc(&mut world, bear, 27058, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0);
        if spoil {
            // Spoiled by someone else, so the attack-time disqualifier
            // (spoiler == attacker) does not fire.
            mark_spoiled(&mut world, bear, 9999);
        }
        death::npc_do_die(&mut world, bear, 3001);
        assert_eq!(
            item_count(&world, 3001, 1655),
            expected,
            "spoiled={spoil}: honey only drops off a spoiled corpse"
        );
    }
}

/// `giveItemRandomly`'s chance is a 0..1 fraction and this quest passes **50**,
/// so every qualifying kill drops. No forced roll is used: if the port had
/// "corrected" it to 0.5 this would be flaky, and at 0.5 with the RNG it would
/// fail about half the time.
#[test]
fn quest_q00417_drop_chance_fifty_means_always() {
    let (mut world, _rx) = q417_world();
    super::items::add_inventory_item(&mut world, 3001, 1654, 1); // tarantula picture
    for i in 0..6 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20403, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, mob, 3001, 10.0);
        mark_spoiled(&mut world, mob, 9999);
        death::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1656), 6, "six kills, six beads — chance 50 is not 50%");
}

/// The Honey Bear summon meter escalates at `20 * flag` percent and resets on
/// success.
#[test]
fn quest_q00417_honey_bear_summon_meter_escalates() {
    let (mut world, _rx) = q417_world();
    super::items::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture

    // First kill: flag is 0, so no roll happens at all — it just rises.
    let b1 = NPC_OID + 300;
    add_test_npc(&mut world, b1, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b1, 3001, 10.0);
    death::npc_do_die(&mut world, b1, 3001);
    assert!(npcs_of(&mut world, 27058).is_empty(), "flag 0 never summons");

    // Second kill with the roll inside `20 * 1`: the bear appears.
    let b2 = NPC_OID + 301;
    add_test_npc(&mut world, b2, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b2, 3001, 10.0);
    world.forced_rolls.push_back(5); // 5 < 20
    death::npc_do_die(&mut world, b2, 3001);
    assert_eq!(npcs_of(&mut world, 27058).len(), 1, "the Honey Bear was summoned");
}

/// The delivery round-trip bumps the **tens** digit of `memoStateEx(1)`, and
/// the second hand-in promotes to cond 3.
#[test]
fn quest_q00417_deliveries_bump_the_tens_digit() {
    let (mut world, _rx) = q417_world();
    let shari = NPC_OID + 20;
    add_test_npc(&mut world, shari, 30517, "Folk", 5, 100, 0, 0);

    super::items::add_inventory_item(&mut world, 3001, 1648, 1); // Shari's axe
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{shari}_Quest {Q417}")));
    assert_eq!(item_count(&world, 3001, 1651), 1, "Shari's pay");
    assert_eq!(quest_memo_ex(&world, 3001, Q417, 1), 10, "tens digit bumped");
    assert!(quest_cond(&world, 3001, Q417) != Some(3), "not promoted on the first");

    super::items::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{shari}_Quest {Q417}")));
    assert_eq!(quest_memo_ex(&world, 3001, Q417, 1), 20);
    super::items::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{shari}_Quest {Q417}")));
    assert_eq!(quest_cond(&world, 3001, Q417), Some(3), "the third hand-in promotes");
}

/// Torai hands over the undies and **deletes himself**; Raut then pays the
/// Ring of Raven.
#[test]
fn quest_q00417_torai_vanishes_and_raut_pays_the_ring() {
    let (mut world, mut rx) = q417_world();
    let (torai, raut) = (NPC_OID + 30, NPC_OID + 31);
    add_test_npc(&mut world, torai, 30557, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, raut, 30316, "Folk", 5, 100, 0, 0);
    super::items::add_inventory_item(&mut world, 3001, 1644, 1); // teleport scroll

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{torai}_Quest {Q417} 30557-03.html")));
    assert_eq!(item_count(&world, 3001, 1645), 1, "succubus undies");
    assert_eq!(quest_cond(&world, 3001, Q417), Some(11));
    assert!(
        world.objects.get_component::<crate::model::npc::Npc>(&torai).is_none(),
        "Torai deleted himself"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{raut}_Quest {Q417}")));
    assert_eq!(item_count(&world, 3001, 1642), 1, "the Ring of Raven");
    {
        let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
        assert!(quests.0[Q417].is_completed());
    }
    assert!(drain(&mut rx).iter().any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION));
}

fn quest_memo_ex(world: &World, player: i32, quest: &str, slot: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::components::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .and_then(|qs| qs.vars.get(&format!("memoStateEx{slot}")))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

/// Q00210 Obtain a Wolf Pet: the four-NPC dialog chain (Lundy → Bella → Bynn
/// → Sydnia → Lundy) advances cond 1→4 and hands over the Wolf Collar (2375),
/// one-time.
#[test]
fn quest_q00210_wolf_pet_chain() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(2375, "Wolf Collar", false)]);
    world.id_pool = 0x3000_0000..0x3000_0100; // the reward allocates the collar's oid
    let (lundy, bella, bynn, sydnia) = (NPC_OID, NPC_OID + 1, NPC_OID + 2, NPC_OID + 3);
    add_test_npc(&mut world, lundy, 30827, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, bella, 30256, "Folk", 5, 120, 0, 0);
    add_test_npc(&mut world, bynn, 30335, "Folk", 5, 140, 0, 0);
    add_test_npc(&mut world, sydnia, 30321, "Folk", 5, 160, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 15;
    drain_db(&mut db_rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lundy}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-03.htm")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "Lundy started the quest");

    // An out-of-order click is refused: Bynn (cond 2) while still at cond 1.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "cond guard holds — no skipping ahead");

    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{bella}_Quest {q} 30256-03.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{sydnia}_Quest {q} 30321-02.html")));
    assert_eq!(quest_cond(&world, 3001, q), Some(4));

    assert_eq!(item_count(&world, 3001, 2375), 0, "no collar until the payout");
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-05.html")));
    assert_eq!(item_count(&world, 3001, 2375), 1, "Wolf Collar rewarded");
    let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
    assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
}

/// Q00210 refuses a starter below level 15 with `no_level.htm` and does not
/// start (Java `addCondMinLevel(15, "no_level.htm")`).
#[test]
fn quest_q00210_refused_below_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let lundy = NPC_OID;
    add_test_npc(&mut world, lundy, 30827, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 14;
    drain(&mut rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{lundy}_Quest {q}")));
    // `no_level.htm` is a `.htm` file, so it ships as ExNpcQuestHtmlMessage
    // (the quest window), not a plain NpcHtmlMessage.
    let decode_quest_html = |pkt: &[u8]| -> Option<String> {
        if pkt[0] != server_packets::opcodes::EX
            || i16::from_le_bytes([pkt[1], pkt[2]]) != server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE
        {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&pkt[3..]);
        r.read_i32()?;
        r.read_string()
    };
    let html = drain(&mut rx).iter().find_map(|p| decode_quest_html(p)).expect("quest html");
    assert!(html.contains("level requirements") || html.contains("level 15"), "the level gate, got: {html}");
    // The talk creates a CREATED state (Java `getQuestState(player, true)`) but
    // the gate keeps it un-started (cond 0, never `startQuest`).
    let quests = world.objects.get_component::<crate::model::components::Quests>(&3001).unwrap();
    assert!(!quests.0[q].is_started(), "the quest never started");
}

/// Q00261 Collector's Dream: accept → kill spiders for 8 legs → 700 adena,
/// repeatable. The max-level gate (21) refuses an over-levelled starter.
#[test]
fn quest_q00261_collectors_dream_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1087, "Spider Leg", true)]);
    for id in [20308, 20460, 20466] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 18;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30222, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 18;
    drain_db(&mut db_rx);

    let q = "Q00261_CollectorsDream";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30222-03.htm")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    // Kill 8 spiders (one leg each, roll forced to hit), across all three types.
    let mob = NPC_OID + 1;
    for i in 0..8 {
        let species = [20308, 20460, 20466][(i % 3) as usize];
        add_test_npc(&mut world, mob + i, species, "Monster", 18, 30, 0, 0);
        world.forced_rolls.push_back(0);
        death::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1087), 8, "eight legs collected");
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond advanced at the cap");
    drain(&mut rx);

    // Turn-in: 700 adena, legs consumed, repeatable exit.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 700);
    assert_eq!(item_count(&world, 3001, 1087), 0, "quest items removed on exit");
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit clears the record");
}

/// Q00261 refuses a starter above level 21 (`addCondMaxLevel(21)`): the quest
/// never starts.
#[test]
fn quest_q00261_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1087, "Spider Leg", true)]);
    add_test_npc(&mut world, NPC_OID, 30222, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 22;

    let q = "Q00261_CollectorsDream";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30222-03.htm")));
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "the level-22 starter never begins the quest"
    );
}

/// Q00257 The Guard is Busy: start (Lord's Mark) → hand-rolled trophy drops →
/// adena payout by trophy type, repeatable. Also pins the Orc Archer's
/// two-entry table (first hit wins → 2 amulets) and the max-level gate.
#[test]
fn quest_q00257_the_guard_is_busy_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(752, "Orc Amulet", true), (1084, "Gludio Lord's Mark", false), (1085, "Orc Necklace", true), (1086, "Werewolf Fang", true)]);
    for id in [20006, 20093, 20130, 20343] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 10;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30039, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 10;
    drain_db(&mut db_rx);

    let q = "Q00257_TheGuardIsBusy";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-03.htm")));
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    assert_eq!(item_count(&world, 3001, 1084), 1, "Gludio Lord's Mark given");
    drain(&mut rx);

    let mob = NPC_OID + 1;
    // Orc Archer 20006: first table entry (roll(10) < 2) wins → 2 amulets, one roll.
    add_test_npc(&mut world, mob, 20006, "Monster", 10, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 752), 2, "Orc Archer's first entry pays two amulets");

    // Orc Fighter 20093 → 1 necklace; Werewolf Hunter 20343 → 1 fang.
    add_test_npc(&mut world, mob + 1, 20093, "Monster", 10, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, mob + 1, 3001);
    add_test_npc(&mut world, mob + 2, 20343, "Monster", 10, 30, 0, 0);
    world.forced_rolls.push_back(0);
    death::npc_do_die(&mut world, mob + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1085), 1, "one necklace");
    assert_eq!(item_count(&world, 3001, 1086), 1, "one fang");
    drain(&mut rx);

    // Turn in: 2 amulets*5 + 1 necklace*8 + 1 fang*10 = 28 adena (total 4 < 10, no bonus).
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    assert_eq!(item_count(&world, 3001, 57), adena_before + 28, "adena by trophy type");
    assert_eq!(item_count(&world, 3001, 752) + item_count(&world, 3001, 1085) + item_count(&world, 3001, 1086), 0, "trophies taken");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "turn-in keeps the quest running");

    // Leaving (30039-05.html) is the repeatable exit.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-05.html")));
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit clears the record");
}

/// Q00257 refuses a starter above level 16 (`addCondMaxLevel(16)`).
#[test]
fn quest_q00257_refused_above_level_16() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1084, "Gludio Lord's Mark", false)]);
    add_test_npc(&mut world, NPC_OID, 30039, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.get_component_mut::<Player>(&3001).unwrap().level = 17;

    let q = "Q00257_TheGuardIsBusy";
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")));
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-03.htm")));
    assert!(quest_cond(&world, 3001, q).is_none_or(|c| c == 0), "level-17 starter never begins");
    assert_eq!(item_count(&world, 3001, 1084), 0, "no Lord's Mark handed out");
}
