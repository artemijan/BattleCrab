//! Q00600s and the Q10800/Q11000 blocks — the high-level area quests.

use super::*;

#[test]
fn quest_q00619_relics_of_the_old_empire() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7254, "Relics", true),
            (7075, "Entrance", false),
            (6881, "Recipe", false),
        ],
    );
    // A killable Imperial Tomb monster in the registered 21396..=21434 range.
    let mut t = crate::data::npc_data::default_template(21400);
    t.type_name = "Monster".into();
    t.level = 74;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31538, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00619_RelicsOfTheOldEmpire";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31538-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill: roll(2)=0 → 2 relics; roll(100)=50 (> 10) → no entrance pass.
    add_test_npc(&mut world, NPC_OID + 1, 21400, "Monster", 74, 30, 0, 0);
    world.force_roll(0);
    world.force_roll(50);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 7254), 2, "2 relics from kill");
    assert_eq!(
        item_count(&world, 3001, 7075),
        0,
        "no entrance pass (roll 50 > 10)"
    );
    // Fast-forward to 1000 relics and turn in (force recipe index 0 → 6881).
    inject(&mut world, 3001, 0x7254_0000, 7254, 998);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31538-09.htm")),
    );
    assert_eq!(item_count(&world, 3001, 6881), 1, "one S-grade recipe");
    assert_eq!(item_count(&world, 3001, 7254), 0, "1000 relics consumed");
    // Repeatable: the quest is still started after turn-in.
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
}

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
    let mut t = crate::data::npc_data::default_template(21318);
    t.type_name = "Monster".into();
    t.level = 71;
    world.data.npc_data.insert_for_test(t);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31521-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Two ingredients full and horn one short, so a single antelope kill tops
    // horn to its cap and flips cond → 2.
    inject(&mut world, 3001, 0x7199_0000, 7199, 100);
    inject(&mut world, 3001, 0x7200_0000, 7200, 100);
    inject(&mut world, 3001, 0x7201_0000, 7201, 99);
    add_test_npc(&mut world, NPC_OID + 1, 21318, "Monster", 71, 30, 0, 0);
    world.force_roll(0); // give_item_randomly roll_f64 → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31521-06.html")),
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
    let mut t = crate::data::npc_data::default_template(22634);
    t.type_name = "Monster".into();
    t.level = 74;
    world.data.npc_data.insert_for_test(t);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31539-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill 22634 (threshold 639): roll(1000)=0 < 639 → 2 torches (plain giveItems).
    add_test_npc(&mut world, NPC_OID + 2, 22634, "Monster", 74, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31539-07.html")),
    );
    assert_eq!(item_count(&world, 3001, 6881), 1, "random S-grade recipe");
    assert_eq!(item_count(&world, 3001, 7264), 0, "1000 torches consumed");
    // Rooney: 1200 torches → the chosen recipe 6883.
    inject(&mut world, 3001, 0x7264_0001, 7264, 1200);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 6883", NPC_OID + 1)),
    );
    assert_eq!(item_count(&world, 3001, 6883), 1, "chosen recipe 6883");
    assert_eq!(item_count(&world, 3001, 7264), 0, "1200 torches consumed");
}

