//! The elven occupation quests: Q00406 Elven Knight, Q00407 Elven Scout,
//! Q00408 Elven Wizard and Q00409 Elven Oracle, with the guards that every
//! branch's page exists in the dist.

use super::super::*;

/// Q00406 hand-rolls its drop instead of calling `giveItemRandomly`, so it is
/// **not** multiplied by `RateQuestDrop`. With the rate at 3× a single kill
/// still yields exactly one topaz — reaching for the helper here would have
/// silently tripled the drop.
#[test]
fn quest_q00406_drop_ignores_the_quest_drop_rate() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1205, "Topaz Piece", true)]);
    let mut t = crate::data::npc_data::default_template(20035);
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);
    world.cfg.rates.rate_quest_drop = 3.0;
    add_test_npc(&mut world, NPC_OID, 30327, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18; // Elven Fighter
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    accept_q406(&mut world);
    drain(&mut rx);

    let mob = NPC_OID + 1;
    add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
    world.force_roll(0); // roll(100) → 0 < 70
    npc::npc_do_die(&mut world, mob, 3001);

    assert_eq!(
        item_count(&world, 3001, 1205),
        1,
        "one piece per kill regardless of RateQuestDrop"
    );
}

/// Drive Q00406 up to "quest started".
fn accept_q406(world: &mut World) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")),
    );
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight ACCEPT"
        )),
    );
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight 30327-06.htm"
        )),
    );
}

/// The whole Elven Knight chain: 20 topaz → Sorius' letter → Kluto → 20
/// emerald → Kluto's box → the brooch. The brooch is what
/// `ElfHumanFighterChange1` consumes, so this is the quest that makes that
/// transfer reachable at all.
#[test]
fn quest_q00406_full_chain_awards_the_brooch() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1202, "Sorius Letter", true),
            (1203, "Kluto Box", true),
            (1205, "Topaz", true),
            (1206, "Emerald", true),
            (1276, "Kluto Memo", true),
            (1204, "Elven Knight Brooch", false),
        ],
    );
    for id in [20035, 20782] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    let kluto = NPC_OID + 50;
    add_test_npc(&mut world, NPC_OID, 30327, "Folk", 5, 100, 0, 0); // Sorius
    add_test_npc(&mut world, kluto, 30317, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18;
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    accept_q406(&mut world);
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(1)
    );
    drain(&mut rx);

    // 20 topaz.
    for i in 0..20 {
        let mob = NPC_OID + 100 + i;
        add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1205), 20);
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(2),
        "20 topaz advances"
    );

    // Sorius hands over his letter.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")),
    );
    assert_eq!(item_count(&world, 3001, 1202), 1, "Sorius' letter");
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(3)
    );

    // Kluto swaps the letter for his memo.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{kluto}_Quest Q00406_PathOfTheElvenKnight 30317-02.html"
        )),
    );
    assert_eq!(item_count(&world, 3001, 1202), 0, "letter consumed");
    assert_eq!(item_count(&world, 3001, 1276), 1, "memo received");
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(4)
    );

    // 20 emerald from Ol Mahum Novices.
    for i in 0..20 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20782, "Monster", 20, 30, 0, 0);
        world.force_roll(0); // roll(100) → 0 < 50
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1206), 20);
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(5)
    );

    // Kluto builds the box, consuming everything.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest Q00406_PathOfTheElvenKnight")),
    );
    assert_eq!(item_count(&world, 3001, 1203), 1, "the box");
    assert_eq!(item_count(&world, 3001, 1205), 0, "topaz consumed");
    assert_eq!(item_count(&world, 3001, 1206), 0, "emerald consumed");
    assert_eq!(
        quest_cond(&world, 3001, "Q00406_PathOfTheElvenKnight"),
        Some(6)
    );
    drain(&mut rx);

    // Sorius pays out.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00406_PathOfTheElvenKnight")),
    );
    assert_eq!(item_count(&world, 3001, 1204), 1, "the Elven Knight Brooch");
    {
        // `exitQuest(false, ...)` — one-time, so the state stays COMPLETED
        // rather than being deleted (that would let it be repeated).
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(
            quests.0["Q00406_PathOfTheElvenKnight"].is_completed(),
            "one-time quest stays COMPLETED"
        );
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// Q00407's tag mechanic: `on_attack` stamps the mob with the attacker's
/// object id and `on_kill` pays only that player. A mob killed without being
/// attacked first drops nothing.
#[test]
fn quest_q00407_only_the_tagging_player_gets_the_letter() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1207, "Reisa Letter", true),
            (1208, "Torn 1", true),
            (1209, "Torn 2", true),
            (1210, "Torn 3", true),
            (1211, "Torn 4", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20053);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    let moretti = NPC_OID + 50;
    add_test_npc(&mut world, NPC_OID, 30328, "Folk", 5, 100, 0, 0); // Reoria
    add_test_npc(&mut world, moretti, 30337, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 18;
        p.base_class_id = 18;
        p.race = 1;
    }
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00407_PathOfTheElvenScout")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00407_PathOfTheElvenScout ACCEPT"
        )),
    );
    assert_eq!(
        item_count(&world, 3001, 1207),
        1,
        "Reisa's letter on accept"
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{moretti}_Quest Q00407_PathOfTheElvenScout 30337-03.html"
        )),
    );
    assert_eq!(
        quest_cond(&world, 3001, "Q00407_PathOfTheElvenScout"),
        Some(2)
    );
    assert_eq!(
        item_count(&world, 3001, 1207),
        0,
        "Moretti takes the letter"
    );
    drain(&mut rx);

    // Killed cold — never attacked, so never tagged.
    let untagged = NPC_OID + 100;
    add_test_npc(&mut world, untagged, 20053, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, untagged, 3001);
    assert_eq!(
        item_count(&world, 3001, 1208),
        0,
        "an untagged mob pays nothing"
    );

    // Attack first, then kill.
    let tagged = NPC_OID + 101;
    add_test_npc(&mut world, tagged, 20053, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, tagged, 3001, 10.0, false);
    npc::npc_do_die(&mut world, tagged, 3001);
    assert_eq!(
        item_count(&world, 3001, 1208),
        1,
        "the tagging player is paid"
    );
}

