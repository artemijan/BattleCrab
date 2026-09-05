//! Q00257-Q00262 — the first collection quests: The Guard Is Busy, Bring
//! Wolf Pelts, Request from the Farm Owner, Collector's Dream and Trade with
//! the Ivory Tower.

use super::super::*;

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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 3;
    drain_db(&mut db_rx);

    // Talk: the single talk-quest short-circuits the chooser; CREATED at
    // level 3 → 30001-02.htm → the quest-window packet (FE:0x8E).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)),
        "quest window html"
    );

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00258_BringWolfPelts 30001-03.html"
        )),
    );
    let pkts = drain(&mut rx);
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.state, model::quest::state::STARTED);
        assert_eq!(qs.cond(), 1);
    }
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::QUEST_LIST),
        "QuestList after accept"
    );
    assert!(
        sound_names(&pkts).contains(&"ItemSound.quest_accept".to_string()),
        "accept sound"
    );
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_HTML_MESSAGE),
        ".html result uses the plain window"
    );
    // Memory-first: cond + state land in the Quests component (they persist on
    // the next flush, not per set).
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = &quests.0["Q00258_BringWolfPelts"];
        assert_eq!(qs.cond(), 1, "cond set in memory");
        assert_eq!(
            qs.state,
            model::quest::state::STARTED,
            "state Started in memory"
        );
    }

    // First wolf kill: one pelt, earned-SM, `InventoryUpdate`, itemget sound.
    let wolf = NPC_OID + 1;
    add_test_npc(&mut world, wolf, 20120, "Monster", 5, 30, 0, 0);
    npc::npc_do_die(&mut world, wolf, 3001);
    let pkts = drain(&mut rx);
    let inv_count = |world: &World| {
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(702)
    };
    assert_eq!(inv_count(&world), 1);
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1),
        "earned SM"
    );
    // Java refreshes the client purely through `InventoryUpdate`, and a
    // first-time item is change type 1 (add) — `PlayerInventory.addItem`'s
    // `addNewItem` arm. A brand-new stack announced as 2 (modify) names an
    // object id the client has no slot for.
    let iu = pkts
        .iter()
        .find(|p| p[0] == 0x21)
        .expect("InventoryUpdate for the first pelt");
    assert_eq!(
        i16::from_le_bytes(iu[1..3].try_into().unwrap()),
        1,
        "one entry"
    );
    assert_eq!(
        i16::from_le_bytes(iu[3..5].try_into().unwrap()),
        1,
        "change type 1 (add) for a newly created stack"
    );
    // No bare `ExQuestItemList`: Java only sends it behind a full `ItemList`
    // (`EnterWorld` / `sendItemList`). Sent alone it appends the whole quest tab
    // again, which is the duplicate-row-per-gain bug that clears on relog.
    assert!(
        !pkts
            .iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_QUEST_ITEM_LIST)),
        "no standalone ExQuestItemList on a quest item gain"
    );
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_itemget".to_string()));

    // 38 more pelts, then the 40th kill flips cond 2 (+ mark + middle).
    inventory::add_inventory_item(&mut world, 3001, 702, 38).unwrap();
    let wolf2 = NPC_OID + 2;
    add_test_npc(&mut world, wolf2, 20442, "Monster", 5, 30, 0, 0);
    npc::npc_do_die(&mut world, wolf2, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 40);
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert_eq!(quests.0["Q00258_BringWolfPelts"].cond(), 2);
    }
    let mark = pkts
        .iter()
        .find(|p| is_ex(p, server_packets::opcodes::EX_SHOW_QUEST_MARK))
        .expect("quest mark");
    assert_eq!(i32::from_le_bytes(mark[3..7].try_into().unwrap()), 258);
    assert_eq!(i32::from_le_bytes(mark[7..11].try_into().unwrap()), 2);
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()));
    // The mirror of the first kill: this pelt merged into a stack the client
    // already has, so change type 2 (modify) — Java's `addModifiedItem` arm.
    let iu = pkts
        .iter()
        .find(|p| p[0] == 0x21)
        .expect("InventoryUpdate for the 40th pelt");
    assert_eq!(
        i16::from_le_bytes(iu[3..5].try_into().unwrap()),
        2,
        "change type 2 (modify) when the stack already existed"
    );

    // Turn-in: roll 0 → Cloth Cap; pelts destroyed; repeatable exit.
    drain_db(&mut db_rx);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00258_BringWolfPelts")),
    );
    let pkts = drain(&mut rx);
    assert_eq!(inv_count(&world), 0, "pelts destroyed on exit");
    // `takeItems` retires the row with a change-type-3 `InventoryUpdate` entry;
    // no standalone quest-list refresh there either (Java `destroyItemByItemId`).
    assert!(
        !pkts
            .iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_QUEST_ITEM_LIST)),
        "no standalone ExQuestItemList on takeItems"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(41),
        1,
        "Cloth Cap rewarded on roll 0"
    );
    assert!(
        !world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0
            .contains_key("Q00258_BringWolfPelts"),
        "repeatable exit forgets the quest"
    );
    assert!(sound_names(&pkts).contains(&"ItemSound.quest_finish".to_string()));
    // The removal reaches the client as a removed-type InventoryUpdate.
    assert!(
        pkts.iter()
            .any(|p| p[0] == 0x21 && i16::from_le_bytes([p[3], p[4]]) == 3),
        "InventoryUpdate with change type 3 (removed)"
    );
    // Memory-first: the pelts are gone from the Inventory component and the
    // quest from the Quests component (both asserted above); the flush reconcile
    // deletes their rows — no per-action DB write.

    // Re-talk: the quest is takeable again (CREATED intro window).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));
    let pkts = drain(&mut rx);
    assert!(
        pkts.iter()
            .any(|p| is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE)),
        "repeatable re-offer"
    );
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    drain_db(&mut db_rx);

    let q = "Q00261_CollectorsDream";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30222-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    // Kill 8 spiders (one leg each, roll forced to hit), across all three types.
    let mob = NPC_OID + 1;
    for i in 0..8 {
        let species = [20308, 20460, 20466][(i % 3) as usize];
        add_test_npc(&mut world, mob + i, species, "Monster", 18, 30, 0, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1087), 8, "eight legs collected");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "cond advanced at the cap"
    );
    drain(&mut rx);

    // Turn-in: 700 adena, legs consumed, repeatable exit.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 57), adena_before + 700);
    assert_eq!(
        item_count(&world, 3001, 1087),
        0,
        "quest items removed on exit"
    );
    assert!(
        quest_cond(&world, 3001, q).is_none(),
        "repeatable exit clears the record"
    );

    // `giveNewbieReward`: the GUIDE_MISSION digit is seeded and the "last duty
    // complete" banner shown. The variable has no reader in this port yet, but
    // it persists, so the credit survives until the newbie-guide UI lands.
    let guide_var = |w: &World| {
        w.objects
            .get_component::<model::components::player::PlayerVariables>(&3001)
            .and_then(|v| v.0.get("GUIDE_MISSION").cloned())
    };
    assert_eq!(
        guide_var(&world).as_deref(),
        Some("100000"),
        "GUIDE_MISSION seeded on the first award"
    );

    // The banner is an ExShowScreenMessage carrying NpcStringId 4155 rather
    // than wire text — the tail after the npcString field must be *empty*,
    // since Java writes the literal only when npcString == -1.
    let banner = |pkts: &[Vec<u8>]| -> Vec<i32> {
        pkts.iter()
            .filter(|p| is_ex(p, server_packets::opcodes::EX_SHOW_SCREEN_MESSAGE))
            .map(|p| {
                let mut r = commons::network::PacketReader::new(&p[3..]);
                for _ in 0..10 {
                    let _ = r.read_i32();
                }
                r.read_i32().unwrap() // npcString
            })
            .collect()
    };

    // rx still holds the turn-in burst, so this is the first award's banner.
    assert_eq!(
        banner(&drain(&mut rx)),
        vec![4155],
        "LAST_DUTY_COMPLETE_N_GO_FIND_THE_NEWBIE_HELPER on the first award"
    );

    // Do the whole loop again: the quest is repeatable, but the newbie credit
    // is not. Java's `!= 1` guard means the second completion sets nothing and
    // — this is the part worth pinning — sends no banner either.
    for i in 8..16 {
        let species = [20308, 20460, 20466][(i % 3) as usize];
        add_test_npc(&mut world, mob + i, species, "Monster", 18, 30, 0, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30222-03.htm")),
    );
    let second = drain(&mut rx);
    assert_eq!(
        guide_var(&world).as_deref(),
        Some("100000"),
        "the digit is credited once, not once per run"
    );
    assert!(
        banner(&second).is_empty(),
        "no second banner: Java's != 1 guard skips the whole branch"
    );
}

