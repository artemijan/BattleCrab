//! Q00300-Q00399 — the hunting and collection quests.

use super::*;

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
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture 30359-04.htm"
        )),
    );
    drain(&mut rx);

    let skel = NPC_OID + 1;
    add_test_npc(&mut world, skel, 20517, "Monster", 5, 30, 0, 0);

    // Roll 0.999999 > 0.18 → no drop.
    world.force_roll(999_999);
    npc::npc_do_die(&mut world, skel, 3001);
    let count_of = |world: &World, id: i32| {
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(id)
    };
    assert_eq!(count_of(&world, 809), 0, "18% roll failed");

    // Roll 0 → drop.
    let skel2 = NPC_OID + 2;
    add_test_npc(&mut world, skel2, 20517, "Monster", 5, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, skel2, 3001);
    assert_eq!(count_of(&world, 809), 1);
    drain(&mut rx);

    // 9 bones banked, the 10th caps the collection: cond 2 + middle sound.
    inventory::add_inventory_item(&mut world, 3001, 809, 8).unwrap();
    let skel3 = NPC_OID + 3;
    add_test_npc(&mut world, skel3, 20517, "Monster", 5, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, skel3, 3001);
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 10);
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert_eq!(quests.0["Q00320_BonesTellTheFuture"].cond(), 2);
    }
    assert!(
        sound_names(&pkts).contains(&"ItemSound.quest_middle".to_string()),
        "limit-reached sound"
    );

    // Turn-in: 500 adena (rates ×1 in tests), bones destroyed, exit.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00320_BonesTellTheFuture")),
    );
    let pkts = drain(&mut rx);
    assert_eq!(count_of(&world, 809), 0);
    assert_eq!(count_of(&world, 57), 500, "500 adena at ×1 rates");
    assert!(
        ids_after_opcode(&pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_HAVE_EARNED_S1_ADENA)
    );
    assert!(
        !world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0
            .contains_key("Q00320_BonesTellTheFuture")
    );
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 10;
    drain_db(&mut db_rx);

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00303_CollectArrowheads 30029-04.htm"
        )),
    );
    assert_eq!(
        quest_cond(&world, 3001, "Q00303_CollectArrowheads"),
        Some(1)
    );
    drain(&mut rx);

    // Kill 10 marksmen with the 40% roll forced to hit each time.
    let mob = NPC_OID + 1;
    for i in 0..10 {
        add_test_npc(&mut world, mob + i, 20361, "Monster", 11, 30, 0, 0);
        world.force_roll(0); // roll_f64 → 0.0 ≤ 0.4
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 963), 10);
    assert_eq!(
        quest_cond(&world, 3001, "Q00303_CollectArrowheads"),
        Some(2)
    );
    drain(&mut rx);

    // Turn-in.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00303_CollectArrowheads")),
    );
    assert_eq!(item_count(&world, 3001, 57), adena_before + 500);
    assert_eq!(
        item_count(&world, 3001, 963),
        0,
        "quest items removed on exit"
    );
    assert!(
        quest_cond(&world, 3001, "Q00303_CollectArrowheads").is_none(),
        "repeatable exit"
    );
}

/// Q00316 Destroy Plague Carriers: the first hit on Varool Foulclaw makes
/// him shout (`on_attack` + script value), his fang drops at most once, and
/// the turn-in pays the fang/wererat ladder.
#[test]
fn quest_q00316_on_attack_say_and_limited_fang() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1042, "Wererat Fang", true),
            (1043, "Varool Foulclaw Fang", true),
        ],
    );
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

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers 30155-04.htm"
        )),
    );
    assert_eq!(
        quest_cond(&world, 3001, "Q00316_DestroyPlagueCarriers"),
        Some(1)
    );
    drain(&mut rx);

    // First hit on Varool: exactly one NpcSay; further hits stay quiet.
    let varool = NPC_OID + 1;
    add_test_npc(&mut world, varool, 27020, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0, false);
    let pkts = drain(&mut rx);
    let says: Vec<_> = pkts
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::NPC_SAY)
        .collect();
    assert_eq!(says.len(), 1, "one shout on the first hit");
    assert_eq!(
        i32::from_le_bytes(says[0][13..17].try_into().unwrap()),
        31603,
        "WHY_DO_YOU_OPPRESS_US_SO"
    );
    combat::npc_receive_damage(&mut world, varool, 3001, 10.0, false);
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "script value keeps him quiet"
    );

    // His fang drops once (chance 10/7 ≥ 1 → guaranteed), never twice.
    npc::npc_do_die(&mut world, varool, 3001);
    assert_eq!(item_count(&world, 3001, 1043), 1);
    let varool2 = NPC_OID + 2;
    add_test_npc(&mut world, varool2, 27020, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, varool2, 3001);
    assert_eq!(
        item_count(&world, 3001, 1043),
        1,
        "only one Varool fang ever"
    );

    // Wererats drop fangs freely (chance 2.0 → always).
    for i in 0..10 {
        let rat = NPC_OID + 3 + i;
        add_test_npc(&mut world, rat, 20040, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(&mut world, rat, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1042), 10);
    drain(&mut rx);

    // Turn-in: 10×5 + 1×1000 + 5000 bonus.
    let adena_before = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00316_DestroyPlagueCarriers")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 50 + 1000 + 5000
    );
    assert_eq!(item_count(&world, 3001, 1042), 0);
    assert_eq!(item_count(&world, 3001, 1043), 0);
}

/// Q00300 Hunting Leto Lizardman: the per-mob 1000-denominator drop gate, the
/// cond-2 trigger at exactly 60 bracelets, and the adena reward branch.
#[test]
fn quest_q00300_leto_loop() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(7139, "Bracelet of Lizardman", true)]);
    let mut t = crate::data::npc_data::default_template(20577);
    t.type_name = "Monster".into();
    t.level = 36;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30126, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 36;
    drain_db(&mut db_rx);

    let q = "Q00300_HuntingLetoLizardman";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30126-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    drain(&mut rx);

    let mob = NPC_OID + 1;
    // Drop gate: a roll >= 360 (Leto Lizardman's chance) yields nothing.
    add_test_npc(&mut world, mob, 20577, "Monster", 36, 30, 0, 0);
    world.force_roll(360);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(
        item_count(&world, 3001, 7139),
        0,
        "roll 360 (not < 360) drops nothing"
    );

    // 59 hits, still cond 1.
    for i in 1..=59 {
        add_test_npc(&mut world, mob + i, 20577, "Monster", 36, 30, 0, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob + i, 3001);
    }
    assert_eq!(item_count(&world, 3001, 7139), 59);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "still collecting at 59"
    );

    // The 60th bracelet flips cond to 2.
    add_test_npc(&mut world, mob + 60, 20577, "Monster", 36, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob + 60, 3001);
    assert_eq!(item_count(&world, 3001, 7139), 60);
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 at exactly 60");
    drain(&mut rx);

    // Turn in with the reward roll forced to the adena branch (< 500).
    let adena_before = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30126-06.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        adena_before + 5000,
        "adena reward"
    );
    assert_eq!(item_count(&world, 3001, 7139), 0, "bracelets taken");
    assert!(quest_cond(&world, 3001, q).is_none(), "repeatable exit");
}

