//! Q00250-Q00297 — the repeatable starter collection quests.

use super::*;

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

/// Q00261 refuses a starter above level 21 (`addCondMaxLevel(21)`): the quest
/// never starts.
#[test]
fn quest_q00261_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1087, "Spider Leg", true)]);
    add_test_npc(&mut world, NPC_OID, 30222, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;

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

/// Q00257 refuses a starter above level 16 (`addCondMaxLevel(16)`).
#[test]
fn quest_q00257_refused_above_level_16() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1084, "Gludio Lord's Mark", false)]);
    add_test_npc(&mut world, NPC_OID, 30039, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 17;

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
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-17 starter never begins"
    );
    assert_eq!(
        item_count(&world, 3001, 1084),
        0,
        "no Lord's Mark handed out"
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

/// Q00259 refuses a starter above level 21 (`addCondMaxLevel(21)`).
#[test]
fn quest_q00259_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1495, "Spider Skin", true)]);
    add_test_npc(&mut world, NPC_OID, 30497, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;

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
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-22 starter never begins"
    );
}

/// Q00293 The Hidden Veins — the full Dwarf loop: kill for ores + rare map
/// fragments, craft 4 fragments into a Hidden Ore Map at Chichirin, hand the
/// lot to Filaur for adena (ore 5a, map 150a).
#[test]
fn quest_q00293_hidden_veins_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1488, "Chrysolite Ore", true),
            (1489, "Torn Map Fragment", true),
            (1490, "Hidden Ore Map", true),
        ],
    );
    for id in [20446, 20447, 20448] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 10;
        world.data.npc_data.insert_for_test(t);
    }
    let (filaur, chichirin) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, filaur, 30535, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, chichirin, 30539, "Folk", 5, 120, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 4; // Dwarf
    }
    drain_db(&mut db_rx);

    let q = "Q00293_TheHiddenVeins";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{filaur}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{filaur}_Quest {q} 30535-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    // One getRandom(100) per kill: 4 fragments (roll 2 < 5), 3 ores (roll 60 > 50).
    let mob = NPC_OID + 2;
    for i in 0..4 {
        add_test_npc(&mut world, mob + i, 20446, "Monster", 10, 30, 0, 0);
        world.force_roll(2);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    for i in 4..7 {
        add_test_npc(&mut world, mob + i, 20447, "Monster", 10, 30, 0, 0);
        world.force_roll(60);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1489), 4, "four map fragments");
    assert_eq!(item_count(&world, 3001, 1488), 3, "three ores");

    // Craft the fragments into a Hidden Ore Map at Chichirin.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{chichirin}_Quest {q} 30539-03.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1490),
        1,
        "one Hidden Ore Map crafted"
    );
    assert_eq!(item_count(&world, 3001, 1489), 0, "four fragments consumed");

    // Hand in at Filaur: 3 ores * 5 + 1 map * 150 = 165 adena (4 items < 10, no bonus).
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{filaur}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 165,
        "ore 5a + map 150a"
    );
    assert_eq!(
        item_count(&world, 3001, 1488) + item_count(&world, 3001, 1490),
        0,
        "ores + maps handed in"
    );
}

/// The Dwarf-only race gate: a non-Dwarf sees a different Filaur page than a
/// Dwarf in the CREATED state (`30535-01.htm` vs `30535-03.htm`).
#[test]
fn quest_q00293_race_gate() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1488, "Chrysolite Ore", true)]);
    add_test_npc(&mut world, NPC_OID, 30535, "Folk", 5, 100, 0, 0);
    let mut dwarf_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut human_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    {
        let d = world.objects.get_component_mut::<Player>(&3001).unwrap();
        d.level = 10;
        d.race = 4; // Dwarf
    }
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 10; // Human (race 0)
    drain(&mut dwarf_rx);
    drain(&mut human_rx);

    fn quest_html(rx: &mut UnboundedReceiver<bytes::Bytes>) -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
                is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE).then(|| {
                    let mut r = commons::network::PacketReader::new(&p[3..]);
                    r.read_i32();
                    r.read_string().unwrap_or_default()
                })
            })
            .unwrap_or_default()
    }

    let q = "Q00293_TheHiddenVeins";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    let dwarf_page = quest_html(&mut dwarf_rx);
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    let human_page = quest_html(&mut human_rx);

    assert!(
        !dwarf_page.is_empty() && !human_page.is_empty(),
        "both got a page"
    );
    assert_ne!(
        dwarf_page, human_page,
        "the Dwarf and non-Dwarf see different Filaur pages"
    );
}

