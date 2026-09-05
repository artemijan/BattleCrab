//! Q00211-Q00216 — the first class trials: Challenger, Duty, Seeker,
//! Scholar, Pilgrim and Guildsman.

use super::super::*;

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
