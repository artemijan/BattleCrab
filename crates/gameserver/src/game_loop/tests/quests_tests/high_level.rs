//! Q00600s — the high-level area quests.

use super::*;

#[test]
fn quest_q00623_the_finest_food() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7199, "Leaf of Flava", true),
            (7200, "Buffalo Meat", true),
            (7201, "Horn of Antelope", true),
            (6849, "Ring of Aurakyra", false),
        ],
    );
    // Thermal Antelope (21318) drops Horn of Antelope.
    register_npc(&mut world, 21318, "Monster", 71);
    add_test_npc(&mut world, NPC_OID, 31521, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 71;
    let q = "Q00623_TheFinestFood";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 31521-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Two ingredients full and horn one short, so a single antelope kill tops
    // horn to its cap and flips cond → 2.
    inject(&mut world, 3001, 0x7199_0000, 7199, 100);
    inject(&mut world, 3001, 0x7200_0000, 7200, 100);
    inject(&mut world, 3001, 0x7201_0000, 7201, 99);
    kill_mob(&mut world, NPC_OID + 1, 21318, 71, 0); // give_item_randomly roll_f64 → hit
    assert_eq!(
        item_count(&world, 3001, 7201),
        100,
        "antelope tops horn to 100"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "all three ingredients → cond 2"
    );
    // Turn in: force reward roll 0 (< 120) → Ring of Aurakyra + 25000 adena, exit.
    let a = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 31521-06.html")),
    );
    assert_eq!(item_count(&world, 3001, 6849), 1, "ring reward");
    assert_eq!(item_count(&world, 3001, 57), a + 25000, "25000 adena");
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00617_gather_the_flames() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7264, "Torch", true),
            (6881, "Recipe A", false),
            (6883, "Recipe B", false),
        ],
    );
    register_npc(&mut world, 22634, "Monster", 74);
    add_test_npc(&mut world, NPC_OID, 31539, "Folk", 70, 100, 0, 0); // Vulcan
    add_test_npc(&mut world, NPC_OID + 1, 32049, "Folk", 70, 100, 0, 0); // Rooney
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00617_GatherTheFlames";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 31539-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill 22634 (threshold 639): roll(1000)=0 < 639 → 2 torches (plain giveItems).
    kill_mob(&mut world, NPC_OID + 2, 22634, 74, 0);
    assert_eq!(
        item_count(&world, 3001, 7264),
        2,
        "22634 drops 2 torches on a low roll"
    );
    // Vulcan: 1000 torches → one random S-grade recipe (force index 0 → 6881).
    inject(&mut world, 3001, 0x7264_0000, 7264, 998);
    world.force_roll(0); // getRandomEntry index
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 31539-07.html")),
    );
    assert_eq!(item_count(&world, 3001, 6881), 1, "random S-grade recipe");
    assert_eq!(item_count(&world, 3001, 7264), 0, "1000 torches consumed");
    // Rooney: 1200 torches → the chosen recipe 6883.
    inject(&mut world, 3001, 0x7264_0001, 7264, 1200);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Script {q} 6883", NPC_OID + 1)),
    );
    assert_eq!(item_count(&world, 3001, 6883), 1, "chosen recipe 6883");
    assert_eq!(item_count(&world, 3001, 7264), 0, "1200 torches consumed");
}

#[test]
fn quest_q00688_defeat_the_elrokian_raiders() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(8785, "Dinosaur Fang Necklace", true)]);
    register_npc(&mut world, 22214, "Monster", 75);
    add_test_npc(&mut world, NPC_OID, 32105, "Folk", 70, 100, 0, 0); // Dinn
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 75;
    let q = "Q00688_DefeatTheElrokianRaiders";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 32105-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill Elroki: DROP_RATE 448 folded into the threshold, roll(1000)=0 < 448.
    kill_mob(&mut world, NPC_OID + 1, 22214, 75, 0);
    assert_eq!(item_count(&world, 3001, 8785), 1, "Elroki drops a necklace");
    // Per-necklace turn-in: 10 necklaces → 30000 adena.
    inject(&mut world, 3001, 0x0688_0000, 8785, 9);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 32105-06.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 30000,
        "10 necklaces → 30000 adena"
    );
    assert_eq!(item_count(&world, 3001, 8785), 0, "necklaces consumed");
    // Donation: 100 necklaces, roll(1000)=0 < 500 → 450000 adena.
    inject(&mut world, 3001, 0x0688_0001, 8785, 100);
    let b = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} donation")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        b + 450000,
        "donation jackpot → 450000 adena"
    );
    assert_eq!(item_count(&world, 3001, 8785), 0, "100 necklaces consumed");
}