#[test]
fn quest_q00688_defeat_the_elrokian_raiders() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(8785, "Dinosaur Fang Necklace", true)]);
    let mut t = crate::data::npc_data::default_template(22214); // Elroki
    t.type_name = "Monster".into();
    t.level = 75;
    world.data.npc_data.insert_for_test(t);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 32105-03.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill Elroki: DROP_RATE 448 folded into the threshold, roll(1000)=0 < 448.
    add_test_npc(&mut world, NPC_OID + 1, 22214, "Monster", 75, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(item_count(&world, 3001, 8785), 1, "Elroki drops a necklace");
    // Per-necklace turn-in: 10 necklaces → 30000 adena.
    inject(&mut world, 3001, 0x0688_0000, 8785, 9);
    let a = item_count(&world, 3001, 57);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 32105-06.html")),
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} donation")),
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
        &bypass_body(&format!("npc_{jeremy}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{jeremy}_Quest {q} 31521-03.htm")),
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
        &bypass_body(&format!("npc_{boelin}_Quest {q} 31547-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kuber}_Quest {q} 31546-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{crocus}_Quest {q} 31545-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(4));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{naff}_Quest {q} 31544-02.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(5));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pulin}_Quest {q} 31543-02.html")),
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
        &bypass_body(&format!("npc_{jeremy}_Quest {q} 31521-06.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(7));
    assert_eq!(item_count(&world, 3001, 7198), 0, "slips consumed");
    // Lietta: roll(1000)=0 < 800 → Quick Step Potion + 18800 adena, exit.
    let a = item_count(&world, 3001, 57);
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lietta}_Quest {q} 31267-02.html")),
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
fn quest_q00628_hunt_golden_ram() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (7248, "Splinter Stakato Chitin", true),
            (7249, "Needle Stakato Chitin", true),
            (7246, "Golden Ram Badge Recruit", false),
            (7247, "Golden Ram Badge Soldier", false),
        ],
    );
    for id in [21508, 21513] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 66;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 31554, "Folk", 66, 100, 0, 0); // Kahman
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 66;
    let q = "Q00628_HuntGoldenRam";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} accept")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Splinter (count 1) drops at cond 1; needle (count 2) does not.
    add_test_npc(&mut world, NPC_OID + 1, 21508, "Monster", 66, 30, 0, 0);
    world.force_roll(0); // roll_f64 (0.0 < 0.5) → hit
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 7248),
        1,
        "splinter drops at cond 1"
    );
    add_test_npc(&mut world, NPC_OID + 2, 21513, "Monster", 66, 30, 0, 0);
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(
        item_count(&world, 3001, 7249),
        0,
        "needle (count 2) does NOT drop at cond 1"
    );
    // 100 splinters → Recruit badge, cond 2.
    inject(&mut world, 3001, 0x0628_0000, 7248, 99);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31554-08.html")),
    );
    assert_eq!(item_count(&world, 3001, 7246), 1, "Recruit badge awarded");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 7248), 0, "splinters consumed");
    // 100 splinter + 100 needle at cond 2 → Soldier badge, cond 3.
    inject(&mut world, 3001, 0x0628_0001, 7248, 100);
    inject(&mut world, 3001, 0x0628_0002, 7249, 100);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(item_count(&world, 3001, 7247), 1, "Soldier badge awarded");
    assert_eq!(item_count(&world, 3001, 7246), 0, "Recruit badge consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
}

#[test]
fn quest_q00606_battle_against_varka_silenos() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(7233, "Varka Mane", true), (7186, "Varka Horn", false)],
    );
    let mut t = crate::data::npc_data::default_template(21350); // chance 500
    t.type_name = "Monster".into();
    t.level = 74;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31370, "Folk", 74, 100, 0, 0); // Kadun
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00606_BattleAgainstVarkaSilenos";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31370-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // roll(1000)=0 < 500 → a mane.
    add_test_npc(&mut world, NPC_OID + 1, 21350, "Monster", 74, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 7233),
        1,
        "Varka recruit drops a mane"
    );
    // 100 manes → 20 horns.
    inject(&mut world, 3001, 0x0606_0000, 7233, 99);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31370-07.html")),
    );
    assert_eq!(item_count(&world, 3001, 7186), 20, "100 manes → 20 horns");
    assert_eq!(item_count(&world, 3001, 7233), 0, "manes consumed");
}