/// The `getRandom(1000)` reward fork: 500..750 → 50 Animal Skin, 750+ → 50
/// Animal Bone (driven through repeatable re-runs, bracelets injected).
#[test]
fn quest_q00300_reward_skin_and_bone() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7139, "Bracelet", true),
            (1867, "Animal Skin", true),
            (1872, "Animal Bone", true),
        ],
    );
    add_test_npc(&mut world, NPC_OID, 30126, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 36;

    let q = "Q00300_HuntingLetoLizardman";
    for (i, (reward_roll, reward_item)) in [(600, 1867), (800, 1872)].into_iter().enumerate() {
        let obj = 0x5000_0000 + i as i32;
        // (Re)start the repeatable quest and inject a full batch of bracelets.
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
        );
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30126-03.htm")),
        );
        {
            let World { objects, data, .. } = &mut world;
            objects
                .get_component_mut::<Inventory>(&3001)
                .unwrap()
                .add_item(&data.item_data, obj, 7139, 60);
        }
        let before = item_count(&world, 3001, reward_item);
        world.force_roll(reward_roll);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30126-06.html")),
        );
        assert_eq!(
            item_count(&world, 3001, reward_item),
            before + 50,
            "roll {reward_roll} → 50 of {reward_item}"
        );
        assert_eq!(item_count(&world, 3001, 7139), 0, "bracelets consumed");
    }
}

/// Q00300 refuses a starter above level 39 (`addCondMaxLevel(39)`).
#[test]
fn quest_q00300_refused_above_level_39() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(7139, "Bracelet", true)]);
    add_test_npc(&mut world, NPC_OID, 30126, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00300_HuntingLetoLizardman";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30126-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-40 starter never begins"
    );
}

#[test]
fn quest_q00328_sense_for_business() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1347, "Carcass", true),
            (1366, "Lens", true),
            (1348, "Gizzard", true),
        ],
    );
    for id in [20055, 20070] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 22;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30436, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;
    let q = "Q00328_SenseForBusiness";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30436-03.htm")),
    );
    let mut m = NPC_OID + 1;
    let mut kill = |world: &mut World, sp: i32, roll: i32| {
        add_test_npc(world, m, sp, "Monster", 22, 30, 0, 0);
        world.force_roll(roll);
        npc::npc_do_die(world, m, 3001);
        m += 1;
    };
    kill(&mut world, 20055, 60); // < 61 → carcass
    kill(&mut world, 20055, 61); // 61 < 62 → lens
    kill(&mut world, 20070, 59); // < 60 → gizzard
    assert_eq!(item_count(&world, 3001, 1347), 1);
    assert_eq!(item_count(&world, 3001, 1366), 1);
    assert_eq!(item_count(&world, 3001, 1348), 1);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 14,
        "carcass 2 + lens 10 + gizzard 2"
    );
    assert_eq!(
        item_count(&world, 3001, 1347)
            + item_count(&world, 3001, 1366)
            + item_count(&world, 3001, 1348),
        0
    );
}

#[test]
fn quest_q00331_arrow_of_vengeance() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1452, "Feather", true),
            (1453, "Venom", true),
            (1454, "Tooth", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20145);
    t.type_name = "Monster".into();
    t.level = 35;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30125, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 35;
    let q = "Q00331_ArrowOfVengeance";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30125-03.htm")),
    );
    add_test_npc(&mut world, NPC_OID + 1, 20145, "Monster", 35, 30, 0, 0);
    world.force_roll(58); // < 59 → feather
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    add_test_npc(&mut world, NPC_OID + 2, 20145, "Monster", 35, 30, 0, 0);
    world.force_roll(59); // ≥ 59 → nothing
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1452), 1);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 57), a + 6, "one feather = 6a");
}

#[test]
fn quest_q00326_vanquish_remnants() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1359, "Red Badge", true),
            (1360, "Blue Badge", true),
            (1361, "Black Badge", true),
            (1369, "Black Lion Mark", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20053);
    t.type_name = "Monster".into();
    t.level = 25;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30435, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 25;
    let q = "Q00326_VanquishRemnants";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30435-03.htm")),
    );
    add_test_npc(&mut world, NPC_OID + 1, 20053, "Monster", 25, 30, 0, 0);
    world.force_roll(60); // < 61 → red badge
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 1359), 1);
    // Push to 100 red badges to earn the Black Lion Mark.
    inject(&mut world, 3001, 0x6400_0000, 1359, 99);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1369),
        1,
        "Black Lion Mark at 100 badges"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 2000,
        "100*10 + 1000 bonus"
    );
    assert_eq!(
        item_count(&world, 3001, 1359),
        0,
        "badges taken (mark kept)"
    );
}

// ===== G22 quest batch 2 (Q264/319/329/360) =====

#[test]
fn quest_q00319_scent_of_death() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(1045, "Zombie Skin", true)]);
    let mut t = crate::data::npc_data::default_template(20015);
    t.type_name = "Monster".into();
    t.level = 13;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30138, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 13;
    let q = "Q00319_ScentOfDeath";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30138-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // roll 8 (> 7) → a skin, and count 1 < 5 sets cond 2 (the quirk).
    add_test_npc(&mut world, NPC_OID + 1, 20015, "Monster", 13, 30, 0, 0);
    world.force_roll(8);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 1045), 1);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "cond 2 set below the target"
    );
    // roll 5 (≤ 7) → nothing.
    add_test_npc(&mut world, NPC_OID + 2, 20015, "Monster", 13, 30, 0, 0);
    world.force_roll(5);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1045), 1, "roll ≤ 7 drops nothing");
    inject(&mut world, 3001, 0x7100_0000, 1045, 4);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 57), a + 500, "500 adena (no rate)");
    assert!(quest_cond(&world, 3001, q).is_none());
}

#[test]
fn quest_q00329_curiosity_of_a_dwarf() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1346, "Golem Heartstone", true),
            (1365, "Broken Heartstone", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20083);
    t.type_name = "Monster".into();
    t.level = 35;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30437, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 35;
    let q = "Q00329_CuriosityOfADwarf";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30437-03.htm")),
    );
    // roll 2 (< 3) → golem heartstone; roll 10 (3..54) → broken heartstone.
    add_test_npc(&mut world, NPC_OID + 1, 20083, "Monster", 35, 30, 0, 0);
    world.force_roll(2);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    add_test_npc(&mut world, NPC_OID + 2, 20083, "Monster", 35, 30, 0, 0);
    world.force_roll(10);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 1346), 1, "golem heartstone");
    assert_eq!(item_count(&world, 3001, 1365), 1, "broken heartstone");
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 1045,
        "40 + 5 + 1000 (inverted <700 bonus)"
    );
    assert_eq!(
        item_count(&world, 3001, 1346) + item_count(&world, 3001, 1365),
        0
    );
}

#[test]
fn quest_q00360_plunder_their_supplies() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(5872, "Supply Items", true)]);
    let mut t = crate::data::npc_data::default_template(20666);
    t.type_name = "Monster".into();
    t.level = 55;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30873, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 55;
    let q = "Q00360_PlunderTheirSupplies";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30873-04.htm")),
    );
    add_test_npc(&mut world, NPC_OID + 1, 20666, "Monster", 55, 30, 0, 0);
    world.force_roll(40); // < 50 → supply
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 5872), 1);
    inject(&mut world, 3001, 0x7200_0000, 5872, 499);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 14000,
        "500 supplies → 14000 adena"
    );
    assert_eq!(item_count(&world, 3001, 5872), 0);
}

