//! Q00264-Q00277 — the race village collection quests: Keen Claws, Pleas of
//! Pixies, Wrath of Verdure, Proof of Valor, Wrath of Ancestors, Skirmish
//! with the Werewolves, Dark Winged Spies, Totem of the Hestui and
//! Gatekeeper's Offering.

use super::super::*;

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