#[test]
fn quest_q00612_battle_against_ketra_orcs() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(7234, "Ketra Molar", true), (7187, "Ketra Seed", false)],
    );
    let mut t = crate::data::npc_data::default_template(21324); // chance 500
    t.type_name = "Monster".into();
    t.level = 74;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31377, "Folk", 74, 100, 0, 0); // Ashas
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;
    let q = "Q00612_BattleAgainstKetraOrcs";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31377-03.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    add_test_npc(&mut world, NPC_OID + 1, 21324, "Monster", 74, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 7234),
        1,
        "Ketra footman drops a molar"
    );
    inject(&mut world, 3001, 0x0612_0000, 7234, 99);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 31377-07.html")),
    );
    assert_eq!(item_count(&world, 3001, 7187), 20, "100 molars → 20 seeds");
    assert_eq!(item_count(&world, 3001, 7234), 0, "molars consumed");
}

#[test]
fn quest_q00634_in_search_of_fragments_of_dimension() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(7079, "Dimension Fragment", true)]);
    let mut t = crate::data::npc_data::default_template(21139); // an aggressive rift mob
    t.type_name = "Monster".into();
    t.level = 40;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 31095, "Folk", 40, 100, 0, 0); // a Dimensional Gate Keeper
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00634_InSearchOfFragmentsOfDimension";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 02.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // roll(100)=0 < 80 → fragments, amount = (int)(40*0.15 + 2.6) = 8.
    add_test_npc(&mut world, NPC_OID + 1, 21139, "Monster", 40, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
    assert_eq!(
        item_count(&world, 3001, 7079),
        8,
        "level-40 mob yields 8 fragments"
    );
    // 05.htm exits (repeatable).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 05.htm")),
    );
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
    let mut t = crate::data::npc_data::default_template(22200); // a MOBS1 dinosaur
    t.type_name = "Monster".into();
    t.level = 75;
    world.data.npc_data.insert_for_test(t);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} quest_accept")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // MOBS1 always pays; roll(1000)=0 < 116 → 2 bones.
    add_test_npc(&mut world, NPC_OID + 1, 22200, "Monster", 75, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 1, 3001);
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 32106-09.html")),
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
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} exchange")),
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
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 78;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
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
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };

    talk(&mut world, dinn);
    ev(&mut world, dinn, "32105-05.html"); // accept
    assert_eq!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0[q]
            .state,
        model::quest::state::STARTED,
        "started"
    );

    // Loot: a Velociraptor drops Tissue (0.309, forced roll 0.0 passes), the
    // Ancient Egg always drops a Dinosaur Egg.
    let mut mob = NPC_OID + 20;
    for _ in 0..3 {
        mob += 1;
        add_test_npc(&mut world, mob, TISSUE_MOB, "Monster", 78, 110, 200, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, DINOSAUR_TISSUE), 3, "3 tissues");
    mob += 1;
    add_test_npc(&mut world, mob, ANCIENT_EGG, "Monster", 78, 110, 200, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob, 3001);
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
    let mut rt = crate::data::npc_data::default_template(RAPTOR);
    rt.type_name = "Monster".into();
    rt.level = 78;
    rt.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(rt);
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
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let grab = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> String {
        drain(rx)
            .iter()
            .find_map(|p| {
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
            .unwrap_or_default()
    };

    // Prereq: without The Name of Evil 2 complete, the statue shows 32109-0b
    // (no accept button to 32109-1).
    talk(&mut world, statue);
    let html = grab(&mut rx);
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
    let html = grab(&mut rx);
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
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 78;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
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
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
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

/// Quest 605 (Alliance with Ketra Orcs) — the shared Alliance engine: accepting,
/// enemy kills dropping the rank-appropriate badge (gated by `can_get_item`'s
/// cap), and the first turn-in climbing from cond 1 to Mark of Alliance Lv1.
#[test]
fn quest_q00605_alliance_with_ketra_orcs() {
    use crate::model::components::social::Quests;

    const WAHKAN: i32 = 31371;
    const SOLDIER: i32 = 7216; // Varka Badge - Soldier
    const KETRA_MARK1: i32 = 7211;
    const RECRUIT: i32 = 21350; // Varka Silenos Recruit — min_cond 1, chance 500
    let q = "Q00605_AllianceWithKetraOrcs";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (SOLDIER, "Varka Badge - Soldier", true),
            (7217, "Varka Badge - Officer", true),
            (7218, "Varka Badge - Captain", true),
            (KETRA_MARK1, "Mark of Ketra's Alliance Lv1", false),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(RECRUIT);
        t.type_name = "Monster".into();
        t.level = 75;
        world.data.npc_data.insert_for_test(t);
    }
    let wahkan = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, wahkan, WAHKAN, "Folk", 75, 100, 200, 0);
    add_test_npc(&mut world, mob, RECRUIT, "Monster", 75, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;

    let started = |w: &World| -> bool {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED)
    };
    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{wahkan}_Quest {q} {e}")));
    };
    let kill = |w: &mut World, roll: i32| {
        w.force_roll(roll);
        quests::notify_kill(w, 3001, mob, RECRUIT, false);
    };

    // --- Accept: the ladder starts at cond 1. ---
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{wahkan}_Quest {q}")),
    );
    event(&mut world, "31371-04.htm");
    assert!(started(&world), "quest accepted");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "starts at rank 1");

    // --- A Varka kill (roll 0 < 500) drops one Soldier badge. ---
    kill(&mut world, 0);
    assert_eq!(
        item_count(&world, 3001, SOLDIER),
        1,
        "the recruit dropped a Soldier badge"
    );

    // A high roll (>= chance) drops nothing.
    kill(&mut world, 999);
    assert_eq!(
        item_count(&world, 3001, SOLDIER),
        1,
        "a missed roll drops nothing"
    );

    // --- The cap: at rank 1 you may bank at most 100 Soldier badges. ---
    inject(&mut world, 3001, 0x0060_5000, SOLDIER, 99); // top up to 100
    assert_eq!(item_count(&world, 3001, SOLDIER), 100);
    kill(&mut world, 0); // roll would succeed, but can_get_item caps it
    assert_eq!(
        item_count(&world, 3001, SOLDIER),
        100,
        "no more Soldier badges past the rank-1 cap"
    );

    // --- First turn-in: 100 Soldier badges → Mark of Ketra's Alliance Lv1, rank 2. ---
    event(&mut world, "31371-12.html");
    assert_eq!(
        item_count(&world, 3001, SOLDIER),
        0,
        "badges spent on the turn-in"
    );
    assert_eq!(
        item_count(&world, 3001, KETRA_MARK1),
        1,
        "Mark of Alliance Lv1 awarded"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "climbed to rank 2");
}