#[test]
fn quest_q00369_collector_of_jewels() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(5882, "Flare Shard", true), (5883, "Freezing Shard", true)],
    );
    // death_fire (20749): flare shard, chance 100, count 2.
    let mut t = crate::data::npc_data::default_template(20749);
    t.type_name = "Monster".into();
    t.level = 30;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30376, "Folk", 30, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 30;
    let q = "Q00369_CollectorOfJewels";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30376-02.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Stage-1 kill: death_fire drops 2 flare (roll(100)=0 < chance 100).
    add_test_npc(&mut world, NPC_OID + 1, 20749, "Monster", 30, 30, 0, 0);
    world.force_roll(0); // outer roll(100) < chance
    world.force_roll(0); // give_item_randomly roll_f64 → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 5882),
        2,
        "death_fire drops 2 flare shards"
    );
    // Fast-forward to 100 combined shards and turn in stage 1 → 3000 adena.
    inject(&mut world, 3001, 0x5882_0000, 5882, 48); // 50 flare
    inject(&mut world, 3001, 0x5883_0000, 5883, 50); // 50 freezing
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 3000,
        "stage 1: 100 shards → 3000 adena"
    );
    assert_eq!(
        item_count(&world, 3001, 5882) + item_count(&world, 3001, 5883),
        0,
        "shards consumed"
    );
    // Advance to stage 2 (memoState 2 → 3, cond 3).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30376-06.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // Fast-forward to 400 combined shards and turn in stage 2 → 12000 adena, exit.
    inject(&mut world, 3001, 0x5882_0001, 5882, 200);
    inject(&mut world, 3001, 0x5883_0001, 5883, 200);
    let b = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        b + 12000,
        "stage 2: 400 shards → 12000 adena"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00358_illegitimate_child_of_the_goddess() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(5868, "Snake Scale", true), (4975, "Recipe", false)],
    );
    // Trives (20672) drops snake scales at 71%.
    let mut t = crate::data::npc_data::default_template(20672);
    t.type_name = "Monster".into();
    t.level = 65;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30862, "Folk", 60, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 65;
    let q = "Q00358_IllegitimateChildOfTheGoddess";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30862-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // At 107 scales, one more kill tops to 108 (the cap) and flips cond → 2.
    inject(&mut world, 3001, 0x5868_0000, 5868, 107);
    add_test_npc(&mut world, NPC_OID + 1, 20672, "Monster", 65, 30, 0, 0);
    world.force_roll(0); // give_item_randomly roll_f64 (0.0 < 0.71) → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 5868),
        108,
        "108th scale tops the cap"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cap reached → cond 2");
    // Turn in: 108 scales → one random recipe (force index 0 → 4975), exit.
    world.force_roll(0); // REWARDS index
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 4975), 1, "random recipe reward");
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00354_conquest_of_alligator_island() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(5863, "Alligator Tooth", true)]);
    for id in [20804, 20808] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30895, "Folk", 40, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00354_ConquestOfAlligatorIsland";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30895-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // crokian_lad (20804, 84%): one tooth.
    add_test_npc(&mut world, NPC_OID + 1, 20804, "Monster", 40, 30, 0, 0);
    world.force_roll(0); // roll_f64 (0.0 < 0.84) → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 5863),
        1,
        "crokian_lad drops a tooth"
    );
    // nos_lad (20808): roll(100)=0 < 14 → double drop.
    add_test_npc(&mut world, NPC_OID + 2, 20808, "Monster", 40, 30, 0, 0);
    world.force_roll(0); // roll(100) < 14 → count 2
    world.force_roll(0); // give_item_randomly roll_f64 (chance 1.0) → hit
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 5863), 3, "nos_lad drops 2 teeth");
    // 400 teeth → 2000 adena via the ADENA bypass, teeth cleared.
    inject(&mut world, 3001, 0x5863_0000, 5863, 397);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} ADENA")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 2000,
        "400 teeth → 2000 adena"
    );
    assert_eq!(item_count(&world, 3001, 5863), 0, "teeth consumed");
    // Repeatable: still started after turn-in.
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
}

#[test]
fn quest_q00356_dig_up_the_sea_of_spores() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5865, "Carnivore Spore", true),
            (5866, "Herbivorous Spore", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20558); // Rotting Tree → herb
    t.type_name = "Monster".into();
    t.level = 45;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30717, "Folk", 45, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    let q = "Q00356_DigUpTheSeaOfSpores";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30717-05.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Carnivore already full, herbivorous one short → a Rotting Tree kill tops
    // herb to 100 with the other kind already full, so cond → 3.
    inject(&mut world, 3001, 0x5865_0000, 5865, 100);
    inject(&mut world, 3001, 0x5866_0000, 5866, 99);
    add_test_npc(&mut world, NPC_OID + 1, 20558, "Monster", 45, 30, 0, 0);
    world.force_roll(0); // roll_f64 (0.0 < 0.73) → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 5866),
        100,
        "herb spore tops to 100"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "both kinds full → cond 3"
    );
    // FINISH: roll(100)=0 < 20 → 3000 adena, exit.
    let a = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} FINISH")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 3000,
        "top reward bucket → 3000 adena"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00355_family_honor() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (4252, "Galfredo Romer's Bust", true),
            (4350, "Sculptor Berona", false),
            (4351, "Ancient Statue Prototype", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20767); // timak_orc_troop_leader
    t.type_name = "Monster".into();
    t.level = 40;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30181, "Folk", 40, 100, 0, 0); // Galibredo
    add_test_npc(&mut world, NPC_OID + 1, 30929, "Folk", 40, 100, 0, 0); // Patrin
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00355_FamilyHonor";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30181-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // 20767: first 560 / second 684. roll(1000)=0 < 560 → a bust.
    add_test_npc(&mut world, NPC_OID + 2, 20767, "Monster", 40, 30, 0, 0);
    world.force_roll(0); // roll(1000) → bust branch
    world.force_roll(0); // give_item_randomly roll_f64
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 4252), 1, "kill drops a bust");
    // roll(1000)=600 → 560..684 → a Sculptor Berona.
    add_test_npc(&mut world, NPC_OID + 3, 20767, "Monster", 40, 30, 0, 0);
    world.force_roll(600); // roll(1000) → berona branch
    world.force_roll(0); // give_item_randomly roll_f64
    npc::npc_do_die(&mut world, NPC_OID + 3, 3001);
    assert_eq!(item_count(&world, 3001, 4350), 1, "kill drops a berona");
    // Sell 100 busts at Galibredo → 100*20 = 2000 adena.
    inject(&mut world, 3001, 0x4252_0000, 4252, 99);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30181-06.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 2000,
        "100 busts → 2000 adena"
    );
    assert_eq!(item_count(&world, 3001, 4252), 0, "busts consumed");
    // Patrin appraises the berona: roll(100)=0 < 2 → the Prototype (4351).
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 30929-03.html", NPC_OID + 1)),
    );
    assert_eq!(
        item_count(&world, 3001, 4351),
        1,
        "berona → ancient statue prototype"
    );
    assert_eq!(item_count(&world, 3001, 4350), 0, "berona consumed");
}