/// Q00293 refuses a starter above level 15 (`addCondMaxLevel(15)`).
#[test]
fn quest_q00293_refused_above_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1488, "Chrysolite Ore", true)]);
    add_test_npc(&mut world, NPC_OID, 30535, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 16;
        p.race = 4;
    }
    let q = "Q00293_TheHiddenVeins";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30535-04.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-16 starter never begins"
    );
}

/// Q00296 Tarantula's Spider Silk: the rare spinnerette drop, Nathan spinning
/// each spinnerette into 15+rnd(9) silk, and Mion's adena turn-in.
#[test]
fn quest_q00296_spider_silk_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1493, "Tarantula Spider Silk", true),
            (1494, "Tarantula Spinnerette", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20394);
    t.type_name = "Monster".into();
    t.level = 18;
    world.data.npc_data.insert_for_test(t);
    let (mion, nathan) = (NPC_OID, NPC_OID + 1);
    add_test_npc(&mut world, mion, 30519, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, nathan, 30548, "Folk", 5, 120, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    drain_db(&mut db_rx);

    let q = "Q00296_TarantulasSpiderSilk";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{mion}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{mion}_Quest {q} 30519-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mob = NPC_OID + 2;
    // 2 rare spinnerettes (gate roll 96 > 95, then the give_item_randomly roll).
    for i in 0..2 {
        add_test_npc(&mut world, mob + i, 20394, "Monster", 18, 30, 0, 0);
        world.force_roll(96);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    // 3 plain silks (gate 50).
    for i in 2..5 {
        add_test_npc(&mut world, mob + i, 20394, "Monster", 18, 30, 0, 0);
        world.force_roll(50);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1494), 2, "two spinnerettes");
    assert_eq!(item_count(&world, 3001, 1493), 3, "three silks");
    drain(&mut rx);

    // Nathan spins: (15 + rnd(9)=0) * 2 spinnerettes = 30 silk; spinnerettes consumed.
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{nathan}_Quest {q} 30548-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1493), 33, "3 + 15*2 silk");
    assert_eq!(item_count(&world, 3001, 1494), 0, "spinnerettes consumed");

    // Spinning again with none does nothing (30548-02).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{nathan}_Quest {q} 30548-03.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1493),
        33,
        "no silk added without a spinnerette"
    );

    // Mion pays 5a per silk (+1000 for 10+): 33*5 + 1000 = 1165.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{mion}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 1165,
        "silk turn-in"
    );
    assert_eq!(item_count(&world, 3001, 1493), 0, "silk handed in");
}

/// Q00296 refuses a starter above level 21 (`addCondMaxLevel(21)`).
#[test]
fn quest_q00296_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1493, "Silk", true)]);
    add_test_npc(&mut world, NPC_OID, 30519, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;
    let q = "Q00296_TarantulasSpiderSilk";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30519-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-22 starter never begins"
    );
}

/// Q00266 Pleas of Pixies: the per-mob variable-amount `getRandom(10)` drop,
/// the limit-100 cond flip, and the (inverted) jackpot reward at bucket 0.
#[test]
fn quest_q00266_pixies_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1334, "Predator's Fang", true), (1336, "Glass Shard", true)],
    );
    for id in [20537, 20525] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 5;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 31852, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 5;
        p.race = 1; // Elf
    }
    drain_db(&mut db_rx);

    let q = "Q00266_PleasOfPixies";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31852-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    // Gray Wolf two-entry table: gate 3 (<5) → 2 fangs; gate 7 (5..10) → 3 fangs.
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, mob, 20525, "Monster", 5, 30, 0, 0);
    world.force_roll(3);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob, 3001);
    add_test_npc(&mut world, mob + 1, 20525, "Monster", 5, 30, 0, 0);
    world.force_roll(7);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 1334),
        5,
        "2 + 3 fangs from the two gate buckets"
    );

    // Inject up to 98, then an Elder Red Keltir (gives 2) hits the 100 cap → cond 2.
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .add_item(&data.item_data, 0x5100_0000, 1334, 93);
    }
    add_test_npc(&mut world, mob + 2, 20537, "Monster", 5, 30, 0, 0);
    world.force_roll(0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1334), 100);
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at 100 fangs");
    drain(&mut rx);

    // Turn in with the reward roll < 2 → bucket 0 (Glass Shard + 100a, jackpot chime).
    let adena_before = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 1336), 1, "Glass Shard");
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 100,
        "100 adena"
    );
    assert_eq!(item_count(&world, 3001, 1334), 0, "fangs consumed");
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}

