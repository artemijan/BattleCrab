//! Q00210-Q00235 — the class trials, testimonies and tests: the
//! certification chain a character walks before each class change.

use super::*;

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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 15;
    drain_db(&mut db_rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-03.htm")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "Lundy started the quest"
    );

    // An out-of-order click is refused: Bynn (cond 2) while still at cond 1.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(1),
        "cond guard holds — no skipping ahead"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bella}_Quest {q} 30256-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bynn}_Quest {q} 30335-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sydnia}_Quest {q} 30321-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));

    assert_eq!(
        item_count(&world, 3001, 2375),
        0,
        "no collar until the payout"
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q} 30827-05.html")),
    );
    assert_eq!(item_count(&world, 3001, 2375), 1, "Wolf Collar rewarded");
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
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
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 14;
    drain(&mut rx);

    let q = "Q00210_ObtainAWolfPet";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lundy}_Quest {q}")),
    );
    // `no_level.htm` is a `.htm` file, so it ships as ExNpcQuestHtmlMessage
    // (the quest window), not a plain NpcHtmlMessage.
    let decode_quest_html = |pkt: &[u8]| -> Option<String> {
        if pkt[0] != server_packets::opcodes::EX
            || i16::from_le_bytes([pkt[1], pkt[2]])
                != server_packets::opcodes::EX_NPC_QUEST_HTML_MESSAGE
        {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&pkt[3..]);
        r.read_i32()?;
        r.read_string()
    };
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_quest_html(p))
        .expect("quest html");
    assert!(
        html.contains("level requirements") || html.contains("level 15"),
        "the level gate, got: {html}"
    );
    // The talk creates a CREATED state (Java `getQuestState(player, true)`) but
    // the gate keeps it un-started (cond 0, never `startQuest`).
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(!quests.0[q].is_started(), "the quest never started");
}