/// Quest 611 (Alliance with Varka Silenos) — the mirror registers and runs on
/// the same engine, and the mutual-exclusion gate refuses to start while the
/// player still holds a Ketra alliance mark.
#[test]
fn quest_q00611_varka_mirror_and_exclusion() {
    use crate::model::components::social::Quests;

    const NARAN: i32 = 31378;
    const KETRA_BADGE_SOLDIER: i32 = 7226;
    const KETRA_MARK1: i32 = 7211; // an *enemy* (Ketra) mark blocks Varka
    const FOOTMAN: i32 = 21324; // Ketra Orc Footman — min_cond 1
    let q = "Q00611_AllianceWithVarkaSilenos";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (KETRA_BADGE_SOLDIER, "Ketra Badge - Soldier", true),
            (7227, "Ketra Badge - Officer", true),
            (7228, "Ketra Badge - Captain", true),
            (KETRA_MARK1, "Mark of Ketra's Alliance Lv1", false),
        ],
    );
    {
        let mut t = crate::data::npc_data::default_template(FOOTMAN);
        t.type_name = "Monster".into();
        t.level = 75;
        world.data.npc_data.insert_for_test(t);
    }
    let naran = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, naran, NARAN, "Folk", 75, 100, 200, 0);
    add_test_npc(&mut world, mob, FOOTMAN, "Monster", 75, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 74;

    let started = |w: &World| -> bool {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED)
    };
    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{naran}_Quest {q} {e}")));
    };

    // --- Holding a Ketra mark, Varka refuses the alliance. ---
    inject(&mut world, 3001, 0x0061_1000, KETRA_MARK1, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{naran}_Quest {q}")),
    );
    event(&mut world, "31378-04.htm");
    assert!(
        !started(&world),
        "cannot ally with Varka while allied to Ketra"
    );

    // --- Drop the Ketra mark and the mirror starts and drops its own badges. ---
    world
        .objects
        .get_component_mut::<Inventory>(&3001)
        .unwrap()
        .remove_item(KETRA_MARK1, 1);
    event(&mut world, "31378-04.htm");
    assert!(
        started(&world),
        "Varka alliance starts once the Ketra mark is gone"
    );

    world.force_roll(0);
    quests::notify_kill(&mut world, 3001, mob, FOOTMAN, false);
    assert_eq!(
        item_count(&world, 3001, KETRA_BADGE_SOLDIER),
        1,
        "a Ketra kill drops a Ketra Soldier badge on the mirror"
    );
}