/// The reward-roll buckets: 20..45 → Blue Onyx + 500a, 45+ → Emerald + 5000a
/// (the common case), driven through repeatable re-runs.
#[test]
fn quest_q00266_reward_buckets() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1334, "Fang", true),
            (1338, "Blue Onyx", true),
            (1337, "Emerald", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20537);
    t.type_name = "Monster".into();
    t.level = 5;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31852, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 5;
        p.race = 1;
    }
    let q = "Q00266_PleasOfPixies";
    let mob = NPC_OID + 1;
    for (mi, (roll, item, adena)) in [(30, 1338, 500), (60, 1337, 5000)].into_iter().enumerate() {
        let (obj, mi) = (0x5200_0000 + mi as i32, mi as i32);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
        );
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31852-04.htm")),
        );
        {
            let World { objects, data, .. } = &mut world;
            objects
                .get_component_mut::<Inventory>(&3001)
                .unwrap()
                .add_item(&data.item_data, obj, 1334, 98);
        }
        add_test_npc(&mut world, mob + mi, 20537, "Monster", 5, 30, 0, 0);
        world.force_roll(0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + mi, 3001);
        assert_eq!(quest_cond(&world, 3001, q), Some(2));
        let adena_before = item_count(&world, 3001, 57);
        world.force_roll(roll);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
        );
        assert_eq!(
            item_count(&world, 3001, item),
            1,
            "roll {roll} → item {item}"
        );
        assert_eq!(
            item_count(&world, 3001, 57),
            adena_before + adena,
            "roll {roll} → {adena}a"
        );
    }
}

/// Q00266 is Elf-only and level 3–8: a non-Elf sees `31852-01.htm`, and a
/// level-9 Elf is refused by `addCondMaxLevel(8)`.
#[test]
fn quest_q00266_race_and_level_gates() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1334, "Fang", true)]);
    add_test_npc(&mut world, NPC_OID, 31852, "Folk", 5, 100, 0, 0);
    let mut elf_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut human_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    {
        let e = world.objects.get_component_mut::<Player>(&3001).unwrap();
        e.level = 5;
        e.race = 1;
    }
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 5; // Human
    drain(&mut elf_rx);
    drain(&mut human_rx);

    let q = "Q00266_PleasOfPixies";
    let quest_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
                is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE).then(|| {
                    let mut r = commons::network::PacketReader::new(&p[3..]);
                    r.read_i32();
                    r.read_string().unwrap_or_default()
                })
            })
            .unwrap_or_default()
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_ne!(
        quest_html(&mut elf_rx),
        quest_html(&mut human_rx),
        "Elf and Human see different pages"
    );

    // A fresh level-9 Elf: `addCondMaxLevel(8)` blocks the start-npc talk from
    // ever creating the state, so the start event has nothing to start.
    let _rx3 = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    {
        let e = world.objects.get_component_mut::<Player>(&3003).unwrap();
        e.level = 9;
        e.race = 1;
    }
    handle_request_bypass_to_server(
        &mut world,
        3,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        3,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31852-04.htm")),
    );
    assert!(
        quest_cond(&world, 3003, q).is_none_or(|c| c == 0),
        "level-9 Elf refused"
    );
}