#[test]
fn quest_q00374_whisper_of_dreams_part1() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5884, "Cave Beast Tooth", true),
            (5885, "Death Wave Light", true),
            (5886, "Sealed Mysterious Stone", true),
            (5887, "Mysterious Stone", false),
            (49475, "Scroll Part EA", false),
        ],
    );
    for id in [20620, 20621] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 60;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30938, "Folk", 60, 100, 0, 0); // Vanutu
    add_test_npc(&mut world, NPC_OID + 1, 31044, "Folk", 60, 100, 0, 0); // Galman
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    let q = "Q00374_WhisperOfDreamsPart1";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30938-01.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // 360 tooth + one Death Wave kill topping light to 360 → cond 2.
    inject(&mut world, 3001, 0x0374_0000, 5884, 360);
    inject(&mut world, 3001, 0x0374_0001, 5885, 359);
    add_test_npc(&mut world, NPC_OID + 2, 20621, "Monster", 60, 30, 0, 0);
    world.force_roll(0); // give_item_randomly(light) roll_f64 (0.0 < 0.9)
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 5885), 360, "light topped to 360");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "both ingredients full → cond 2"
    );
    // reward1: hand both stacks over for the scroll + 9000 adena.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} reward1")),
    );
    assert_eq!(item_count(&world, 3001, 49475), 1, "scroll reward");
    assert_eq!(item_count(&world, 3001, 57), a + 9000, "9000 adena");
    assert_eq!(
        item_count(&world, 3001, 5884) + item_count(&world, 3001, 5885),
        0,
        "ingredients consumed"
    );
    // Advance to cond 3, where kills also drop the Sealed Mysterious Stone.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30938-06.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    add_test_npc(&mut world, NPC_OID + 3, 20620, "Monster", 60, 30, 0, 0);
    world.force_roll(0); // give_item_randomly(tooth) roll_f64
    world.force_roll(0); // give_item_randomly(sealed stone) roll_f64 (0.0 < 0.2)
    npc::npc_do_die(&mut world, NPC_OID + 3, 3001);
    assert_eq!(
        item_count(&world, 3001, 5886),
        1,
        "sealed stone drops at cond 3"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(4),
        "sealed stone → cond 4"
    );
    // Galman exchanges the sealed stone for the Mysterious Stone (Part 2 opener).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 31044-01.html", NPC_OID + 1)),
    );
    assert_eq!(
        item_count(&world, 3001, 5887),
        1,
        "Mysterious Stone (opens Part 2)"
    );
    assert_eq!(item_count(&world, 3001, 5886), 0, "sealed stone consumed");
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00306_crystal_of_fire_and_ice() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1020, "Flame Shard", true), (1021, "Ice Shard", true)],
    );
    for id in [20109, 20110] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30004, "Folk", 20, 100, 0, 0); // Katerina
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    let q = "Q00306_CrystalOfFireAndIce";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30004-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Chance is 1000/count > 1.0, so every kill drops a shard.
    add_test_npc(&mut world, NPC_OID + 1, 20109, "Monster", 20, 30, 0, 0); // Salamander → flame
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 1020),
        1,
        "salamander always drops a flame shard"
    );
    add_test_npc(&mut world, NPC_OID + 2, 20110, "Monster", 20, 30, 0, 0); // Undine → ice
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(
        item_count(&world, 3001, 1021),
        1,
        "undine always drops an ice shard"
    );
    // Turn in 5 + 5 = 10 shards → 10*15 + 5000 bonus = 5150 adena.
    inject(&mut world, 3001, 0x0306_0000, 1020, 4);
    inject(&mut world, 3001, 0x0306_0001, 1021, 4);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 5150,
        "10 shards → 150 + 5000 bonus"
    );
    assert_eq!(
        item_count(&world, 3001, 1020) + item_count(&world, 3001, 1021),
        0,
        "shards consumed"
    );
}

#[test]
fn quest_q00375_whisper_of_dreams_part2() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5888, "Karik Horn", true),
            (5889, "Limal Karinness Blood", true),
            (5887, "Mysterious Stone", false),
            (49474, "Scroll Part EW", false),
        ],
    );
    for id in [20628, 20629] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 65;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30938, "Folk", 65, 100, 0, 0); // Vanutu
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 65;
    // The Mysterious Stone from Part 1 is the ticket in.
    inject(&mut world, 3001, 0x0375_0000, 5887, 1);
    let q = "Q00375_WhisperOfDreamsPart2";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 5887),
        0,
        "the Mysterious Stone is consumed on first talk"
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30938-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // 325 horns + one Limal kill topping blood to 325 → cond 2.
    inject(&mut world, 3001, 0x0375_0001, 5888, 325);
    inject(&mut world, 3001, 0x0375_0002, 5889, 324);
    add_test_npc(&mut world, NPC_OID + 1, 20628, "Monster", 65, 30, 0, 0);
    world.force_roll(0); // give_item_randomly(blood) roll_f64 (0.0 < 0.95)
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 5889), 325, "blood topped to 325");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "both stacks full → cond 2"
    );
    // reward1: hand both stacks over for the scroll + 9000 adena.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} reward1")),
    );
    assert_eq!(item_count(&world, 3001, 49474), 1, "scroll reward");
    assert_eq!(item_count(&world, 3001, 57), a + 9000, "9000 adena");
    assert_eq!(
        item_count(&world, 3001, 5888) + item_count(&world, 3001, 5889),
        0,
        "325 of each consumed"
    );
}

#[test]
fn quest_q00325_grim_collector() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1349, "Anatomy Diagram", true),
            (1350, "Zombie Head", true),
            (1351, "Zombie Heart", true),
            (1352, "Zombie Liver", true),
            (1353, "Skull", true),
            (1354, "Rib Bone", true),
            (1355, "Spine", true),
            (1356, "Arm Bone", true),
            (1357, "Thigh Bone", true),
            (1358, "Complete Skeleton", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20026); // ladder: head/heart/liver
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30336, "Folk", 15, 100, 0, 0); // Curtiz
    add_test_npc(&mut world, NPC_OID + 1, 30342, "Folk", 15, 100, 0, 0); // Varsak
    add_test_npc(&mut world, NPC_OID + 2, 30434, "Folk", 15, 100, 0, 0); // Samed
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    let q = "Q00325_GrimCollector";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30336-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Samed hands out the Anatomy Diagram (the drop gate).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 30434-03.html", NPC_OID + 2)),
    );
    assert_eq!(item_count(&world, 3001, 1349), 1, "diagram received");
    // Kill 20026: roll(100)=0 < 30 → the head branch of the ladder.
    add_test_npc(&mut world, NPC_OID + 3, 20026, "Monster", 20, 30, 0, 0);
    world.force_roll(0); // roll(100) → head
    world.force_roll(0); // give_item_randomly roll_f64
    npc::npc_do_die(&mut world, NPC_OID + 3, 3001);
    assert_eq!(
        item_count(&world, 3001, 1350),
        1,
        "ladder drops a zombie head"
    );
    // Assemble a Complete Skeleton at Varsak: five bones + roll(5)=0 (<4) success.
    for id in [1355, 1356, 1353, 1354, 1357] {
        inject(&mut world, 3001, 0x0325_0000 + id, id, 1);
    }
    world.force_roll(0); // roll(5) < 4 → success
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} assembleSkeleton", NPC_OID + 1)),
    );
    assert_eq!(
        item_count(&world, 3001, 1358),
        1,
        "five bones assemble a skeleton"
    );
    assert_eq!(
        item_count(&world, 3001, 1355),
        0,
        "bones consumed by assembly"
    );
    // Sell (30434-07): pays only with ALL ten registered items. Stock one of each
    // part so the gate opens; head is already 1, complete is 1, diagram is 1.
    for id in [1351, 1352, 1353, 1354, 1355, 1356, 1357] {
        inject(&mut world, 3001, 0x0325_1000 + id, id, 1);
    }
    // Now: head1 heart1 liver1 skull1 rib1 spine1 arm1 thigh1 complete1 → total 9.
    // sum = 8+5+5+25+5+5+5+5 = 63; complete → +543+341 = 947.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 30434-07.html", NPC_OID + 2)),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 947,
        "full collection sells for 947 adena"
    );
    assert_eq!(
        item_count(&world, 3001, 1349),
        0,
        "all registered items consumed by the sale"
    );
    assert_eq!(item_count(&world, 3001, 1358), 0, "skeleton consumed too");
}

