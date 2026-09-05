//! Q00222-Q00225 — the third class tests for the weapon classes: Duelist,
//! Champion, Sagittarius and Searcher.

use super::super::*;

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