/// Q00271 Proof of Valor: the 25%-double-drop capped so it can't overshoot 50,
/// the cond flip at 50, and the necklace (+13% potion) reward.
#[test]
fn quest_q00271_proof_of_valor_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1473, "Kasha Wolf Fang", true),
            (1507, "Necklace of Valor", false),
            (1539, "Healing Potion", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20475);
    t.type_name = "Monster".into();
    t.level = 6;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30577, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 5;
        p.race = 3; // Orc
    }
    drain_db(&mut db_rx);

    let q = "Q00271_ProofOfValor";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30577-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mob = NPC_OID + 1;
    // roll 10 (<25) at count 0 → double drop; roll 50 → single.
    add_test_npc(&mut world, mob, 20475, "Monster", 6, 30, 0, 0);
    world.force_roll(10);
    npc::npc_do_die(&mut world, mob, 3001);
    add_test_npc(&mut world, mob + 1, 20475, "Monster", 6, 30, 0, 0);
    world.force_roll(50);
    npc::npc_do_die(&mut world, mob + 1, 3001);
    assert_eq!(item_count(&world, 3001, 1473), 3, "2 + 1 fangs");

    // Fill to 49, then a <25 roll still gives ONE (count 49 is not < 49) → exactly 50, cond 2.
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .add_item(&data.item_data, 0x5300_0000, 1473, 46);
    }
    add_test_npc(&mut world, mob + 2, 20475, "Monster", 6, 30, 0, 0);
    world.force_roll(10);
    npc::npc_do_die(&mut world, mob + 2, 3001);
    assert_eq!(
        item_count(&world, 3001, 1473),
        50,
        "the double-drop cap held at 49"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at 50");
    drain(&mut rx);

    // Turn in with the 13% roll hitting → necklace + potion.
    world.force_roll(5);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 1507), 1, "Necklace of Valor");
    assert_eq!(
        item_count(&world, 3001, 1539),
        1,
        "Healing Potion (13% roll hit)"
    );
    assert_eq!(item_count(&world, 3001, 1473), 0, "fangs consumed");
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}

/// Gates: non-Orc / necklace-held pages differ, and a fresh level-9 Orc is
/// refused (the `30577-02.htm` page from `addCondMaxLevel`).
#[test]
fn quest_q00271_gates_and_necklace_page() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1473, "Fang", true), (1507, "Necklace of Valor", false)],
    );
    add_test_npc(&mut world, NPC_OID, 30577, "Folk", 5, 100, 0, 0);
    let mut orc_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut necklace_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let mut human_rx = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    for (oid, race) in [(3001, 3), (3002, 3), (3003, 0)] {
        let p = world.objects.get_component_mut::<Player>(&oid).unwrap();
        p.level = 5;
        p.race = race;
    }
    {
        // Player 3002 already owns the necklace.
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3002)
            .unwrap()
            .add_item(&data.item_data, 0x5400_0000, 1507, 1);
    }
    for rx in [&mut orc_rx, &mut necklace_rx, &mut human_rx] {
        drain(rx);
    }

    let q = "Q00271_ProofOfValor";
    let page = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
                is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE).then(|| {
                    let mut r = commons::network::PacketReader::new(&p[3..]);
                    r.read_i32();
                    r.read_string().unwrap_or_default()
                })
            })
            .unwrap_or_default()
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        3,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    let (orc, necklace, human) = (
        page(&mut orc_rx),
        page(&mut necklace_rx),
        page(&mut human_rx),
    );
    assert!(
        !orc.is_empty() && orc != human,
        "non-Orc sees a different page"
    );
    assert_ne!(orc, necklace, "necklace-held Orc sees a different page");

    // A fresh level-9 Orc: refused before the state is created.
    let _rx4 = ingame_player(&mut world, 4, 3004, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3004).unwrap();
        p.level = 9;
        p.race = 3;
    }
    handle_request_bypass_to_server(
        &mut world,
        4,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        4,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30577-04.htm")),
    );
    assert!(
        quest_cond(&world, 3004, q).is_none_or(|c| c == 0),
        "level-9 Orc refused"
    );
}

/// Q00277 Gatekeeper's Offering: collect 20 starstones (unrolled, capped) for
/// 2 Gatekeeper Charms; the min-level gate lives in the start event.
#[test]
fn quest_q00277_gatekeepers_offering_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1572, "Starstone", true), (1658, "Gatekeeper Charm", true)],
    );
    let mut t = crate::data::npc_data::default_template(20333);
    t.type_name = "Monster".into();
    t.level = 18;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30576, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    drain_db(&mut db_rx);

    let q = "Q00277_GatekeepersOffering";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30576-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    // Inject 19, then one golem kill hits the cap → cond 2.
    {
        let World { objects, data, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .add_item(&data.item_data, 0x5500_0000, 1572, 19);
    }
    add_test_npc(&mut world, NPC_OID + 1, 20333, "Monster", 18, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 1572), 20);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "cond 2 at 20 starstones"
    );

    // A further kill past the cap adds nothing.
    add_test_npc(&mut world, NPC_OID + 2, 20333, "Monster", 18, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1572), 20, "capped at 20");

    // Turn in: 2 charms, starstones cleared by the repeatable exit.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 1658), 2, "two Gatekeeper Charms");
    assert_eq!(
        item_count(&world, 3001, 1572),
        0,
        "starstones removed on exit"
    );
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}