#[test]
fn quest_q00373_supplier_of_reagents() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5904, "Mixing Stone", true),
            (6317, "Mixing Manual", true),
            (6011, "Wyrm's Blood", false),
            (6017, "Blood Root", false),
            (6021, "Dracoplasm", false),
            (6010, "Reagent Box", false),
        ],
    );
    for id in [21111, 21066] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 60;
        world.data.npc_data.insert_for_test(t);
    }
    let wesley = NPC_OID;
    let urn = NPC_OID + 1;
    add_test_npc(&mut world, wesley, 30166, "Folk", 55, 100, 0, 0);
    add_test_npc(&mut world, urn, 31149, "Folk", 55, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;
    let q = "Q00373_SupplierOfReagents";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{wesley}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{wesley}_Quest {q} 30166-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 5904), 1, "mixing stone given");
    assert_eq!(item_count(&world, 3001, 6317), 1, "mixing manual given");
    // Pair drop: Lava Wyrm, roll(1000)=0 < 505 → Wyrm's Blood.
    add_test_npc(&mut world, NPC_OID + 10, 21111, "Monster", 60, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(
        item_count(&world, 3001, 6011),
        1,
        "Lava Wyrm drops Wyrm's Blood on a low roll"
    );
    // Single drop: Platinum Guardian Shaman, roll(1_000_000)=0 < 442000 → Reagent Box.
    add_test_npc(&mut world, NPC_OID + 11, 21066, "Monster", 60, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 11, 3001);
    assert_eq!(
        item_count(&world, 3001, 6010),
        1,
        "Platinum Guardian Shaman drops a Reagent Box"
    );
    // Alchemy: 10 Wyrm's Blood + 1 Blood Root, mixed at temperature 1 → Dracoplasm.
    inject(&mut world, 3001, 0x0373_0000, 6011, 9); // 10 total
    inject(&mut world, 3001, 0x0373_0001, 6017, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{urn}_Quest {q} 31149-03-6011.htm")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{urn}_Quest {q} 31149-06-6017.htm")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{urn}_Quest {q} 31149-12-1")),
    );
    assert_eq!(
        item_count(&world, 3001, 6021),
        1,
        "temperature-1 mix yields one Dracoplasm"
    );
    assert_eq!(
        item_count(&world, 3001, 6011),
        0,
        "10 Wyrm's Blood consumed"
    );
    assert_eq!(item_count(&world, 3001, 6017), 0, "Blood Root consumed");
}

#[test]
fn quest_q00344_1000_years_the_end_of_lamentation() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (4269, "Articles of Sacrifice", true),
            (4271, "Old Hilt", true),
            (1874, "Oriharukon Ore", false),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20236); // Cave Servant, chance 0.58
    t.type_name = "Monster".into();
    t.level = 50;
    world.data.npc_data.insert_for_test(t);
    let gilmore = NPC_OID;
    let kaien = NPC_OID + 1;
    add_test_npc(&mut world, gilmore, 30754, "Folk", 50, 100, 0, 0);
    add_test_npc(&mut world, kaien, 30623, "Folk", 50, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 50;
    let q = "Q00344_1000YearsTheEndOfLamentation";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Quest {q} 30754-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill a Cave Servant → an Article.
    add_test_npc(&mut world, NPC_OID + 10, 20236, "Monster", 50, 30, 0, 0);
    world.force_roll(0); // give_item_randomly roll_f64 (0.0 < 0.58)
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(
        item_count(&world, 3001, 4269),
        1,
        "Cave Servant drops an Article"
    );
    // Turn-in gamble: 5 articles, roll(1000)=0 < 5 → relic; roll(4)=0 → Old Hilt (memo 1, cond 2).
    inject(&mut world, 3001, 0x0344_0000, 4269, 4); // 5 total
    world.force_roll(0); // roll(1000) < count → relic path
    world.force_roll(0); // roll(4) → Old Hilt
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Quest {q} 30754-08.html")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "relic jackpot → cond 2"
    );
    assert_eq!(
        item_count(&world, 3001, 4271),
        1,
        "received the Old Hilt relic"
    );
    assert_eq!(
        item_count(&world, 3001, 4269),
        0,
        "articles consumed by the turn-in"
    );
    // Kaien exchanges the Old Hilt: roll(100)=0 ≤ 52 → 25 Oriharukon Ore, back to cond 1.
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kaien}_Quest {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1874),
        25,
        "Kaien pays 25 Oriharukon Ore for the hilt"
    );
    assert_eq!(item_count(&world, 3001, 4271), 0, "hilt consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "back to collecting");
    // Adena path: 3 articles, roll(1000)=500 ≥ 3 → 3*60 = 180 adena.
    inject(&mut world, 3001, 0x0344_0001, 4269, 3);
    let a = item_count(&world, 3001, 57);
    world.force_roll(500);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Quest {q} 30754-08.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 180,
        "3 articles → 180 adena"
    );
    assert_eq!(item_count(&world, 3001, 4269), 0, "articles consumed");
}