/// Quest 640 (The Zero Hour) — Spiked Stakato kills drop Fangs, exchanged in
/// fixed lots for crafting materials.
#[test]
fn quest_q00640_the_zero_hour() {
    use crate::model::components::social::Quests;

    const KAHMAN: i32 = 31554;
    const FANG: i32 = 8085;
    const ENRIA: i32 = 4042;
    const STAKATO: i32 = 22617;
    let q = "Q00640_TheZeroHour";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(FANG, "Fang of Stakato", true), (ENRIA, "Enria", false)],
    );
    {
        let mut t = crate::data::npc_data::default_template(STAKATO);
        t.type_name = "Monster".into();
        t.level = 67;
        world.data.npc_data.insert_for_test(t);
    }
    let kahman = NPC_OID;
    let mob = NPC_OID + 1;
    add_test_npc(&mut world, kahman, KAHMAN, "Folk", 67, 100, 200, 0);
    add_test_npc(&mut world, mob, STAKATO, "Monster", 67, 300, 300, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 66;
    // Mark Q109 (the prerequisite) completed.
    {
        let quests = world.objects.get_component_mut::<Quests>(&3001).unwrap();
        let qs = quests
            .0
            .entry("Q00109_InSearchOfTheNest".to_string())
            .or_default();
        qs.state = model::quest::state::COMPLETED;
    }

    let event = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{kahman}_Quest {q} {e}")));
    };
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kahman}_Quest {q}")),
    );
    event(&mut world, "31554-02.htm"); // accept
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");

    // A Stakato kill drops a fang.
    quests::notify_kill(&mut world, 3001, mob, STAKATO, false);
    assert_eq!(
        item_count(&world, 3001, FANG),
        1,
        "a stakato kill drops a fang"
    );

    // Exchange 12 fangs for Enria (button "1").
    inject(&mut world, 3001, 0x0064_0000, FANG, 11); // top up to 12
    event(&mut world, "1");
    assert_eq!(item_count(&world, 3001, ENRIA), 1, "12 fangs → Enria");
    assert_eq!(item_count(&world, 3001, FANG), 0, "the 12 fangs are spent");
}