/// The start-event min-level gate (`30576-01.htm`, not a talk gate) and the
/// `addCondMaxLevel(21)` max-level gate.
#[test]
fn quest_q00277_level_gates() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1572, "Starstone", true)]);
    add_test_npc(&mut world, NPC_OID, 30576, "Folk", 5, 100, 0, 0);
    let q = "Q00277_GatekeepersOffering";

    // A level-14 player reaches the start button (the talk has no level gate)
    // but the event refuses with 30576-01.htm and does not start.
    let mut low_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 14;
    drain(&mut low_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30576-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-14 start refused by the event"
    );

    // A fresh level-22 player is blocked before the state is even created.
    let _hi_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 22;
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30576-03.htm")),
    );
    assert!(
        quest_cond(&world, 3002, q).is_none_or(|c| c == 0),
        "level-22 refused by addCondMaxLevel"
    );
}

/// Q00295 Dreaming of the Skies: the variable amount (1 or 2) capped at 50, the
/// first-time Ring of Firefly reward, and the repeat-run 200-adena branch.
#[test]
fn quest_q00295_dreaming_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1492, "Floating Stone", true),
            (1509, "Ring of Firefly", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20153);
    t.type_name = "Monster".into();
    t.level = 13;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30536, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 13;
    drain_db(&mut db_rx);

    let q = "Q00295_DreamingOfTheSkies";
    let mut obj = 0x5600_0000;
    let mut mob = NPC_OID + 1;

    // Helper: fill to 48 by injection then a double-drop kill closes to 50 → cond 2.
    let start_and_fill = |world: &mut World, obj: &mut i32, mob: &mut i32| {
        handle_request_bypass_to_server(
            world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
        );
        handle_request_bypass_to_server(
            world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30536-03.htm")),
        );
        {
            let World { objects, data, .. } = world;
            objects
                .get_component_mut::<Inventory>(&3001)
                .unwrap()
                .add_item(&data.item_data, *obj, 1492, 48);
        }
        *obj += 1;
        add_test_npc(world, *mob, 20153, "Monster", 13, 30, 0, 0);
        world.force_roll(10); // <=25 → amount 2
        world.force_roll(0); // give_item_randomly roll
        npc::npc_do_die(world, *mob, 3001);
        *mob += 1;
        assert_eq!(quest_cond(world, 3001, q), Some(2), "cond 2 at 50 stones");
    };

    // First run: earn the Ring of Firefly.
    start_and_fill(&mut world, &mut obj, &mut mob);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1509),
        1,
        "first run: Ring of Firefly"
    );
    assert_eq!(item_count(&world, 3001, 1492), 0, "stones cleared");

    // Second run (ring already held): 200 adena instead of a second ring.
    let adena_before = item_count(&world, 3001, 57);
    start_and_fill(&mut world, &mut obj, &mut mob);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 1509), 1, "still just one ring");
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 200,
        "repeat run pays 200 adena"
    );
    let _ = &mut rx;
}

/// Q00295 refuses a starter above level 15 (`addCondMaxLevel(15)`).
#[test]
fn quest_q00295_refused_above_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1492, "Floating Stone", true)]);
    add_test_npc(&mut world, NPC_OID, 30536, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 16;
    let q = "Q00295_DreamingOfTheSkies";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30536-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-16 starter never begins"
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

/// Q00262 refuses a starter above level 16 (`addCondMaxLevel(16)`).
#[test]
fn quest_q00262_refused_above_level_16() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(707, "Spore Sac", true)]);
    add_test_npc(&mut world, NPC_OID, 30137, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 17;
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
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-17 starter never begins"
    );
}