/// Quest 350 (Enhance Your Weapon) — the Soul Crystal system: accept, take a
/// Red crystal, and level it by killing a skill-absorb mob. The absorb gate
/// (Soul Crystal skill cast below half HP) must be satisfied first.
#[test]
fn quest_q00350_enhance_your_weapon() {
    use crate::data::soul_crystal_data::{AbsorbType, LevelingInfo};
    use crate::model::components::stats::Vitals;

    const ROLENTO: i32 = 30115;
    const RED0: i32 = 4629; // Red Soul Crystal - stage 0
    const RED1: i32 = 4630; // stage 1
    const MOB: i32 = 20583; // a skill-needed leveling mob (Timak Orc)
    const SOUL_CRYSTAL_SKILL: i32 = 2096;
    let q = "Q00350_EnhanceYourWeapon";

    let (mut world, _db, _l) = quest_test_world();
    // Populate the crystal data and rebuild the registry so MOB is registered
    // for this quest's kill / skill-see hooks (the registry snapshots the
    // leveling-npc ids at construction, before the test injects them).
    world
        .data
        .soul_crystal_data
        .insert_crystal_for_test(RED0, 0, RED1);
    world
        .data
        .soul_crystal_data
        .insert_crystal_for_test(RED1, 1, 4631);
    world.data.soul_crystal_data.insert_npc_level_for_test(
        MOB,
        0,
        LevelingInfo {
            absorb_type: AbsorbType::LastHit,
            skill_needed: true,
            chance: 100,
        },
    );
    world.quests = Arc::new(crate::scripts::build_registry(vec![MOB]));

    add_quest_items(
        &mut world,
        &[
            (RED0, "Red Soul Crystal", false),
            (RED1, "Red Soul Crystal - Stage 1", false),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(MOB);
        t.type_name = "Monster".into();
        t.level = 45;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let rolento = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, rolento, ROLENTO, "Folk", 45, 100, 200, 0);
    add_test_npc(&mut world, mob, MOB, "Monster", 45, 300, 300, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;

    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(
            w,
            1,
            &bypass_body(&format!("npc_{rolento}_Quest {q} {e}")),
        );
    };
    use crate::network::server_packets::sm_ids as smid;

    // --- Accept and take a Red Soul Crystal (stage 0). ---
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{rolento}_Quest {q}")),
    );
    event(&mut world, "30115-04.htm"); // start
    event(&mut world, "30115-09.htm"); // give Red crystal
    assert_eq!(
        item_count(&world, 3001, RED0),
        1,
        "Red Soul Crystal granted"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "quest started");

    // --- A skill-needed mob killed WITHOUT absorbing does not level. ---
    drain(&mut rx);
    quests::notify_kill(&mut world, 3001, mob, MOB, false);
    assert_eq!(
        item_count(&world, 3001, RED0),
        1,
        "no level without an absorb"
    );
    assert_eq!(item_count(&world, 3001, RED1), 0);
    // Java bails before `levelCrystal` when the absorb gate is unmet, so none
    // of the flavour lines fire — the zero case, which is the half a
    // "message is sent" assertion cannot see on its own.
    let quiet = ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE);
    for id in [
        smid::THE_SOUL_CRYSTAL_SUCCEEDED_IN_ABSORBING_A_SOUL,
        smid::THE_SOUL_CRYSTAL_WAS_NOT_ABLE_TO_ABSORB_THE_SOUL,
        smid::THE_SOUL_CRYSTAL_IS_REFUSING_TO_ABSORB_THE_SOUL,
    ] {
        assert!(
            !quiet.contains(&id),
            "no absorb flavour without a cast: {id}"
        );
    }

    // --- Cast the Soul Crystal skill below half HP, then kill → level up. ---
    // Test NPCs come up with max_hp 100 (add_test_npc), so drop below 50.
    world
        .objects
        .get_component_mut::<Vitals>(&mob)
        .unwrap()
        .cur_hp = 40.0; // ≤ half HP, the absorb condition
    quests::notify_skill_see(&mut world, 3001, mob, MOB, SOUL_CRYSTAL_SKILL);
    world.force_roll(0); // roll(100)=0 <= chance 100 → success
    drain(&mut rx);
    quests::notify_kill(&mut world, 3001, mob, MOB, false);
    assert_eq!(
        item_count(&world, 3001, RED0),
        0,
        "the stage-0 crystal is consumed"
    );
    assert_eq!(
        item_count(&world, 3001, RED1),
        1,
        "the crystal leveled to stage 1 after a valid absorb-kill"
    );
    // Java's `exchangeCrystal` sends the success line before the
    // `YOU_HAVE_EARNED_S1` that `give_items` emits — assert the order, not
    // just the presence.
    let ids = ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE);
    let succeeded = ids
        .iter()
        .position(|&i| i == smid::THE_SOUL_CRYSTAL_SUCCEEDED_IN_ABSORBING_A_SOUL)
        .expect("succeeded-in-absorbing sent");
    let earned = ids
        .iter()
        .position(|&i| i == smid::YOU_HAVE_EARNED_S1)
        .expect("the new crystal is announced");
    assert!(succeeded < earned, "flavour precedes the grant: {ids:?}");

    // --- A failed roll sends the "not able to absorb" line instead. ---
    world
        .objects
        .get_component_mut::<Vitals>(&mob)
        .unwrap()
        .cur_hp = 40.0;
    quests::notify_skill_see(&mut world, 3001, mob, MOB, SOUL_CRYSTAL_SKILL);
    world.data.soul_crystal_data.insert_npc_level_for_test(
        MOB,
        1,
        LevelingInfo {
            absorb_type: AbsorbType::LastHit,
            skill_needed: true,
            chance: 50,
        },
    );
    world.force_roll(99); // roll(100)=99 > chance 50 → failure
    drain(&mut rx);
    quests::notify_kill(&mut world, 3001, mob, MOB, false);
    assert_eq!(
        item_count(&world, 3001, RED1),
        1,
        "a failed roll neither consumes nor levels the crystal"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&smid::THE_SOUL_CRYSTAL_WAS_NOT_ABLE_TO_ABSORB_THE_SOUL),
        "the failure line is sent"
    );
}

/// Quest 370 (An Elder Sows Seeds) — ant kills drop Spellbook Pages; a matched
/// set of four elemental Chapters cashes in for 3,600 adena each.
#[test]
fn quest_q00370_an_elder_sows_seeds() {
    const CASIAN: i32 = 30612;
    const PAGE: i32 = 5916;
    const ANT: i32 = 20082; // MOBS_PERCENT, 9%
    let q = "Q00370_AnElderSowsSeeds";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (PAGE, "Spellbook Page", true),
            (5917, "Chapter of Fire", true),
            (5918, "Chapter of Water", true),
            (5919, "Chapter of Wind", true),
            (5920, "Chapter of Earth", true),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(ANT);
        t.type_name = "Monster".into();
        t.level = 30;
        world.data.npc_data.insert_for_test(t);
    }
    let casian = NPC_OID;
    let ant = NPC_OID + 1;
    add_test_npc(&mut world, casian, CASIAN, "Folk", 30, 100, 200, 0);
    add_test_npc(&mut world, ant, ANT, "Monster", 30, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 30;

    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{casian}_Quest {q} {e}")));
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{casian}_Quest {q}")),
    );
    event(&mut world, "30612-04.htm"); // accept
    assert!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED),
        "started"
    );

    // Kill an ant: the 9% roll succeeds and a page drops.
    world.force_roll(0); // roll(100)=0 < 9
    world.force_roll(0); // roll_f64=0.0 ≤ chance
    quests::notify_kill(&mut world, 3001, ant, ANT, false);
    assert_eq!(
        item_count(&world, 3001, PAGE),
        1,
        "an ant kill drops a page"
    );

    // A matched set of four chapters cashes in for 3,600 adena.
    for c in [5917, 5918, 5919, 5920] {
        inject(&mut world, 3001, 0x0037_0000 + c, c, 1);
    }
    let adena_before = item_count(&world, 3001, 57);
    event(&mut world, "REWARD");
    assert_eq!(
        item_count(&world, 3001, 57) - adena_before,
        3600,
        "one matched chapter set pays 3,600 adena"
    );
    assert_eq!(
        item_count(&world, 3001, 5917),
        0,
        "the chapters are consumed"
    );
}