/// Q00257 The Guard is Busy: start (Lord's Mark) → hand-rolled trophy drops →
/// adena payout by trophy type, repeatable. Also pins the Orc Archer's
/// two-entry table (first hit wins → 2 amulets) and the max-level gate.
#[test]
fn quest_q00257_the_guard_is_busy_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (752, "Orc Amulet", true),
            (1084, "Gludio Lord's Mark", false),
            (1085, "Orc Necklace", true),
            (1086, "Werewolf Fang", true),
        ],
    );
    for id in [20006, 20093, 20130, 20343] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 10;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30039, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 10;
    drain_db(&mut db_rx);

    let q = "Q00257_TheGuardIsBusy";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    assert_eq!(
        item_count(&world, 3001, 1084),
        1,
        "Gludio Lord's Mark given"
    );
    drain(&mut rx);

    let mob = NPC_OID + 1;
    // Orc Archer 20006: first table entry (roll(10) < 2) wins → 2 amulets, one roll.
    add_test_npc(&mut world, mob, 20006, "Monster", 10, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(
        item_count(&world, 3001, 752),
        2,
        "Orc Archer's first entry pays two amulets"
    );

    // Orc Fighter 20093 → 1 necklace; Werewolf Hunter 20343 → 1 fang.
    add_test_npc(&mut world, mob + 1, 20093, "Monster", 10, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob + 1, 3001);
    add_test_npc(&mut world, mob + 2, 20343, "Monster", 10, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1085), 1, "one necklace");
    assert_eq!(item_count(&world, 3001, 1086), 1, "one fang");
    drain(&mut rx);

    // Turn in: 2 amulets*5 + 1 necklace*8 + 1 fang*10 = 28 adena (total 4 < 10, no bonus).
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 28,
        "adena by trophy type"
    );
    assert_eq!(
        item_count(&world, 3001, 752)
            + item_count(&world, 3001, 1085)
            + item_count(&world, 3001, 1086),
        0,
        "trophies taken"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "turn-in keeps the quest running"
    );

    // Leaving (30039-05.html) is the repeatable exit.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-05.html")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none(),
        "repeatable exit clears the record"
    );
}