#[test]
fn quest_q00622_specialty_liquor_delivery() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7197, "Special Drink", true),
            (7198, "Special Drink Price", true),
            (734, "Quick Step Potion", false),
        ],
    );
    // Seven talk NPCs, one fixture each.
    let jeremy = NPC_OID;
    let boelin = NPC_OID + 1;
    let kuber = NPC_OID + 2;
    let crocus = NPC_OID + 3;
    let naff = NPC_OID + 4;
    let pulin = NPC_OID + 5;
    let lietta = NPC_OID + 6;
    add_test_npc(&mut world, jeremy, 31521, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, boelin, 31547, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, kuber, 31546, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, crocus, 31545, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, naff, 31544, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, pulin, 31543, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, lietta, 31267, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 70;
    let q = "Q00622_SpecialtyLiquorDelivery";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{jeremy}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{jeremy}_Script {q} 31521-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(
        item_count(&world, 3001, 7197),
        5,
        "Jeremy hands over 5 drinks"
    );
    // Deliver to the five bartenders in order (Boelin, Kuber, Crocus, Naff, Pulin).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{boelin}_Script {q} 31547-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kuber}_Script {q} 31546-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{crocus}_Script {q} 31545-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{naff}_Script {q} 31544-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pulin}_Script {q} 31543-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(6));
    assert_eq!(item_count(&world, 3001, 7197), 0, "all drinks delivered");
    assert_eq!(
        item_count(&world, 3001, 7198),
        5,
        "five payment slips collected"
    );
    // Jeremy takes the five slips → cond 7.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{jeremy}_Script {q} 31521-06.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(item_count(&world, 3001, 7198), 0, "slips consumed");
    // Lietta: roll(1000)=0 < 800 → Quick Step Potion + 18800 adena, exit.
    let a = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lietta}_Script {q} 31267-02.html")),
    );
    assert_eq!(item_count(&world, 3001, 734), 1, "Quick Step Potion reward");
    assert_eq!(item_count(&world, 3001, 57), a + 18800, "18800 adena");
    assert_eq!(
        quest_cond(&world, 3001, q),
        None,
        "repeatable exit removes the quest"
    );
}

#[test]
fn quest_q00643_rise_and_fall_of_the_elroki_tribe() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (8776, "Bones of a Plains Dinosaur", true),
            (8712, "Sirra's Blade Edge", false),
        ],
    );
    register_npc(&mut world, 22200, "Monster", 75);
    add_test_npc(&mut world, NPC_OID, 32106, "Folk", 75, 100, 0, 0); // Singsing
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 75;
    let q = "Q00643_RiseAndFallOfTheElrokiTribe";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} quest_accept")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // MOBS1 always pays; roll(1000)=0 < 116 → 2 bones.
    kill_mob(&mut world, NPC_OID + 1, 22200, 75, 0);
    assert_eq!(
        item_count(&world, 3001, 8776),
        2,
        "a MOBS1 dinosaur drops 2 bones"
    );
    // Sell at Singsing (32106-09): 1374 adena per bone.
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} 32106-09.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 2 * 1374,
        "2 bones → 2748 adena"
    );
    assert_eq!(item_count(&world, 3001, 8776), 0, "bones consumed");
    // Exchange 300 bones for 5 of a random weapon piece (force index 0 → 8712).
    inject(&mut world, 3001, 0x0643_0000, 8776, 300);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script {q} exchange")),
    );
    assert_eq!(
        item_count(&world, 3001, 8712),
        5,
        "exchange yields 5 weapon pieces"
    );
    assert_eq!(item_count(&world, 3001, 8776), 0, "300 bones consumed");
}