/// Quest 662 (A Game of Cards) — chip drops, staking 50 chips to deal a hand,
/// flipping all five cards, and scoring a pair for its prize.
#[test]
fn quest_q00662_a_game_of_cards() {
    use crate::model::components::social::Quests;

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
        let mut t = crate::data::npc_data::default_template(BLOOD_QUEEN);
        t.type_name = "Monster".into();
        t.level = 63;
        world.data.npc_data.insert_for_test(t);
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
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{klump}_Quest {q} {e}")));
    };
    let get_var = |w: &World, v: &str| -> i32 {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .and_then(|qs| qs.vars.get(v))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{klump}_Quest {q}")),
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

/// **Q10866 is a courier run: three talks, three conds, one payout.**
///
/// The reward branch re-checks `isStarted()`, so the test also fires the
/// payout bypass from a fresh player who never took the quest — Java's guard
/// against a forged `34020-02.html`.
#[test]
fn quest_q10866_punitive_operation_on_the_devil_isle() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q10866_PunitiveOperationOnTheDevilIsle";
    // Rodemai starts it; Ein / Fethin / Nikia are the three stops.
    add_test_npc(&mut world, NPC_OID, 30756, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 34017, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 2, 34019, "Folk", 70, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 3, 34020, "Folk", 70, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 70;

    let say = |world: &mut World, oid: i32, ev: &str| {
        handle_request_bypass_to_server(
            world,
            1,
            &bypass_body(&format!("npc_{oid}_Quest {q} {ev}")),
        );
    };

    // Java's `onTalk` does `getQuestState(player, true)` — the first click is
    // what creates the state the button then starts.
    say(&mut world, NPC_OID, "");
    say(&mut world, NPC_OID, "30756-02.htm");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started at Rodemai");
    say(&mut world, NPC_OID + 1, "34017-02.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "Ein sends you on");
    say(&mut world, NPC_OID + 2, "34019-02.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(3), "Fethin sends you on");

    let adena = item_count(&world, 3001, 57);
    say(&mut world, NPC_OID + 3, "34020-02.html");
    assert_eq!(
        item_count(&world, 3001, 57),
        adena + 13_136,
        "Nikia pays 13136 adena"
    );
    assert!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0[q]
            .is_completed(),
        "and the quest is over"
    );

    // A player who *talked* to Rodemai but never accepted has a CREATED state,
    // so `has_qs()` is true and only the inner `isStarted()` stands between
    // them and a free 13 136 adena.
    let _rx2 = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3002)
        .unwrap()
        .level = 70;
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    assert_eq!(
        quest_cond(&world, 3002, q),
        Some(0),
        "state created by the talk, but not started"
    );
    let before = item_count(&world, 3002, 57);
    handle_request_bypass_to_server(
        &mut world,
        2,
        &bypass_body(&format!("npc_{}_Quest {q} 34020-02.html", NPC_OID + 3)),
    );
    assert_eq!(
        item_count(&world, 3002, 57),
        before,
        "a forged payout bypass pays nothing"
    );
}

/// **Q11001's cond 4 needs both drops, and the turn-in leaves the swords.**
///
/// The two halves of cond 4 are the part worth pinning: reaching ten Broken
/// Swords alone must *not* advance, because Java's Orc Warrior branch also
/// tests the Werewolf Fangs (and vice versa). A test that only fed one would
/// pass against a port that dropped the second half and leave players stuck.
#[test]
fn quest_q11001_tombs_of_ancestors() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q11001_TombsOfAncestors";
    add_quest_items(
        &mut world,
        &[
            (90199, "Hunter's Memo", false),
            (90200, "Wolf Pelt", true),
            (90201, "Orc Amulet", true),
            (90202, "Werewolf's Fang", true),
            (90203, "Broken Sword", true),
            (49039, "Necklace of the Novice", true),
            (49041, "Ring of the Novice", true),
            (49043, "Sword of Solidarity", false),
        ],
    );
    for id in [20093, 20132] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 10;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30598, "Folk", 20, 100, 0, 0); // Newbie Guide
    add_test_npc(&mut world, NPC_OID + 1, 30283, "Folk", 20, 100, 0, 0); // Altran
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 0; // Human — the quest's `addCondRace`
    }

    // The first click creates the quest state (`getQuestState(player, true)`);
    // the button then starts it.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30598-02.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "started");
    // Talking to Altran at cond 1 hands over the memo and moves to cond 2.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 1)),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(item_count(&world, 3001, 90199), 1, "Hunter's Memo given");

    // Skip the two collection stages the test isn't about.
    inject(&mut world, 3001, 0x1100_1000, 90200, 10);
    inject(&mut world, 3001, 0x1100_1001, 90201, 10);
    set_quest_cond(&mut world, 3001, q, 4);

    // Ten Broken Swords with no Fangs must NOT advance.
    inject(&mut world, 3001, 0x1100_1002, 90203, 9);
    add_test_npc(&mut world, NPC_OID + 2, 20093, "Monster", 10, 30, 0, 0);
    world.force_roll(0); // roll(100)=0 < 89 → drops
    npc::npc_do_die(&mut world, NPC_OID + 2, 3001);
    assert_eq!(item_count(&world, 3001, 90203), 10, "tenth sword collected");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(4),
        "still cond 4 — the fangs are missing"
    );

    // The tenth Fang closes the other half and now it advances.
    inject(&mut world, 3001, 0x1100_1003, 90202, 9);
    add_test_npc(&mut world, NPC_OID + 3, 20132, "Monster", 10, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, NPC_OID + 3, 3001);
    assert_eq!(quest_cond(&world, 3001, q), Some(5), "both halves done");

    // Turn in: the weapon branch, and the swords deliberately survive it.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} reward1", NPC_OID + 1)),
    );
    assert_eq!(item_count(&world, 3001, 49043), 1, "Sword of Solidarity");
    assert_eq!(item_count(&world, 3001, 49041), 2, "two novice rings");
    assert_eq!(item_count(&world, 3001, 49039), 1, "novice necklace");
    assert_eq!(item_count(&world, 3001, 90200), 0, "pelts taken");
    assert_eq!(item_count(&world, 3001, 90202), 0, "fangs taken");
    assert!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0[q]
            .is_completed(),
        "quest complete"
    );
}