/// Q00267 Wrath of Verdure: the flat 50% club drop, the `2 + count` adena
/// formula (turn-in without leaving), and the separate exit.
#[test]
fn quest_q00267_wrath_of_verdure_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1335, "Goblin Club", true)]);
    let mut t = crate::data::npc_data::default_template(20325);
    t.type_name = "Monster".into();
    t.level = 6;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31853, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 6;
        p.race = 1; // Elf
    }
    drain_db(&mut db_rx);

    let q = "Q00267_WrathOfVerdure";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31853-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mut mob = NPC_OID + 1;
    let mut kill = |world: &mut World, roll: i32| {
        add_test_npc(world, mob, 20325, "Monster", 6, 30, 0, 0);
        world.force_roll(roll);
        npc::npc_do_die(world, mob, 3001);
        mob += 1;
    };
    kill(&mut world, 2); // < 5 → club
    kill(&mut world, 7); // ≥ 5 → nothing
    kill(&mut world, 0); // → club
    assert_eq!(
        item_count(&world, 3001, 1335),
        2,
        "two clubs from three kills"
    );

    // Turn in: 2 + 2 clubs = 4 adena, clubs taken, quest STILL running.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 4,
        "2 + club count"
    );
    assert_eq!(item_count(&world, 3001, 1335), 0, "clubs handed in");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "turn-in does not end the quest"
    );

    // Leaving is a separate event.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31853-07.html")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none(),
        "the leave event exits"
    );
}

/// Q00267 is Elf-only (non-Elf → `31853-01.htm`) and refuses above level 9.
#[test]
fn quest_q00267_race_and_level_gates() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1335, "Goblin Club", true)]);
    add_test_npc(&mut world, NPC_OID, 31853, "Folk", 5, 100, 0, 0);
    let mut elf_rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut human_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    {
        let e = world.objects.get_component_mut::<Player>(&3001).unwrap();
        e.level = 6;
        e.race = 1;
    }
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 6;
    drain(&mut elf_rx);
    drain(&mut human_rx);

    let q = "Q00267_WrathOfVerdure";
    let page = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
                is_ex(p, server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE).then(|| {
                    let mut r = commons::network::PacketReader::new(&p[3..]);
                    r.read_i32();
                    r.read_string().unwrap_or_default()
                })
            })
            .unwrap_or_default()
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_ne!(
        page(&mut elf_rx),
        page(&mut human_rx),
        "Elf and Human see different pages"
    );

    let _rx3 = ingame_player(&mut world, 3, 3003, 0, 0, 0);
    {
        let e = world.objects.get_component_mut::<Player>(&3003).unwrap();
        e.level = 10;
        e.race = 1;
    }
    handle_request_bypass_to_server(
        &mut world,
        3,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        3,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31853-04.htm")),
    );
    assert!(
        quest_cond(&world, 3003, q).is_none_or(|c| c == 0),
        "level-10 Elf refused"
    );
}

// ===== G22 quest batch (Q297/272/328/331/294/274/326) =====

#[test]
fn quest_q00297_gatekeepers_favor() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1573, "Starstone", true), (736, "Gatekeeper Token", true)],
    );
    let mut t = crate::data::npc_data::default_template(20521);
    t.type_name = "Monster".into();
    t.level = 18;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30540, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 18;
    let q = "Q00297_GatekeepersFavor";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30540-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    inject(&mut world, 3001, 0x6000_0000, 1573, 19);
    add_test_npc(&mut world, NPC_OID + 1, 20521, "Monster", 18, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 736), 2, "two Gatekeeper Tokens");
    assert!(quest_cond(&world, 3001, q).is_none());
}

#[test]
fn quest_q00272_wrath_of_ancestors() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(1474, "Grave Robber's Head", true)]);
    let mut t = crate::data::npc_data::default_template(20319);
    t.type_name = "Monster".into();
    t.level = 8;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30572, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 8;
        p.race = 3;
    }
    let q = "Q00272_WrathOfAncestors";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30572-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    inject(&mut world, 3001, 0x6100_0000, 1474, 49);
    add_test_npc(&mut world, NPC_OID + 1, 20319, "Monster", 8, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 57), a + 100);
    assert!(quest_cond(&world, 3001, q).is_none());
}