#[test]
fn quest_q00642_a_powerful_primeval_creature() {
    const DINN: i32 = 32105;
    const TISSUE_MOB: i32 = 22196; // Velociraptor (0.309)
    const ANCIENT_EGG: i32 = 18344;
    const DINOSAUR_TISSUE: i32 = 8774;
    const DINOSAUR_EGG: i32 = 8775;
    const ADENA: i32 = 57;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(DINOSAUR_TISSUE, "q", true), (DINOSAUR_EGG, "q", true)],
    );
    for id in [TISSUE_MOB, ANCIENT_EGG] {
        register_npc_hp(&mut world, id, "Monster", 78, 100.0);
    }
    let dinn = NPC_OID;
    add_test_npc(&mut world, dinn, DINN, "Folk", 78, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 78;
    let q = "Q00642_APowerfulPrimevalCreature";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q}")));
    };

    talk(&mut world, dinn);
    ev(&mut world, dinn, "32105-05.html"); // accept
    assert_eq!(
        quest_state(&world, 3001, q),
        Some(model::quest::state::STARTED),
        "started"
    );

    // Loot: a Velociraptor drops Tissue (0.309, forced roll 0.0 passes), the
    // Ancient Egg always drops a Dinosaur Egg.
    let mut mob = NPC_OID + 20;
    for _ in 0..3 {
        mob += 1;
        kill_mob_at(&mut world, mob, TISSUE_MOB, 78, 110, 200, 0);
    }
    assert_eq!(item_count(&world, 3001, DINOSAUR_TISSUE), 3, "3 tissues");
    mob += 1;
    kill_mob_at(&mut world, mob, ANCIENT_EGG, 78, 110, 200, 0);
    assert_eq!(
        item_count(&world, 3001, DINOSAUR_EGG),
        1,
        "Dinosaur Egg from Ancient Egg"
    );

    // Turn in the tissue for adena (5000 each), consuming it.
    let adena_before = item_count(&world, 3001, ADENA);
    ev(&mut world, dinn, "32105-09.html");
    assert_eq!(
        item_count(&world, 3001, DINOSAUR_TISSUE),
        0,
        "tissue turned in"
    );
    assert!(
        item_count(&world, 3001, ADENA) > adena_before,
        "paid for tissue"
    );

    // Exit (repeatable): the quest state is removed so it can be retaken.
    ev(&mut world, dinn, "exit");
    assert!(
        quest_cond(&world, 3001, q).is_none(),
        "repeatable exit forgets the quest"
    );
}

#[test]
fn quest_q00641_attack_sailren() {
    const STATUE: i32 = 32109;
    const RAPTOR: i32 = 22196;
    const GAZKH_FRAGMENT: i32 = 8782;
    const GAZKH: i32 = 8784;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(GAZKH_FRAGMENT, "q", true), (GAZKH, "key", false)],
    );
    register_npc_hp(&mut world, RAPTOR, "Monster", 78, 100.0);
    let statue = NPC_OID;
    add_test_npc(&mut world, statue, STATUE, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 78;
    let q = "Q00641_AttackSailren";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q}")));
    };

    // Prereq: without The Name of Evil 2 complete, the statue shows 32109-0b
    // (no accept button to 32109-1).
    talk(&mut world, statue);
    let html = any_html(&mut rx);
    assert!(
        !html.contains("32109-1.html"),
        "no accept without prereq: {html}"
    );

    // Mark The Name of Evil 2 (126) complete → the accept page opens.
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = quests
            .0
            .entry("Q00126_TheNameOfEvil2".to_string())
            .or_default();
        qs.state = model::quest::state::COMPLETED;
    }
    talk(&mut world, statue);
    let html = any_html(&mut rx);
    // The 0a page (prereq met) leads on to 0c → the accept; 0b (unmet) does not.
    assert!(
        html.contains("32109-0c"),
        "accept path opens with prereq: {html}"
    );

    // Accept, grind 30 Gazkh Fragments (100% per raptor).
    ev(&mut world, statue, "32109-1.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started at cond 1");
    let mut mob = NPC_OID + 20;
    for _ in 0..30 {
        mob += 1;
        add_test_npc(&mut world, mob, RAPTOR, "Monster", 78, 110, 200, 0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, GAZKH_FRAGMENT), 30, "30 fragments");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "30th fragment → cond 2"
    );

    // Fuse the fragments into a Gazkh; repeatable exit.
    ev(&mut world, statue, "32109-2a.html");
    assert_eq!(item_count(&world, 3001, GAZKH), 1, "Gazkh forged");
    assert_eq!(
        item_count(&world, 3001, GAZKH_FRAGMENT),
        0,
        "fragments consumed on exit"
    );
    assert!(
        quest_cond(&world, 3001, q).is_none(),
        "repeatable exit forgets the quest"
    );
}