/// **The uncapped stage variant drops on every kill, with no roll.**
///
/// `Q11013` and its siblings omit both Java's `< need` guard and the
/// `getRandom` roll. The *cap* half turns out to be unobservable here — the
/// kill that reaches ten also advances the cond, so the stage stops being
/// live before an extra drop can happen (see `newbie_chain`'s module note).
/// What this test does pin is the **roll**: ten kills with no `forced_rolls`
/// queued yield exactly ten tails, which a stage carrying a chance below 100
/// could not manage.
#[test]
fn quest_q11013_uncapped_stage_collects_past_the_requirement() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q11013_ShilensHunt";
    add_quest_items(
        &mut world,
        &[(90237, "Elder's Note", false), (90238, "Wolf Tail", true)],
    );
    let mut t = crate::data::npc_data::default_template(20456); // Ashen Wolf
    t.type_name = "Monster".into();
    t.level = 5;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30600, "Folk", 20, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30141, "Folk", 20, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 2; // Dark Elf
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30600-02.htm")),
    );
    // The second NPC's briefing is a talk, not a button.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 1)),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "briefed");

    // Ten kills, no forced rolls: every one drops, because this stage has no
    // chance gate at all.
    for i in 0..10 {
        add_test_npc(&mut world, NPC_OID + 10 + i, 20456, "Monster", 5, 30, 0, 0);
        npc::npc_do_die(&mut world, NPC_OID + 10 + i, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 90238),
        10,
        "ten kills, ten tails — no roll to fail"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(3), "advanced at ten");
}

/// **A capped stage stops at the requirement.** The other half of the pair
/// above: `Q11001`'s wolf pelts carry Java's `< need` guard, so an extra kill
/// after ten adds nothing.
#[test]
fn quest_q11001_capped_stage_stops_at_the_requirement() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q11001_TombsOfAncestors";
    add_quest_items(
        &mut world,
        &[(90199, "Hunter's Memo", false), (90200, "Wolf Pelt", true)],
    );
    let mut t = crate::data::npc_data::default_template(20120); // Wolf
    t.type_name = "Monster".into();
    t.level = 5;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30598, "Folk", 20, 100, 0, 0);
    add_test_npc(&mut world, NPC_OID + 1, 30283, "Folk", 20, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 10;
        p.race = 0;
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30598-02.htm")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 1)),
    );
    inject(&mut world, 3001, 0x1100_1100, 90200, 10);
    add_test_npc(&mut world, NPC_OID + 10, 20120, "Monster", 5, 30, 0, 0);
    world.force_roll(0); // would drop if the cap were gone
    npc::npc_do_die(&mut world, NPC_OID + 10, 3001);
    assert_eq!(
        item_count(&world, 3001, 90200),
        10,
        "capped at ten — the eleventh kill gives nothing"
    );
}