/// Both quests' pages exist. The extension is **mixed within one quest** —
/// `.htm` before accept, `.html` after — and Prias ships no `-03`, which Java
/// never names either.
#[test]
fn elven_path_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/"
    );
    let htm: [(&str, &str, &[&str]); 2] = [
        (
            "Q00406_PathOfTheElvenKnight",
            "30327",
            &["01", "02", "02a", "03", "04", "05", "06"],
        ),
        (
            "Q00407_PathOfTheElvenScout",
            "30328",
            &["01", "02", "02a", "03", "04", "05"],
        ),
    ];
    for (dir, npc, pages) in htm {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.htm");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {dir}/{npc}-{p}.htm"
            );
        }
    }
    let html: [(&str, &str, &[&str]); 6] = [
        (
            "Q00406_PathOfTheElvenKnight",
            "30327",
            &["07", "08", "09", "10", "11"],
        ),
        (
            "Q00406_PathOfTheElvenKnight",
            "30317",
            &["01", "02", "03", "04", "05", "06"],
        ),
        ("Q00407_PathOfTheElvenScout", "30328", &["06", "07", "08"]),
        ("Q00407_PathOfTheElvenScout", "30334", &["01"]),
        (
            "Q00407_PathOfTheElvenScout",
            "30337",
            &["01", "02", "03", "04", "05", "06", "07", "08", "09"],
        ),
        ("Q00407_PathOfTheElvenScout", "30426", &["01", "02", "04"]),
    ];
    for (dir, npc, pages) in html {
        for p in pages {
            let path = format!("{DIST}{dir}/{npc}-{p}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {dir}/{npc}-{p}.html"
            );
        }
    }
    // Prias' gap is real; the port must not invent a -03 to "complete" the run.
    let gap = format!("{DIST}Q00407_PathOfTheElvenScout/30426-03.html");
    assert!(
        !std::path::Path::new(&gap).exists(),
        "30426-03 genuinely does not ship"
    );
}

const Q409: &str = "Q00409_PathOfTheElvenOracle";