#[test]
fn quest_q00235_mimirs_elixir() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (5011, "Star of Destiny", false),
            (6320, "Pure Silver", true),
            (6321, "True Gold", true),
            (6322, "Sage Stone", true),
            (6318, "Blood Fire", true),
            (6319, "Mimir's Elixir", true),
            (5905, "Magister Mixing Stone", true),
            (729, "Scroll: Enchant Weapon (A)", false),
        ],
    );
    for id in [20965, 21090] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 75;
        world.data.npc_data.insert_for_test(t);
    }
    let ladd = NPC_OID;
    let joan = NPC_OID + 1;
    let urn = NPC_OID + 2;
    add_test_npc(&mut world, ladd, 30721, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, joan, 30718, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, urn, 31149, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 76;
    inject(&mut world, 3001, 0x0235_9000, 5011, 1); // Star of Destiny (Fate's Whisper prereq)
    let q = "Q00235_MimirsElixir";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, ladd);
    ev(&mut world, ladd, "30721-06.htm"); // start, cond 1
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // cond 1 → 2 needs Pure Silver (the Q00373 product).
    inject(&mut world, 3001, 0x0235_0000, 6320, 1);
    talk(&mut world, ladd);
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Joan → cond 3, then a Sage Stone drop → cond 4.
    ev(&mut world, joan, "30718-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    add_test_npc(&mut world, NPC_OID + 10, 20965, "Monster", 75, 30, 0, 0);
    world.force_roll(0); // roll(10) < 2 → Sage Stone
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(item_count(&world, 3001, 6322), 1, "Sage Stone drops");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Joan forges True Gold (cond 4 → 5).
    talk(&mut world, joan);
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    assert_eq!(item_count(&world, 3001, 6321), 1, "True Gold forged");
    assert_eq!(item_count(&world, 3001, 6322), 0, "Sage Stone consumed");
    // Ladd hands over the Magister Mixing Stone (cond 5 → 6).
    ev(&mut world, ladd, "30721-12.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    assert_eq!(item_count(&world, 3001, 5905), 1, "Mixing Stone received");
    // A Blood Fire drop → cond 7.
    add_test_npc(&mut world, NPC_OID + 11, 21090, "Monster", 75, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 11, 3001);
    assert_eq!(item_count(&world, 3001, 6318), 1, "Blood Fire drops");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // Mix at the Urn → Mimir's Elixir (cond 7 → 8), consuming silver/gold/fire.
    ev(&mut world, urn, "31149-success.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    assert_eq!(item_count(&world, 3001, 6319), 1, "Mimir's Elixir brewed");
    assert_eq!(
        item_count(&world, 3001, 6320)
            + item_count(&world, 3001, 6321)
            + item_count(&world, 3001, 6318),
        0,
        "silver/gold/fire consumed"
    );
    // Ladd redeems the elixir for the A-grade enchant scroll and finishes.
    ev(&mut world, ladd, "30721-16.htm");
    assert_eq!(
        item_count(&world, 3001, 729),
        1,
        "Scroll: Enchant Weapon (A) awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 5011),
        0,
        "Star of Destiny consumed"
    );
    assert_eq!(item_count(&world, 3001, 6319), 0, "elixir consumed");
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(8),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00222_test_of_the_duelist() {
    let (mut world, _db, _l) = quest_test_world();
    // 5 orders + 10 stage-1 trophies + final order + 5 stage-2 trophies + mark.
    let mut items: Vec<(i32, &str, bool)> = (2762..=2783).map(|id| (id, "Q222", true)).collect();
    items.push((2762, "Mark of Duelist", false)); // reward (overwrites, non-quest)
    add_quest_items(&mut world, &items);
    let stage1_mobs = [
        20085, 20090, 20202, 20234, 20270, 20552, 20564, 20582, 20601, 20602,
    ];
    let stage2_mobs = [20214, 20217, 20554, 20588, 20604];
    for &id in stage1_mobs.iter().chain(stage2_mobs.iter()) {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30623, "Folk", 40, 100, 0, 0); // Kaien
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 1; // Warrior
    }
    let q = "Q00222_TestOfTheDuelist";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} ACCEPT")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 2763), 1, "Order of Gludio given");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30623-08.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Stage 1: 9 of each of 10 trophies pre-stocked, then one kill of each mob.
    // The kill counter reaches 9 exactly as the 10th kill fills the last trophy.
    for id in 2768..=2777 {
        inject(&mut world, 3001, 0x0222_0000 + id, id, 9);
    }
    let mut oid = NPC_OID + 100;
    for &mob in &stage1_mobs {
        add_test_npc(&mut world, oid, mob, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(&mut world, oid, 3001);
        oid += 1;
    }
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "all ten trophies + kill counter → cond 3"
    );
    assert_eq!(
        item_count(&world, 3001, 2768),
        10,
        "Puncher's Shard capped at 10"
    );
    // Turn in the ten trophies + five orders for the Final Order (memo 2, cond 4).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30623-16.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    assert_eq!(item_count(&world, 3001, 2778), 1, "Final Order received");
    assert_eq!(
        item_count(&world, 3001, 2768),
        0,
        "stage-1 trophies consumed"
    );
    assert_eq!(item_count(&world, 3001, 2763), 0, "region orders consumed");
    // Stage 2: 2 each of four trophies + 1 of the fifth, then kills. The counter
    // needs >= 5, so the fifth trophy takes two kills to cap.
    for id in [2779, 2780, 2781, 2782] {
        inject(&mut world, 3001, 0x0222_1000 + id, id, 2);
    }
    inject(&mut world, 3001, 0x0222_1000 + 2783, 2783, 1);
    let s2_kills = [20214, 20217, 20554, 20588, 20604, 20604]; // last mob twice
    for &mob in &s2_kills {
        add_test_npc(&mut world, oid, mob, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(&mut world, oid, 3001);
        oid += 1;
    }
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(5),
        "stage-2 trophies + counter → cond 5"
    );
    // Kaien awards the Mark of Duelist and finishes.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 2762), 1, "Mark of Duelist awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(5),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00231_test_of_the_maestro() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        2864, 2865, 2866, 2868, 2869, 2870, 2871, 2872, 2873, 2874, 2875, 2876, 2877, 2878, 2916,
    ]
    .iter()
    .map(|&id| (id, "Q231", true))
    .collect();
    items.push((2867, "Mark of Maestro", false));
    add_quest_items(&mut world, &items);
    for id in [27133, 20225, 20150] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let lockirin = NPC_OID;
    let balanki = NPC_OID + 1;
    let filaur = NPC_OID + 2;
    let arin = NPC_OID + 3;
    let toma = NPC_OID + 4;
    let croto = NPC_OID + 5;
    let lorain = NPC_OID + 6;
    for (oid, npc) in [
        (lockirin, 30531),
        (balanki, 30533),
        (filaur, 30535),
        (arin, 30536),
        (toma, 30556),
        (croto, 30671),
        (lorain, 30673),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 56; // Artisan
    }
    let q = "Q00231_TestOfTheMaestro";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    talk(&mut world, lockirin);
    ev(&mut world, lockirin, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Balanki's recommendation (memo 2): Croto → Evil Eye Lord → letter. ---
    talk(&mut world, balanki);
    ev(&mut world, balanki, "30533-02.html"); // memo 2
    talk(&mut world, croto);
    ev(&mut world, croto, "30671-02.html"); // Paint of Kamuru
    assert_eq!(item_count(&world, 3001, 2869), 1, "Paint of Kamuru");
    add_test_npc(&mut world, NPC_OID + 20, 27133, "Monster", 40, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 20, 3001);
    assert_eq!(
        item_count(&world, 3001, 2870),
        1,
        "Evil Eye Lord drops the Necklace of Kamutu"
    );
    talk(&mut world, croto); // necklace → Letter of Solder Detachment
    assert_eq!(item_count(&world, 3001, 2868), 1, "Letter received");
    talk(&mut world, balanki); // letter → Recommendation of Balanki
    assert_eq!(
        item_count(&world, 3001, 2864),
        1,
        "Recommendation of Balanki"
    );
    // --- Arin's recommendation (memo 3): Toma's teleport-device errand. ---
    talk(&mut world, arin); // Paint of Teleport Device, memo 3
    assert_eq!(
        item_count(&world, 3001, 2871),
        1,
        "Paint of Teleport Device"
    );
    talk(&mut world, toma);
    ev(&mut world, toma, "30556-05.html"); // Broken device + teleport + timer
    assert_eq!(item_count(&world, 3001, 2916), 1, "Broken Teleport Device");
    // The 5-second timer conjures three King Bugbears at the arrival spot.
    advance_ticks(&mut world, 50);
    assert_eq!(
        npcs_of(&mut world, 20150).len(),
        3,
        "on_timer ambush spawns three King Bugbears"
    );
    // The errand teleported the player to Cruma; walk back to the NPCs so the
    // bypass interaction-distance guard lets the remaining turn-ins through.
    {
        let pos = world.objects.get_component_mut::<Position>(&3001).unwrap();
        pos.x = 100;
        pos.y = 0;
        pos.z = 0;
    }
    talk(&mut world, toma); // broken → 5 Teleport Devices
    assert_eq!(item_count(&world, 3001, 2872), 5, "5 Teleport Devices");
    talk(&mut world, arin); // devices → Recommendation of Arin
    assert_eq!(item_count(&world, 3001, 2866), 1, "Recommendation of Arin");
    // --- Filaur's recommendation (memo 4): Lorain's antidote errand. ---
    talk(&mut world, filaur); // Architecture of Cruma, memo 4
    talk(&mut world, lorain); // Ingredients of Antidote
    assert_eq!(item_count(&world, 3001, 2875), 1, "Ingredients of Antidote");
    add_test_npc(&mut world, NPC_OID + 21, 20225, "Monster", 40, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 21, 3001);
    assert_eq!(
        item_count(&world, 3001, 2878),
        1,
        "Giant Mist Leech drops Blood of Leech"
    );
    // Fast-forward the rest of the antidote reagents (no kill counter here).
    inject(&mut world, 3001, 0x0231_0000, 2878, 9); // 10 leech blood
    inject(&mut world, 3001, 0x0231_0001, 2876, 10); // wasp needle
    inject(&mut world, 3001, 0x0231_0002, 2877, 10); // spider web
    ev(&mut world, lorain, "30673-04.html"); // → Report of Cruma
    assert_eq!(item_count(&world, 3001, 2874), 1, "Report of Cruma");
    talk(&mut world, filaur); // report → Recommendation of Filaur, cond 2
    assert_eq!(
        item_count(&world, 3001, 2865),
        1,
        "Recommendation of Filaur"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "all three recommendations → cond 2"
    );
    // --- Lockirin awards the Mark of Maestro. ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, lockirin);
    assert_eq!(item_count(&world, 3001, 2867), 1, "Mark of Maestro awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00223_test_of_the_champion() {
    let (mut world, _db, _l) = quest_test_world();
    let qitems: Vec<(i32, &str, bool)> = [
        3277, 3278, 3279, 3280, 3281, 3282, 3283, 3284, 3285, 3286, 3287, 3288, 3289, 3290, 3291,
        3292,
    ]
    .iter()
    .map(|&id| (id, "Q223", true))
    .collect();
    let mut items = qitems;
    items.push((3276, "Mark of Champion", false));
    add_quest_items(&mut world, &items);
    for id in [20145, 20158, 20551, 20553, 20577, 20780] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0; // survive the on_attack probe without dying
        world.data.npc_data.insert_for_test(t);
    }
    let ascalon = NPC_OID;
    let groot = NPC_OID + 1;
    let mouen = NPC_OID + 2;
    let mason = NPC_OID + 3;
    for (oid, npc) in [
        (ascalon, 30624),
        (groot, 30093),
        (mouen, 30196),
        (mason, 30625),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 1; // Warrior
    }
    let q = "Q00223_TestOfTheChampion";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, ascalon);
    ev(&mut world, ascalon, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 3277), 1, "Ascalon's 1st Letter");
    // --- Leg 1: Mason → Iron Rose Ring → Bloody Axe Elite heads. ---
    talk(&mut world, mason);
    ev(&mut world, mason, "30625-03.html");
    assert_eq!(item_count(&world, 3001, 3279), 1, "Iron Rose Ring");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // onAttack ambush: first hit on an elite (ring held, heads<10, roll 0) conjures a second elite.
    add_test_npc(&mut world, NPC_OID + 20, 20780, "Monster", 40, 40, 0, 0);
    world.force_roll(0); // roll(2) == 0 → spawn
    combat::npc_receive_damage(&mut world, NPC_OID + 20, 3001, 10.0, false);
    assert_eq!(
        npcs_of(&mut world, 20780).len(),
        2,
        "on_attack conjures a second Bloody Axe Elite"
    );
    inject(&mut world, 3001, 0x0223_0000, 3290, 9); // 9 heads
    kill(&mut world, 20780); // 10th head → cond 3
    assert_eq!(
        item_count(&world, 3001, 3290),
        10,
        "Bloody Axe Head reaches 10"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, mason); // ring + heads → Mason's Letter
    assert_eq!(item_count(&world, 3001, 3278), 1, "Mason's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Leg 2: Ascalon relay → Groot → White Rose Insignia. ---
    ev(&mut world, ascalon, "30624-10.html"); // Mason's Letter → 2nd Letter, cond 5
    assert_eq!(item_count(&world, 3001, 3280), 1, "Ascalon's 2nd Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, groot);
    ev(&mut world, groot, "30093-02.html"); // 2nd Letter → Insignia, cond 6
    assert_eq!(item_count(&world, 3001, 3281), 1, "White Rose Insignia");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // --- Leg 3: hunt 30 each of eggs/venom/bile → cond 7. ---
    kill(&mut world, 20145); // one real Harpy kill drops 2 eggs
    assert_eq!(
        item_count(&world, 3001, 3287),
        2,
        "Harpy drops eggs (insignia-gated)"
    );
    inject(&mut world, 3001, 0x0223_0001, 3287, 28); // eggs → 30
    inject(&mut world, 3001, 0x0223_0002, 3288, 30); // venom → 30
    inject(&mut world, 3001, 0x0223_0003, 3289, 27); // bile → 27
    kill(&mut world, 20553); // Windsus: 27 → 30, all three complete → cond 7
    assert_eq!(
        item_count(&world, 3001, 3289),
        30,
        "Windsus Bile reaches 30"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, groot); // insignia + all → Groot's Letter, cond 8
    assert_eq!(item_count(&world, 3001, 3282), 1, "Groot's Letter");
    assert_eq!(item_count(&world, 3001, 3287), 0, "eggs consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    // --- Leg 4: Ascalon relay → Mouen → 1st Order. ---
    ev(&mut world, ascalon, "30624-14.html"); // Groot's Letter → 3rd Letter, cond 9
    assert_eq!(item_count(&world, 3001, 3283), 1, "Ascalon's 3rd Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, mouen);
    ev(&mut world, mouen, "30196-03.html"); // 3rd Letter → 1st Order, cond 10
    assert_eq!(item_count(&world, 3001, 3284), 1, "Mouen's 1st Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // --- Leg 5: Road Scavenger ratman heads → 2nd Order. ---
    inject(&mut world, 3001, 0x0223_0004, 3291, 9); // 9 ratman heads
    kill(&mut world, 20551); // 10th → cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    ev(&mut world, mouen, "30196-06.html"); // 1st Order + heads → 2nd Order, cond 12
    assert_eq!(item_count(&world, 3001, 3285), 1, "Mouen's 2nd Order");
    assert_eq!(item_count(&world, 3001, 3291), 0, "ratman heads consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    // --- Leg 6: Leto Lizardman fangs → Mouen's Letter. ---
    inject(&mut world, 3001, 0x0223_0005, 3292, 9); // 9 fangs
    kill(&mut world, 20577); // 10th → cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, mouen); // 2nd Order + fangs → Mouen's Letter, cond 14
    assert_eq!(item_count(&world, 3001, 3286), 1, "Mouen's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    // --- Completion: Ascalon awards the Mark of the Champion. ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, ascalon);
    assert_eq!(
        item_count(&world, 3001, 3276),
        1,
        "Mark of the Champion awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 229764,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(14),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00224_test_of_sagittarius() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        3028, 3294, 3295, 3296, 3297, 3298, 3299, 3300, 3301, 3302, 3303, 3304, 3305, 3306,
    ]
    .iter()
    .map(|&id| (id, "Q224", true))
    .collect();
    items.push((3293, "Mark of Sagittarius", false));
    items.push((17, "Wooden Arrow", false));
    add_quest_items(&mut world, &items);
    for id in [20079, 20269, 20233, 20230, 20577, 27090] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let bernard = NPC_OID;
    let vokian = NPC_OID + 1;
    let hamil = NPC_OID + 2;
    let aron = NPC_OID + 3;
    let gauen = NPC_OID + 4;
    for (oid, npc) in [
        (bernard, 30702),
        (vokian, 30514),
        (hamil, 30626),
        (aron, 30653),
        (gauen, 30717),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 7; // Rogue
    }
    let q = "Q00224_TestOfSagittarius";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, bernard);
    ev(&mut world, bernard, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 3294), 1, "Bernard's Introduction");
    // --- Hamil → Aron letter relay. ---
    talk(&mut world, hamil);
    ev(&mut world, hamil, "30626-03.html"); // intro → 1st Letter, memo/cond 2
    assert_eq!(item_count(&world, 3001, 3295), 1, "Hamil's 1st Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, aron);
    ev(&mut world, aron, "30653-02.html"); // 1st Letter → memo/cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // --- Leg: 10 Hunter's 1st Runes from ants → memo/cond 4. ---
    inject(&mut world, 3001, 0x0224_0000, 3298, 9);
    kill(&mut world, 20079); // 10th rune → memo 4
    assert_eq!(
        item_count(&world, 3001, 3298),
        10,
        "Hunter's 1st Rune reaches 10"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, hamil);
    ev(&mut world, hamil, "30626-07.html"); // runes → 2nd Letter, memo/cond 5
    assert_eq!(item_count(&world, 3001, 3296), 1, "Hamil's 2nd Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, vokian);
    ev(&mut world, vokian, "30514-02.html"); // 2nd Letter → memo/cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // --- Leg: 10 Hunter's 2nd Runes from Breka orcs → memo/cond 7 + Talisman of Snake. ---
    inject(&mut world, 3001, 0x0224_0001, 3299, 9);
    kill(&mut world, 20269); // 10th → memo 7 + Talisman of Snake
    assert_eq!(item_count(&world, 3001, 3301), 1, "Talisman of Snake");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, vokian); // Talisman of Snake → memo/cond 8
    assert_eq!(
        item_count(&world, 3001, 3301),
        0,
        "Talisman of Snake consumed"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, hamil); // → 3rd Letter, memo/cond 9
    assert_eq!(item_count(&world, 3001, 3297), 1, "Hamil's 3rd Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, gauen); // 3rd Letter → memo/cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // --- Leg: four bow-materials, set completes in any order → memo/cond 11. ---
    kill(&mut world, 20233); // Marsh Spider: bowstring, but set incomplete → still memo 10
    assert_eq!(item_count(&world, 3001, 3304), 1, "Reinforced Bowstring");
    assert_eq!(
        quest_memo(&world, 3001, q),
        10,
        "set incomplete: no advance"
    );
    inject(&mut world, 3001, 0x0224_0002, 3302, 1); // Mithril Clip
    inject(&mut world, 3001, 0x0224_0003, 3305, 1); // Manashen's Horn
    kill(&mut world, 20230); // Stakato: chitin completes the set → memo/cond 11
    assert_eq!(item_count(&world, 3001, 3303), 1, "Stakato Chitin");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, gauen); // materials → Crescent Moon Bow + arrows, memo/cond 12
    assert_eq!(item_count(&world, 3001, 3028), 1, "Crescent Moon Bow");
    assert_eq!(item_count(&world, 3001, 17), 10, "Wooden Arrows");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, hamil); // bow → memo/cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    // --- Blood farming spawns Serpent Demon Kadesh probabilistically. ---
    inject(&mut world, 3001, 0x0224_0004, 3306, 5); // low stack → else branch
    kill(&mut world, 20577); // ((5-10)*5) < roll → just one more Blood
    assert_eq!(
        item_count(&world, 3001, 3306),
        6,
        "low stack: Blood accrues, no spawn"
    );
    let before = npcs_of(&mut world, 27090).len();
    inject(&mut world, 3001, 0x0224_0005, 3306, 24); // stack now 30
    world.force_roll(0); // ((30-10)*5)=100 > 0 → spawn Kadesh
    kill(&mut world, 20577);
    assert_eq!(
        npcs_of(&mut world, 27090).len(),
        before + 1,
        "Kadesh conjured"
    );
    assert_eq!(
        item_count(&world, 3001, 3306),
        0,
        "Blood consumed on the summon"
    );
    // --- Kadesh with the wrong weapon: no Talisman, he respawns. ---
    kill(&mut world, 27090); // no weapon equipped → respawn, no reward
    assert_eq!(
        item_count(&world, 3001, 3300),
        0,
        "wrong weapon: no Talisman of Kadesh"
    );
    assert_eq!(
        quest_memo(&world, 3001, q),
        13,
        "wrong weapon: still memo 13"
    );
    assert!(
        !npcs_of(&mut world, 27090).is_empty(),
        "wrong-weapon kill respawns Kadesh"
    );
    // --- Equip the Crescent Moon Bow and fell him properly. ---
    equip_weapon_row(&mut world, 3001, 3028);
    kill(&mut world, 27090); // killing blow with the bow → Talisman of Kadesh, memo/cond 14
    assert_eq!(
        item_count(&world, 3001, 3300),
        1,
        "Talisman of Kadesh awarded"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    // --- Completion. ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, hamil);
    assert_eq!(
        item_count(&world, 3001, 3293),
        1,
        "Mark of Sagittarius awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(14),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00225_test_of_the_searcher() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2784..=2808).map(|id| (id, "Q225", true)).collect();
    items.push((2809, "Mark of Searcher", false));
    add_quest_items(&mut world, &items);
    for id in [20781, 27093, 20555, 20551, 20144, 27092] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    // The Ancient Tree conjures a Strong Wooden Chest; it needs a template.
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(30628));
    let luther = NPC_OID;
    let alex = NPC_OID + 1;
    let leirynn = NPC_OID + 2;
    let borys = NPC_OID + 3;
    let tyra = NPC_OID + 4;
    let jax = NPC_OID + 5;
    let tree = NPC_OID + 6;
    let chest = NPC_OID + 7;
    for (oid, npc) in [
        (luther, 30690),
        (alex, 30291),
        (leirynn, 30728),
        (borys, 30729),
        (tyra, 30420),
        (jax, 30730),
        (tree, 30627),
        (chest, 30628),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 7; // Rogue
    }
    let q = "Q00225_TestOfTheSearcher";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, luther);
    ev(&mut world, luther, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 2784), 1, "Luther's Letter");
    talk(&mut world, alex); // Luther's Letter → Alex's Warrant, cond 2
    assert_eq!(item_count(&world, 3001, 2785), 1, "Alex's Warrant");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, leirynn); // Warrant → 1st Order, cond 3
    assert_eq!(item_count(&world, 3001, 2786), 1, "Leirynn's 1st Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // onAttack: first hit on a Delu Shaman (1st Order held) conjures a Neer Bodyguard.
    add_test_npc(&mut world, NPC_OID + 20, 20781, "Monster", 40, 40, 0, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 20, 3001, 10.0, false);
    assert_eq!(
        npcs_of(&mut world, 27092).len(),
        1,
        "onAttack conjures a Neer Bodyguard"
    );
    // Collect 10 Delu Totems (cond stays 3 — the totem leg advances at Leirynn).
    inject(&mut world, 3001, 0x0225_0000, 2787, 9);
    kill(&mut world, 20781);
    assert_eq!(item_count(&world, 3001, 2787), 10, "Delu Totem reaches 10");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "totem kill does not fire the dead cond 4"
    );
    talk(&mut world, leirynn); // 1st Order + totems → 2nd Order, cond 5
    assert_eq!(item_count(&world, 3001, 2788), 1, "Leirynn's 2nd Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    kill(&mut world, 27093); // Delu Chief Kalkis → fang + Stringe's Map, cond 6
    assert_eq!(item_count(&world, 3001, 2789), 1, "Chief Kalkis's Fang");
    assert_eq!(item_count(&world, 3001, 2791), 1, "Stringe's Map");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, leirynn); // 2nd Order + fang → Report, cond 7
    assert_eq!(item_count(&world, 3001, 2790), 1, "Leirynn's Report");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, alex);
    ev(&mut world, alex, "30291-07.html"); // Report + Stringe → Lambert's Map + Letter + Order, cond 8
    assert_eq!(item_count(&world, 3001, 2792), 1, "Lambert's Map");
    assert_eq!(item_count(&world, 3001, 2793), 1, "Alex's Letter");
    assert_eq!(item_count(&world, 3001, 2794), 1, "Alex's Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, borys); // Letter → Wine Catalog, cond 9
    assert_eq!(item_count(&world, 3001, 2795), 1, "Wine Catalog");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, tyra);
    ev(&mut world, tyra, "30420-01a.html"); // Catalog → Tyra's Contract, cond 10
    assert_eq!(item_count(&world, 3001, 2796), 1, "Tyra's Contract");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    inject(&mut world, 3001, 0x0225_0001, 2797, 9); // 9 Red Spore Dust
    kill(&mut world, 20555); // 10th dust → cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, tyra); // Contract + dust → Malrukian Wine, cond 12
    assert_eq!(item_count(&world, 3001, 2798), 1, "Malrukian Wine");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, borys); // Wine → Old Order, cond 13
    assert_eq!(item_count(&world, 3001, 2799), 1, "Old Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, jax);
    ev(&mut world, jax, "30730-01d.html"); // Old Order → Jax's Diary, cond 14
    assert_eq!(item_count(&world, 3001, 2800), 1, "Jax's Diary");
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    // --- Map pieces: Solt's (deterministic) + Makel's (50/50) → cond 15. ---
    kill(&mut world, 20551); // Road Scavenger: first torn piece
    assert_eq!(
        item_count(&world, 3001, 2801),
        1,
        "Torn Map Piece 1st accrues"
    );
    inject(&mut world, 3001, 0x0225_0002, 2801, 2); // → 3 pieces
    kill(&mut world, 20551); // at 3 → Solt's Map (no cond 15 yet, Makel's absent)
    assert_eq!(item_count(&world, 3001, 2803), 1, "Solt's Map");
    assert_eq!(
        item_count(&world, 3001, 2801),
        0,
        "torn 1st pieces consumed"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(15),
        "one map is not enough"
    );
    world.force_roll(0); // roll(100) < 50 → drop
    kill(&mut world, 20144); // Hangman Tree: first torn 2nd piece
    assert_eq!(
        item_count(&world, 3001, 2802),
        1,
        "Torn Map Piece 2nd accrues"
    );
    inject(&mut world, 3001, 0x0225_0003, 2802, 2); // → 3 pieces
    world.force_roll(0); // roll(100) < 50 → convert
    kill(&mut world, 20144); // at 3 → Makel's Map + cond 15 (Solt's held)
    assert_eq!(item_count(&world, 3001, 2804), 1, "Makel's Map");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, jax); // both maps → Combined Map, cond 16
    assert_eq!(item_count(&world, 3001, 2805), 1, "Combined Map");
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    // --- Ancient Tree → Rusted Key + a conjured chest; open it for gold. ---
    talk(&mut world, tree);
    ev(&mut world, tree, "30627-01a.html"); // Rusted Key + spawn chest, cond 17
    assert_eq!(item_count(&world, 3001, 2806), 1, "Rusted Key");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    assert!(
        !npcs_of(&mut world, 30628).is_empty(),
        "the tree conjures a Strong Wooden Chest"
    );
    // Java's `if (npc.getSummonedNpcCount() < 5)` wraps the whole block: the
    // tree stops after five chests, and a sixth attempt hands out no key
    // either. Re-enter the dialog until the cap bites. (The fixture also
    // places a *static* chest as a dialog target, so count what the tree
    // itself conjured, not every 30628 in the world.)
    for _ in 0..6 {
        ev(&mut world, tree, "30627-01a.html");
    }
    let conjured = world
        .objects
        .get_component::<model::components::summons::SummonedNpcs>(&tree)
        .map(|l| l.0.len())
        .unwrap_or(0);
    assert_eq!(
        conjured, 5,
        "the tree caps itself at five chests, however often it is asked"
    );
    // The guard wraps the key hand-out too, so those five attempts also paid
    // out five keys — take the surplus back so the walkthrough below reads the
    // ordinary one-key case.
    assert_eq!(
        item_count(&world, 3001, 2806),
        5,
        "one key per allowed spawn"
    );
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&3001) {
        inv.remove_item(2806, 4);
    }
    talk(&mut world, chest);
    ev(&mut world, chest, "30628-01a.html"); // key → 20 Gold Bars, cond 18, chest deleted
    assert_eq!(item_count(&world, 3001, 2807), 20, "20 Gold Bars");
    assert_eq!(item_count(&world, 3001, 2806), 0, "Rusted Key consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    talk(&mut world, alex); // Order + Combined Map + 20 gold → Alex's Recommend, cond 19
    assert_eq!(item_count(&world, 3001, 2808), 1, "Alex's Recommendation");
    assert_eq!(item_count(&world, 3001, 2807), 0, "gold handed over");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    // --- Completion at Luther. ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, luther);
    assert_eq!(
        item_count(&world, 3001, 2809),
        1,
        "Mark of the Searcher awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(19),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00211_trial_of_the_challenger() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [2628, 2629, 2630, 2631, 2632]
        .iter()
        .map(|&id| (id, "Q211", true))
        .collect();
    for id in [2627, 1904, 1936, 1940, 1943, 1946, 2918, 2927, 2030] {
        items.push((id, "reward", false));
    }
    add_quest_items(&mut world, &items);
    for id in [27110, 27112, 27113, 27114] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(30647));
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(30646));
    let kash = NPC_OID;
    let martian = NPC_OID + 1;
    let raldo = NPC_OID + 2;
    let filaur = NPC_OID + 3;
    let chest = NPC_OID + 4;
    for (oid, npc) in [
        (kash, 30644),
        (martian, 30645),
        (raldo, 30646),
        (filaur, 30535),
        (chest, 30647),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 1; // Warrior (WARRIOR_GROUP)
    }
    let q = "Q00211_TrialOfTheChallenger";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, kash);
    ev(&mut world, kash, "30644-06.htm"); // startQuest, cond 1
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    kill(&mut world, 27110); // Shyslassys → Scroll + Broken Key + chest, cond 2
    assert_eq!(item_count(&world, 3001, 2631), 1, "Scroll of Shyslassys");
    assert_eq!(item_count(&world, 3001, 2632), 1, "Broken Key");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert!(
        !npcs_of(&mut world, 30647).is_empty(),
        "Shyslassys conjures a chest"
    );
    // Chest gamble: force the jackpot (roll(10) < 2), then the top reward tier (roll > 90).
    world.force_roll(0); // roll(10) = 0 → jackpot
    world.force_roll(95); // roll(100) = 95 → top tier
    talk(&mut world, chest);
    ev(&mut world, chest, "30647-02.html");
    assert_eq!(
        item_count(&world, 3001, 2918),
        1,
        "jackpot: Mithril Scale Gaiters Material"
    );
    assert_eq!(
        item_count(&world, 3001, 2927),
        1,
        "jackpot: Brigamdine Gauntlet Pattern"
    );
    assert_eq!(
        item_count(&world, 3001, 2632),
        0,
        "Broken Key consumed by the gamble"
    );
    talk(&mut world, kash); // Scroll → Letter of Kash, cond 3
    assert_eq!(item_count(&world, 3001, 2628), 1, "Letter of Kash");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, martian);
    ev(&mut world, martian, "30645-02.html"); // cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    kill(&mut world, 27112); // Gorr → Watcher's Eye 1, cond 5
    assert_eq!(item_count(&world, 3001, 2629), 1, "Watcher's Eye 1");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, martian); // Eye 1 → cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    kill(&mut world, 27113); // Baraham → Watcher's Eye 2 + Raldo, cond 7
    assert_eq!(item_count(&world, 3001, 2630), 1, "Watcher's Eye 2");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, raldo);
    ev(&mut world, raldo, "30646-04.html"); // Eye 2 → cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, filaur); // cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    kill(&mut world, 27114); // Queen of Succubus → cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // Completion at Raldo.
    let a = item_count(&world, 3001, 57);
    talk(&mut world, raldo);
    assert_eq!(
        item_count(&world, 3001, 2627),
        1,
        "Mark of the Challenger awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 194556,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(10),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00212_trial_of_duty() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        2634, 2635, 2636, 2637, 2638, 2639, 2640, 2641, 2643, 2644, 2645, 2646, 3027,
    ]
    .iter()
    .map(|&id| (id, "Q212", true))
    .collect();
    items.push((2633, "Mark of Duty", false));
    add_quest_items(&mut world, &items);
    for id in [20190, 27119, 20200, 20144, 20577, 20270, 30656] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let hanna = NPC_OID;
    let aron = NPC_OID + 1;
    let kiel = NPC_OID + 2;
    let isael = NPC_OID + 3;
    let dustin = NPC_OID + 4;
    let collin = NPC_OID + 5;
    let talianus = NPC_OID + 6;
    for (oid, npc) in [
        (hanna, 30109),
        (aron, 30653),
        (kiel, 30654),
        (isael, 30655),
        (dustin, 30116),
        (collin, 30311),
        (talianus, 30656),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 4; // Knight (KNIGHT_GROUP)
    }
    world.data.categories.insert_for_test("KNIGHT_GROUP", &[4]);
    let q = "Q00212_TrialOfDuty";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, hanna);
    ev(&mut world, hanna, "quest_accept");
    assert_eq!(quest_memo(&world, 3001, q), 1, "accepted → memo 1");
    talk(&mut world, aron); // Old Knight's Sword, memo 2, cond 2
    assert_eq!(item_count(&world, 3001, 3027), 1, "Old Knight's Sword");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Escalating flag conjures Sir Herod's spirit off skeletons.
    kill(&mut world, 20190); // flag 0 → 1 (threshold 0, no spawn)
    assert!(
        npcs_of(&mut world, 27119).is_empty(),
        "flag 0: no spawn yet"
    );
    world.force_roll(5); // roll(100)=5 < flag(1)*10 → spawn
    kill(&mut world, 20190);
    assert_eq!(
        npcs_of(&mut world, 27119).len(),
        1,
        "flag 1: Sir Herod conjured"
    );
    // Only the Old Knight's Sword yields the Knight's Tear.
    equip_weapon_row(&mut world, 3001, 3027);
    kill(&mut world, 27119); // memo 2 + sword → Knight's Tear, memo 3, cond 3
    assert_eq!(item_count(&world, 3001, 2635), 1, "Knight's Tear");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, aron); // tear + sword → memo 4, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, kiel); // memo 5, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // 10 report pieces via giveItemRandomly.
    inject(&mut world, 3001, 0x0212_0000, 2638, 9);
    kill(&mut world, 20200); // Strain: 10th piece → Talianus's Report, cond 6
    assert_eq!(item_count(&world, 3001, 2639), 1, "Talianus's Report");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, kiel); // report → Mirror of Orpic, memo 6, cond 7
    assert_eq!(item_count(&world, 3001, 2636), 1, "Mirror of Orpic");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // Escalating flag conjures Sir Talianus's spirit off Hangman Trees (needs flag >= 4).
    for _ in 0..4 {
        kill(&mut world, 20144); // flags 0..3, thresholds <= 0: no spawn
    }
    assert!(
        npcs_of(&mut world, 30656).len() <= 1,
        "spirit not conjured below flag 4"
    );
    world.force_roll(10); // roll(100)=10 < (flag(4)-3)*33=33 → spawn
    kill(&mut world, 20144);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(8),
        "Sir Talianus conjured → cond 8"
    );
    talk(&mut world, talianus); // mirror + report → Tear of Confession, memo 7, cond 9
    assert_eq!(item_count(&world, 3001, 2637), 1, "Tear of Confession");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, kiel); // confession → memo 8, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, isael); // memo 9, cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    // 20 militia articles via giveItemRandomly.
    inject(&mut world, 3001, 0x0212_0001, 2641, 19);
    kill(&mut world, 20577); // Leto: 20th article → cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, isael); // 20 articles → Tear of Loyalty, memo 10, cond 13
    assert_eq!(item_count(&world, 3001, 2640), 1, "Tear of Loyalty");
    assert_eq!(
        item_count(&world, 3001, 2641),
        0,
        "militia articles consumed"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, dustin);
    ev(&mut world, dustin, "30116-05.html"); // loyalty → memo 11, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    // Athebaldt's three bones off Breka Orc Prefects.
    kill(&mut world, 20270); // skull
    kill(&mut world, 20270); // ribs
    kill(&mut world, 20270); // shin → cond 15
    assert_eq!(item_count(&world, 3001, 2645), 1, "Athebaldt's Shin");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, dustin); // bones → Saint's Ashes Urn, memo 12, cond 16
    assert_eq!(
        item_count(&world, 3001, 2641),
        1,
        "Saint's Ashes Urn (shares id 2641)"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, collin); // urn → Letter of Windawood, memo 13, cond 17
    assert_eq!(item_count(&world, 3001, 2646), 1, "Letter of Windawood");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    talk(&mut world, dustin); // Windawood → Letter of Dustin, memo 14, cond 18
    assert_eq!(item_count(&world, 3001, 2634), 1, "Letter of Dustin");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // Completion at Hannavalt.
    let a = item_count(&world, 3001, 57);
    talk(&mut world, hanna);
    assert_eq!(item_count(&world, 3001, 2633), 1, "Mark of Duty awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 138968,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(18),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00213_trial_of_the_seeker() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2647..=2672).map(|id| (id, "Q213", true)).collect();
    items.push((2673, "Mark of Seeker", false));
    add_quest_items(&mut world, &items);
    for id in [
        20198, 20211, 20495, 20080, 20249, 20158, 20234, 20270, 20088, 20580,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let dufner = NPC_OID;
    let terry = NPC_OID + 1;
    let viktor = NPC_OID + 2;
    let marina = NPC_OID + 3;
    let brunon = NPC_OID + 4;
    for (oid, npc) in [
        (dufner, 30106),
        (terry, 30064),
        (viktor, 30684),
        (marina, 30715),
        (brunon, 30526),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 7; // Rogue
    }
    let q = "Q00213_TrialOfTheSeeker";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, dufner);
    ev(&mut world, dufner, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 2647), 1, "Dufner's Letter");
    talk(&mut world, terry);
    ev(&mut world, terry, "30064-03.html"); // Dufner's Letter → 1st Order, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    world.force_roll(0); // getRandomBoolean → true
    kill(&mut world, 20198); // Neer Ghoul → Mysterious Spirit Ore, cond 3
    assert_eq!(item_count(&world, 3001, 2653), 1, "Mysterious Spirit Ore");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, terry);
    ev(&mut world, terry, "30064-06.html"); // 1st Order → 2nd Order, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Four class spirit ores (set completes in order; last → cond 5).
    kill(&mut world, 20211); // Ol Mahum
    kill(&mut world, 20495); // Turek
    kill(&mut world, 20080); // Ant
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(5),
        "three ores is not the set"
    );
    kill(&mut world, 20249); // Turak Bugbear → set complete, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, terry);
    ev(&mut world, terry, "30064-10.html"); // ores → Terry's Letter + Box, cond 6
    assert_eq!(item_count(&world, 3001, 2650), 1, "Terry's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, viktor);
    ev(&mut world, viktor, "30684-05.html"); // Terry's Letter → Viktor's Letter, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, terry); // Viktor's Letter → Hawkeye's Letter, cond 8
    assert_eq!(item_count(&world, 3001, 2652), 1, "Hawkeye's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, viktor);
    ev(&mut world, viktor, "30684-11.html"); // → Viktor's Request, cond 9
    assert_eq!(item_count(&world, 3001, 2659), 1, "Viktor's Request");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    inject(&mut world, 3001, 0x0213_0000, 2660, 9); // 9 Medusa Scales
    kill(&mut world, 20158); // 10th scale → cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, viktor);
    ev(&mut world, viktor, "30684-15.html"); // → Shilen's Ore + Analysis Request, cond 11
    assert_eq!(item_count(&world, 3001, 2661), 1, "Shilen's Spirit Ore");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, marina);
    ev(&mut world, marina, "30715-02.html"); // → Marina's Letter, cond 12
    assert_eq!(item_count(&world, 3001, 2663), 1, "Marina's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, brunon); // Marina's Letter → Experiment Tools, cond 13
    assert_eq!(item_count(&world, 3001, 2664), 1, "Experiment Tools");
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, marina);
    ev(&mut world, marina, "30715-05.html"); // → Analysis Result, cond 14
    assert_eq!(item_count(&world, 3001, 2665), 1, "Analysis Result");
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    talk(&mut world, terry);
    ev(&mut world, terry, "30064-18.html"); // → List of Host, cond 15
    assert_eq!(item_count(&world, 3001, 2667), 1, "List of Host");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    // Four Abyss spirit ores.
    kill(&mut world, 20234); // Marsh Stakato Drone
    kill(&mut world, 20270); // Breka Orc Overlord
    kill(&mut world, 20088); // Ant Warrior Captain
    kill(&mut world, 20580); // Leto Lizardman Warrior → set complete, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, terry); // abyss ores → Terry's Report, cond 17
    assert_eq!(item_count(&world, 3001, 2672), 1, "Terry's Report");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // Completion at Dufner.
    let a = item_count(&world, 3001, 57);
    talk(&mut world, dufner);
    assert_eq!(
        item_count(&world, 3001, 2673),
        1,
        "Mark of the Seeker awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 187606,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(17),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00215_trial_of_the_pilgrim() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2722..=2733).map(|id| (id, "Q215", true)).collect();
    items.push((2721, "Mark of Pilgrim", false));
    add_quest_items(&mut world, &items);
    for id in [27116, 27117, 27118] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let santiago = NPC_OID;
    let tanapi = NPC_OID + 1;
    let martankus = NPC_OID + 2;
    let gauri = NPC_OID + 3;
    let gerald = NPC_OID + 4;
    let dorf = NPC_OID + 5;
    let primos = NPC_OID + 6;
    let petron = NPC_OID + 7;
    let andellia = NPC_OID + 8;
    let uruha = NPC_OID + 9;
    let casian = NPC_OID + 10;
    for (oid, npc) in [
        (santiago, 30648),
        (tanapi, 30571),
        (martankus, 30649),
        (gauri, 30550),
        (gerald, 30650),
        (dorf, 30651),
        (primos, 30117),
        (petron, 30036),
        (andellia, 30362),
        (uruha, 30652),
        (casian, 30612),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 15; // Cleric (HEAL_GROUP)
    }
    world.data.categories.insert_for_test("HEAL_GROUP", &[15]);
    let q = "Q00215_TrialOfThePilgrim";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, santiago);
    ev(&mut world, santiago, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    talk(&mut world, tanapi); // voucher → memo 2, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, martankus); // memo 3, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    kill(&mut world, 27116); // Lava Salamander → Essence of Flame, memo 4, cond 4
    assert_eq!(item_count(&world, 3001, 2725), 1, "Essence of Flame");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, martankus, "30649-04.html"); // → Spirit of Flame, memo 5, cond 5
    assert_eq!(item_count(&world, 3001, 2724), 1, "Spirit of Flame");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, tanapi); // memo 5 + spirit → cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, gauri); // spirit → Tag of Rumor, memo 6, cond 7
    assert_eq!(item_count(&world, 3001, 2733), 1, "Tag of Rumor");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // Gerald sells the Book of Gerald for 5000 adena.
    inject(&mut world, 3001, 0x0215_0000, 57, 5000);
    talk(&mut world, gerald);
    ev(&mut world, gerald, "30650-02.html"); // 5000 adena → Book of Gerald, memo 7
    assert_eq!(item_count(&world, 3001, 2726), 1, "Book of Gerald");
    assert_eq!(item_count(&world, 3001, 57), 0, "5000 adena spent");
    talk(&mut world, dorf); // tag → Grey Badge, memo 8
    assert_eq!(item_count(&world, 3001, 2727), 1, "Grey Badge");
    // Gerald refunds the 5000 adena for the finished badge + book.
    talk(&mut world, gerald);
    assert_eq!(
        item_count(&world, 3001, 57),
        5000,
        "Gerald refunds 5000 adena"
    );
    assert_eq!(item_count(&world, 3001, 2726), 0, "Book of Gerald returned");
    talk(&mut world, dorf); // memo 8 → cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, primos); // memo 9, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, petron); // Picture of Nahir, memo 10, cond 10
    assert_eq!(item_count(&world, 3001, 2728), 1, "Picture of Nahir");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    kill(&mut world, 27117); // Nahir → Hair of Nahir, memo 11, cond 11
    assert_eq!(item_count(&world, 3001, 2729), 1, "Hair of Nahir");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, petron); // picture + hair → Statue of Einhasad, memo 12, cond 12
    assert_eq!(item_count(&world, 3001, 2730), 1, "Statue of Einhasad");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, andellia); // memo 13, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    kill(&mut world, 27118); // Black Willow → Debris of Willow, memo 14, cond 14
    assert_eq!(item_count(&world, 3001, 2732), 1, "Debris of Willow");
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    talk(&mut world, uruha);
    ev(&mut world, uruha, "30652-02.html"); // debris → Book of Darkness, memo 15, cond 15
    assert_eq!(item_count(&world, 3001, 2731), 1, "Book of Darkness");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, andellia);
    ev(&mut world, andellia, "30362-04.html"); // memo 16, cond 16 (keeps book)
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, casian); // memo 17: Book of Sage + consumes badge/spirit/statue/darkness
    assert_eq!(item_count(&world, 3001, 2722), 1, "Book of Sage");
    assert_eq!(
        item_count(&world, 3001, 2731),
        0,
        "Book of Darkness consumed for the bonus"
    );
    talk(&mut world, casian); // memo 17 → cond 17
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // Completion at Santiago.
    talk(&mut world, santiago);
    assert_eq!(
        item_count(&world, 3001, 2721),
        1,
        "Mark of the Pilgrim awarded"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(17),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00216_trial_of_the_guildsman() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        3024, 3025, 3120, 3121, 3122, 3123, 3124, 3125, 3126, 3127, 3128, 3129, 3130, 3131, 3132,
        3133, 3134, 3135, 3136, 3137, 3138, 3139,
    ]
    .iter()
    .map(|&id| (id, "Q216", true))
    .collect();
    items.push((3119, "Mark of Guildsman", false));
    add_quest_items(&mut world, &items);
    for id in [20154, 20267, 20200, 20083, 20202, 20168, 20079] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let valkon = NPC_OID;
    let norman = NPC_OID + 1;
    let altran = NPC_OID + 2;
    let pinter = NPC_OID + 3;
    let duning = NPC_OID + 4;
    for (oid, npc) in [
        (valkon, 30103),
        (norman, 30210),
        (altran, 30283),
        (pinter, 30298),
        (duning, 30688),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 56; // Artisan
    }
    let q = "Q00216_TrialOfTheGuildsman";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    inject(&mut world, 3001, 0x0216_0000, 57, 2000); // entry fee
    talk(&mut world, valkon);
    ev(&mut world, valkon, "ACCEPT"); // 2000 adena → Valkon's Recommendation, cond 1
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 3120), 1, "Valkon's Recommendation");
    assert_eq!(item_count(&world, 3001, 57), 0, "entry fee paid");
    talk(&mut world, altran); // cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, valkon); // cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    kill(&mut world, 20154); // Mandragora → Berry, cond 4
    assert_eq!(item_count(&world, 3001, 3121), 1, "Mandragora Berry");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, altran, "30283-03.html"); // → Alltran's Instructions + recs, cond 5
    assert_eq!(item_count(&world, 3001, 3122), 1, "Alltran's Instructions");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // --- Track A: Norman/Duning → 7 Journeyman Gems. ---
    ev(&mut world, norman, "30210-04.html"); // → Norman's Instructions + Receipt
    assert_eq!(item_count(&world, 3001, 3125), 1, "Norman's Instructions");
    ev(&mut world, duning, "30688-02.html"); // Receipt → Duning's Instructions
    assert_eq!(item_count(&world, 3001, 3127), 1, "Duning's Instructions");
    inject(&mut world, 3001, 0x0216_0001, 3128, 29); // 29 Duning's Keys
    kill(&mut world, 20267); // Breka → 30th key, consumes Duning's Instructions
    assert_eq!(item_count(&world, 3001, 3128), 30, "30 Duning's Keys");
    assert_eq!(
        item_count(&world, 3001, 3127),
        0,
        "Duning's Instructions consumed at 30 keys"
    );
    ev(&mut world, norman, "30210-10.html"); // keys → Norman's List
    assert_eq!(item_count(&world, 3001, 3129), 1, "Norman's List");
    inject(&mut world, 3001, 0x0216_0002, 3130, 65); // Gray Bone Powder
    kill(&mut world, 20200); // Strain: +5 → 70
    inject(&mut world, 3001, 0x0216_0003, 3131, 63); // Granite Whetstone
    kill(&mut world, 20083); // Granite Golem: +7 → 70
    inject(&mut world, 3001, 0x0216_0004, 3132, 63); // Red Pigment
    kill(&mut world, 20202); // Dead Seeker: +7 → 70
    inject(&mut world, 3001, 0x0216_0005, 3133, 60); // Braided Yarn
    kill(&mut world, 20168); // Silenos: +10 → 70
    talk(&mut world, norman); // materials → 7 Journeyman Gems (deco absent, no cond 6 yet)
    assert_eq!(item_count(&world, 3001, 3134), 7, "7 Journeyman Gems");
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(6),
        "cond 6 needs deco beads too"
    );
    // --- Track B: Pinter → 7 Journeyman Deco Beads → cond 6. ---
    ev(&mut world, pinter, "30298-04.html"); // Artisan → Recipe + Pinter's Instructions
    assert_eq!(item_count(&world, 3001, 3135), 1, "Pinter's Instructions");
    inject(&mut world, 3001, 0x0216_0006, 3136, 65); // 65 Amber Beads
    world.force_roll(0); // roll(2)==0 → Amber Lump (Artisan)
    kill(&mut world, 20079); // Ant: +5 amber → 70
    assert!(item_count(&world, 3001, 3136) >= 70, "70 Amber Beads");
    assert_eq!(
        item_count(&world, 3001, 3137),
        1,
        "Amber Lump (Artisan bonus)"
    );
    talk(&mut world, pinter); // amber → 7 Deco Beads; gem >= 7 → cond 6
    assert_eq!(item_count(&world, 3001, 3138), 7, "7 Journeyman Deco Beads");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // --- Craft 7 Journeyman Rings (recipe system; supplied directly). ---
    inject(&mut world, 3001, 0x0216_0007, 3139, 7);
    let a = item_count(&world, 3001, 57);
    talk(&mut world, valkon);
    ev(&mut world, valkon, "30103-09a.html"); // rings → Mark of the Guildsman
    assert_eq!(
        item_count(&world, 3001, 3119),
        1,
        "Mark of the Guildsman awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 187606,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(6),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00226_test_of_the_healer() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2810..=2819).map(|id| (id, "Q226", true)).collect();
    items.push((2820, "Mark of Healer", false));
    add_quest_items(&mut world, &items);
    for id in [27122, 27123, 27124, 27125, 27126, 27127, 27134] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let bandellos = NPC_OID;
    let perrin = NPC_OID + 1;
    let allana = NPC_OID + 2;
    let gupu = NPC_OID + 3;
    let windy = NPC_OID + 4;
    let sorius = NPC_OID + 5;
    let daurin = NPC_OID + 6;
    let mde = NPC_OID + 7;
    let piper = NPC_OID + 8;
    let kristina = NPC_OID + 9;
    for (oid, npc) in [
        (bandellos, 30473),
        (perrin, 30428),
        (allana, 30424),
        (gupu, 30658),
        (windy, 30660),
        (sorius, 30327),
        (daurin, 30674),
        (mde, 30661),
        (piper, 30662),
        (kristina, 30665),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 15; // Cleric (WHITE_MAGIC_GROUP)
    }
    world
        .data
        .categories
        .insert_for_test("WHITE_MAGIC_GROUP", &[15]);
    let q = "Q00226_TestOfTheHealer";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, bandellos);
    ev(&mut world, bandellos, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    talk(&mut world, perrin);
    ev(&mut world, perrin, "30428-02.html"); // cond 2 + Tatoma ambush
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert!(
        !npcs_of(&mut world, 27134).is_empty(),
        "Tatoma ambush spawned"
    );
    kill(&mut world, 27134); // Tatoma → memo 2, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, perrin); // memo 3, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, allana); // memo 4, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, gupu); // cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    inject(&mut world, 3001, 0x0226_0000, 57, 1000);
    ev(&mut world, gupu, "30658-02.html"); // 1000 adena → Picture of Windy, cond 7
    assert_eq!(item_count(&world, 3001, 2812), 1, "Picture of Windy");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, windy);
    ev(&mut world, windy, "30660-03.html"); // → Windy's Pebbles, cond 8
    assert_eq!(item_count(&world, 3001, 2814), 1, "Windy's Pebbles");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, gupu); // pebbles → Golden Statue, memo 5
    assert_eq!(item_count(&world, 3001, 2813), 1, "Golden Statue");
    talk(&mut world, gupu); // memo 5 → cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, sorius); // Order of Sorius, memo 6, cond 10
    assert_eq!(item_count(&world, 3001, 2815), 1, "Order of Sorius");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, daurin);
    ev(&mut world, daurin, "30674-02.html"); // cond 11, spawn Leros
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    kill(&mut world, 27123); // Lero Leader → Secret Letter 1, cond 12
    assert_eq!(item_count(&world, 3001, 2816), 1, "Secret Letter 1");
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, daurin); // memo 8, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    // Four secret letters: the Mysterious Dark Elf deletes itself each step.
    let re_add_mde = |w: &mut World| add_test_npc(w, mde, 30661, "Folk", 40, 100, 0, 0);
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn assassins, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    kill(&mut world, 27124); // Assassin → Secret Letter 2, cond 15
    assert_eq!(item_count(&world, 3001, 2817), 1, "Secret Letter 2");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn snipers, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    kill(&mut world, 27125); // Sniper → Secret Letter 3, cond 17
    assert_eq!(item_count(&world, 3001, 2818), 1, "Secret Letter 3");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    re_add_mde(&mut world);
    talk(&mut world, mde); // spawn wizards + lord, cond 18
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    kill(&mut world, 27127); // Lord → Secret Letter 4, cond 19
    assert_eq!(item_count(&world, 3001, 2819), 1, "Secret Letter 4");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    re_add_mde(&mut world);
    talk(&mut world, mde); // all four → cond 20
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, piper); // directions → cond 21
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    talk(&mut world, kristina);
    ev(&mut world, kristina, "30665-02.html"); // 4 letters → Cristina's Letter, memo 9, cond 22
    assert_eq!(item_count(&world, 3001, 2811), 1, "Cristina's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(22));
    talk(&mut world, sorius); // Cristina's Letter → memo 10, cond 23
    assert_eq!(quest_cond(&world, 3001, q), Some(23));
    // Completion at Bandellos (healer kept the Golden Statue → lesser reward path).
    talk(&mut world, bandellos); // memo 10 + statue → 30473-07
    ev(&mut world, bandellos, "30473-09.html");
    assert_eq!(
        item_count(&world, 3001, 2820),
        1,
        "Mark of the Healer awarded"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(23),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00228_test_of_magus() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2841..=2863).map(|id| (id, "Q228", true)).collect();
    items.push((2840, "Mark of Magus", false));
    add_quest_items(&mut world, &items);
    for id in [
        27095, 27096, 27097, 20564, 20565, 20566, 27098, 20145, 20176, 20553, 20157,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let rukal = NPC_OID;
    let parina = NPC_OID + 1;
    let casian = NPC_OID + 2;
    let earth = NPC_OID + 3;
    let flame = NPC_OID + 4;
    let sylph = NPC_OID + 5;
    let undine = NPC_OID + 6;
    for (oid, npc) in [
        (rukal, 30629),
        (parina, 30391),
        (casian, 30612),
        (earth, 30409),
        (flame, 30411),
        (sylph, 30412),
        (undine, 30413),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00228_TestOfMagus";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, rukal);
    ev(&mut world, rukal, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, parina);
    ev(&mut world, parina, "30391-02.html"); // → Parina's Letter, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, casian);
    ev(&mut world, casian, "30612-02.html"); // → Lilac Charm, cond 3
    assert_eq!(item_count(&world, 3001, 2843), 1, "Lilac Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    kill(&mut world, 27095); // Phantasm → Golden Seed 1
    kill(&mut world, 27096); // Nightmare → Golden Seed 2
    kill(&mut world, 27097); // Darkling → Golden Seed 3, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, rukal);
    ev(&mut world, rukal, "30629-10.html"); // seeds → Score of Elements, cond 5
    assert_eq!(item_count(&world, 3001, 2847), 1, "Score of Elements");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // --- Earth (Serpent) ---
    talk(&mut world, earth);
    ev(&mut world, earth, "30409-03.html"); // Serpent Charm
    inject(&mut world, 3001, 0x0228_0000, 2853, 9);
    kill(&mut world, 20564); // Monstereye → 10 shells
    inject(&mut world, 3001, 0x0228_0001, 2854, 9);
    kill(&mut world, 20565); // Stolen Golem → 10 powder
    inject(&mut world, 3001, 0x0228_0002, 2855, 9);
    kill(&mut world, 20566); // Iron Golem → 10 scrap
    talk(&mut world, earth); // → Tone of Earth
    assert_eq!(item_count(&world, 3001, 2859), 1, "Tone of Earth");
    // --- Fire (Salamander) ---
    talk(&mut world, flame); // gives Salamander Charm
    inject(&mut world, 3001, 0x0228_0003, 2849, 4);
    world.force_roll(0);
    kill(&mut world, 27098); // Ghost Fire → 5 crystals
    talk(&mut world, flame); // → Tone of Fire
    assert_eq!(item_count(&world, 3001, 2857), 1, "Tone of Fire");
    // --- Wind (Sylph) ---
    talk(&mut world, sylph);
    ev(&mut world, sylph, "30412-02.html"); // Sylph Charm
    inject(&mut world, 3001, 0x0228_0004, 2850, 19);
    kill(&mut world, 20145); // Harpy → 20 feathers
    inject(&mut world, 3001, 0x0228_0005, 2851, 9);
    world.force_roll(0);
    kill(&mut world, 20176); // Wyrm → 10 wingbone
    inject(&mut world, 3001, 0x0228_0006, 2852, 9);
    world.force_roll(0);
    kill(&mut world, 20553); // Windsus → 10 mane
    talk(&mut world, sylph); // → Tone of Wind
    assert_eq!(item_count(&world, 3001, 2858), 1, "Tone of Wind");
    // --- Water (Undine): the fourth tone → cond 6 ---
    talk(&mut world, undine); // gives Undine Charm
    inject(&mut world, 3001, 0x0228_0007, 2848, 19);
    kill(&mut world, 20157); // Marsh Stakato → 20 drops
    talk(&mut world, undine); // → Tone of Water, cond 6
    assert_eq!(item_count(&world, 3001, 2856), 1, "Tone of Water");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // --- Completion ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, rukal);
    assert_eq!(item_count(&world, 3001, 2840), 1, "Mark of Magus awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(6),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00232_test_of_the_lord() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3391..=3416).map(|id| (id, "Q232", true)).collect();
    items.push((3390, "Mark of Lord", false));
    items.push((1341, "Bone Arrow", false));
    add_quest_items(&mut world, &items);
    for id in [20269, 20583, 20233, 20564, 20778] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(30643));
    let kakai = NPC_OID;
    let manakia = NPC_OID + 1;
    let jakal = NPC_OID + 2;
    let sumari = NPC_OID + 3;
    let somak = NPC_OID + 4;
    let varkees = NPC_OID + 5;
    let tantus = NPC_OID + 6;
    let hatos = NPC_OID + 7;
    let takuna = NPC_OID + 8;
    let chianta = NPC_OID + 9;
    let martankus = NPC_OID + 10;
    let first_orc = NPC_OID + 11;
    for (oid, npc) in [
        (kakai, 30565),
        (manakia, 30515),
        (jakal, 30558),
        (sumari, 30564),
        (somak, 30510),
        (varkees, 30566),
        (tantus, 30567),
        (hatos, 30568),
        (takuna, 30641),
        (chianta, 30642),
        (martankus, 30649),
        (first_orc, 30643),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 50; // Orc Shaman
        p.race = 3; // Orc
    }
    let q = "Q00232_TestOfTheLord";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, kakai);
    ev(&mut world, kakai, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(item_count(&world, 3001, 3391), 1, "Ordeal Necklace");
    // --- Leg: Varkees → Huge Orc Fang ---
    ev(&mut world, varkees, "30566-02.html"); // Varkees Charm
    talk(&mut world, manakia); // Manakia's Orders
    inject(&mut world, 3001, 0x0232_0000, 3398, 18); // 18 Breka Orc Fangs
    kill(&mut world, 20269); // +2 → 20
    talk(&mut world, manakia); // fangs → Manakia's Amulet
    assert_eq!(item_count(&world, 3001, 3399), 1, "Manakia's Amulet");
    talk(&mut world, varkees); // charm + amulet → Huge Orc Fang
    assert_eq!(item_count(&world, 3001, 3400), 1, "Huge Orc Fang");
    // --- Leg: Tantus → Axe of Ceremony ---
    ev(&mut world, tantus, "30567-02.html"); // Tantus Charm
    inject(&mut world, 3001, 0x0232_0001, 57, 1000);
    ev(&mut world, jakal, "30558-02.html"); // 1000 adena → Neruga Axe Blade
    inject(&mut world, 3001, 0x0232_0002, 1341, 1000); // 1000 Bone Arrows
    talk(&mut world, tantus); // blade + arrows → Axe of Ceremony
    assert_eq!(item_count(&world, 3001, 3406), 1, "Axe of Ceremony");
    // --- Leg: Hatos → Sword into Skull ---
    ev(&mut world, hatos, "30568-02.html"); // Hatos Charm
    talk(&mut world, sumari); // Sumari's Letter
    talk(&mut world, somak); // letter → Urutu Blade
    assert_eq!(item_count(&world, 3001, 3402), 1, "Urutu Blade");
    inject(&mut world, 3001, 0x0232_0003, 3403, 9); // 9 Timak Orc Skulls
    kill(&mut world, 20583); // +1 → 10
    talk(&mut world, hatos); // blade + skulls → Sword into Skull
    assert_eq!(item_count(&world, 3001, 3404), 1, "Sword into Skull");
    // --- Leg: Takuna → Handiwork Spider Brooch ---
    ev(&mut world, takuna, "30641-02.html"); // Takuna Charm
    inject(&mut world, 3001, 0x0232_0004, 3407, 8); // 8 feelers
    kill(&mut world, 20233); // +2 → 10 feelers
    inject(&mut world, 3001, 0x0232_0005, 3408, 8); // 8 feet
    kill(&mut world, 20233); // +2 → 10 feet
    talk(&mut world, takuna); // → Handiwork Spider Brooch
    assert_eq!(item_count(&world, 3001, 3409), 1, "Handiwork Spider Brooch");
    // --- Leg: Chianta → Monster Eye Woodcarving (fifth → cond 2) ---
    ev(&mut world, chianta, "30642-02.html"); // Chianta Charm
    inject(&mut world, 3001, 0x0232_0006, 3410, 19); // 19 Cornea
    kill(&mut world, 20564); // +1 → 20
    talk(&mut world, chianta); // → Monster Eye Woodcarving, cond 2
    assert_eq!(item_count(&world, 3001, 3411), 1, "Monster Eye Woodcarving");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Kakai forges the Bear Fang Necklace ---
    talk(&mut world, kakai);
    ev(&mut world, kakai, "30565-08.html"); // all five → Bear Fang Necklace, cond 3
    assert_eq!(item_count(&world, 3001, 3412), 1, "Bear Fang Necklace");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, martankus);
    ev(&mut world, martankus, "30649-04.html"); // → Martankus Charm, cond 4
    assert_eq!(item_count(&world, 3001, 3413), 1, "Martankus Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    kill(&mut world, 20778); // Ragna → Chief Notice
    kill(&mut world, 20778); // Ragna → Ragna Orc Head, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, martankus); // notice + head → Immortal Flame, cond 6
    assert_eq!(item_count(&world, 3001, 3416), 1, "Immortal Flame");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, first_orc); // cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    // --- Completion ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, kakai);
    assert_eq!(
        item_count(&world, 3001, 3390),
        1,
        "Mark of the Lord awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(7),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00233_test_of_the_war_spirit() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2880..=2914).map(|id| (id, "Q233", true)).collect();
    items.push((2879, "Mark of Warspirit", false));
    add_quest_items(&mut world, &items);
    for id in [20089, 20581, 27108, 20158, 20213, 20214, 20215, 20601] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let somak = NPC_OID;
    let racoy = NPC_OID + 1;
    let vivyan = NPC_OID + 2;
    let sarien = NPC_OID + 3;
    let pekiron = NPC_OID + 4;
    let manakia = NPC_OID + 5;
    let orim = NPC_OID + 6;
    let martankus = NPC_OID + 7;
    for (oid, npc) in [
        (somak, 30510),
        (racoy, 30507),
        (vivyan, 30030),
        (sarien, 30436),
        (pekiron, 30682),
        (manakia, 30515),
        (orim, 30630),
        (martankus, 30649),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 50; // Orc Shaman
        p.race = 3; // Orc
    }
    let q = "Q00233_TestOfTheWarSpirit";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, somak);
    ev(&mut world, somak, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Leg Kiruna (Racoy): totem + insect book + 5 randomly-tiered bones ---
    ev(&mut world, racoy, "30507-02.html"); // Racoy's Totem
    ev(&mut world, vivyan, "30030-04.html"); // Viviante's Letter
    talk(&mut world, sarien); // letter → Insect Diagram Book
    assert_eq!(item_count(&world, 3001, 2904), 1, "Insect Diagram Book");
    for r in [70, 70, 50, 50, 10] {
        world.force_roll(r); // thigh, arm, spine, rib, skull
        kill(&mut world, 20089);
    }
    talk(&mut world, racoy); // bones → Kiruna's Remains 1
    assert_eq!(item_count(&world, 3001, 2910), 1, "Kiruna's Remains 1");
    // --- Leg Tonar (Pekiron): 5 sequential bones off Leto ---
    ev(&mut world, pekiron, "30682-02.html"); // Pekiron's Totem
    for _ in 0..5 {
        kill(&mut world, 20581);
    }
    talk(&mut world, pekiron); // → Tonar's Remains 1
    assert_eq!(item_count(&world, 3001, 2894), 1, "Tonar's Remains 1");
    // --- Leg Hermodt (Manakia): skull off Stenoa + 4 bones off Medusa ---
    ev(&mut world, manakia, "30515-02.html"); // Manakia's Totem
    kill(&mut world, 27108); // Stenoa → Hermodt's Skull
    for _ in 0..4 {
        kill(&mut world, 20158); // Medusa → rib/spine/arm/thigh
    }
    talk(&mut world, manakia); // → Hermodt's Remains 1
    assert_eq!(item_count(&world, 3001, 2901), 1, "Hermodt's Remains 1");
    // --- Leg Brakis (Orim): 30 reagents → fourth remains → cond 2 ---
    ev(&mut world, orim, "30630-04.html"); // Orim's Contract
    inject(&mut world, 3001, 0x0233_0000, 2884, 8); // 8 Porta's Eyes
    kill(&mut world, 20213); // +2 → 10
    inject(&mut world, 3001, 0x0233_0001, 2885, 5); // 5 Excuro's Scales
    kill(&mut world, 20214); // +5 → 10
    inject(&mut world, 3001, 0x0233_0002, 2886, 5); // 5 Mordeo's Talons
    kill(&mut world, 20215); // +5 → 10
    talk(&mut world, orim); // 30 reagents → Brakis's Remains 1, cond 2
    assert_eq!(item_count(&world, 3001, 2887), 1, "Brakis's Remains 1");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Somak forges the Vendetta Totem ---
    talk(&mut world, somak); // 4 remains → Vendetta Totem, cond 3
    assert_eq!(item_count(&world, 3001, 2880), 1, "Vendetta Totem");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    inject(&mut world, 3001, 0x0233_0003, 2881, 12); // 12 Tamlin Orc Heads
    kill(&mut world, 20601); // 13th → cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, somak); // 13 heads → War Spirit Totem + remains2, cond 5
    assert_eq!(item_count(&world, 3001, 2882), 1, "War Spirit Totem");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // --- Completion at Martankus ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, martankus);
    ev(&mut world, martankus, "30649-03.html");
    assert_eq!(
        item_count(&world, 3001, 2879),
        1,
        "Mark of the War Spirit awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 161806,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(5),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00217_testimony_of_trust() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (2735..=2761).map(|id| (id, "Q217", true)).collect();
    items.push((2734, "Mark of Trust", false));
    add_quest_items(&mut world, &items);
    for id in [
        20013, 27121, 20036, 27120, 20550, 20082, 20157, 20553, 20213,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let hollint = NPC_OID;
    let biotin = NPC_OID + 1;
    let asterios = NPC_OID + 2;
    let thifiell = NPC_OID + 3;
    let clayton = NPC_OID + 4;
    let manakia = NPC_OID + 5;
    let lockirin = NPC_OID + 6;
    let kakai = NPC_OID + 7;
    let nikola = NPC_OID + 8;
    let seresin = NPC_OID + 9;
    for (oid, npc) in [
        (hollint, 30191),
        (biotin, 30031),
        (asterios, 30154),
        (thifiell, 30358),
        (clayton, 30464),
        (manakia, 30515),
        (lockirin, 30531),
        (kakai, 30565),
        (nikola, 30621),
        (seresin, 30657),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 1; // Warrior (Human, HUMAN_2ND_GROUP)
        p.race = 0; // Human
    }
    world
        .data
        .categories
        .insert_for_test("HUMAN_2ND_GROUP", &[1]);
    let q = "Q00217_TestimonyOfTrust";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, hollint);
    ev(&mut world, hollint, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    talk(&mut world, asterios);
    ev(&mut world, asterios, "30154-03.html"); // → Order of Asterios, memo 2, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Elf leg: conjure and slay Actea and Luell ---
    world.force_roll(0); // roll(100)=0 < 33 → spawn Actea
    kill(&mut world, 20013); // Dryad
    assert_eq!(npcs_of(&mut world, 27121).len(), 1, "Actea conjured");
    kill(&mut world, 27121); // Actea → Seed of Verdure
    assert_eq!(item_count(&world, 3001, 2747), 1, "Seed of Verdure");
    world.force_roll(0);
    kill(&mut world, 20036); // Lirein → spawn Luell
    kill(&mut world, 27120); // Luell → Breath of Winds, memo 3, cond 3
    assert_eq!(item_count(&world, 3001, 2746), 1, "Breath of Winds");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, asterios); // → Scroll of Elf Trust, memo 4, cond 4
    assert_eq!(item_count(&world, 3001, 2741), 1, "Scroll of Elf Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Dark Elf leg: Thifiell → Clayton → three reagents ---
    talk(&mut world, thifiell);
    ev(&mut world, thifiell, "30358-02.html"); // → Letter of Thifiell, memo 5, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, clayton); // → Order of Clayton, memo 6, cond 6
    assert_eq!(item_count(&world, 3001, 2755), 1, "Order of Clayton");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    inject(&mut world, 3001, 0x0217_0000, 2749, 4); // 4 basilisk blood
    kill(&mut world, 20550); // 5th → Basilisk Plasma
    inject(&mut world, 3001, 0x0217_0001, 2750, 4); // 4 giant aphid
    kill(&mut world, 20082); // 5th → Honey Dew
    inject(&mut world, 3001, 0x0217_0002, 2751, 4); // 4 stakato fluids
    kill(&mut world, 20157); // 5th → Stakato Ichor, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, clayton); // cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, thifiell); // → Scroll of Dark Elf Trust, memo 7, cond 9
    assert_eq!(
        item_count(&world, 3001, 2740),
        1,
        "Scroll of Dark Elf Trust"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, hollint); // → Letter to Seresin, memo 8, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, seresin);
    ev(&mut world, seresin, "30657-03.html"); // → Letters to Dwarf + Orc, memo 9, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    // --- Orc leg: Kakai → Manakia → windsus parasites ---
    talk(&mut world, kakai);
    ev(&mut world, kakai, "30565-02.html"); // → Letter to Manakia, memo 10, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, manakia);
    ev(&mut world, manakia, "30515-02.html"); // memo 11, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    inject(&mut world, 3001, 0x0217_0003, 2756, 8); // 8 parasites
    kill(&mut world, 20553); // +2 → 10, memo 12, cond 15
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, manakia); // → Letter of Manakia, memo 13, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, kakai); // → Scroll of Orc Trust, memo 14, cond 17
    assert_eq!(item_count(&world, 3001, 2743), 1, "Scroll of Orc Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // --- Dwarf leg: Lockirin → Nikola → Porta heart ---
    talk(&mut world, lockirin);
    ev(&mut world, lockirin, "30531-02.html"); // → Letter to Nichola, memo 15, cond 18
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    talk(&mut world, nikola);
    ev(&mut world, nikola, "30621-02.html"); // → Order of Nichola, memo 16, cond 19
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    kill(&mut world, 20213); // Porta → Heart of Porta, cond 20
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, nikola); // heart → memo 17, cond 21
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    talk(&mut world, lockirin); // → Scroll of Dwarf Trust, memo 18, cond 22
    assert_eq!(item_count(&world, 3001, 2742), 1, "Scroll of Dwarf Trust");
    assert_eq!(quest_cond(&world, 3001, q), Some(22));
    talk(&mut world, hollint); // → Recommendation of Hollin, memo 19, cond 23
    assert_eq!(
        item_count(&world, 3001, 2744),
        1,
        "Recommendation of Hollin"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(23));
    // --- Completion at Biotin ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, biotin);
    assert_eq!(item_count(&world, 3001, 2734), 1, "Mark of Trust awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 252212,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(23),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00219_testimony_of_fate() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3173..=3202).map(|id| (id, "Q219", true)).collect();
    items.push((3172, "Mark of Fate", false));
    add_quest_items(&mut world, &items);
    for id in [
        20144, 20158, 20233, 20202, 20192, 20157, 20270, 20554, 20582, 20600, 27079,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let kaira = NPC_OID;
    let metheus = NPC_OID + 1;
    let ixia = NPC_OID + 2;
    let roa = NPC_OID + 3;
    let norman = NPC_OID + 4;
    let thifiell = NPC_OID + 5;
    let arkenia = NPC_OID + 6;
    let pixy = NPC_OID + 7;
    let treant = NPC_OID + 8;
    for (oid, npc) in [
        (kaira, 30476),
        (metheus, 30614),
        (ixia, 30463),
        (roa, 30114),
        (norman, 30210),
        (thifiell, 30358),
        (arkenia, 30419),
        (pixy, 31845),
        (treant, 31850),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 32; // Palus Knight (Dark Elf, DELF_2ND_GROUP)
        p.race = 2; // Dark Elf
    }
    world
        .data
        .categories
        .insert_for_test("DELF_2ND_GROUP", &[32]);
    let q = "Q00219_TestimonyOfFate";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, kaira);
    ev(&mut world, kaira, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, metheus); // → Metheus's Funeral Jar, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    kill(&mut world, 20144); // Hangman → Kasandra's Remains, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, metheus); // → Herbalism Textbook, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    talk(&mut world, ixia); // textbook → Ixia's List, cond 6
    assert_eq!(item_count(&world, 3001, 3177), 1, "Ixia's List");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Five poison reagents, each to 10 → cond 7.
    inject(&mut world, 3001, 0x0219_0000, 3178, 9);
    kill(&mut world, 20158); // Medusa's Ichor → 10
    inject(&mut world, 3001, 0x0219_0001, 3179, 9);
    kill(&mut world, 20233); // Marsh Spider Fluids → 10
    inject(&mut world, 3001, 0x0219_0002, 3180, 9);
    kill(&mut world, 20202); // Dead Seeker Dung → 10
    inject(&mut world, 3001, 0x0219_0003, 3181, 9);
    kill(&mut world, 20192); // Tyrant's Blood → 10
    inject(&mut world, 3001, 0x0219_0004, 3182, 9);
    kill(&mut world, 20157); // Nightshade Root → 10, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, ixia); // reagents → Belladonna, cond 8
    assert_eq!(item_count(&world, 3001, 3183), 1, "Belladonna");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, metheus); // Belladonna → Alder's Skull 1, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    talk(&mut world, kaira); // → Alder's Skull 2, cond 10
    assert_eq!(item_count(&world, 3001, 3185), 1, "Alder's Skull 2");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, kaira); // cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, roa);
    ev(&mut world, roa, "30114-04.html"); // → Alder's Receipt, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, norman); // receipt → Revelations Manuscript, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, kaira);
    ev(&mut world, kaira, "30476-12.html"); // → Kaira's Recommendation, cond 15
    assert_eq!(item_count(&world, 3001, 3189), 1, "Kaira's Recommendation");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    talk(&mut world, thifiell); // → Palus Charm + Thifiell's Letter, cond 16
    assert_eq!(item_count(&world, 3001, 3190), 1, "Palus Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-02.html"); // → Arkenia's Note, cond 17
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    // --- Alchemy: Red Fairy Dust from four overlord skulls ---
    ev(&mut world, pixy, "31845-02.html"); // Pixy Garnet
    kill(&mut world, 20554); // Grandis's Skull
    kill(&mut world, 20600); // Karul Bugbear Skull
    kill(&mut world, 20270); // Breka Overlord Skull
    kill(&mut world, 20582); // Leto Overlord Skull
    talk(&mut world, pixy); // skulls → Red Fairy Dust
    assert_eq!(item_count(&world, 3001, 3198), 1, "Red Fairy Dust");
    // --- Alchemy: Blight Treant Sap ---
    ev(&mut world, treant, "31850-02.html"); // Timiriran Seed
    kill(&mut world, 27079); // Black Willow Lurker → Black Willow Leaf
    talk(&mut world, treant); // → Blight Treant Sap
    assert_eq!(item_count(&world, 3001, 3201), 1, "Blight Treant Sap");
    // --- Arkenia's Letter, cond 18 ---
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-05.html"); // dust + sap → Arkenia's Letter, cond 18
    assert_eq!(item_count(&world, 3001, 3202), 1, "Arkenia's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // --- Completion at Thifiell ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, thifiell);
    assert_eq!(item_count(&world, 3001, 3172), 1, "Mark of Fate awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 247708,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(18),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00218_testimony_of_life() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3141..=3171).map(|id| (id, "Q218", true)).collect();
    items.push((3026, "Talin's Spear", true));
    items.push((3140, "Mark of Life", false));
    add_quest_items(&mut world, &items);
    for id in [20550, 20082, 20176, 20145, 20233, 20581, 27077] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let cardien = NPC_OID;
    let asterios = NPC_OID + 1;
    let pushkin = NPC_OID + 2;
    let thalia = NPC_OID + 3;
    let arkenia = NPC_OID + 4;
    let adonius = NPC_OID + 5;
    let isael = NPC_OID + 6;
    for (oid, npc) in [
        (cardien, 30460),
        (asterios, 30154),
        (pushkin, 30300),
        (thalia, 30371),
        (arkenia, 30419),
        (adonius, 30375),
        (isael, 30655),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 19; // Elven Knight (ELF_2ND_GROUP)
        p.race = 1; // Elf
    }
    world
        .data
        .categories
        .insert_for_test("ELF_2ND_GROUP", &[19]);
    let q = "Q00218_TestimonyOfLife";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, cardien);
    ev(&mut world, cardien, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, asterios);
    ev(&mut world, asterios, "30154-07.html"); // → Hierarch's Letter + Moonflower Charm, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, thalia);
    ev(&mut world, thalia, "30371-03.html"); // → Grail Diagram, cond 3
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, pushkin);
    ev(&mut world, pushkin, "30300-06.html"); // → Pushkin's List, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    inject(&mut world, 3001, 0x0218_0000, 3161, 8); // mithril ore
    kill(&mut world, 20550); // Basilisk +2 → 10
    inject(&mut world, 3001, 0x0218_0001, 3162, 18); // ant acid
    kill(&mut world, 20082); // Ant +2 → 20
    inject(&mut world, 3001, 0x0218_0002, 3163, 16); // wyrm talon
    kill(&mut world, 20176); // Wyrm +4 → 20, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, pushkin);
    ev(&mut world, pushkin, "30300-10.html"); // → Pure Mithril Cup, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, thalia); // cup → Thalia's 1st Letter, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, arkenia);
    ev(&mut world, arkenia, "30419-04.html"); // → Arkenia's Contract + Instructions, cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, adonius);
    ev(&mut world, adonius, "30375-02.html"); // → Adonius's List, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    inject(&mut world, 3001, 0x0218_0003, 3165, 16); // harpy down
    kill(&mut world, 20145); // Harpy +4 → 20
    inject(&mut world, 3001, 0x0218_0004, 3164, 16); // spider ichor
    kill(&mut world, 20233); // Spider +4 → 20, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, adonius); // → Andariel Scripture Copy, cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, arkenia); // → Stardust, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    talk(&mut world, thalia);
    ev(&mut world, thalia, "30371-11.html"); // → Thalia's 2nd Letter, cond 14
    assert_eq!(quest_cond(&world, 3001, q), Some(14));
    talk(&mut world, isael);
    ev(&mut world, isael, "30655-02.html"); // → Isael's Instructions, cond 15
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    for _ in 0..6 {
        kill(&mut world, 20581); // Leto → six spear parts
    }
    talk(&mut world, isael); // parts → Talin's Spear + Isael's Letter, cond 17
    assert_eq!(item_count(&world, 3001, 3026), 1, "Talin's Spear assembled");
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    talk(&mut world, thalia); // spear + letter → Grail of Purity, cond 18
    assert_eq!(item_count(&world, 3001, 3158), 1, "Grail of Purity");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // Slay the Unicorn with Talin's Spear equipped.
    equip_weapon_row(&mut world, 3001, 3026);
    inject(&mut world, 3001, 0x0218_0005, 3144, 1); // re-supply moonflower/spear/grail
    inject(&mut world, 3001, 0x0218_0006, 3026, 1);
    inject(&mut world, 3001, 0x0218_0007, 3158, 1);
    kill(&mut world, 27077); // Unicorn (spear-struck) → Tears of Unicorn, cond 19
    assert_eq!(item_count(&world, 3001, 3159), 1, "Tears of Unicorn");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    talk(&mut world, thalia); // tears → Water of Life, cond 20
    assert_eq!(item_count(&world, 3001, 3160), 1, "Water of Life");
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    talk(&mut world, asterios); // moonflower + water → Camomile Charm, cond 21
    assert_eq!(item_count(&world, 3001, 3142), 1, "Camomile Charm");
    assert_eq!(quest_cond(&world, 3001, q), Some(21));
    // --- Completion at Cardien ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, cardien);
    assert_eq!(item_count(&world, 3001, 3140), 1, "Mark of Life awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 342288,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(21),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00229_test_of_witchcraft() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3308..=3335).map(|id| (id, "Q229", true)).collect();
    items.push((3029, "Sword of Binding", true));
    items.push((3307, "Mark of Witchcraft", false));
    add_quest_items(&mut world, &items);
    for id in [20557, 20565, 20577, 20601, 27099, 27100, 27101] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let orim = NPC_OID;
    let alexandria = NPC_OID + 1;
    let iker = NPC_OID + 2;
    let kaira = NPC_OID + 3;
    let lara = NPC_OID + 4;
    let nestle = NPC_OID + 5;
    let leopold = NPC_OID + 6;
    let vasper = NPC_OID + 7;
    let vadin = NPC_OID + 8;
    let evert = NPC_OID + 9;
    for (oid, npc) in [
        (orim, 30630),
        (alexandria, 30098),
        (iker, 30110),
        (kaira, 30476),
        (lara, 30063),
        (nestle, 30314),
        (leopold, 30435),
        (vasper, 30417),
        (vadin, 30188),
        (evert, 30633),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00229_TestOfWitchcraft";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, orim);
    ev(&mut world, orim, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    talk(&mut world, alexandria);
    ev(&mut world, alexandria, "30098-03.htm"); // Diagram → Alexandria's Book, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // Gem 1 (Iker): 20 each of three reagents.
    ev(&mut world, iker, "30110-03.htm"); // Iker's List
    inject(&mut world, 3001, 0x0229_0000, 3311, 19); // dire wyrm fang
    kill(&mut world, 20557); // +1 → 20
    inject(&mut world, 3001, 0x0229_0001, 3313, 20); // golem heartstone
    inject(&mut world, 3001, 0x0229_0002, 3312, 20); // leto charm
    talk(&mut world, iker); // → Aklantoth 1st Gem
    assert_eq!(item_count(&world, 3001, 3317), 1, "Aklantoth 1st Gem");
    // Gem 2 (Kaira)
    ev(&mut world, kaira, "30476-02.htm");
    assert_eq!(item_count(&world, 3001, 3318), 1, "Aklantoth 2nd Gem");
    // Gem 3 (Lara → Nameless Revenant)
    ev(&mut world, lara, "30063-02.htm"); // Lara's Memo
    kill(&mut world, 27099); // Nameless Revenant → 3rd Gem
    assert_eq!(item_count(&world, 3001, 3319), 1, "Aklantoth 3rd Gem");
    // Gems 4-6 (Nestle/Leopold → Skeletal Mercenary)
    ev(&mut world, nestle, "30314-02.htm"); // Nestle's Memo
    ev(&mut world, leopold, "30435-02.htm"); // Leopold's Journal
    kill(&mut world, 27100); // 4th Gem
    kill(&mut world, 27100); // 5th Gem
    kill(&mut world, 27100); // 6th Gem → cond 3
    assert_eq!(item_count(&world, 3001, 3322), 1, "Aklantoth 6th Gem");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    // Orim forges the First Brimstone and summons Zeruel.
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-14.htm"); // gems → First Brimstone, cond 4
    assert_eq!(item_count(&world, 3001, 3323), 1, "First Brimstone");
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // Driving Zeruel off (attack while holding the brimstone) → cond 5.
    add_test_npc(&mut world, NPC_OID + 20, 27101, "Monster", 40, 40, 0, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 20, 3001, 10.0, false);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(5),
        "Zeruel driven off → cond 5"
    );
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-16.htm"); // → Orim's Instructions + two letters, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Sword of Binding path.
    ev(&mut world, vasper, "30417-03.htm"); // 1st Letter → Sir Vasper's Letter
    talk(&mut world, vadin); // Vasper's Letter → Vadin's Crucifix
    assert_eq!(item_count(&world, 3001, 3328), 1, "Vadin's Crucifix");
    inject(&mut world, 3001, 0x0229_0003, 3329, 19); // 19 Tamlin amulets
    world.force_roll(0); // roll(100)=0 < 50 → drop
    kill(&mut world, 20601); // +1 → 20
    talk(&mut world, vadin); // amulets → Vadin's Sanctions
    assert_eq!(item_count(&world, 3001, 3330), 1, "Vadin's Sanctions");
    talk(&mut world, vasper); // Sanctions → Sword of Binding
    assert_eq!(item_count(&world, 3001, 3029), 1, "Sword of Binding");
    // Soultrap Crystal path → cond 7.
    ev(&mut world, iker, "30110-08.htm"); // 2nd Letter → Soultrap Crystal, cond 7
    assert_eq!(item_count(&world, 3001, 3332), 1, "Soultrap Crystal");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, orim); // cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    // Evert summons Zeruel for the binding.
    ev(&mut world, evert, "30633-02.htm"); // → Second Brimstone, cond 9
    assert_eq!(item_count(&world, 3001, 3335), 1, "Second Brimstone");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // Bind Zeruel with a killing blow from the Sword of Binding.
    equip_weapon_row(&mut world, 3001, 3029);
    for (obj, item) in [
        (0x0229_0004, 3324),
        (0x0229_0005, 3335),
        (0x0229_0006, 3029),
        (0x0229_0007, 3332),
    ] {
        inject(&mut world, 3001, obj, item, 1); // instructions, brimstone2, sword, soultrap
    }
    kill(&mut world, 27101); // Zeruel (sword-struck) → Zeruel Bind Crystal, cond 10
    assert_eq!(item_count(&world, 3001, 3334), 1, "Zeruel Bind Crystal");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    // --- Completion at Orim ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, orim);
    ev(&mut world, orim, "30630-22.htm");
    assert_eq!(
        item_count(&world, 3001, 3307),
        1,
        "Mark of Witchcraft awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 372154,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(10),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00220_testimony_of_glory() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3204..=3237).map(|id| (id, "Q220", true)).collect();
    items.push((3203, "Mark of Glory", false));
    add_quest_items(&mut world, &items);
    for id in [
        20563, 20192, 20550, 20583, 20601, 20778, 27080, 27081, 27082, 27083, 27086,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let vokian = NPC_OID;
    let chianta = NPC_OID + 1;
    let kasman = NPC_OID + 2;
    let manakia = NPC_OID + 3;
    let driko = NPC_OID + 4;
    let burai = NPC_OID + 5;
    let harak = NPC_OID + 6;
    let voltar = NPC_OID + 7;
    let kepra = NPC_OID + 8;
    let tanapi = NPC_OID + 9;
    let kakai = NPC_OID + 10;
    for (oid, npc) in [
        (vokian, 30514),
        (chianta, 30642),
        (kasman, 30501),
        (manakia, 30515),
        (driko, 30619),
        (burai, 30617),
        (harak, 30618),
        (voltar, 30615),
        (kepra, 30616),
        (tanapi, 30571),
        (kakai, 30565),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 45; // Orc Raider
        p.race = 3; // Orc
    }
    world
        .data
        .categories
        .insert_for_test("ORC_2ND_GROUP", &[45]);
    let q = "Q00220_TestimonyOfGlory";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, vokian);
    ev(&mut world, vokian, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Vokian's three subjugation reagents → cond 2.
    inject(&mut world, 3001, 0x0220_0000, 3205, 9);
    kill(&mut world, 20563); // Manashen shard → 10
    inject(&mut world, 3001, 0x0220_0001, 3206, 9);
    kill(&mut world, 20192); // Tyrant talon → 10
    inject(&mut world, 3001, 0x0220_0002, 3207, 9);
    kill(&mut world, 20550); // Basilisk fang → 10, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    talk(&mut world, vokian); // → Order2 + Necklace of Authority, cond 3
    assert_eq!(item_count(&world, 3001, 3209), 1, "Necklace of Authority");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    talk(&mut world, chianta);
    ev(&mut world, chianta, "30642-03.html"); // → Chianta's 1st Order, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Five scepter legs ---
    // Vuku (Driko): letter → contract → 30 husks
    ev(&mut world, kasman, "30501-02.html"); // Kasman's 1st Letter
    ev(&mut world, driko, "30619-03.html"); // → Driko's Contract
    inject(&mut world, 3001, 0x0220_0003, 3234, 30); // 30 stakato drone husks
    talk(&mut world, driko); // → Scepter of Vuku
    assert_eq!(item_count(&world, 3001, 3213), 1, "Scepter of Vuku");
    // Turek (Burai): letter → glove → 2 makum heads
    ev(&mut world, kasman, "30501-05.html"); // Kasman's 2nd Letter
    ev(&mut world, burai, "30617-03.html"); // → Glove of Burai
    kill(&mut world, 27083); // Makum head 1
    kill(&mut world, 27083); // Makum head 2
    talk(&mut world, burai); // → Scepter of Turek
    assert_eq!(item_count(&world, 3001, 3214), 1, "Scepter of Turek");
    // Tunath (Harak): letter → scepter directly
    ev(&mut world, kasman, "30501-08.html"); // Kasman's 3rd Letter
    ev(&mut world, harak, "30618-03.html"); // → Scepter of Tunath
    assert_eq!(item_count(&world, 3001, 3215), 1, "Scepter of Tunath");
    // Breka (Voltar): letter → glove → Pashika + Vultus heads
    ev(&mut world, manakia, "30515-04.html"); // Manakia's 1st Letter
    ev(&mut world, voltar, "30615-04.html"); // → Glove of Voltar
    kill(&mut world, 27080); // Pashika's Head
    kill(&mut world, 27081); // Vultus's Head
    talk(&mut world, voltar); // → Scepter of Breka
    assert_eq!(item_count(&world, 3001, 3211), 1, "Scepter of Breka");
    // Enku (Kepra): letter → glove → 4 Enku heads (fifth scepter → cond 5)
    ev(&mut world, manakia, "30515-05.html"); // Manakia's 2nd Letter
    ev(&mut world, kepra, "30616-04.html"); // → Glove of Kepra
    for _ in 0..4 {
        kill(&mut world, 27082); // Enku overlord heads
    }
    talk(&mut world, kepra); // → Scepter of Enku, cond 5
    assert_eq!(item_count(&world, 3001, 3212), 1, "Scepter of Enku");
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    // Chianta assembles the scepters → 3rd Order.
    talk(&mut world, chianta);
    ev(&mut world, chianta, "30642-07.html"); // scepters → Chianta's 3rd Order, cond 6
    assert_eq!(item_count(&world, 3001, 3217), 1, "Chianta's 3rd Order");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    // Timak heads + Tamlin skulls → cond 7 (via Tamlin, per the faithful bug).
    inject(&mut world, 3001, 0x0220_0004, 3219, 19); // timak heads
    kill(&mut world, 20583); // → 20
    inject(&mut world, 3001, 0x0220_0005, 3218, 19); // tamlin skulls
    kill(&mut world, 20601); // → 20, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, chianta); // → Scepter Box, cond 8
    assert_eq!(item_count(&world, 3001, 3220), 1, "Scepter Box");
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    talk(&mut world, tanapi);
    ev(&mut world, tanapi, "30571-03.html"); // → Tanapi's Order, cond 9
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // Ragna summons the Revenant; slay it for the Scepter of Tantos.
    kill(&mut world, 20778); // Ragna → spawns Revenant
    assert!(
        !npcs_of(&mut world, 27086).is_empty(),
        "Revenant of Tantos conjured"
    );
    kill(&mut world, 27086); // Revenant → Scepter of Tantos, cond 10
    assert_eq!(item_count(&world, 3001, 3236), 1, "Scepter of Tantos");
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, tanapi); // → Ritual Box, cond 11
    assert_eq!(item_count(&world, 3001, 3237), 1, "Ritual Box");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    // --- Completion at Kakai ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, kakai);
    assert_eq!(item_count(&world, 3001, 3203), 1, "Mark of Glory awarded");
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 262720,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(11),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00221_testimony_of_prosperity() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = (3239..=3275).map(|id| (id, "Q221", true)).collect();
    items.push((3023, "Recipe Titan Key", true));
    items.push((3030, "Key of Titan", true));
    items.push((3428, "Crystal Brooch", true));
    items.push((3238, "Mark of Prosperity", false));
    items.push((1867, "Animal Skin", false));
    add_quest_items(&mut world, &items);
    for id in [20154, 20228, 20157, 20231, 20233] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let n = |i| NPC_OID + i;
    let (parman, wilford, lilith, bright, lockirin, shari, mion, toma) =
        (n(0), n(1), n(2), n(3), n(4), n(5), n(6), n(7));
    let (spiron, balanki, keef, filaur, arin, maryse, bolter, torocco) =
        (n(8), n(9), n(10), n(11), n(12), n(13), n(14), n(15));
    let (piotur, emily, nikola, boxt) = (n(16), n(17), n(18), n(19));
    for (oid, npc) in [
        (parman, 30104),
        (wilford, 30005),
        (lilith, 30368),
        (bright, 30466),
        (lockirin, 30531),
        (shari, 30517),
        (mion, 30519),
        (toma, 30556),
        (spiron, 30532),
        (balanki, 30533),
        (keef, 30534),
        (filaur, 30535),
        (arin, 30536),
        (maryse, 30553),
        (bolter, 30554),
        (torocco, 30555),
        (piotur, 30597),
        (emily, 30620),
        (nikola, 30621),
        (boxt, 30622),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 54; // Scavenger
        p.race = 4; // Dwarf
    }
    world
        .data
        .categories
        .insert_for_test("DWARF_2ND_GROUP", &[54]);
    let q = "Q00221_TestimonyOfProsperity";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, parman);
    ev(&mut world, parman, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Proof 1: Blessed Seed ---
    ev(&mut world, piotur, "30597-02.html");
    assert_eq!(item_count(&world, 3001, 3242), 1, "Blessed Seed");
    // --- Proof 2: Lilith's Elven Wafer ---
    ev(&mut world, wilford, "30005-04.html"); // Crystal Brooch
    ev(&mut world, lilith, "30368-03.html"); // → Lilith's Elven Wafer
    assert_eq!(item_count(&world, 3001, 3244), 1, "Lilith's Elven Wafer");
    // --- Proof 3: Emily's Recipe ---
    ev(&mut world, bright, "30466-03.html"); // Bright's List
    inject(&mut world, 3001, 0x0221_0000, 3265, 19);
    kill(&mut world, 20154); // Mandragora petal → 20
    inject(&mut world, 3001, 0x0221_0001, 3266, 9);
    kill(&mut world, 20228); // Crimson moss → 10
    talk(&mut world, bright); // → Mandragora Bouquet
    assert_eq!(item_count(&world, 3001, 3267), 1, "Mandragora Bouquet");
    ev(&mut world, emily, "30620-03.html"); // → Emily's Recipe
    assert_eq!(item_count(&world, 3001, 3243), 1, "Emily's Recipe");
    // --- Proof 4: Old Account Book (five guild contributions) ---
    ev(&mut world, lockirin, "30531-03.html"); // license + 5 notices
    // Receipt 1 (Spiron/Shari)
    talk(&mut world, spiron); // takes 1st notice
    talk(&mut world, shari); // Contribution of Shari
    talk(&mut world, spiron); // → Receipt 1st
    assert_eq!(item_count(&world, 3001, 3258), 1, "Receipt 1st");
    // Receipt 2 (Balanki/Mion+Maryse)
    talk(&mut world, balanki); // takes 2nd notice
    talk(&mut world, mion); // Contribution of Mion
    talk(&mut world, maryse); // Maryse's Request
    inject(&mut world, 3001, 0x0221_0002, 1867, 10); // 10 animal skin
    talk(&mut world, maryse); // → Contribution of Maryse
    talk(&mut world, balanki); // → Receipt 2nd
    assert_eq!(item_count(&world, 3001, 3259), 1, "Receipt 2nd");
    // Receipt 3 (Keef/Torocco, 5000 adena)
    talk(&mut world, keef); // takes 3rd notice
    ev(&mut world, torocco, "30555-02.html"); // Procuration of Torocco
    inject(&mut world, 3001, 0x0221_0003, 57, 5000);
    ev(&mut world, keef, "30534-03a.html"); // 5000 adena → Receipt 3rd
    assert_eq!(item_count(&world, 3001, 3260), 1, "Receipt 3rd");
    // Receipt 4 (Filaur/Bolter)
    talk(&mut world, filaur); // takes 4th notice
    talk(&mut world, bolter); // Receipt of Bolter
    talk(&mut world, filaur); // → Receipt 4th
    assert_eq!(item_count(&world, 3001, 3261), 1, "Receipt 4th");
    // Receipt 5 (Arin/Toma)
    talk(&mut world, arin); // takes 5th notice
    talk(&mut world, toma); // Contribution of Toma
    talk(&mut world, arin); // → Receipt 5th
    assert_eq!(item_count(&world, 3001, 3262), 1, "Receipt 5th");
    talk(&mut world, lockirin); // 5 receipts → Old Account Book, cond 2
    assert_eq!(item_count(&world, 3001, 3241), 1, "Old Account Book");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    // --- Parman: First Ring → Second Ring ---
    talk(&mut world, parman);
    ev(&mut world, parman, "30104-08.html"); // → Ring 2nd + Parman's Letter, cond 4
    assert_eq!(
        item_count(&world, 3001, 3240),
        1,
        "Second Ring of Testimony"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    // --- Phase 2: Key of Titan ---
    talk(&mut world, nikola);
    ev(&mut world, nikola, "30621-04.html"); // → Clay Dough, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, boxt);
    ev(&mut world, boxt, "30622-02.html"); // → Pattern of Keyhole, cond 6
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    talk(&mut world, nikola); // → Recipe + Nikola's List, cond 7
    assert_eq!(item_count(&world, 3001, 3272), 1, "Nikola's List");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    inject(&mut world, 3001, 0x0221_0004, 3273, 19);
    kill(&mut world, 20157); // Stakato shell → 20
    inject(&mut world, 3001, 0x0221_0005, 3274, 9);
    kill(&mut world, 20231); // Toad lord sac → 10
    inject(&mut world, 3001, 0x0221_0006, 3275, 9);
    kill(&mut world, 20233); // Marsh spider thorn → 10, cond 8
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    inject(&mut world, 3001, 0x0221_0007, 3030, 1); // crafted Key of Titan
    talk(&mut world, boxt);
    ev(&mut world, boxt, "30622-04.html"); // key → Maphr's Tablet Fragment, cond 9
    assert_eq!(item_count(&world, 3001, 3245), 1, "Maphr's Tablet Fragment");
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    // --- Completion at Parman ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, parman);
    assert_eq!(
        item_count(&world, 3001, 3238),
        1,
        "Mark of Prosperity awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 217682,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(9),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00214_trial_of_the_scholar() {
    let (mut world, _db, _l) = quest_test_world();
    let ids: Vec<i32> = (2674..=2720).filter(|&i| i != 2712).collect();
    let mut items: Vec<(i32, &str, bool)> = ids.iter().map(|&id| (id, "Q214", true)).collect();
    items.push((2674, "Mark of Scholar", false));
    add_quest_items(&mut world, &items);
    for id in [
        20580, 20068, 20269, 20235, 20554, 20158, 20201, 20552, 20567,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let n = |i| NPC_OID + i;
    let (mirien, sylvain, maria, lucas, creta) = (n(0), n(1), n(2), n(3), n(4));
    let (jurek, cronos, dieter, edroc, raut) = (n(5), n(6), n(7), n(8), n(9));
    let (triff, valkon, poitan, casian) = (n(10), n(11), n(12), n(13));
    for (oid, npc) in [
        (mirien, 30461),
        (sylvain, 30070),
        (maria, 30608),
        (lucas, 30071),
        (creta, 30609),
        (jurek, 30115),
        (cronos, 30610),
        (dieter, 30111),
        (edroc, 30230),
        (raut, 30316),
        (triff, 30611),
        (valkon, 30103),
        (poitan, 30458),
        (casian, 30612),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00214_TrialOfTheScholar";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 40;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    talk(&mut world, mirien);
    ev(&mut world, mirien, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // --- Symbol of Sylvain ---
    ev(&mut world, sylvain, "30070-02.html"); // High Priest's Sigil + Sylvain's Letter, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    ev(&mut world, maria, "30608-02.html"); // Maria's 1st Letter, cond 3
    talk(&mut world, lucas); // Lucas's Letter, cond 4
    talk(&mut world, maria); // Maria's 2nd Letter, cond 5
    ev(&mut world, creta, "30609-05.html"); // Creta's 1st Letter, cond 6
    ev(&mut world, maria, "30608-08.html"); // Lucilla's Handbag, cond 7
    ev(&mut world, creta, "30609-09.html"); // Crera's Painting1, cond 8
    talk(&mut world, maria); // Painting2, cond 9
    ev(&mut world, lucas, "30071-04.html"); // Painting3, cond 10
    assert_eq!(quest_cond(&world, 3001, q), Some(10));
    talk(&mut world, maria); // cond 11
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    inject(&mut world, 3001, 0x0214_0000, 2687, 4);
    kill(&mut world, 20580); // Leto → 5 brown scraps, cond 12
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    ev(&mut world, maria, "30608-14.html"); // Crystal of Purity 1, cond 13
    talk(&mut world, sylvain); // → Symbol of Sylvain, cond 14
    assert_eq!(item_count(&world, 3001, 2693), 1, "Symbol of Sylvain");
    talk(&mut world, mirien); // → Mirien's 2nd Sigil, cond 15
    assert_eq!(item_count(&world, 3001, 2676), 1, "Mirien's 2nd Sigil");
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    // --- Symbol of Jurek ---
    ev(&mut world, jurek, "30115-03.html"); // Jurek's List + Grand Magister Sigil, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    inject(&mut world, 3001, 0x0214_0001, 2695, 4);
    kill(&mut world, 20068); // Monster Eye skin → 5
    inject(&mut world, 3001, 0x0214_0002, 2696, 4);
    kill(&mut world, 20269); // Shaman's necklace → 5
    inject(&mut world, 3001, 0x0214_0003, 2697, 1);
    kill(&mut world, 20235); // Shackle's scalp → 2, cond 17
    assert_eq!(quest_cond(&world, 3001, q), Some(17));
    talk(&mut world, jurek); // → Symbol of Jurek, cond 18
    assert_eq!(item_count(&world, 3001, 2698), 1, "Symbol of Jurek");
    talk(&mut world, mirien);
    ev(&mut world, mirien, "30461-10.html"); // → Mirien's 3rd Sigil, cond 19
    assert_eq!(item_count(&world, 3001, 2677), 1, "Mirien's 3rd Sigil");
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    // --- Symbol of Cronos ---
    ev(&mut world, cronos, "30610-10.html"); // Cronos Sigil + Letter, cond 20
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    ev(&mut world, dieter, "30111-05.html"); // Dieter's Key, cond 21
    ev(&mut world, creta, "30609-14.html"); // Creta's 2nd Letter, cond 22
    ev(&mut world, dieter, "30111-09.html"); // Dieter's Letter + Diary, cond 23
    ev(&mut world, edroc, "30230-02.html"); // Raut's Letter Envelope, cond 24
    ev(&mut world, raut, "30316-02.html"); // Scripture Chapter 1 + Strong Liquor, cond 25
    ev(&mut world, triff, "30611-04.html"); // Triff's Ring, cond 26
    assert_eq!(item_count(&world, 3001, 2705), 1, "Triff's Ring");
    assert_eq!(quest_cond(&world, 3001, q), Some(26));
    // Casian turns the player away while chapters 2 and 3 are missing, and that
    // refusal is itself the journal step: cond 27 is what names the places the
    // missing chapters come from (Valkon in Giran, Grandis in Death Pass).
    talk(&mut world, poitan); // Poitan's Notes
    assert_eq!(item_count(&world, 3001, 2711), 1, "Poitan's Notes");
    talk(&mut world, casian); // 30612-01 — only chapter 1 in hand
    assert_eq!(quest_cond(&world, 3001, q), Some(27));
    // Chapter 2 (Valkon/Maria) — the hand-over marks Maria on the radar, since
    // neither Valkon's page nor any journal step named her before. Every
    // RadarControl in the drained batch, as (showRadar, radarType) pairs.
    let radar = |pkts: &[Vec<u8>]| -> Vec<(i32, i32)> {
        pkts.iter()
            .filter(|p| p[0] == 0xF1)
            .map(|p| {
                (
                    i32::from_le_bytes(p[1..5].try_into().unwrap()),
                    i32::from_le_bytes(p[5..9].try_into().unwrap()),
                )
            })
            .collect()
    };
    drain(&mut rx);
    ev(&mut world, valkon, "30103-04.html"); // Valkon's Request
    assert_eq!(
        radar(&drain(&mut rx)),
        vec![(0, 2)],
        "quest-type marker (not the red flag) on Maria with Valkon's Request"
    );
    talk(&mut world, maria); // → Crystal of Purity 2
    assert!(
        radar(&drain(&mut rx)).contains(&(2, 2)),
        "marker retired once the Crystal of Purity is in hand"
    );
    talk(&mut world, valkon); // → Scripture Chapter 2
    assert_eq!(item_count(&world, 3001, 2707), 1, "Scripture Chapter 2");
    // Chapter 3 (Grandis)
    kill(&mut world, 20554); // → Scripture Chapter 3
    assert_eq!(item_count(&world, 3001, 2708), 1, "Scripture Chapter 3");
    // Chapter 4 (Poitan/Casian + four reagents)
    talk(&mut world, poitan); // Poitan's Notes
    drain(&mut rx);
    ev(&mut world, casian, "30612-04.html"); // Casian's List, cond 28
    assert_eq!(quest_cond(&world, 3001, q), Some(28));
    // The reagent page must name the gargoyle the way the npc/item data and
    // the client's tables do. It shipped saying "Enhanced Gargoyle Nails"
    // against data reading "Reinforced Gargoyle's Nail", which left the errand
    // pointing at a monster no name in the world matched — see
    // docs/CUSTOM_DIST_DEVIATIONS.md. A re-sync from the Java reference dist
    // would silently restore the old wording, so pin it.
    let list_page = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("Casian's reagent list");
    assert!(
        list_page.contains("Reinforced Gargoyle's Nails"),
        "reagent page names the item as the data does: {list_page}"
    );
    assert!(
        !list_page.contains("Enhanced Gargoyle"),
        "the retail wording must not come back: {list_page}"
    );
    inject(&mut world, 3001, 0x0214_0004, 2717, 11);
    kill(&mut world, 20158); // Medusa's Blood → 12
    inject(&mut world, 3001, 0x0214_0005, 2716, 9);
    kill(&mut world, 20201); // Ghoul's Skin → 10 — but ichor and nails are still owed
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(28),
        "a single finished reagent must not send the player back to Casian"
    );
    inject(&mut world, 3001, 0x0214_0006, 2718, 4);
    kill(&mut world, 20552); // Fettered Soul's Ichor → 5
    assert_eq!(quest_cond(&world, 3001, q), Some(28));
    inject(&mut world, 3001, 0x0214_0007, 2719, 4);
    kill(&mut world, 20567); // Gargoyle's Nail → 5: the set completes (32), cond 29
    assert_eq!(quest_cond(&world, 3001, q), Some(29));
    ev(&mut world, casian, "30612-07.html"); // → Scripture Chapter 4, cond 30
    assert_eq!(item_count(&world, 3001, 2709), 1, "Scripture Chapter 4");
    assert_eq!(quest_cond(&world, 3001, q), Some(30));
    ev(&mut world, cronos, "30610-14.html"); // → Symbol of Cronos, cond 31
    assert_eq!(item_count(&world, 3001, 2720), 1, "Symbol of Cronos");
    assert_eq!(quest_cond(&world, 3001, q), Some(31));
    // --- Completion at Mirien ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, mirien);
    assert_eq!(
        item_count(&world, 3001, 2674),
        1,
        "Mark of the Scholar awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 319628,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(31),
        "one-time quest finished"
    );
}

#[test]
fn quest_q00227_test_of_the_reformer() {
    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        2822, 2823, 2824, 2825, 2826, 2827, 2828, 2829, 2830, 2831, 2832, 2833, 2834, 2835, 2836,
        2837, 2838, 3037, 5567, 5568,
    ]
    .iter()
    .map(|&id| (id, "Q227", true))
    .collect();
    items.push((2821, "Mark of Reformer", false));
    add_quest_items(&mut world, &items);
    for id in [27099, 27128, 27129, 27130, 27131, 27132, 20022, 20100] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let pupina = NPC_OID;
    let sla = NPC_OID + 1;
    let katari = NPC_OID + 2;
    let pilgrim = NPC_OID + 3;
    let kakan = NPC_OID + 4;
    let nyakuri = NPC_OID + 5;
    let ramus = NPC_OID + 6;
    for (oid, npc) in [
        (pupina, 30118),
        (sla, 30666),
        (katari, 30668),
        (pilgrim, 30732),
        (kakan, 30669),
        (nyakuri, 30670),
        (ramus, 30667),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 0, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 15; // Cleric
    }
    let q = "Q00227_TestOfTheReformer";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 30, 0, 0);
        npc::npc_do_die(w, mob, 3001);
    };
    // A skill hit: stash the skill id the way the damage path does, then strike.
    let skill_hit = |w: &mut World, npc_oid: i32, skill_id: i32| {
        w.quest_attack_skill = Some(skill_id);
        combat::npc_receive_damage(w, npc_oid, 3001, 10.0, false);
        w.quest_attack_skill = None;
    };
    talk(&mut world, pupina);
    ev(&mut world, pupina, "ACCEPT");
    assert_eq!(quest_memo(&world, 3001, q), 1);
    // --- Nameless Revenant: only Disrupt Undead (1031) marks it (scriptValue 1). ---
    // Negative first: a plain melee kill drops nothing.
    inject(&mut world, 3001, 0x0227_0000, 2831, 6); // 6 ripped diaries
    kill(&mut world, 27099); // melee (no skill) → scriptValue stays 0 → no drop
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "melee kill does not reform the revenant"
    );
    // Now with Disrupt Undead:
    add_test_npc(&mut world, NPC_OID + 20, 27099, "Monster", 40, 40, 0, 0);
    skill_hit(&mut world, NPC_OID + 20, 1031); // Disrupt Undead → scriptValue 1
    npc::npc_do_die(&mut world, NPC_OID + 20, 3001); // 7th diary → spawn Araurune, cond 2
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert!(!npcs_of(&mut world, 27128).is_empty(), "Araurune conjured");
    kill(&mut world, 27128); // Araurune → Huge Nail, memo 3, cond 3
    assert_eq!(item_count(&world, 3001, 2832), 1, "Huge Nail");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    ev(&mut world, pupina, "30118-06.html"); // → Letter of Introduction, memo 4, cond 4
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    ev(&mut world, sla, "30666-04.html"); // → Sla's Letter, memo 5, cond 5
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    talk(&mut world, katari); // memo 6, cond 6, spawn inspector
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    kill(&mut world, 27129); // Ol Mahum Inspector → memo 7, cond 7
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    talk(&mut world, pilgrim); // Ol Mahum Money, memo 8
    assert_eq!(item_count(&world, 3001, 2826), 1, "Ol Mahum Money");
    talk(&mut world, katari); // memo 8, cond 8, spawn betrayer
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    kill(&mut world, 27130); // Ol Mahum Betrayer → memo 9, cond 9, Letter of Betrayer
    assert_eq!(item_count(&world, 3001, 2833), 1, "Letter of Betrayer");
    talk(&mut world, katari); // → Katari's Letter, memo 10, cond 10
    assert_eq!(item_count(&world, 3001, 2827), 1, "Katari's Letter");
    talk(&mut world, sla); // money → Greetings, memo 11, cond 11
    assert_eq!(item_count(&world, 3001, 2825), 1, "Greetings");
    assert_eq!(quest_cond(&world, 3001, q), Some(11));
    talk(&mut world, kakan);
    // Register the duel monster's template so the staged spawn can conjure it.
    if world.data.npc_data.get(27131).is_none() {
        let mut t = crate::data::npc_data::default_template(27131);
        t.type_name = "Monster".into();
        world.data.npc_data.insert_for_test(t);
    }
    ev(&mut world, kakan, "30669-03.html"); // cond 12, spawn the staged duel
    assert_eq!(quest_cond(&world, 3001, q), Some(12));
    // The staged duel: a decoy Ol Mahum Pilgrim and the werewolf appear at
    // Java's fixed spots, and the wolf's hate points at the *decoy*, not the
    // player.
    {
        let wolf = npcs_of(&mut world, 27131)[0];
        let hate_on_decoy = world
            .objects
            .get_component::<AggroList>(&wolf)
            .is_some_and(|a| {
                a.0.keys().any(|t| {
                    world
                        .objects
                        .get_component::<model::npc::Npc>(t)
                        .is_some_and(|n| n.npc_id == 30732)
                })
            });
        assert!(hate_on_decoy, "the werewolf opens on the decoy pilgrim");
    }
    // --- Crimson Werewolf: flees from melee, credited only to a mage attack. ---
    add_test_npc(&mut world, NPC_OID + 21, 27131, "Monster", 40, 40, 0, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 21, 3001, 10.0, false); // melee → flees
    assert!(
        npcs_of(&mut world, 27131)
            .iter()
            .all(|&o| o != NPC_OID + 21),
        "melee makes the werewolf flee"
    );
    add_test_npc(&mut world, NPC_OID + 22, 27131, "Monster", 40, 40, 0, 0);
    skill_hit(&mut world, NPC_OID + 22, 1177); // Wind Strike → scriptValue = player
    npc::npc_do_die(&mut world, NPC_OID + 22, 3001); // → memo 12, cond 13
    assert_eq!(quest_cond(&world, 3001, q), Some(13));
    talk(&mut world, kakan); // → Kakan's Letter, memo 13, cond 14
    assert_eq!(item_count(&world, 3001, 3037), 1, "Kakan's Letter");
    talk(&mut world, nyakuri);
    ev(&mut world, nyakuri, "30670-03.html"); // cond 15, spawn krudel
    assert_eq!(quest_cond(&world, 3001, q), Some(15));
    kill(&mut world, 27132); // Krudel Lizardman → memo 14, cond 16
    assert_eq!(quest_cond(&world, 3001, q), Some(16));
    talk(&mut world, nyakuri); // → Nyakuri's Letter, memo 15, cond 17
    assert_eq!(item_count(&world, 3001, 2828), 1, "Nyakuri's Letter");
    talk(&mut world, ramus); // → Undead List, memo 16, cond 18
    assert_eq!(item_count(&world, 3001, 2829), 1, "Undead List");
    assert_eq!(quest_cond(&world, 3001, q), Some(18));
    // Five bone fragments → memo 17, cond 19.
    for id in [2834, 2835, 2836, 2837] {
        inject(&mut world, 3001, 0x0227_0100 + id, id, 1);
    }
    kill(&mut world, 20100); // Skeleton Archer → Bone Fragment 8 (fifth), cond 19
    assert_eq!(quest_cond(&world, 3001, q), Some(19));
    talk(&mut world, ramus); // bones → Ramus's Letter, memo 18, cond 20
    assert_eq!(item_count(&world, 3001, 2830), 1, "Ramus's Letter");
    assert_eq!(quest_cond(&world, 3001, q), Some(20));
    // --- Completion at Sla ---
    let a = item_count(&world, 3001, 57);
    talk(&mut world, sla);
    assert_eq!(
        item_count(&world, 3001, 2821),
        1,
        "Mark of the Reformer awarded"
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 226528,
        "final adena reward"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(20),
        "one-time quest finished"
    );
}

/// The Test-of-the-Summoner (230) arcana-duel primitive, end to end: a
/// servitor's blow reaches `on_attack` marked `is_summon`, the quest sends the
/// rival NPC back at the servitor (`make_npc_attack`), and the servitor's kill
/// is credited to the owner in `on_kill` (VICTORY). Proves the pieces the
/// deferred Q230 needs — `attack_is_summon`, `owner_servitor`, `make_npc_attack`,
/// `is_oid_dead` — cooperate over real servitor combat.
#[test]
fn servitor_arcana_duel_round_trip() {
    const OPPONENT: i32 = 27102; // Pako the Cat
    const SERVITOR_NPC: i32 = 14100; // a Cat servitor template
    const STARTING: i32 = 3360;
    const INPROGRESS: i32 = 3361;
    const VICTORY: i32 = 3364;

    struct ArcanaBattleTest;
    impl quests::QuestScript for ArcanaBattleTest {
        fn id(&self) -> i32 {
            -30
        }
        fn name(&self) -> &'static str {
            "ArcanaBattleTest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[]
        }
        fn kill_npcs(&self) -> &[i32] {
            &[OPPONENT]
        }
        fn attack_npcs(&self) -> &[i32] {
            &[OPPONENT]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_kill(&self, ctx: &mut quests::QuestCtx) {
            if ctx.quest_items_count(INPROGRESS) > 0 {
                ctx.take_items(INPROGRESS, -1);
                ctx.give_items(VICTORY, 1);
            }
        }
        fn on_attack(&self, ctx: &mut quests::QuestCtx) {
            match ctx.npc_script_value() {
                0 if ctx.attack_is_summon()
                    && let Some(servitor) = ctx.owner_servitor() =>
                {
                    ctx.set_npc_var_int("ATTACKER", servitor);
                    ctx.set_npc_script_value(1);
                    ctx.start_quest_timer("KILLED_ATTACKER", 5000);
                    if ctx.quest_items_count(STARTING) > 0 {
                        ctx.take_items(STARTING, -1);
                        ctx.give_items(INPROGRESS, 1);
                        ctx.make_npc_attack(servitor); // the rival strikes back
                    }
                }
                1 if !ctx.attack_is_summon()
                    || ctx.owner_servitor() != Some(ctx.npc_var_int("ATTACKER")) =>
                {
                    // A foul: the player, or a different summon, interfered.
                    ctx.set_npc_script_value(2);
                    ctx.delete_npc();
                }
                _ => {}
            }
        }
        fn on_timer(&self, ctx: &mut quests::QuestCtx, name: &str) {
            if name == "KILLED_ATTACKER" && ctx.is_oid_dead(ctx.npc_var_int("ATTACKER")) {
                ctx.delete_npc();
            }
        }
    }

    let (mut world, _db, _l) = quest_test_world();
    world.quests = Arc::new(quests::QuestRegistry::new(vec![Arc::new(ArcanaBattleTest)]));
    add_quest_items(
        &mut world,
        &[
            (STARTING, "start", true),
            (INPROGRESS, "prog", true),
            (VICTORY, "win", true),
        ],
    );
    // A Servitor template for the owner and a quest-monster for the rival.
    let mut st = crate::data::npc_data::default_template(SERVITOR_NPC);
    st.type_name = "Servitor".into();
    st.base_hp_max = 400.0;
    st.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(st);
    let mut ot = crate::data::npc_data::default_template(OPPONENT);
    ot.type_name = "Monster".into();
    ot.level = 40;
    ot.base_hp_max = 100_000.0;
    world.data.npc_data.insert_for_test(ot);

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    inject(&mut world, 3001, 0x0230_0000, STARTING, 1);
    let servitor = crate::game_loop::servitor::summon_servitor(
        &mut world,
        3001,
        SERVITOR_NPC,
        283,
        1200,
        0,
        0,
    )
    .expect("servitor summoned");
    let opponent = NPC_OID + 5;
    add_test_npc(&mut world, opponent, OPPONENT, "Monster", 40, 120, 200, 0);

    // The servitor lands the first blow: reaches on_attack marked is_summon.
    combat::npc_receive_damage(&mut world, opponent, servitor, 10.0, false);
    assert_eq!(
        item_count(&world, 3001, STARTING),
        0,
        "Starting crystal consumed"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS),
        1,
        "In-Progress crystal granted"
    );
    // The rival was set on the servitor (make_npc_attack seeded its aggro).
    let seeded = world
        .objects
        .get_component::<AggroList>(&opponent)
        .is_some_and(|a| a.0.contains_key(&servitor));
    assert!(seeded, "the rival strikes back at the servitor");

    // A foul: the owner (not their summon) hits the rival → it quits (deleted).
    add_test_npc(
        &mut world,
        NPC_OID + 6,
        OPPONENT,
        "Monster",
        40,
        120,
        200,
        0,
    );
    combat::npc_receive_damage(&mut world, NPC_OID + 6, servitor, 1.0, false); // servitor engages it
    combat::npc_receive_damage(&mut world, NPC_OID + 6, 3001, 1.0, false); // the OWNER interferes
    assert!(
        world
            .objects
            .get_component::<Vitals>(&(NPC_OID + 6))
            .is_none_or(|v| v.dead),
        "a player-struck rival fouls out and despawns"
    );

    // The servitor finishes the real duel: its kill is credited to the owner.
    npc::npc_do_die(&mut world, opponent, servitor);
    assert_eq!(
        item_count(&world, 3001, INPROGRESS),
        0,
        "In-Progress consumed on victory"
    );
    assert_eq!(
        item_count(&world, 3001, VICTORY),
        1,
        "Victory crystal awarded to the owner"
    );
}

/// Test of the Summoner (230) end to end: the class/level gate, Grocer Lara's
/// list + token farm (with its list gating), the Beginner's Arcana turn-in, a
/// full arcana duel driven through real servitor combat (foul path and victory
/// path), redeeming Victory crystals for all six Summoner arcanas, and Galatea's
/// completion reward.
#[test]
fn quest_q00230_test_of_the_summoner() {
    // Item ids (see q00230_test_of_the_summoner.rs).
    const GALATEAS_LETTER: i32 = 3352;
    const LARAS_1ST_LIST: i32 = 3347;
    const LETO_AMULET: i32 = 3337;
    const SAC_OF_REDSPORES: i32 = 3338;
    const KARUL_TOTEM: i32 = 3339;
    const BEGINNERS_ARCANA: i32 = 3353;
    const STARTING_1ST: i32 = 3360;
    const INPROGRESS_1ST: i32 = 3361;
    const FOUL_1ST: i32 = 3362;
    const VICTORY_1ST: i32 = 3364;
    const ALMORS_ARCANA: i32 = 3354;
    const MARK_OF_SUMMONER: i32 = 3336;
    // NPCs
    const GALATEA: i32 = 30634;
    const LARA: i32 = 30063;
    const ALMORS: i32 = 30635;
    const CAMONIELL: i32 = 30636;
    const BELTHUS: i32 = 30637;
    const BASILLA: i32 = 30638;
    const CELESTIEL: i32 = 30639;
    const BRYNTHEA: i32 = 30640;
    // Monsters
    const PAKO: i32 = 27102;
    const LETO: i32 = 20577;
    const KARUL: i32 = 20600;
    const SERVITOR_NPC: i32 = 14100;

    let (mut world, _db, _l) = quest_test_world();
    // Every quest item this test moves, plus the (tradeable) reward.
    let mut items: Vec<(i32, &str, bool)> = [
        GALATEAS_LETTER,
        LARAS_1ST_LIST,
        3348,
        3349,
        3350,
        3351, // lists 2..5
        LETO_AMULET,
        SAC_OF_REDSPORES,
        KARUL_TOTEM,
        BEGINNERS_ARCANA,
        STARTING_1ST,
        INPROGRESS_1ST,
        FOUL_1ST,
        3363, // DEFEAT_1ST
        VICTORY_1ST,
        ALMORS_ARCANA,
        3355,
        3356,
        3357,
        3358,
        3359, // other 5 arcanas
        3369,
        3374,
        3379,
        3384,
        3389, // VICTORY 2nd..6th
    ]
    .iter()
    .map(|&id| (id, "Q230", true))
    .collect();
    items.push((MARK_OF_SUMMONER, "Mark of Summoner", false));
    add_quest_items(&mut world, &items);

    // A Cat servitor template for the owner; a durable Pako for the duel.
    let mut sv = crate::data::npc_data::default_template(SERVITOR_NPC);
    sv.type_name = "Servitor".into();
    sv.base_hp_max = 400.0;
    sv.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(sv);
    for id in [PAKO, LETO, KARUL] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }

    let galatea = NPC_OID;
    let lara = NPC_OID + 1;
    let almors = NPC_OID + 2;
    let camoniell = NPC_OID + 3;
    let belthus = NPC_OID + 4;
    let basilla = NPC_OID + 5;
    let celestiel = NPC_OID + 6;
    let brynthea = NPC_OID + 7;
    for (oid, npc) in [
        (galatea, GALATEA),
        (lara, LARA),
        (almors, ALMORS),
        (camoniell, CAMONIELL),
        (belthus, BELTHUS),
        (basilla, BASILLA),
        (celestiel, CELESTIEL),
        (brynthea, BRYNTHEA),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00230_TestOfTheSummoner";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };

    // Grab the first HTML from a talk, whether it went out as a `.html`
    // (`NpcHtmlMessage`) or a `.htm` (`ExNpcQuestHtmlMessage`).
    let grab_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> Option<String> {
        drain(rx).iter().find_map(|p| {
            if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(p)
            } else if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?; // ex opcode
                r.read_i32()?; // npc oid
                r.read_string()
            } else {
                None
            }
        })
    };

    // --- Class / level gate on the start NPC. ---
    talk(&mut world, galatea);
    let html = grab_html(&mut rx).expect("Galatea greets a Wizard");
    // The 30634-03 offer page carries the "accept the trial" button (→30634-04).
    assert!(
        html.contains("30634-04.htm"),
        "level-39 Wizard is offered the trial: {html}"
    );
    // A non-caster is turned away — the refusal page has no accept button.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .class_id = 10; // Human Fighter
    talk(&mut world, galatea);
    let html = grab_html(&mut rx).unwrap();
    assert!(
        !html.contains("30634-04.htm"),
        "a fighter is refused (no accept button): {html}"
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .class_id = 11;

    // --- Accept: Galatea's Letter, quest started. ---
    ev(&mut world, galatea, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(
        item_count(&world, 3001, GALATEAS_LETTER),
        1,
        "Galatea's Letter"
    );

    // --- Lara hands out a hunting list (forced to the 1st), takes the Letter. ---
    world.force_roll(0); // getRandom(5) → LARAS_1ST_LIST
    ev(&mut world, lara, "30063-02.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(
        item_count(&world, 3001, LARAS_1ST_LIST),
        1,
        "1st list granted"
    );
    assert_eq!(
        item_count(&world, 3001, GALATEAS_LETTER),
        0,
        "Letter surrendered"
    );

    // --- Token farm: a matching kill drops, a mismatched one does not. ---
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 110, 200, 0);
        w.force_roll(0); // give_item_randomly roll_f64 → 0.0 ≤ chance
        npc::npc_do_die(w, mob, 3001);
    };
    kill(&mut world, LETO); // list1 held → Leto Lizardman Amulet drops
    assert!(
        item_count(&world, 3001, LETO_AMULET) >= 1,
        "amulet dropped while holding 1st list"
    );
    kill(&mut world, KARUL); // Karul needs the 2nd list → nothing
    assert_eq!(
        item_count(&world, 3001, KARUL_TOTEM),
        0,
        "no drop for a mismatched list"
    );

    // `Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true)` gates every
    // branch: a party member who was nowhere near the kill collects nothing.
    // 1500 is the configured range, so 5000 units out is comfortably outside.
    {
        let held = item_count(&world, 3001, LETO_AMULET);
        let far = NPC_OID + 90;
        add_test_npc(&mut world, far, LETO, "Monster", 40, 5_000, 5_000, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, far, 3001);
        assert_eq!(
            item_count(&world, 3001, LETO_AMULET),
            held,
            "a kill outside AltPartyRange drops nothing"
        );
    }

    // --- Turn in the 1st list: 30 + 30 tokens → two Beginner's Arcana, cond 3. ---
    inject(&mut world, 3001, 0x0230_1000, LETO_AMULET, 30);
    inject(&mut world, 3001, 0x0230_2000, SAC_OF_REDSPORES, 30);
    talk(&mut world, lara);
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(
        item_count(&world, 3001, BEGINNERS_ARCANA),
        2,
        "two Beginner's Arcana"
    );
    assert_eq!(
        item_count(&world, 3001, LARAS_1ST_LIST),
        0,
        "list consumed on turn-in"
    );

    // --- Summoner Almors: the offer needs an arcana; buying starts the duel. ---
    let _ = drain(&mut rx); // clear queued html from the prior turn-in
    ev(&mut world, almors, "30635-03.html"); // gated: shows the offer (arcana in hand)
    let html = grab_html(&mut rx).expect("offer page");
    assert!(
        html.contains("30635-04.htm") || html.contains("Almors"),
        "offer shown with an arcana in hand: {html}"
    );
    ev(&mut world, almors, "30635-04.html"); // Arcana → Crystal of Starting (1st)
    assert_eq!(
        item_count(&world, 3001, STARTING_1ST),
        1,
        "Crystal of Starting granted"
    );
    assert_eq!(
        item_count(&world, 3001, BEGINNERS_ARCANA),
        1,
        "one arcana spent"
    );

    // Summon the servitor that will fight the duels.
    let servitor = crate::game_loop::servitor::summon_servitor(
        &mut world,
        3001,
        SERVITOR_NPC,
        283,
        1200,
        0,
        0,
    )
    .expect("servitor summoned");

    // --- Foul path: servitor engages, then the *player* interferes. ---
    let pako1 = NPC_OID + 60;
    add_test_npc(&mut world, pako1, PAKO, "Monster", 40, 120, 200, 0);
    combat::npc_receive_damage(&mut world, pako1, servitor, 10.0, false); // servitor engages
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        1,
        "duel engaged: In-Progress"
    );
    assert_eq!(
        item_count(&world, 3001, STARTING_1ST),
        0,
        "Starting consumed on engage"
    );
    combat::npc_receive_damage(&mut world, pako1, 3001, 10.0, false); // the OWNER fouls it
    assert_eq!(
        item_count(&world, 3001, FOUL_1ST),
        1,
        "a player strike fouls the duel"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        0,
        "In-Progress lost on foul"
    );

    // --- Victory path: buy a fresh Starting (clears the Foul), win by servitor. ---
    ev(&mut world, almors, "30635-04.html");
    assert_eq!(
        item_count(&world, 3001, FOUL_1ST),
        0,
        "Foul cleared by a fresh Starting"
    );
    assert_eq!(item_count(&world, 3001, STARTING_1ST), 1);
    let pako2 = NPC_OID + 61;
    add_test_npc(&mut world, pako2, PAKO, "Monster", 40, 120, 200, 0);
    combat::npc_receive_damage(&mut world, pako2, servitor, 10.0, false); // engage
    assert_eq!(item_count(&world, 3001, INPROGRESS_1ST), 1);
    npc::npc_do_die(&mut world, pako2, servitor); // servitor kill → owner-credited
    assert_eq!(
        item_count(&world, 3001, VICTORY_1ST),
        1,
        "Victory on a servitor kill"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        0,
        "In-Progress consumed on victory"
    );

    // --- Redeem Victory for the Almors Arcana. ---
    talk(&mut world, almors);
    assert_eq!(
        item_count(&world, 3001, ALMORS_ARCANA),
        1,
        "Almors Arcana earned"
    );
    assert_eq!(item_count(&world, 3001, VICTORY_1ST), 0, "Victory redeemed");

    // --- The other five duels: inject Victory crystals and redeem each. The
    // final redemption (all six arcanas held) advances to cond 4. ---
    for (obj, victory, summoner) in [
        (0x0230_3000, 3369, basilla),   // 2nd → Basillia
        (0x0230_4000, 3374, camoniell), // 3rd → Camoniell
        (0x0230_5000, 3379, celestiel), // 4th → Celestiel
        (0x0230_6000, 3384, belthus),   // 5th → Belthus
        (0x0230_7000, 3389, brynthea),  // 6th → Brynthea
    ] {
        inject(&mut world, 3001, obj, victory, 1);
        talk(&mut world, summoner);
    }
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(4),
        "all six arcanas → cond 4"
    );
    for arcana in [3355, 3356, 3357, 3358, 3359] {
        assert_eq!(
            item_count(&world, 3001, arcana),
            1,
            "arcana {arcana} earned"
        );
    }

    // --- Galatea completes the test: Mark of Summoner, exit. ---
    talk(&mut world, galatea);
    assert_eq!(
        item_count(&world, 3001, MARK_OF_SUMMONER),
        1,
        "Mark of Summoner awarded"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
}

/// Fate's Whisper (234) end to end: the level-75 gate, the boss-chest material
/// chain (Soul Orb + three Infernium Scepters), the Varnish/Hammer/Mold forging
/// steps, the Pipette-on-Baium fill, and Reorin's templated B→A grade weapon
/// upgrade that awards the Star of Destiny. Also proves `onKill` drops a chest.
#[test]
fn quest_q00234_fates_whisper() {
    // Items
    const REIRIA_SOUL_ORB: i32 = 4666;
    const KERMON: i32 = 4667;
    const GOLKONDA: i32 = 4668;
    const HALLATE: i32 = 4669;
    const INFERNIUM_VARNISH: i32 = 4672;
    const REORIN_HAMMER: i32 = 4670;
    const REORIN_MOLD: i32 = 4671;
    const PIPETTE_KNIFE: i32 = 4665;
    const RED_PIPETTE_KNIFE: i32 = 4673;
    const CRYSTAL_B: i32 = 1460;
    const STAR_OF_DESTINY: i32 = 5011;
    const B_GRADE: i32 = 79; // Sword of Damascus
    const A_GRADE: i32 = 6580; // the upgraded weapon
    // NPCs
    const REORIN: i32 = 31002;
    const N30182: i32 = 30182;
    const N30847: i32 = 30847;
    const N30178: i32 = 30178;
    const N30833: i32 = 30833;
    const BAIUM: i32 = 29020;
    const BOSS_25035: i32 = 25035;

    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        REIRIA_SOUL_ORB,
        KERMON,
        GOLKONDA,
        HALLATE,
        INFERNIUM_VARNISH,
        REORIN_HAMMER,
        REORIN_MOLD,
        PIPETTE_KNIFE,
        RED_PIPETTE_KNIFE,
        CRYSTAL_B,
        B_GRADE,
    ]
    .iter()
    .map(|&id| (id, "Q234", true))
    .collect();
    items.push((STAR_OF_DESTINY, "Star of Destiny", false));
    items.push((A_GRADE, "Sirra's Blade", false));
    add_quest_items(&mut world, &items);

    // Baium + the chest-dropping boss.
    for id in [BAIUM, BOSS_25035] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 80;
        t.base_hp_max = 1_000_000.0;
        world.data.npc_data.insert_for_test(t);
    }

    let reorin = NPC_OID;
    let n30182 = NPC_OID + 1;
    let n30847 = NPC_OID + 2;
    let n30178 = NPC_OID + 3;
    let n30833 = NPC_OID + 4;
    let chest27 = NPC_OID + 5;
    let chest28 = NPC_OID + 6;
    let chest29 = NPC_OID + 7;
    let chest30 = NPC_OID + 8;
    for (oid, npc) in [
        (reorin, REORIN),
        (n30182, N30182),
        (n30847, N30847),
        (n30178, N30178),
        (n30833, N30833),
        (chest27, 31027),
        (chest28, 31028),
        (chest29, 31029),
        (chest30, 31030),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00234_FatesWhisper";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let grab_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> Option<String> {
        drain(rx).iter().find_map(|p| {
            if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(p)
            } else if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?;
                r.read_i32()?;
                r.read_string()
            } else {
                None
            }
        })
    };

    // --- Level gate: below 75 is refused (the 31002-01 page has no start link). ---
    talk(&mut world, reorin);
    let html = grab_html(&mut rx).expect("Reorin greeting");
    assert!(
        !html.contains("31002-03.htm"),
        "no start offered below 75: {html}"
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 76;

    // --- Accept. ---
    ev(&mut world, reorin, "31002-03.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started at cond 1");

    // --- onKill drops a chest beside the boss. ---
    let before = npcs_of(&mut world, 31027).len();
    add_test_npc(
        &mut world,
        NPC_OID + 40,
        BOSS_25035,
        "Monster",
        80,
        300,
        300,
        0,
    );
    npc::npc_do_die(&mut world, NPC_OID + 40, 3001);
    assert!(
        npcs_of(&mut world, 31027).len() > before,
        "killing boss 25035 spawns a 31027 chest"
    );
    // Java's `addSpawn(…, true, 120000)`: the chest is on a two-minute fuse,
    // so a drop nobody collects clears itself instead of standing for ever.
    // Just short of two minutes it is still there…
    advance_ticks(&mut world, 1190);
    assert!(
        npcs_of(&mut world, 31027).len() > before,
        "the chest is still standing before its 120 s are up"
    );
    // …and past it, gone.
    advance_ticks(&mut world, 30);
    assert_eq!(
        npcs_of(&mut world, 31027).len(),
        before,
        "the chest despawns after two minutes"
    );
    // Re-spawn one for the rest of the walkthrough, which opens it.
    add_test_npc(
        &mut world,
        NPC_OID + 41,
        BOSS_25035,
        "Monster",
        80,
        300,
        300,
        0,
    );
    npc::npc_do_die(&mut world, NPC_OID + 41, 3001);

    // --- Chest 31027 → Reiria Soul Orb, then Reorin advances to cond 2. ---
    talk(&mut world, chest27);
    assert_eq!(
        item_count(&world, 3001, REIRIA_SOUL_ORB),
        1,
        "Soul Orb from chest"
    );
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "orb turned in → cond 2"
    );
    assert_eq!(item_count(&world, 3001, REIRIA_SOUL_ORB), 0, "orb consumed");

    // --- Chests 31028/29/30 → the three Infernium Scepters, Reorin → cond 3. ---
    talk(&mut world, chest28);
    talk(&mut world, chest29);
    talk(&mut world, chest30);
    assert_eq!(item_count(&world, 3001, KERMON), 1);
    assert_eq!(item_count(&world, 3001, GOLKONDA), 1);
    assert_eq!(item_count(&world, 3001, HALLATE), 1);
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "scepters turned in → cond 3"
    );

    // --- 30182 hands over the Infernium Varnish (via its 01c event). ---
    ev(&mut world, n30182, "30182-01c.htm");
    assert_eq!(
        item_count(&world, 3001, INFERNIUM_VARNISH),
        1,
        "Varnish received"
    );
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(4), "varnish → cond 4");

    // --- 30847 gives the Reorin Hammer. ---
    talk(&mut world, n30847);
    assert_eq!(
        item_count(&world, 3001, REORIN_HAMMER),
        1,
        "Hammer received"
    );
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(5), "hammer → cond 5");

    // --- 30178 (cond 5) → its 01a event advances to cond 6. ---
    ev(&mut world, n30178, "30178-01a.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(6));

    // --- 30833 (cond 6) → its 01b event gives the Pipette Knife, cond 7. ---
    ev(&mut world, n30833, "30833-01b.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(
        item_count(&world, 3001, PIPETTE_KNIFE),
        1,
        "Pipette Knife received"
    );

    // --- Baium: striking with the Pipette Knife equipped fills it (Red). ---
    equip_weapon_row(&mut world, 3001, PIPETTE_KNIFE); // RHand = Pipette Knife
    add_test_npc(&mut world, NPC_OID + 51, BAIUM, "Monster", 80, 120, 200, 0);
    combat::npc_receive_damage(&mut world, NPC_OID + 51, 3001, 10.0, false);
    assert_eq!(
        item_count(&world, 3001, RED_PIPETTE_KNIFE),
        1,
        "Pipette filled on Baium"
    );
    assert_eq!(
        item_count(&world, 3001, PIPETTE_KNIFE),
        0,
        "empty Pipette consumed"
    );

    // --- 30833 (cond 7, holding Red Pipette) → Reorin Mold, cond 8. ---
    talk(&mut world, n30833);
    assert_eq!(quest_cond(&world, 3001, q), Some(8));
    assert_eq!(item_count(&world, 3001, REORIN_MOLD), 1, "Mold received");

    // --- Reorin: cond 8 → cond 9 (mold consumed), then 984 crystals → cond 10. ---
    talk(&mut world, reorin);
    assert_eq!(quest_cond(&world, 3001, q), Some(9));
    assert_eq!(item_count(&world, 3001, REORIN_MOLD), 0, "mold consumed");
    inject(&mut world, 3001, 0x0234_1000, CRYSTAL_B, 984);
    talk(&mut world, reorin);
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(10),
        "984 crystals → cond 10"
    );
    assert_eq!(item_count(&world, 3001, CRYSTAL_B), 0, "crystals consumed");

    // --- The weapon UI: select a B-grade, confirm, then upgrade to A-grade. ---
    let _ = drain(&mut rx); // clear the queued BGradeList page
    ev(&mut world, reorin, "selectBGrade_79");
    let html = grab_html(&mut rx).expect("B-grade confirm page");
    assert!(
        html.contains("Sword of Damascus"),
        "%weaponname% substituted into 31002-13: {html}"
    );
    ev(&mut world, reorin, "confirmWeapon");
    let _ = drain(&mut rx);
    // The A-grade trade needs the B-grade weapon in the bag.
    inject(&mut world, 3001, 0x0234_2000, B_GRADE, 1);
    ev(&mut world, reorin, &format!("selectAGrade_{A_GRADE}"));
    assert_eq!(item_count(&world, 3001, B_GRADE), 0, "B-grade consumed");
    assert_eq!(
        item_count(&world, 3001, A_GRADE),
        1,
        "A-grade weapon forged"
    );
    assert_eq!(
        item_count(&world, 3001, STAR_OF_DESTINY),
        1,
        "Star of Destiny awarded"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "quest completes on the upgrade");
}