/// Q00259 Request from the Farm Owner — the Edmond adena path: kill spiders for
/// skins, hand them in for 25a each (+250 for 10+), repeatable.
#[test]
fn quest_q00259_edmond_adena_path() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1495, "Spider Skin", true)]);
    for id in [20103, 20106, 20108] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 18;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30497, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    drain_db(&mut db_rx);

    let q = "Q00259_RequestFromTheFarmOwner";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30497-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mob = NPC_OID + 1;
    for i in 0..10 {
        add_test_npc(
            &mut world,
            mob + i,
            [20103, 20106, 20108][(i % 3) as usize],
            "Monster",
            18,
            30,
            0,
            0,
        );
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 1495),
        10,
        "one skin per kill (unrolled)"
    );

    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 500,
        "10*25 + 250 bonus"
    );
    assert_eq!(item_count(&world, 3001, 1495), 0, "skins handed in");

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30497-06.html")),
    );
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}

/// The Marius branch: 10 skins trade for a batch of consumables (Greater
/// Healing Potions) instead of adena, and the skins are consumed.
#[test]
fn quest_q00259_marius_consumables_path() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1495, "Spider Skin", true),
            (1061, "Greater Healing Potion", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20103);
    t.type_name = "Monster".into();
    t.level = 18;
    world.data.npc_data.insert_for_test(t);
    let edmond = NPC_OID;
    let marius = NPC_OID + 1;
    add_test_npc(&mut world, edmond, 30497, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, marius, 30405, "Folk", 5, 120, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    drain_db(&mut db_rx);

    let q = "Q00259_RequestFromTheFarmOwner";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{edmond}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{edmond}_Quest {q} 30497-03.html")),
    );
    let mob = NPC_OID + 2;
    for i in 0..10 {
        add_test_npc(&mut world, mob + i, 20103, "Monster", 18, 30, 0, 0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1495), 10);
    drain(&mut rx);

    // Trade the batch at Marius for 2 Greater Healing Potions.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{marius}_Quest {q} 30405-04.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1061),
        2,
        "two potions from the trade"
    );
    assert_eq!(
        item_count(&world, 3001, 1495),
        0,
        "ten skins consumed by the trade"
    );
}

/// Q00262 Trade with the Ivory Tower: the rate-in-threshold drop
/// (`roll(10) < base`, distinct per mob), the cond flip at 10, and the 300a
/// turn-in.
#[test]
fn quest_q00262_ivory_tower_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(707, "Spore Sac", true)]);
    for id in [20007, 20400] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 12;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30137, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 12;
    drain_db(&mut db_rx);

    let q = "Q00262_TradeWithTheIvoryTower";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30137-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mut mob = NPC_OID + 1;
    let mut kill = |world: &mut World, species: i32, roll: i32| {
        add_test_npc(world, mob, species, "Monster", 12, 30, 0, 0);
        world.force_roll(roll);
        npc::npc_do_die(world, mob, 3001);
        mob += 1;
    };
    kill(&mut world, 20007, 2); // Green base 3: 2 < 3 → drop
    kill(&mut world, 20007, 5); // 5 ≥ 3 → nothing
    kill(&mut world, 20400, 3); // Blood base 4: 3 < 4 → drop (would be nothing on Green)
    assert_eq!(
        item_count(&world, 3001, 707),
        2,
        "the per-mob thresholds differ (3 vs 4)"
    );

    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .add_item(&data.item_data, 0x5700_0000, 707, 7);
    }
    kill(&mut world, 20007, 0); // 10th sac → cond 2
    assert_eq!(item_count(&world, 3001, 707), 10);
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at 10");
    drain(&mut rx);

    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 300,
        "300 adena"
    );
    assert_eq!(item_count(&world, 3001, 707), 0, "sacs cleared");
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}