/// An Elven Mage with Q00409 accepted, plus Allana and Perrin placed.
fn q409_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1235, "Leaf of Oracle", false)];
    for id in [1231, 1232, 1233, 1234, 1236, 1275] {
        items.push((id, "Q409 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [27032, 27033, 27034, 27035] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30293, "Folk", 5, 100, 0, 0); // Manuel
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409} ACCEPT")),
    );
    assert_eq!(item_count(&world, 3001, 1231), 1, "the Crystal Medallion");
    drain(&mut rx);
    (world, rx)
}

/// `replay_1` conjures three lizardmen **and sets them on the player** — the
/// new `spawn_attacker` primitive has to wire both halves, so this asserts the
/// spawn and the aggro together.
#[test]
fn quest_q00409_allana_spawns_three_ambushers_that_aggro() {
    let (mut world, _rx) = q409_world();
    let allana = NPC_OID + 20;
    add_test_npc(&mut world, allana, 30424, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{allana}_Quest {Q409}")),
    );
    assert_eq!(
        quest_cond(&world, 3001, Q409),
        Some(2),
        "Allana starts the tale"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{allana}_Quest {Q409} replay_1")),
    );

    for id in [27032, 27033, 27034] {
        let spawned = npcs_of(&mut world, id);
        assert_eq!(spawned.len(), 1, "one {id} ambusher was conjured");
        assert!(
            world
                .objects
                .get_component::<AggroList>(&spawned[0])
                .is_some_and(|a| a.0.contains_key(&3001)),
            "ambusher {id} was set on the player"
        );
    }
}

/// The ambush tag pays only the first attacker — and unlike quests 401/403
/// there is **no weapon requirement**, so an unarmed first hit still qualifies.
#[test]
fn quest_q00409_ambush_pays_only_the_first_attacker() {
    // Killed cold: never attacked, so script value stays 0.
    let (mut world, _rx) = q409_world();
    let cold = NPC_OID + 100;
    add_test_npc(&mut world, cold, 27033, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, cold, 3001);
    assert_eq!(
        item_count(&world, 3001, 1234),
        0,
        "an untagged ambusher pays nothing"
    );

    // Attacked first (bare-handed) then killed: qualifies.
    let tagged = NPC_OID + 101;
    add_test_npc(&mut world, tagged, 27033, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, tagged, 3001, 10.0, false);
    npc::npc_do_die(&mut world, tagged, 3001);
    assert_eq!(
        item_count(&world, 3001, 1234),
        1,
        "no weapon gate here, unlike 401/403"
    );
    assert_eq!(quest_cond(&world, 3001, Q409), Some(3));
}

/// `memoState` and `cond` are separate axes and move independently: losing the
/// re-enactment rewinds `memoState` 2 → 1 while pushing `cond` to 8.
#[test]
fn quest_q00409_memo_state_rewinds_independently_of_cond() {
    let (mut world, mut rx) = q409_world();
    let allana = NPC_OID + 20;
    add_test_npc(&mut world, allana, 30424, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{allana}_Quest {Q409}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{allana}_Quest {Q409} replay_1")),
    );
    assert_eq!(
        quest_memo(&world, 3001, Q409),
        2,
        "the re-enactment is running"
    );
    drain(&mut rx);

    // Back to Manuel empty-handed: the tale is reset, the window jumps to 8.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q409}")),
    );

    assert_eq!(quest_memo(&world, 3001, Q409), 1, "memoState rewound");
    assert_eq!(
        quest_cond(&world, 3001, Q409),
        Some(8),
        "cond moved the other way"
    );
}

#[test]
fn elven_oracle_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00409_PathOfTheElvenOracle/"
    );
    for p in ["01", "02", "02a", "03", "04", "05"] {
        let path = format!("{DIST}30293-{p}.htm");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30293-{p}.htm"
        );
    }
    for p in ["06", "07", "08", "09"] {
        let path = format!("{DIST}30293-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30293-{p}.html"
        );
    }
    for p in ["01", "02", "03", "04", "05", "06", "07", "08", "09"] {
        let path = format!("{DIST}30424-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30424-{p}.html"
        );
    }
    for p in ["01", "02", "03", "04", "05", "06"] {
        let path = format!("{DIST}30428-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30428-{p}.html"
        );
    }
}

const Q408: &str = "Q00408_PathOfTheElvenWizard";