/// Quest 327 (Recover the Farmland) — Turek Orc kills drop tokens + relic
/// fragments; the tokens cash out with Piotur, and the fragments feed Asha's
/// gamble and Nestle's consumable trade.
#[test]
fn quest_q00327_recover_the_farmland() {
    const PIOTUR: i32 = 30597;
    const ASHA: i32 = 30313;
    const NESTLE: i32 = 30314;
    const ARCHER: i32 = 20496; // fragment drop prob 21%
    const DOG_TAG: i32 = 1846;
    const CLAY_FRAGMENT: i32 = 1848;
    const ANCIENT_CLAY_URN: i32 = 1852;
    const SOULSHOT_D: i32 = 1463;
    const ADENA: i32 = 57;
    let q = "Q00327_RecoverTheFarmland";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (DOG_TAG, "Turek Dog Tag", true),
            (1847, "Turek Medallion", true),
            (5012, "Leikan's Letter", true),
            (CLAY_FRAGMENT, "Clay Urn Fragment", true),
            (ANCIENT_CLAY_URN, "Ancient Clay Urn", true),
            (SOULSHOT_D, "Soulshot D", true),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(ARCHER);
        t.type_name = "Monster".into();
        t.level = 28;
        world.data.npc_data.insert_for_test(t);
    }
    let piotur = NPC_OID;
    let asha = NPC_OID + 1;
    let nestle = NPC_OID + 2;
    let mob = NPC_OID + 3;
    add_test_npc(&mut world, piotur, PIOTUR, "Folk", 28, 100, 200, 0);
    add_test_npc(&mut world, asha, ASHA, "Folk", 28, 110, 200, 0);
    add_test_npc(&mut world, nestle, NESTLE, "Folk", 28, 120, 200, 0);
    add_test_npc(&mut world, mob, ARCHER, "Monster", 28, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 27;

    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };

    // --- Accept via Piotur. ---
    talk(&mut world, piotur);
    ev(&mut world, piotur, "30597-03.htm");
    assert!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED),
        "started"
    );

    // --- An Archer kill drops a Dog Tag, and (forced) a relic fragment. ---
    world.force_roll(0); // roll(100)=0 < 21 → a fragment drops
    world.force_roll(0); // roll(4)=0 → Clay Urn Fragment (1848)
    quests::notify_kill(&mut world, 3001, mob, ARCHER, false);
    assert_eq!(
        item_count(&world, 3001, DOG_TAG),
        1,
        "the kill dropped a Dog Tag"
    );
    assert_eq!(
        item_count(&world, 3001, CLAY_FRAGMENT),
        1,
        "and a relic fragment"
    );

    // --- Piotur cashes 10 tokens: 10×8 + 1000 bonus = 1080 adena. ---
    inject(&mut world, 3001, 0x0032_7000, DOG_TAG, 9); // top up to 10
    let adena_before = item_count(&world, 3001, ADENA);
    talk(&mut world, piotur);
    assert_eq!(
        item_count(&world, 3001, ADENA) - adena_before,
        10 * 8 + 1000,
        "ten tokens pay 1080 adena"
    );
    assert_eq!(item_count(&world, 3001, DOG_TAG), 0, "tokens spent");

    // --- Asha gambles 5 fragments into an Ancient Clay Urn (forced success).
    // The kill already dropped one Clay fragment, so top up by four to reach 5.
    inject(&mut world, 3001, 0x0032_7100, CLAY_FRAGMENT, 4);
    world.force_roll(0); // roll(6)=0 < 5 → success
    ev(&mut world, asha, "30313-03.html");
    assert_eq!(
        item_count(&world, 3001, ANCIENT_CLAY_URN),
        1,
        "relic assembled"
    );
    assert_eq!(
        item_count(&world, 3001, CLAY_FRAGMENT),
        0,
        "5 fragments consumed"
    );

    // --- Nestle trades the relic for 70 Soulshot D (forced low roll). ---
    world.force_roll(0); // roll(41)=0 → 70 shots
    ev(&mut world, nestle, "30314-03.html");
    assert_eq!(
        item_count(&world, 3001, SOULSHOT_D),
        70,
        "70 Soulshot D awarded"
    );
    assert_eq!(
        item_count(&world, 3001, ANCIENT_CLAY_URN),
        0,
        "the relic is spent"
    );
}

/// Quest 348 (An Arrogant Search) — the full Seven Signs cond ladder: hunt a
/// Shell of Monsters, summon and slay Stone Watchman Ezekiel for the Book of
/// Saint, gather White Cloth from the Platinum Tribe, and redeem it for Blooded
/// Fabric.
#[test]
fn quest_q00348_an_arrogant_search() {
    use crate::model::components::social::Quests;

    const HANELLIN: i32 = 30864;
    const TABLE_OF_VISION: i32 = 31646;
    const CLAUDIA: i32 = 31001;
    const DRAKE: i32 = 20670;
    const PLATINUM: i32 = 20828;
    const EZEKIEL: i32 = 27296;
    const SHELL: i32 = 14857;
    const BOOK: i32 = 4397;
    const HEALING_POTION: i32 = 1061;
    const WHITE_CLOTH_PLATINUM: i32 = 4294;
    const BLOODED_FABRIC: i32 = 4295;
    let q = "Q00348_AnArrogantSearch";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (SHELL, "Shell of Monsters", true),
            (BOOK, "Book of Saint", true),
            (HEALING_POTION, "Healing Potion", true),
            (WHITE_CLOTH_PLATINUM, "White Cloth", true),
            (BLOODED_FABRIC, "Blooded Fabric", true),
        ],
    );
    for id in [DRAKE, PLATINUM, EZEKIEL] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 62;
        world.data.npc_data.insert_for_test(t);
    }
    let hanellin = NPC_OID;
    let table = NPC_OID + 1;
    let drake = NPC_OID + 2;
    let platinum = NPC_OID + 3;
    let claudia = NPC_OID + 4;
    add_test_npc(&mut world, hanellin, HANELLIN, "Folk", 62, 100, 200, 0);
    add_test_npc(&mut world, table, TABLE_OF_VISION, "Folk", 62, 110, 200, 0);
    add_test_npc(&mut world, drake, DRAKE, "Monster", 62, 300, 300, 0);
    add_test_npc(&mut world, platinum, PLATINUM, "Monster", 62, 320, 300, 0);
    add_test_npc(&mut world, claudia, CLAUDIA, "Folk", 62, 120, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 60;

    let cond = |w: &World| quest_cond(w, 3001, q);
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };

    // Accept → cond 2.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{hanellin}_Quest {q}")),
    );
    ev(&mut world, hanellin, "30864-03.htm");
    assert_eq!(cond(&world), Some(2), "accepted → cond 2");

    // A Drake kill (forced coin-flip) → Shell of Monsters, cond 3.
    world.force_roll(0); // roll(2)=0 → the coin flip lands
    quests::notify_kill(&mut world, 3001, drake, DRAKE, false);
    assert_eq!(
        item_count(&world, 3001, SHELL),
        1,
        "the drake dropped a Shell"
    );
    assert_eq!(cond(&world), Some(3), "→ cond 3");

    // Return the shell (cond 3 → 4 → 5).
    ev(&mut world, hanellin, "30864-04.htm");
    assert_eq!(cond(&world), Some(4), "shell returned → cond 4");
    ev(&mut world, hanellin, "30864-05.htm");
    assert_eq!(cond(&world), Some(5), "→ cond 5");

    // Claudia Athebalt points the way: Java's `addRadar` on the Table of
    // Vision, which `Radar.addMarker` sends as a clear/show pair.
    let radar = |pkts: &[Vec<u8>]| -> Vec<(i32, i32, i32, i32, i32)> {
        pkts.iter()
            .filter(|p| p[0] == 0xF1)
            .map(|p| {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                (
                    r.read_i32().unwrap(),
                    r.read_i32().unwrap(),
                    r.read_i32().unwrap(),
                    r.read_i32().unwrap(),
                    r.read_i32().unwrap(),
                )
            })
            .collect()
    };
    drain(&mut rx);
    ev(&mut world, claudia, "31001-01.htm");
    assert_eq!(
        radar(&drain(&mut rx)),
        vec![(2, 2, 120112, 30912, -3616), (0, 1, 120112, 30912, -3616),],
        "Claudia pins the Table of Vision"
    );

    // The Table of Vision summons Stone Watchman Ezekiel, and arriving retires
    // the ping — Java sends a raw RadarControl(2, 2, 0, 0, 0), clearing the
    // whole board rather than that one marker.
    ev(&mut world, table, "31646-01.htm");
    assert_eq!(
        radar(&drain(&mut rx)),
        vec![(2, 2, 0, 0, 0)],
        "arriving clears the board"
    );
    let ezekiel = *npcs_of(&mut world, EZEKIEL)
        .first()
        .expect("Ezekiel summoned");

    // Slay Ezekiel → Book of Saint, cond 6.
    quests::notify_kill(&mut world, 3001, ezekiel, EZEKIEL, false);
    assert_eq!(
        item_count(&world, 3001, BOOK),
        1,
        "Ezekiel dropped the Book of Saint"
    );
    assert_eq!(cond(&world), Some(6), "→ cond 6");

    // cond 6 → 7, then spend a Healing Potion → the Platinum path (cond 8).
    ev(&mut world, hanellin, "30864-06.htm");
    assert_eq!(cond(&world), Some(7), "→ cond 7");
    inject(&mut world, 3001, 0x0034_8000, HEALING_POTION, 1);
    ev(&mut world, hanellin, "30864-07.htm");
    assert_eq!(
        cond(&world),
        Some(8),
        "healing potion spent → cond 8 (Platinum path)"
    );

    // 100 White Cloth from the Platinum Tribe → cond 10.
    inject(&mut world, 3001, 0x0034_8100, WHITE_CLOTH_PLATINUM, 99); // top up to 100 on the kill
    world.force_roll(0); // roll_f64 → the cloth drops
    quests::notify_kill(&mut world, 3001, platinum, PLATINUM, false);
    assert_eq!(
        item_count(&world, 3001, WHITE_CLOTH_PLATINUM),
        100,
        "the 100th cloth dropped"
    );
    assert_eq!(cond(&world), Some(10), "100 cloth → cond 10");

    // Redeem for Blooded Fabric; the repeatable quest is forgotten.
    ev(&mut world, hanellin, "end.htm");
    assert_eq!(
        item_count(&world, 3001, BLOODED_FABRIC),
        1,
        "Blooded Fabric awarded"
    );
    assert!(
        world
            .objects
            .get_component::<Quests>(&3001)
            .is_none_or(|qc| !qc.0.contains_key(q)),
        "the repeatable quest is forgotten on completion"
    );
}