/// Four Goblets (620): the Imperial Tomb quest — accept, farm a tomb undead for
/// Relics/Grave Pass/Sealed Box, slay the four bosses for their goblets, redeem
/// them for the Antique Brooch, open a Sealed Box, and trade 1,000 Relics for a
/// Sealed recipe.
#[test]
fn quest_q00620_four_goblets() {
    const NAMELESS: i32 = 31453;
    const WIGOTH_2: i32 = 31454;
    const ENTRANCE_PASS: i32 = 7075;
    const GRAVE_PASS: i32 = 7261;
    const RELIC: i32 = 7254;
    const SEALED_BOX: i32 = 7255;
    const GOBLETS: [i32; 4] = [7256, 7257, 7258, 7259];
    const ANTIQUE_BROOCH: i32 = 7262;
    const ADENA: i32 = 57;
    const RECIPE: i32 = 6881;
    const TOMB_MOB: i32 = 18120;
    const BOSSES: [i32; 4] = [25339, 25342, 25346, 25349];

    let (mut world, _db, _l) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = [
        ENTRANCE_PASS,
        GRAVE_PASS,
        RELIC,
        SEALED_BOX,
        GOBLETS[0],
        GOBLETS[1],
        GOBLETS[2],
        GOBLETS[3],
        ANTIQUE_BROOCH,
    ]
    .iter()
    .map(|&i| (i, "q", true))
    .collect();
    items.push((ADENA, "adena", false));
    items.push((RECIPE, "recipe", false));
    add_quest_items(&mut world, &items);
    for id in [TOMB_MOB, BOSSES[0], BOSSES[1], BOSSES[2], BOSSES[3]] {
        register_npc_hp(&mut world, id, "Monster", 78, 100.0);
    }
    let nameless = NPC_OID;
    let wigoth2 = NPC_OID + 1;
    add_test_npc(&mut world, nameless, NAMELESS, "Folk", 78, 100, 200, 0);
    add_test_npc(&mut world, wigoth2, WIGOTH_2, "Folk", 78, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 78;
    let q = "Q00620_FourGoblets";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Script {q}")));
    };

    // Accept (level 74-80) → Entrance Pass, cond 1.
    talk(&mut world, nameless);
    ev(&mut world, nameless, "accept");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    assert_eq!(item_count(&world, 3001, ENTRANCE_PASS), 1, "Entrance Pass");

    // Kill a tomb undead: Relic + Grave Pass, and (forced 15% roll) a Sealed Box.
    add_test_npc(
        &mut world,
        NPC_OID + 20,
        TOMB_MOB,
        "Monster",
        78,
        110,
        200,
        0,
    );
    world.force_roll(0); // roll(100) < 15 → Sealed Box
    npc::npc_do_die(&mut world, NPC_OID + 20, 3001);
    assert_eq!(item_count(&world, 3001, RELIC), 1, "Relic dropped");
    assert_eq!(
        item_count(&world, 3001, GRAVE_PASS),
        1,
        "Grave Pass dropped"
    );
    assert_eq!(
        item_count(&world, 3001, SEALED_BOX),
        1,
        "Sealed Box dropped"
    );

    // Slay the four bosses for the four goblets.
    let mut boss_oid = NPC_OID + 30;
    for (i, &boss) in BOSSES.iter().enumerate() {
        boss_oid += 1;
        add_test_npc(&mut world, boss_oid, boss, "Monster", 78, 110, 200, 0);
        npc::npc_do_die(&mut world, boss_oid, 3001);
        assert_eq!(
            item_count(&world, 3001, GOBLETS[i]),
            1,
            "goblet {i} from its boss"
        );
    }

    // Redeem the four goblets for the Antique Brooch (cond 2).
    ev(&mut world, nameless, "12");
    assert_eq!(
        item_count(&world, 3001, ANTIQUE_BROOCH),
        1,
        "Antique Brooch"
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "goblets turned in → cond 2"
    );
    assert_eq!(item_count(&world, 3001, GOBLETS[0]), 0, "goblets consumed");

    // Open the Sealed Box (forced roll 0 → the 10,000 Adena reward).
    world.force_roll(0); // roll(5) == 0
    ev(&mut world, wigoth2, "11");
    assert_eq!(item_count(&world, 3001, SEALED_BOX), 0, "box consumed");
    assert!(
        item_count(&world, 3001, ADENA) >= 10000,
        "Adena from the box"
    );

    // Trade 1,000 Relics for a Sealed recipe.
    inject(&mut world, 3001, 0x0620_1000, RELIC, 1000);
    ev(&mut world, wigoth2, "6881");
    assert_eq!(
        item_count(&world, 3001, RECIPE),
        1,
        "Sealed recipe for 1000 Relics"
    );
    assert!(item_count(&world, 3001, RELIC) <= 1, "relics consumed");
}