/// An Elven Mage with Q00408 accepted (Rossela at `NPC_OID`).
fn q408_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1230, "Eternity Diamond", false)];
    for id in [
        1218, 1219, 1220, 1221, 1222, 1223, 1224, 1225, 1226, 1229, 1272, 1273, 1274,
    ] {
        items.push((id, "Q408 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20019, 20047, 20466] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30414, "Folk", 5, 100, 0, 0); // Rossela
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
        p.race = 1;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408} ACCEPT")),
    );
    assert_eq!(item_count(&world, 3001, 1229), 1, "the Fertility Peridot");
    drain(&mut rx);
    (world, rx)
}

/// All three errands, then the diamond. Errands 1 and 2 swap the introduction
/// for a charm through a dialog event; **errand 3 has no such event** and
/// swaps on the talk itself.
#[test]
fn quest_q00408_three_errands_award_the_eternity_diamond() {
    let (mut world, mut rx) = q408_world();
    let (greenis, thalia, northwind) = (NPC_OID + 20, NPC_OID + 21, NPC_OID + 22);
    add_test_npc(&mut world, greenis, 30157, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, thalia, 30371, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, northwind, 30423, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;

    // (offer event, specialist oid, swap event, mob, material, need, gem)
    let errands: [(&str, i32, Option<&str>, i32, i32, i64, i32); 3] = [
        (
            "30414-10.html",
            greenis,
            Some("30157-02.html"),
            20466,
            1219,
            5,
            1220,
        ),
        (
            "30414-12.html",
            thalia,
            Some("30371-02.html"),
            20019,
            1223,
            5,
            1221,
        ),
        ("30414-16.html", northwind, None, 20047, 1225, 2, 1226),
    ];
    for (offer, specialist, swap, mob, material, need, gem) in errands {
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408} {offer}")),
        );
        match swap {
            // Greenis / Thalia: the swap needs the dialog event.
            Some(ev) => {
                handle_request_bypass_to_server(
                    &mut world,
                    1,
                    &bypass_body(&format!("npc_{specialist}_Quest {Q408} {ev}")),
                );
            }
            // Northwind: talking is enough.
            None => {
                handle_request_bypass_to_server(
                    &mut world,
                    1,
                    &bypass_body(&format!("npc_{specialist}_Quest {Q408}")),
                );
            }
        }
        for _ in 0..need {
            mob_oid += 1;
            add_test_npc(&mut world, mob_oid, mob, "Monster", 20, 30, 0, 0);
            world.force_roll(0); // inside every chance
            npc::npc_do_die(&mut world, mob_oid, 3001);
        }
        assert_eq!(
            item_count(&world, 3001, material),
            need,
            "collected material {material}"
        );
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{specialist}_Quest {Q408}")),
        );
        assert_eq!(item_count(&world, 3001, gem), 1, "gem {gem} awarded");
        assert_eq!(
            item_count(&world, 3001, material),
            0,
            "material handed over"
        );
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q408}")),
    );
    assert_eq!(item_count(&world, 3001, 1230), 1, "the Eternity Diamond");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q408].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// The charm is the drop gate: the same mob pays nothing before the
/// introduction has been swapped for it.
#[test]
fn quest_q00408_drops_need_the_charm() {
    let (mut world, _rx) = q408_world();
    let mob = NPC_OID + 400;
    add_test_npc(&mut world, mob, 20466, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1219), 0, "no charm, no Red Down");
}

/// Northwind ships only three pages, which is *why* his errand has no swap
/// event — there is no fourth page to route one to.
#[test]
fn elven_wizard_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00408_PathOfTheElvenWizard/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        let path = format!("{DIST}30414-{p}.htm");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30414-{p}.htm"
        );
    }
    for n in 7..=23 {
        let path = format!("{DIST}30414-{n:02}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30414-{n:02}.html"
        );
    }
    for npc in ["30157", "30371"] {
        for p in ["01", "02", "03", "04"] {
            let path = format!("{DIST}{npc}-{p}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{p}.html"
            );
        }
    }
    for p in ["01", "02", "03"] {
        let path = format!("{DIST}30423-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30423-{p}.html"
        );
    }
    assert!(
        !std::path::Path::new(&format!("{DIST}30423-04.html")).exists(),
        "Northwind has no fourth page — hence no swap event"
    );
}