/// **A capstone books the chosen class path and its trainer pays out.**
///
/// Also pins the Java bug this quest carries: `a_cleric.html` sets cond 5, the
/// *wizard's* cond, while Zigaunt (the cleric trainer) answers only at cond 6.
/// A cleric is therefore served by Parina. Both pay the same reward, so the
/// quest completes either way — but the page you see is the wrong one.
#[test]
fn quest_q11006_future_people_class_paths() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q11006_FuturePeople";
    add_quest_items(&mut world, &[(49087, "Improved SoE", true)]);
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 20, 100, 0, 0); // Lector
    add_test_npc(&mut world, NPC_OID + 1, 30010, "Folk", 20, 100, 0, 0); // Auron
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.race = 0;
        p.class_id = 0; // Fighter
    }
    // The prerequisite quest, marked complete.
    world
        .objects
        .get_component_mut::<model::components::social::Quests>(&3001)
        .unwrap()
        .0
        .entry("Q11005_PerfectLeatherArmor3".to_string())
        .or_default()
        .state = model::quest::state::COMPLETED;

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} a_warrior.html")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "warrior path booked");

    // Auron pays out; `getCond() > 1` is what gates it.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 30010-02.html", NPC_OID + 1)),
    );
    assert_eq!(item_count(&world, 3001, 49087), 1, "Improved SoE paid");
    assert!(
        world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap()
            .0[q]
            .is_completed(),
        "quest complete"
    );
}

/// **Moon Knight stalls at cond 8, in Java too.**
///
/// Rolento's hand-over gives items 49559 and 49560, neither of which exists in
/// this datapack, and Gudz then gates on holding both. The test walks to cond
/// 8 and shows Gudz answering with the no-quest page rather than his cond-8
/// html — reproducing the dead end rather than papering over it.
#[test]
fn quest_q11000_moon_knight_stalls_where_java_does() {
    let (mut world, _db, _l) = quest_test_world();
    let q = "Q11000_MoonKnight";
    add_quest_items(
        &mut world,
        &[
            (49557, "Armor Trade Contract", false),
            (49558, "Turek Orc Order", false),
        ],
    );
    add_test_npc(&mut world, NPC_OID, 30939, "Folk", 40, 100, 0, 0); // Jones
    add_test_npc(&mut world, NPC_OID + 1, 30437, "Folk", 40, 100, 0, 0); // Rolento
    add_test_npc(&mut world, NPC_OID + 2, 30941, "Folk", 40, 100, 0, 0); // Gudz
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 30;

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30939-02.htm")),
    );
    set_quest_cond(&mut world, 3001, q, 7);
    inject(&mut world, 3001, 0x1100_0000, 49557, 1);
    inject(&mut world, 3001, 0x1100_0001, 49558, 1);

    // Rolento's hand-over: takes the contract, "gives" two items that do not
    // exist.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q} 30437-03.html", NPC_OID + 1)),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(8), "at cond 8");
    assert_eq!(item_count(&world, 3001, 49557), 0, "contract taken");
    assert_eq!(
        item_count(&world, 3001, 49559),
        0,
        "and the bag does not exist to be given"
    );

    // Gudz cannot see the items, so he has nothing to say — the dead end.
    drain(&mut rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{}_Quest {q}", NPC_OID + 2)),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    assert!(
        html.contains("not on a quest"),
        "Gudz has nothing to say, got: {html}"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(8), "and it stays at 8");
}