/// Quest 333 (Hunt of the Black Lion) — the core mercenary spine: take an order,
/// hunt for trophy materials + cargo, turn the materials in to Sophya for adena,
/// assemble a Statue of Shilen, and complete with a Black Lion Mark.
#[test]
fn quest_q00333_hunt_of_the_black_lion() {
    use crate::model::components::social::Quests;

    const SOPHYA: i32 = 30735;
    const UNDRIAS: i32 = 30130;
    const RUPIO: i32 = 30471;
    const NEER_CRAWLER: i32 = 20160; // order-1 mob: ash 50%, cargo 11%
    const BLACK_LION_MARK: i32 = 1369;
    const ORDER_1: i32 = 3671;
    const UNDEAD_ASH: i32 = 3848;
    const CARGO_1: i32 = 3440;
    const STATUE_HEAD: i32 = 3457;
    const COMPLETE_STATUE: i32 = 3461;
    const ADENA: i32 = 57;
    let q = "Q00333_HuntOfTheBlackLion";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (BLACK_LION_MARK, "Black Lion Mark", true),
            (ORDER_1, "Sophya's 1st Order", true),
            (UNDEAD_ASH, "Undead Ash", true),
            (CARGO_1, "Cargo Box 1st", true),
            (STATUE_HEAD, "Statue Head", true),
            (3458, "Statue Torso", true),
            (3459, "Statue Arm", true),
            (3460, "Statue Leg", true),
            (COMPLETE_STATUE, "Complete Statue", true),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(NEER_CRAWLER);
        t.type_name = "Monster".into();
        t.level = 27;
        world.data.npc_data.insert_for_test(t);
    }
    let sophya = NPC_OID;
    let undrias = NPC_OID + 1;
    let rupio = NPC_OID + 2;
    let mob = NPC_OID + 3;
    add_test_npc(&mut world, sophya, SOPHYA, "Folk", 27, 100, 200, 0);
    add_test_npc(&mut world, undrias, UNDRIAS, "Folk", 27, 110, 200, 0);
    add_test_npc(&mut world, rupio, RUPIO, "Folk", 27, 120, 200, 0);
    add_test_npc(&mut world, mob, NEER_CRAWLER, "Monster", 27, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 25;
    inject(&mut world, 3001, 0x0033_3000, BLACK_LION_MARK, 1);

    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let adena = |w: &World| item_count(w, 3001, ADENA);

    // Accept and take the 1st hunting order.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sophya}_Quest {q}")),
    );
    ev(&mut world, sophya, "30735-04.htm");
    ev(&mut world, sophya, "30735-10.html");
    assert_eq!(
        item_count(&world, 3001, ORDER_1),
        1,
        "took Sophya's 1st order"
    );

    // A Neer Crawler kill (order held) drops Undead Ash and a Cargo Box.
    world.force_roll(0); // material roll 0 < 50
    world.force_roll(0); // cargo roll 0 < 11
    quests::notify_kill(&mut world, 3001, mob, NEER_CRAWLER, false);
    assert_eq!(
        item_count(&world, 3001, UNDEAD_ASH),
        1,
        "the kill dropped Undead Ash"
    );
    assert_eq!(item_count(&world, 3001, CARGO_1), 1, "and a Cargo Box");

    // Turn the material in to Sophya (talk): 1 ash → 10 adena, ash cleared.
    let before = adena(&world);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sophya}_Quest {q}")),
    );
    assert_eq!(
        adena(&world) - before,
        10,
        "1 Undead Ash turned in for 10 adena"
    );
    assert_eq!(
        item_count(&world, 3001, UNDEAD_ASH),
        0,
        "the material is spent"
    );

    // Assemble a Statue of Shilen from four parts (forced success).
    for p in [STATUE_HEAD, 3458, 3459, 3460] {
        inject(&mut world, 3001, 0x0033_3100 + p, p, 1);
    }
    world.force_roll(0); // roll(100) 0 < 50 → assembly succeeds
    ev(&mut world, rupio, "30471-03.html");
    assert_eq!(
        item_count(&world, 3001, COMPLETE_STATUE),
        1,
        "statue assembled"
    );
    assert_eq!(
        item_count(&world, 3001, STATUE_HEAD),
        0,
        "the parts are consumed"
    );

    // Undrias buys the complete statue for 30,000 adena.
    let before = adena(&world);
    ev(&mut world, undrias, "30130-04.html");
    assert_eq!(
        adena(&world) - before,
        30000,
        "the statue paid 30,000 adena"
    );

    // Complete the quest with the Black Lion Mark for 12,400 adena.
    let before = adena(&world);
    ev(&mut world, sophya, "30735-26.html");
    assert_eq!(
        adena(&world) - before,
        12400,
        "completion paid 12,400 adena"
    );
    assert!(
        world
            .objects
            .get_component::<Quests>(&3001)
            .is_none_or(|qc| !qc.0.contains_key(q)),
        "the repeatable quest is forgotten on completion"
    );
}