#[test]
fn quest_q00294_covert_business() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1491, "Bat Fang", true), (1508, "Ring of Raccoon", false)],
    );
    let mut t = crate::data::npc_data::default_template(20370);
    t.type_name = "Monster".into();
    t.level = 12;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30534, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 12;
        p.race = 4;
    }
    let q = "Q00294_CovertBusiness";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30534-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    inject(&mut world, 3001, 0x6200_0000, 1491, 96);
    // 20370 table [6,3,1,-1], roll 0 → count 4 → 96+4 = 100 → cond 2.
    add_test_npc(&mut world, NPC_OID + 1, 20370, "Monster", 12, 30, 0, 0);
    world.force_roll(0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at 100 fangs");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 1508), 1, "Ring of Raccoon");
    assert_eq!(item_count(&world, 3001, 1491), 0);
}

#[test]
fn quest_q00274_skirmish_with_the_werewolves() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1477, "Werewolf Head", true),
            (1501, "Totem", true),
            (1507, "Necklace of Valor", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20363);
    t.type_name = "Monster".into();
    t.level = 12;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30569, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 12;
        p.race = 3;
    }
    inject(&mut world, 3001, 0x6300_0000, 1507, 1); // Necklace of Valor gates the start
    let q = "Q00274_SkirmishWithTheWerewolves";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30569-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    inject(&mut world, 3001, 0x6301_0000, 1477, 39);
    add_test_npc(&mut world, NPC_OID + 1, 20363, "Monster", 12, 30, 0, 0);
    world.force_roll(50); // > 5 → no totem
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 1477), 40);
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at 40 heads");
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 57), a + 200);
    assert!(quest_cond(&world, 3001, q).is_none());
}

#[test]
fn quest_q00264_keen_claws() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1367, "Wolf Claw", true),
            (734, "Reward A", true),
            (35, "Reward B", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20003);
    t.type_name = "Monster".into();
    t.level = 5;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30136, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 5;
    let q = "Q00264_KeenClaws";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30136-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    inject(&mut world, 3001, 0x7000_0000, 1367, 42);
    // 20003 table [(2,25),(8,50)]: roll 30 → second entry → 8 claws → 50 → cond 2.
    add_test_npc(&mut world, NPC_OID + 1, 20003, "Monster", 5, 30, 0, 0);
    world.force_roll(30);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 1367),
        50,
        "the second table entry gives 8 claws"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Reward roll(17) == 0 → item 734 (+ jackpot); 735 is unreachable.
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 734), 1, "roll 0 → reward 734");
    assert!(quest_cond(&world, 3001, q).is_none());
}

#[test]
fn quest_q00292_brigands_sweep() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1483, "Goblin Necklace", true),
            (1484, "Goblin Pendant", true),
            (1485, "Goblin Lord Pendant", true),
            (1486, "Suspicious Memo", true),
            (1487, "Suspicious Contract", true),
        ],
    );
    // Goblin Brigand (20322) drops the necklace.
    let mut t = crate::data::npc_data::default_template(20322);
    t.type_name = "Monster".into();
    t.level = 10;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30532, "Folk", 5, 100, 0, 0); // Spiron
    add_test_npc(&mut world, NPC_OID + 1, 30533, "Folk", 5, 100, 0, 0); // Balanki
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 4; // Dwarf
    }
    let q = "Q00292_BrigandsSweep";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30532-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Memo path: three chance==5 kills assemble a Suspicious Contract and flip
    // cond → 2 (each give_item_randomly for the memo has its roll_f64 forced).
    let mob = NPC_OID + 2;
    for i in 0..3 {
        add_test_npc(&mut world, mob + i, 20322, "Monster", 10, 30, 0, 0);
        world.force_roll(5); // roll(10)==5 → memo branch
        world.force_roll(0); // give_item_randomly(MEMO) roll_f64 → hit
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 1487),
        1,
        "3 memos assemble a contract"
    );
    assert_eq!(item_count(&world, 3001, 1486), 0, "memos consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "contract → cond 2");
    // Balanki pays 620 for the contract.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 1)),
    );
    assert_eq!(item_count(&world, 3001, 57), a + 620, "Balanki pays 620");
    assert_eq!(item_count(&world, 3001, 1487), 0, "contract consumed");
    // Goblin-token turn-in at Spiron: 10 necklaces → 10*6 + 1000 bonus.
    inject(&mut world, 3001, 0x1483_0000, 1483, 10);
    let b = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        b + 1060,
        "10 necklaces → 60 + 1000 bonus"
    );
    assert_eq!(item_count(&world, 3001, 1483), 0, "necklaces consumed");
}

