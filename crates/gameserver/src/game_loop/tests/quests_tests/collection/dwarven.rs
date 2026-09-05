//! Q00292-Q00297 — the Dwarven Village collection quests: Brigands Sweep,
//! The Hidden Veins, Covert Business, Dreaming of the Skies, Tarantula's
//! Spider Silk and Gatekeeper's Favor.

use super::super::*;

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