/// Quest 662 (A Game of Cards) — chip drops, staking 50 chips to deal a hand,
/// flipping all five cards, and scoring a pair for its prize.
#[test]
fn quest_q00662_a_game_of_cards() {
    const KLUMP: i32 = 30845;
    const RED_GEM: i32 = 8765;
    const BLOOD_QUEEN: i32 = 20142; // chip value 232
    const ONE_PAIR_PRIZE: i32 = 956; // i6 == 10 pays 2× item 956
    let q = "Q00662_AGameOfCards";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (RED_GEM, "Red Gem", true),
            (8868, "Ziggo's Gemstone", true),
            (ONE_PAIR_PRIZE, "Recipe", true),
        ],
    );
    {
        register_npc(&mut world, BLOOD_QUEEN, "Monster", 63);
    }
    let klump = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, klump, KLUMP, "Folk", 63, 100, 200, 0);
    add_test_npc(&mut world, mob, BLOOD_QUEEN, "Monster", 63, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 61;

    let ev = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{klump}_Script {q} {e}")));
    };
    let get_var = |w: &World, v: &str| quest_var_int(w, 3001, q, v);

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{klump}_Script {q}")),
    );
    ev(&mut world, "30845-03.htm");

    // A Blood Queen kill (value 232 < forced roll 999) drops a chip.
    world.force_roll(999); // roll(1000) → 999 > 232
    world.force_roll(0); // roll_f64 for the give
    quests::notify_kill(&mut world, 3001, mob, BLOOD_QUEEN, false);
    assert_eq!(
        item_count(&world, 3001, RED_GEM),
        1,
        "a kill dropped a chip"
    );

    // Stake 50 chips and deal a forced hand: raw [1,15,2,3,4] folds to values
    // [1,1,2,3,4] — a single pair (i1 == i2).
    inject(&mut world, 3001, 0x0066_2000, RED_GEM, 49); // top up to 50
    for r in [0, 14, 1, 2, 3] {
        world.force_roll(r); // roll(70)+1 → the five raw cards
    }
    ev(&mut world, "30845-11.html");
    assert_eq!(item_count(&world, 3001, RED_GEM), 0, "50 chips staked");
    assert_eq!(
        get_var(&world, "v1"),
        3020101,
        "hidden cards packed (i4i3i2i1)"
    );
    assert_eq!(get_var(&world, "ExMemoState"), 4, "the fifth card is 4");

    // Flip all five cards; the fifth flip triggers scoring.
    for card in ["turncard1", "turncard2", "turncard3", "turncard4"] {
        ev(&mut world, card);
    }
    assert_eq!(
        item_count(&world, 3001, ONE_PAIR_PRIZE),
        0,
        "no prize until all up"
    );
    ev(&mut world, "turncard5");
    assert_eq!(
        item_count(&world, 3001, ONE_PAIR_PRIZE),
        2,
        "a single pair pays 2× the prize item"
    );
    assert_eq!(
        get_var(&world, "v1"),
        0,
        "the board is cleared after scoring"
    );
}