#[test]
fn quest_q00276_totem_of_the_hestui() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1480, "Kasha Parasite", true),
            (1481, "Kasha Crystal", true),
            (29, "Leather Shirt", false),
            (1500, "Reward Token", false),
        ],
    );
    for id in [20479, 27044] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 18;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30571, "Folk", 5, 100, 0, 0); // Tanapi
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 18;
        p.race = 3; // Orc
    }
    let q = "Q00276_TotemOfTheHestui";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30571-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kasha Bear kill at 0 parasites → below every ladder threshold, so no totem;
    // one parasite is paid instead.
    let bear = NPC_OID + 10;
    add_test_npc(&mut world, bear, 20479, "Monster", 18, 30, 0, 0);
    world.force_roll(50); // roll(100) chance2 (irrelevant with 0 parasites)
    world.force_roll(0); // give_item_randomly(PARASITE) roll_f64 → hit
    npc::npc_do_die(&mut world, bear, 3001);
    assert_eq!(
        item_count(&world, 3001, 1480),
        1,
        "kasha bear yields a parasite"
    );
    assert!(
        npcs_of(&mut world, 27044).is_empty(),
        "no totem below threshold"
    );
    // Stock 79 parasites → the next bear kill certainly conjures the totem
    // (ladder head (79, 100)) and wipes the hoard.
    inject(&mut world, 3001, 0x1480_0000, 1480, 78);
    let bear2 = NPC_OID + 11;
    add_test_npc(&mut world, bear2, 20479, "Monster", 18, 30, 0, 0);
    world.force_roll(0); // roll(100)=0 ≤ 100 → spawn
    npc::npc_do_die(&mut world, bear2, 3001);
    assert_eq!(
        item_count(&world, 3001, 1480),
        0,
        "spawning the totem consumes the hoard"
    );
    let totems = npcs_of(&mut world, 27044);
    assert_eq!(totems.len(), 1, "a Kasha Bear Totem was conjured");
    // Slaying the totem yields the Kasha Crystal and advances to cond 2.
    world.force_roll(0); // give_item_randomly(CRYSTAL) roll_f64 → hit
    npc::npc_do_die(&mut world, totems[0], 3001);
    assert_eq!(item_count(&world, 3001, 1481), 1, "totem drops the crystal");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Turn in at Tanapi → both rewards, repeatable exit.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 29), 1, "leather shirt reward");
    assert_eq!(item_count(&world, 3001, 1500), 1, "second reward");
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

/// Quest 275 (Dark Winged Spies) — Orc-only fang collection; reaching 70 fangs
/// flips to cond 2, then the turn-in pays 5 adena per fang.
#[test]
fn quest_q00275_dark_winged_spies() {
    const TANTUS: i32 = 30567;
    const FANG: i32 = 1478;
    const BAT: i32 = 20316;
    let q = "Q00275_DarkWingedSpies";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (FANG, "Darkwing Bat Fang", true),
            (1479, "Varangka's Parasite", true),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(BAT);
        t.type_name = "Monster".into();
        t.level = 13;
        world.data.npc_data.insert_for_test(t);
    }
    let tantus = NPC_OID;
    let bat = NPC_OID + 1;
    add_test_npc(&mut world, tantus, TANTUS, "Folk", 13, 100, 200, 0);
    add_test_npc(&mut world, bat, BAT, "Monster", 13, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 12;
        p.race = 3; // Orc
    }

    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{tantus}_Quest {q} {e}")));
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{tantus}_Quest {q}")),
    );
    event(&mut world, "30567-03.htm"); // accept
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");

    // Sitting at 69 fangs, one more bat kill reaches the 70 cap → cond 2.
    inject(&mut world, 3001, 0x0027_5000, FANG, 69);
    world.force_roll(0); // roll_f64 → 0.0 ≤ chance, the fang drops
    quests::notify_kill(&mut world, 3001, bat, BAT, false);
    assert_eq!(item_count(&world, 3001, FANG), 70, "the 70th fang dropped");
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "70 fangs → cond 2");

    // Turn in: 70 fangs × 5 adena.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{tantus}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57) - adena_before,
        70 * 5,
        "turn-in pays 5 adena per fang"
    );
}
