//! Q00400s — the occupation-change quests, the guards that every branch's
//! html page exists in the dist and that the dead branches stay dead, and the
//! quest-window behaviour those fixtures exercise.

use super::*;

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

/// Accept Q00401 and return the world to "quest started".
fn accept_q401(world: &mut World) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior")),
    );
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior ACCEPT"
        )),
    );
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00401_PathOfTheWarrior 30010-06.htm"
        )),
    );
}

/// Q00401's spider legs are gated purely on the weapon/solo tag — there is no
/// chance roll — so an unarmed kill pays nothing and a kill with Auron's
/// sharpened sword always pays.
#[test]
fn quest_q00401_spider_legs_require_the_quest_sword() {
    for (equip_sword, expected) in [(false, 0), (true, 1)] {
        let (mut world, mut db_rx, _link_rx) = quest_test_world();
        add_quest_items(
            &mut world,
            &[
                (1138, "Auron Letter", true),
                (1142, "Rusted Sword 3", true),
                (1144, "Spider Leg", true),
            ],
        );
        let mut t = crate::data::npc_data::default_template(20038);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
        add_test_npc(&mut world, NPC_OID, 30010, "Folk", 5, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.level = 19;
            p.class_id = 0; // Human Fighter
            p.base_class_id = 0;
        }
        if equip_sword {
            equip_weapon_row(&mut world, 3001, 1142);
        }
        drain_db(&mut db_rx);
        accept_q401(&mut world);
        drain(&mut rx);

        let spider = NPC_OID + 1;
        add_test_npc(&mut world, spider, 20038, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, spider, 3001, 10.0, false);
        npc::npc_do_die(&mut world, spider, 3001);

        assert_eq!(
            item_count(&world, 3001, 1144),
            expected,
            "sword equipped = {equip_sword}: the tag is the only gate"
        );
    }
}

/// Q00401's rusted-sword drop is `getRandom(10) < 4`. Forcing the roll to 4
/// must *not* drop — if it were read as `getRandom(100) < 40` it would.
#[test]
fn quest_q00401_rusted_sword_chance_is_out_of_ten() {
    for (forced, expected) in [(3, 1), (4, 0)] {
        let (mut world, mut db_rx, _link_rx) = quest_test_world();
        add_quest_items(
            &mut world,
            &[
                (1138, "Auron Letter", true),
                (1139, "Guild Mark", true),
                (1140, "Rusted Sword 1", true),
            ],
        );
        let mut t = crate::data::npc_data::default_template(20035);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
        add_test_npc(&mut world, NPC_OID, 30010, "Folk", 5, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.level = 19;
            p.class_id = 0;
            p.base_class_id = 0;
        }
        drain_db(&mut db_rx);
        accept_q401(&mut world);
        inventory::add_inventory_item(&mut world, 3001, 1139, 1); // guild mark
        drain(&mut rx);

        let mob = NPC_OID + 1;
        add_test_npc(&mut world, mob, 20035, "Monster", 20, 30, 0, 0);
        world.force_roll(forced);
        npc::npc_do_die(&mut world, mob, 3001);

        assert_eq!(
            item_count(&world, 3001, 1140),
            expected,
            "roll {forced} against `getRandom(10) < 4`"
        );
    }
}

/// Q00403's drop table is the same `ItemChanceHolder` type quest 406 uses with
/// `getRandom(100)`, but this quest rolls `getRandom(REQUIRED_ITEM_COUNT)` —
/// out of **10**. A forced roll can't tell the two apart (the forced value
/// ignores the bound), so this asserts the *rate*: Ruin Spartoi are chance 8,
/// i.e. 80%, and 40 kills reliably cap the 10-bone collection. Read as a
/// percentage it would be 8% and cap essentially never.
#[test]
fn quest_q00403_bone_chance_is_out_of_ten_not_a_hundred() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1180, "Bezique Letter", true),
            (1181, "Neti Bow", true),
            (1182, "Neti Dagger", true),
            (1183, "Spartois Bones", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(20054);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30379, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0;
        p.base_class_id = 0;
    }
    equip_weapon_row(&mut world, 3001, 1181); // Neti's bow satisfies the tag
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00403_PathOfTheRogue 30379-06.htm"
        )),
    );
    drain(&mut rx);

    for i in 0..40 {
        let mob = NPC_OID + 100 + i;
        add_test_npc(&mut world, mob, 20054, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, mob, 3001, 10.0, false);
        npc::npc_do_die(&mut world, mob, 3001);
    }

    assert_eq!(
        item_count(&world, 3001, 1183),
        10,
        "80% drop caps at 10 well within 40 kills"
    );
    assert_eq!(quest_cond(&world, 3001, "Q00403_PathOfTheRogue"), Some(3));
}

/// The Cat's Eye Bandit taunts its attacker on the first qualifying hit —
/// **to that player only** — and on death broadcasts a different line and
/// yields one of the four stolen goods.
#[test]
fn quest_q00403_cats_eye_bandit_taunts_then_drops_loot() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1180, "Bezique Letter", true),
            (1181, "Neti Bow", true),
            (1185, "Most Wanted", true),
            (1186, "Stolen Jewelry", true),
            (1187, "Stolen Tomes", true),
            (1188, "Stolen Ring", true),
            (1189, "Stolen Necklace", true),
        ],
    );
    let mut t = crate::data::npc_data::default_template(27038);
    t.type_name = "Monster".into();
    t.level = 20;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, NPC_OID, 30379, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0;
        p.base_class_id = 0;
    }
    equip_weapon_row(&mut world, 3001, 1181);
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest Q00403_PathOfTheRogue ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!(
            "npc_{NPC_OID}_Quest Q00403_PathOfTheRogue 30379-06.htm"
        )),
    );
    inventory::add_inventory_item(&mut world, 3001, 1185, 1); // the most-wanted list
    drain(&mut rx);

    let bandit = NPC_OID + 1;
    add_test_npc(&mut world, bandit, 27038, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, bandit, 3001, 10.0, false);
    let says: Vec<_> = drain(&mut rx)
        .into_iter()
        .filter(|p| p[0] == server_packets::opcodes::NPC_SAY)
        .collect();
    assert_eq!(says.len(), 1, "one taunt on the first qualifying hit");
    assert_eq!(
        i32::from_le_bytes(says[0][13..17].try_into().unwrap()),
        40306,
        "the taunt line"
    );

    // A second hit must not re-taunt (script value is no longer 0).
    combat::npc_receive_damage(&mut world, bandit, 3001, 10.0, false);
    assert!(
        !drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::NPC_SAY),
        "the taunt fires once"
    );

    world.force_roll(0); // pick STOLEN_JEWELRY
    npc::npc_do_die(&mut world, bandit, 3001);
    assert_eq!(item_count(&world, 3001, 1186), 1, "one of the stolen goods");
    assert!(
        drain(&mut rx).iter().any(|p| {
            p[0] == server_packets::opcodes::NPC_SAY
                && i32::from_le_bytes(p[13..17].try_into().unwrap()) == 40307
        }),
        "the death line, which is broadcast rather than whispered"
    );
}

/// Both quests' pages exist, with the same mixed `.htm`/`.html` split as the
/// elven pair.
#[test]
fn warrior_rogue_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/"
    );
    let htm: [(&str, &str, &[&str]); 2] = [
        (
            "Q00401_PathOfTheWarrior",
            "30010",
            &["01", "02", "02a", "03", "04", "05", "06"],
        ),
        (
            "Q00403_PathOfTheRogue",
            "30379",
            &["01", "02", "02a", "03", "04", "05", "06"],
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
    let html: [(&str, &str, &[&str]); 4] = [
        (
            "Q00401_PathOfTheWarrior",
            "30010",
            &["07", "08", "09", "10", "11", "12", "13"],
        ),
        (
            "Q00401_PathOfTheWarrior",
            "30253",
            &["01", "02", "03", "04", "05", "06"],
        ),
        (
            "Q00403_PathOfTheRogue",
            "30379",
            &["07", "08", "09", "10", "11"],
        ),
        (
            "Q00403_PathOfTheRogue",
            "30425",
            &["01", "02", "03", "04", "05", "06", "07", "08"],
        ),
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
}

const Q402: &str = "Q00402_PathOfTheHumanKnight";

/// Q00402 world: Vasper at NPC_OID, quest accepted, `coins` Coins of Lords
/// already in the bag.
fn q402_world_with_coins(coins: usize) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![
        (1271, "Squire's Mark", true),
        (1161, "Sword of Ritual", false),
    ];
    for id in [1162, 1163, 1164, 1165, 1166, 1167] {
        items.push((id, "Coin of Lords", true));
    }
    for id in [
        1168, 1169, 1170, 1171, 1172, 1173, 1174, 1175, 1176, 1177, 1178, 1179,
    ] {
        items.push((id, "Badge or trophy", true));
    }
    add_quest_items(&mut world, &items);
    add_test_npc(&mut world, NPC_OID, 30417, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 0; // Human Fighter
        p.base_class_id = 0;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-08.htm")),
    );
    for id in [1162, 1163, 1164, 1165, 1166, 1167].iter().take(coins) {
        inventory::add_inventory_item(&mut world, 3001, *id, 1);
    }
    drain(&mut rx);
    (world, rx)
}

/// With exactly three coins, talking to Vasper only *offers* — the sword comes
/// from the confirmation bypass.
#[test]
fn quest_q00402_three_coins_needs_the_confirm_button() {
    let (mut world, _rx) = q402_world_with_coins(3);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1161),
        0,
        "talking alone does not pay at 3 coins"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-13.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1161),
        1,
        "the confirm button awards the Sword of Ritual"
    );
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(
            quests.0[Q402].is_completed(),
            "one-time quest stays COMPLETED"
        );
    }
}

/// Six coins is the one path that completes **inside `onTalk`**, with no
/// confirmation step. Asymmetric, and deliberate — see the module header.
#[test]
fn quest_q00402_six_coins_completes_on_talk_without_a_confirm() {
    let (mut world, _rx) = q402_world_with_coins(6);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402}")),
    );

    assert_eq!(
        item_count(&world, 3001, 1161),
        1,
        "six coins pays out on the talk itself"
    );
    for id in [1162, 1163, 1164, 1165, 1166, 1167] {
        assert_eq!(item_count(&world, 3001, id), 0, "coin {id} consumed");
    }
    assert_eq!(
        item_count(&world, 3001, 1271),
        0,
        "the Squire's Mark is taken"
    );
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q402].is_completed());
    }
}

/// Each confirm button is bound to its own coin range, so a client replaying
/// the wrong one gets nothing.
#[test]
fn quest_q00402_confirm_buttons_check_their_coin_range() {
    // `-13` is the "exactly 3" button; with 4 coins it must refuse.
    let (mut world, _rx) = q402_world_with_coins(4);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-13.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1161),
        0,
        "the 3-coin button refuses 4 coins"
    );
    // `-14` is the "4 or 5" button; it must refuse a full set of 6.
    let (mut world6, _rx6) = q402_world_with_coins(6);
    handle_request_bypass_to_server(
        &mut world6,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q402} 30417-14.html")),
    );
    assert_eq!(
        item_count(&world6, 3001, 1161),
        0,
        "the 4-5 coin button refuses 6 coins"
    );
}

/// One officer's sub-quest end to end — and Bathis' Bugbear Necklace is one of
/// the two trophies with **no chance roll**, so ten kills is exactly ten
/// necklaces.
#[test]
fn quest_q00402_badge_to_coin_and_the_unrolled_drop() {
    let (mut world, mut rx) = q402_world_with_coins(0);
    let mut t = crate::data::npc_data::default_template(20775); // Bugbear Raider
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);
    let bathis = NPC_OID + 60;
    add_test_npc(&mut world, bathis, 30332, "Folk", 5, 100, 0, 0);

    // The badge hand-over.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bathis}_Quest {Q402} 30332-02.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1168),
        1,
        "Gludio Guard's 1st badge"
    );
    drain(&mut rx);

    // Ten kills, no roll forced — every one must pay.
    for i in 0..10 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20775, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 1169),
        10,
        "the necklace has no chance roll"
    );

    // Turn in for the coin.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{bathis}_Quest {Q402}")),
    );
    assert_eq!(item_count(&world, 3001, 1162), 1, "Coin of Lords 1");
    assert_eq!(item_count(&world, 3001, 1168), 0, "badge returned");
    assert_eq!(item_count(&world, 3001, 1169), 0, "necklaces handed over");
}

/// The drop is gated on holding that officer's badge — killing the right mob
/// without it pays nothing.
#[test]
fn quest_q00402_drops_need_the_matching_badge() {
    let (mut world, _rx) = q402_world_with_coins(0);
    let mut t = crate::data::npc_data::default_template(20775);
    t.type_name = "Monster".into();
    t.level = 20;
    world.data.npc_data.insert_for_test(t);

    let mob = NPC_OID + 200;
    add_test_npc(&mut world, mob, 20775, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob, 3001);

    assert_eq!(item_count(&world, 3001, 1169), 0, "no badge, no trophy");
}

/// Vasper's pages alternate extensions (`-06` html, `-07`/`-08` htm, `-09`+
/// html) rather than splitting on a prefix like the other Path quests, and
/// Raymond alone ships six pages.
#[test]
fn human_knight_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00402_PathOfTheHumanKnight/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "07", "08"] {
        let path = format!("{DIST}30417-{p}.htm");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30417-{p}.htm"
        );
    }
    for p in ["06", "09", "10", "11", "12", "13", "14", "15"] {
        let path = format!("{DIST}30417-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30417-{p}.html"
        );
    }
    // The alternation is real, not a tidy prefix split.
    assert!(!std::path::Path::new(&format!("{DIST}30417-07.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30417-06.htm")).exists());

    let officers: [(&str, &[&str]); 6] = [
        ("30332", &["01", "02", "03", "04", "05"]),
        ("30289", &["01", "02", "03", "04", "05", "06"]), // the six-page one
        ("30379", &["01", "02", "03", "04", "05"]),
        ("30037", &["01", "02", "03", "04", "05"]),
        ("30039", &["01", "02", "03", "04", "05"]),
        ("30031", &["01", "02", "03", "04", "05"]),
    ];
    for (npc, pages) in officers {
        for p in pages {
            let path = format!("{DIST}{npc}-{p}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{p}.html"
            );
        }
    }
    assert!(std::path::Path::new(&format!("{DIST}30653-01.html")).exists());
    // Only Raymond has a sixth page.
    for npc in ["30332", "30379", "30037", "30039", "30031"] {
        assert!(
            !std::path::Path::new(&format!("{DIST}{npc}-06.html")).exists(),
            "{npc} must not ship a -06"
        );
    }
}

const Q404: &str = "Q00404_PathOfTheHumanWizard";

const Q405: &str = "Q00405_PathOfTheCleric";

/// A mage at Parina with Q00404 accepted.
fn q404_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1292, "Bead of Season", false)];
    for id in 1280..=1291 {
        items.push((id, "Q404 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20021, 20359, 27030] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30391, "Folk", 5, 100, 0, 0); // Parina
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 10; // Mage
        p.base_class_id = 10;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404} ACCEPT")),
    );
    drain(&mut rx);
    (world, rx)
}

/// `QuestLink`'s simulated-`onTalk` filter: a finished Q404 must vanish from
/// Parina's quest window rather than be listed as a grey "(Done)" button.
///
/// Parina carries two talk quests (404 and 228, whose start NPC is Bard
/// Rukal), so the window took the chooser branch and rendered the completed
/// 404 as `<fstring>40403</fstring>` — a client string that does not exist
/// (`NpcStringId` ships 40401 and 40402 only), i.e. a blank button that
/// answered `noquest.htm` when clicked. Java probes `onTalk` first and drops
/// every quest with nothing to say here, leaving the plain no-quest html.
#[test]
fn quest_window_drops_a_finished_quest_with_nothing_to_say() {
    // Parina also carries `Q11006_FuturePeople` (its `addTalkId` lists her), so
    // the window is not empty any more — the assertion is about Q404 alone.
    // Q11006 shows Lector's *mage* page here because Java's `else if
    // (getClassId() == MAGE)` arm carries no NPC check; see that quest's file.
    let (mut world, mut rx) = q404_world();
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        quests.0.entry(Q404.to_string()).or_default().state = model::quest::state::COMPLETED;
    }
    drain(&mut rx);

    // The bare `Quest` bypass — the "Quest" link on Parina's chat window.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    assert!(
        !html.contains("Q00404") && !html.contains("40404"),
        "the finished Q404 contributes no button and no chooser row, got: {html}"
    );
}

/// The same probe must not *cost* the player anything. Java leaves
/// `AbstractScript.takeItems`/`giveItems` unguarded under simulation, so the
/// filter's `onTalk` probe strips all four trinkets and hands out the Bead
/// while the swallowed `exitQuest` leaves the quest started — and the real
/// `onTalk` that follows then finds an empty inventory and answers "go
/// collect them" (30391-05). The simulated context here is inert, so
/// clicking Parina's `Quest` link with the four trinkets in hand completes
/// the quest exactly as talking to her directly does.
#[test]
fn quest_window_probe_does_not_consume_the_turn_in_items() {
    let (mut world, mut rx) = q404_world();
    for id in TRINKETS_Q404 {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain(&mut rx);

    // The bare `Quest` bypass probes every quest at Parina, then talks.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body(&format!("npc_{NPC_OID}_Quest")));

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .unwrap_or_default();
    assert!(
        !html.contains("30391-05"),
        "the probe did not eat the trinkets, got: {html}"
    );
    assert_eq!(item_count(&world, 3001, 1292), 1, "Bead of Season awarded");
    for id in TRINKETS_Q404 {
        assert_eq!(item_count(&world, 3001, id), 0, "trinket {id} handed in");
    }
    let state = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .and_then(|q| q.0.get(Q404).map(|qs| qs.state));
    assert_eq!(
        state,
        Some(model::quest::state::COMPLETED),
        "quest finished"
    );
}

/// Flame Earring, Wind Bangle, Water Necklace, Earth Ring.
const TRINKETS_Q404: [i32; 4] = [1282, 1285, 1288, 1291];

/// The whole elemental chain: Fire → Wind → Water → Earth → the Bead of
/// Season. Exercises the branch table in order, including the conds.
#[test]
fn quest_q00404_full_elemental_chain_awards_the_bead() {
    let (mut world, mut rx) = q404_world();
    let (salamander, sylph, lizardman, undine, snake) = (
        NPC_OID + 11,
        NPC_OID + 12,
        NPC_OID + 10,
        NPC_OID + 13,
        NPC_OID + 9,
    );
    add_test_npc(&mut world, salamander, 30411, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, sylph, 30412, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, lizardman, 30410, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, undine, 30413, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, snake, 30409, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        world.force_roll(0); // always inside the chance
        npc::npc_do_die(world, mob_oid, 3001);
    };

    // Fire: map → key (Ratman Warrior) → earring.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{salamander}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1280), 1, "Map of Luster");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(2));
    kill(&mut world, 20359);
    assert_eq!(item_count(&world, 3001, 1281), 1, "Key of Flame");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{salamander}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1282), 1, "Flame Earring");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(4));

    // Wind: mirror → feather (from DIALOG, not a kill) → bangle.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sylph}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1283), 1, "Broken Bronze Mirror");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lizardman}_Quest {Q404} 30410-03.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1284),
        1,
        "Wind Feather comes from the lizardman's dialog"
    );
    assert_eq!(quest_cond(&world, 3001, Q404), Some(6));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sylph}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1285), 1, "Wind Bangle");

    // Water: diary → two pebbles → necklace.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{undine}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1286), 1, "Rama's Diary");
    kill(&mut world, 27030);
    assert_eq!(
        quest_cond(&world, 3001, Q404),
        Some(8),
        "one pebble is not enough"
    );
    kill(&mut world, 27030);
    assert_eq!(item_count(&world, 3001, 1287), 2, "two Sparkle Pebbles");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(9));
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{undine}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1288), 1, "Water Necklace");

    // Earth: coin → red soil → ring.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{snake}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1289), 1, "Rusty Coin");
    kill(&mut world, 20021);
    assert_eq!(item_count(&world, 3001, 1290), 1, "Red Soil");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{snake}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1291), 1, "Earth Ring");
    assert_eq!(quest_cond(&world, 3001, Q404), Some(13));
    drain(&mut rx);

    // Parina takes all four trinkets.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q404}")),
    );
    assert_eq!(item_count(&world, 3001, 1292), 1, "the Bead of Season");
    for id in [1282, 1285, 1288, 1291] {
        assert_eq!(item_count(&world, 3001, id), 0, "trinket {id} handed over");
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "the completion animation"
    );
}

/// Each spirit refuses until the previous element's trinket is in hand, so the
/// chain can't be entered out of order.
#[test]
fn quest_q00404_branches_are_gated_on_the_previous_trinket() {
    let (mut world, _rx) = q404_world();
    let sylph = NPC_OID + 12;
    add_test_npc(&mut world, sylph, 30412, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{sylph}_Quest {Q404}")),
    );

    assert_eq!(
        item_count(&world, 3001, 1283),
        0,
        "no mirror without the Flame Earring"
    );
}

/// A Q00405 world with the quest accepted (ACCEPT issues the first letter).
fn q405_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let mut items: Vec<(i32, &str, bool)> = vec![(1201, "Mark of Faith", false)];
    for id in 1191..=1200 {
        items.push((id, "Q405 item", true));
    }
    add_quest_items(&mut world, &items);
    for id in [20026, 20029] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30022, "Folk", 5, 100, 0, 0); // Zigaunt
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 10;
        p.base_class_id = 10;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405} ACCEPT")),
    );
    assert_eq!(
        item_count(&world, 3001, 1191),
        1,
        "ACCEPT issues the 1st Letter of Order"
    );
    drain(&mut rx);
    (world, rx)
}

/// Simplon hands over a **stack of three** where the other two book-givers
/// give one each, and cond 2 only lands once all three books are held.
#[test]
fn quest_q00405_simplon_gives_three_books() {
    let (mut world, _rx) = q405_world();
    let (vivyan, simplon, praga) = (NPC_OID + 30, NPC_OID + 31, NPC_OID + 32);
    add_test_npc(&mut world, vivyan, 30030, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, simplon, 30253, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, praga, 30333, "Folk", 5, 100, 0, 0);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{vivyan}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1194), 1, "Vivyan gives one");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{simplon}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1195), 3, "Simplon gives THREE");
    assert!(
        quest_cond(&world, 3001, Q405) != Some(2),
        "Praga's book is still missing"
    );

    // Praga: necklace on loan, pendant from a zombie (no chance roll), then
    // the book.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{praga}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1199), 1, "Necklace of Mother");
    let zombie = NPC_OID + 300;
    add_test_npc(&mut world, zombie, 20026, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, zombie, 3001);
    assert_eq!(
        item_count(&world, 3001, 1198),
        1,
        "the pendant drops with no roll"
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{praga}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1196), 1, "Book of Praga");
    assert_eq!(
        quest_cond(&world, 3001, Q405),
        Some(2),
        "all three books held"
    );

    // Zigaunt swaps the letters and takes ALL THREE of Simplon's books.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1195),
        0,
        "the whole stack of three is taken"
    );
    assert_eq!(item_count(&world, 3001, 1192), 1, "2nd Letter of Order");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(3));
}

/// The second errand: Lionel → Gallint → Lionel → Zigaunt for the Mark of
/// Faith.
#[test]
fn quest_q00405_courier_loop_awards_the_mark_of_faith() {
    let (mut world, mut rx) = q405_world();
    let (lionel, gallint) = (NPC_OID + 40, NPC_OID + 41);
    add_test_npc(&mut world, lionel, 30408, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, gallint, 30017, "Folk", 5, 100, 0, 0);
    // Jump the first errand by granting the 2nd letter directly. Zigaunt's
    // 1st-letter branch is only reached when the 2nd is absent, so leaving
    // the 1st in the bag doesn't change any path under test.
    inventory::add_inventory_item(&mut world, 3001, 1192, 1);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lionel}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1193), 1, "Lionel's Book");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(4));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gallint}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1197), 1, "Certificate of Gallint");
    assert_eq!(item_count(&world, 3001, 1193), 0, "book handed over");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(5));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{lionel}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1200), 1, "Lemoniell's Covenant");
    assert_eq!(quest_cond(&world, 3001, Q405), Some(6));
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q405}")),
    );
    assert_eq!(item_count(&world, 3001, 1201), 1, "the Mark of Faith");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q405].is_completed());
    }
}

/// Pages for both quests, including 404's uniform four-page scheme across all
/// four elemental spirits.
#[test]
fn wizard_cleric_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/"
    );
    for p in ["01", "02", "02a", "03", "04", "07"] {
        let path = format!("{DIST}Q00404_PathOfTheHumanWizard/30391-{p}.htm");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30391-{p}.htm"
        );
    }
    for p in ["05", "06"] {
        let path = format!("{DIST}Q00404_PathOfTheHumanWizard/30391-{p}.html");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30391-{p}.html"
        );
    }
    // All four spirits (and the lizardman) use the same 01..04 scheme.
    for npc in ["30409", "30410", "30411", "30412", "30413"] {
        for p in ["01", "02", "03", "04"] {
            let path = format!("{DIST}Q00404_PathOfTheHumanWizard/{npc}-{p}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{p}.html"
            );
        }
    }
    for p in ["01", "02", "02a", "03", "04", "05"] {
        let path = format!("{DIST}Q00405_PathOfTheCleric/30022-{p}.htm");
        assert!(
            std::path::Path::new(&path).exists(),
            "missing 30022-{p}.htm"
        );
    }
    let cleric: [(&str, &[&str]); 6] = [
        ("30022", &["06", "07", "08", "09"]),
        ("30017", &["01", "02"]),
        ("30030", &["01", "02"]),
        ("30253", &["01", "02"]),
        ("30333", &["01", "02", "03", "04"]),
        ("30408", &["01", "02", "03", "04", "05"]),
    ];
    for (npc, pages) in cleric {
        for p in pages {
            let path = format!("{DIST}Q00405_PathOfTheCleric/{npc}-{p}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{p}.html"
            );
        }
    }
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

const Q410: &str = "Q00410_PathOfThePalusKnight";

const Q411: &str = "Q00411_PathOfTheAssassin";

/// A Dark Fighter with the given quest accepted; `start_npc` sits at NPC_OID.
fn dark_elf_quest_world(
    quest: &str,
    start_npc: i32,
    accept_page: Option<&str>,
    items: &[i32],
    mobs: &[i32],
) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = items.iter().map(|id| (*id, "Q item", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in mobs {
        let mut t = crate::data::npc_data::default_template(*id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, start_npc, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 31; // Dark Fighter
        p.base_class_id = 31;
        p.race = 2;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} ACCEPT")),
    );
    if let Some(page) = accept_page {
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} {page}")),
        );
    }
    drain(&mut rx);
    (world, rx)
}

/// Q00410 end to end. Every drop is **unrolled**, so the exact kill counts
/// below are the whole requirement — no forced rolls anywhere in this test.
#[test]
fn quest_q00410_full_chain_awards_the_gaze_of_abyss() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q410,
        30329,
        Some("30329-06.htm"),
        &[1237, 1238, 1239, 1240, 1241, 1242, 1243, 1244],
        &[20038, 20043, 20049],
    );
    let kalinta = NPC_OID + 20;
    add_test_npc(&mut world, kalinta, 30422, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(world, mob_oid, 3001);
    };

    // 13 lycanthrope skulls — 13 kills, no rolls.
    for _ in 0..13 {
        kill(&mut world, 20049);
    }
    assert_eq!(
        item_count(&world, 3001, 1238),
        13,
        "unrolled: 13 kills = 13 skulls"
    );
    assert_eq!(quest_cond(&world, 3001, Q410), Some(2));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q410} 30329-10.html")),
    );
    assert_eq!(item_count(&world, 3001, 1239), 1, "Virgil's letter");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kalinta}_Quest {Q410} 30422-02.html")),
    );
    assert_eq!(item_count(&world, 3001, 1240), 1, "Morte talisman");

    // One carapace and five silks.
    kill(&mut world, 20038);
    for _ in 0..5 {
        kill(&mut world, 20043);
    }
    assert_eq!(item_count(&world, 3001, 1241), 1, "carapace");
    assert_eq!(item_count(&world, 3001, 1242), 5, "silks");
    assert_eq!(
        quest_cond(&world, 3001, Q410),
        Some(5),
        "collection complete"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kalinta}_Quest {Q410} 30422-06.html")),
    );
    assert_eq!(item_count(&world, 3001, 1243), 1, "the coffin");
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q410}")),
    );
    assert_eq!(item_count(&world, 3001, 1244), 1, "the Gaze of Abyss");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q410].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

/// Each drop is gated on the matching talisman: the right mob pays nothing
/// while the wrong stage is active.
#[test]
fn quest_q00410_drops_are_gated_on_the_talisman() {
    let (mut world, _rx) = dark_elf_quest_world(
        Q410,
        30329,
        Some("30329-06.htm"),
        &[1237, 1238, 1240, 1241, 1242],
        &[20038, 20043],
    );
    // Holding the Pallus talisman, not the Morte one: spiders pay nothing.
    let mob = NPC_OID + 400;
    add_test_npc(&mut world, mob, 20038, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(
        item_count(&world, 3001, 1241),
        0,
        "carapace needs the Morte talisman"
    );
}

/// Q00411's token chain, all the way to the Iron Heart.
#[test]
fn quest_q00411_token_chain_awards_the_iron_heart() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q411,
        30416,
        None, // ACCEPT starts the quest directly
        &[1245, 1246, 1247, 1248, 1250, 1251, 1252],
        &[20369, 27036],
    );
    assert_eq!(item_count(&world, 3001, 1245), 1, "Shilen's Call");
    let (leikan, arkenia) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, leikan, 30382, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(world, mob_oid, 3001);
    };

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{arkenia}_Quest {Q411} 30419-05.html")),
    );
    assert_eq!(item_count(&world, 3001, 1246), 1, "Arkenia's letter");
    assert_eq!(
        item_count(&world, 3001, 1245),
        0,
        "the call is consumed — one token at a time"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{leikan}_Quest {Q411} 30382-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1247), 1, "Leikan's note");
    assert_eq!(item_count(&world, 3001, 1246), 0);

    for _ in 0..10 {
        kill(&mut world, 20369);
    }
    assert_eq!(
        item_count(&world, 3001, 1248),
        10,
        "unrolled: 10 kills = 10 molars"
    );
    assert_eq!(quest_cond(&world, 3001, Q411), Some(4));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{leikan}_Quest {Q411}")),
    );
    assert_eq!(item_count(&world, 3001, 1248), 0, "molars handed over");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(5));

    kill(&mut world, 27036); // Calpico
    assert_eq!(item_count(&world, 3001, 1250), 1, "Shilen's Tears");
    assert_eq!(quest_cond(&world, 3001, Q411), Some(6));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{arkenia}_Quest {Q411}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1251),
        1,
        "Arkenia's recommendation"
    );
    assert_eq!(quest_cond(&world, 3001, Q411), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q411}")),
    );
    assert_eq!(item_count(&world, 3001, 1252), 1, "the Iron Heart");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q411].is_completed());
    }
}

/// Leikan answers on the same token but a different molar count — the one
/// place in 411 where an item outside the token chain changes the page.
#[test]
fn quest_q00411_leikan_page_tracks_the_molar_count() {
    let (mut world, mut rx) = dark_elf_quest_world(
        Q411,
        30416,
        None,
        &[1245, 1246, 1247, 1248, 1250, 1251, 1252],
        &[20369],
    );
    let (leikan, arkenia) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, leikan, 30382, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{arkenia}_Quest {Q411} 30419-05.html")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{leikan}_Quest {Q411} 30382-03.html")),
    );

    let dist = |page: &str| {
        let path = format!(
            "{}/../../dist/game/data/scripts/quests/Q00411_PathOfTheAssassin/{page}",
            env!("CARGO_MANIFEST_DIR")
        );
        crate::data::htm_cache::strip_htm(&std::fs::read_to_string(&path).expect("dist page"))
            .replace("%objectId%", &leikan.to_string())
    };

    // 0 molars.
    drain(&mut rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{leikan}_Quest {Q411}")),
    );
    assert_eq!(
        drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .unwrap_or_default(),
        dist("30382-05.html"),
        "note in hand, no molars"
    );

    // Partway.
    let mob = NPC_OID + 600;
    add_test_npc(&mut world, mob, 20369, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob, 3001);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{leikan}_Quest {Q411}")),
    );
    assert_eq!(
        drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .unwrap_or_default(),
        dist("30382-06.html"),
        "some molars but not ten"
    );
}

#[test]
fn dark_elf_path_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/"
    );
    // The two quests split .htm/.html at *different* points: 410's accept
    // page `30329-06` is `.htm`, while 411's `30416-06` is `.html` (its
    // accept page is `-05`). Asserted separately so the split can't be
    // assumed uniform across the tier.
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00410_PathOfThePalusKnight/30329-{p}.htm"))
                .exists(),
            "missing 30329-{p}.htm"
        );
    }
    for p in ["01", "02", "02a", "03", "04", "05"] {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-{p}.htm")).exists(),
            "missing 30416-{p}.htm"
        );
    }
    assert!(
        !std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-06.htm")).exists(),
        "411's -06 is .html, unlike 410's"
    );
    for n in 7..=12 {
        assert!(
            std::path::Path::new(&format!(
                "{DIST}Q00410_PathOfThePalusKnight/30329-{n:02}.html"
            ))
            .exists()
        );
    }
    for n in 1..=6 {
        assert!(
            std::path::Path::new(&format!(
                "{DIST}Q00410_PathOfThePalusKnight/30422-{n:02}.html"
            ))
            .exists()
        );
    }
    for n in 6..=11 {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30416-{n:02}.html"))
                .exists()
        );
    }
    for n in 1..=9 {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30382-{n:02}.html"))
                .exists()
        );
    }
    for n in 1..=11 {
        assert!(
            std::path::Path::new(&format!("{DIST}Q00411_PathOfTheAssassin/30419-{n:02}.html"))
                .exists()
        );
    }
}

const Q412: &str = "Q00412_PathOfTheDarkWizard";

const Q413: &str = "Q00413_PathOfTheShillienOracle";

/// A Dark Mage with the given quest accepted; start NPC at NPC_OID.
fn dark_mage_quest_world(
    quest: &str,
    start_npc: i32,
    accept_page: Option<&str>,
    items: &[i32],
    mobs: &[i32],
) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = items.iter().map(|id| (*id, "Q item", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in mobs {
        let mut t = crate::data::npc_data::default_template(*id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, start_npc, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 38; // Dark Mage
        p.base_class_id = 38;
        p.race = 2;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} ACCEPT")),
    );
    if let Some(page) = accept_page {
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} {page}")),
        );
    }
    drain(&mut rx);
    (world, rx)
}

/// Q00412's three seed errands, then the jewel. Arkenia hands her tool over on
/// the **talk**; the other two need a dialog event — the same asymmetry as
/// quest 408, exercised here in one loop.
#[test]
fn quest_q00412_three_seeds_award_the_jewel_of_darkness() {
    let (mut world, mut rx) = dark_mage_quest_world(
        Q412,
        30421,
        None,
        &[
            1253, 1254, 1255, 1256, 1257, 1259, 1260, 1261, 1277, 1278, 1279,
        ],
        &[20015, 20022, 20045, 20517, 20518],
    );
    assert_eq!(item_count(&world, 3001, 1254), 1, "the Seed of Despair");
    let (charkeren, annika, arkenia) = (NPC_OID + 20, NPC_OID + 21, NPC_OID + 22);
    add_test_npc(&mut world, charkeren, 30415, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, annika, 30418, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, arkenia, 30419, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;

    // (specialist, tool event, tool, mob, material, need, seed)
    let errands: [(i32, Option<&str>, i32, i32, i32, i64, i32); 3] = [
        (charkeren, Some("30415-03.html"), 1277, 20015, 1257, 3, 1253),
        (annika, Some("30418-02.html"), 1278, 20022, 1259, 2, 1255),
        (arkenia, None, 1279, 20045, 1260, 3, 1256),
    ];
    for (npc, tool_event, tool, mob, material, need, seed) in errands {
        match tool_event {
            Some(ev) => {
                handle_request_bypass_to_server(
                    &mut world,
                    1,
                    &bypass_body(&format!("npc_{npc}_Quest {Q412} {ev}")),
                );
            }
            None => {
                // Arkenia: the talk itself hands the Hub Scent over.
                handle_request_bypass_to_server(
                    &mut world,
                    1,
                    &bypass_body(&format!("npc_{npc}_Quest {Q412}")),
                );
            }
        }
        assert_eq!(item_count(&world, 3001, tool), 1, "tool {tool} received");
        for _ in 0..need {
            mob_oid += 1;
            add_test_npc(&mut world, mob_oid, mob, "Monster", 20, 30, 0, 0);
            world.force_roll(0); // `getRandom(2) == 0`
            npc::npc_do_die(&mut world, mob_oid, 3001);
        }
        assert_eq!(
            item_count(&world, 3001, material),
            need,
            "material {material}"
        );
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{npc}_Quest {Q412}")),
        );
        assert_eq!(item_count(&world, 3001, seed), 1, "seed {seed} grown");
        assert_eq!(item_count(&world, 3001, tool), 0, "tool spent");
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q412}")),
    );
    assert_eq!(item_count(&world, 3001, 1261), 1, "the Jewel of Darkness");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q412].is_completed());
    }
}

/// Q00412 rolls `getRandom(2) == 0` — **equality**. A forced roll of 1 must
/// not drop; read as `getRandom(2) < 2` every kill would pay.
#[test]
fn quest_q00412_drop_is_a_coin_flip_on_equality() {
    for (forced, expected) in [(0, 1), (1, 0)] {
        let (mut world, _rx) =
            dark_mage_quest_world(Q412, 30421, None, &[1253, 1254, 1257, 1277], &[20015]);
        let charkeren = NPC_OID + 20;
        add_test_npc(&mut world, charkeren, 30415, "Folk", 5, 100, 0, 0);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{charkeren}_Quest {Q412} 30415-03.html")),
        );
        let mob = NPC_OID + 400;
        add_test_npc(&mut world, mob, 20015, "Monster", 20, 30, 0, 0);
        world.force_roll(forced);
        npc::npc_do_die(&mut world, mob, 3001);
        assert_eq!(
            item_count(&world, 3001, 1257),
            expected,
            "roll {forced} against `== 0`"
        );
    }
}

/// Q00413's succubus kill is a **swap**: it spends a Blank Sheet to make a
/// Bloody Rune, so the two counts move in opposite directions and the stage
/// ends when the sheets run out.
#[test]
fn quest_q00413_succubus_swaps_sheets_for_runes() {
    let (mut world, _rx) = dark_mage_quest_world(
        Q413,
        30330,
        Some("30330-06.htm"),
        &[1262, 1263, 1264, 1265, 1266, 1267, 1268, 1269, 1270],
        &[20776],
    );
    let talbot = NPC_OID + 20;
    add_test_npc(&mut world, talbot, 30377, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{talbot}_Quest {Q413} 30377-02.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1263),
        5,
        "Talbot gives a stack of five sheets"
    );

    for i in 1..=5 {
        let mob = NPC_OID + 500 + i;
        add_test_npc(&mut world, mob, 20776, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(&mut world, mob, 3001);
        assert_eq!(item_count(&world, 3001, 1264), i as i64, "rune {i} made");
        assert_eq!(
            item_count(&world, 3001, 1263),
            5 - i as i64,
            "sheet {i} spent"
        );
    }
    assert_eq!(
        quest_cond(&world, 3001, Q413),
        Some(3),
        "sheets exhausted AND five runes"
    );

    // A sixth succubus has no sheet left to spend.
    let extra = NPC_OID + 600;
    add_test_npc(&mut world, extra, 20776, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, extra, 3001);
    assert_eq!(item_count(&world, 3001, 1264), 5, "no sheet, no rune");
}

/// Q00413 end to end.
#[test]
fn quest_q00413_full_chain_awards_the_orb_of_abyss() {
    let (mut world, mut rx) = dark_mage_quest_world(
        Q413,
        30330,
        Some("30330-06.htm"),
        &[1262, 1263, 1264, 1265, 1266, 1267, 1268, 1269, 1270],
        &[20776, 20457],
    );
    let (adonius, talbot) = (NPC_OID + 21, NPC_OID + 20);
    add_test_npc(&mut world, talbot, 30377, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, adonius, 30375, "Folk", 5, 100, 0, 0);
    let mut mob_oid = NPC_OID + 500;
    let mut kill = |world: &mut World, npc_id: i32| {
        mob_oid += 1;
        add_test_npc(world, mob_oid, npc_id, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(world, mob_oid, 3001);
    };

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{talbot}_Quest {Q413} 30377-02.html")),
    );
    for _ in 0..5 {
        kill(&mut world, 20776);
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{talbot}_Quest {Q413}")),
    );
    assert_eq!(item_count(&world, 3001, 1265), 1, "Garmiel's Book");
    assert_eq!(item_count(&world, 3001, 1266), 1, "Prayer of Adonius");
    assert_eq!(quest_cond(&world, 3001, Q413), Some(4));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{adonius}_Quest {Q413} 30375-04.html")),
    );
    assert_eq!(item_count(&world, 3001, 1267), 1, "Penitent's Mark");
    for _ in 0..10 {
        kill(&mut world, 20457);
    }
    assert_eq!(
        item_count(&world, 3001, 1268),
        10,
        "unrolled: 10 kills = 10 bones"
    );
    assert_eq!(quest_cond(&world, 3001, Q413), Some(6));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{adonius}_Quest {Q413}")),
    );
    assert_eq!(item_count(&world, 3001, 1269), 1, "Andariel's Book");
    assert_eq!(quest_cond(&world, 3001, Q413), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q413}")),
    );
    assert_eq!(item_count(&world, 3001, 1270), 1, "the Orb of Abyss");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q413].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

const Q414: &str = "Q00414_PathOfTheOrcRaider";

/// An Orc Fighter with Q00414 accepted (Karukia at NPC_OID).
fn q414_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = [1578, 1579, 1580, 1589, 1590, 1591, 1592, 8544]
        .iter()
        .map(|id| (*id, "Q414", true))
        .collect();
    add_quest_items(&mut world, &rows);
    for id in [20320, 27045, 27054] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30570, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
        p.race = 3;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414} ACCEPT")),
    );
    assert_eq!(item_count(&world, 3001, 1579), 1, "the Goblin Dwelling Map");
    drain(&mut rx);
    (world, rx)
}

/// Green blood is a rising **summon meter**, not loot: a roll above the held
/// count gains one, a roll at or below it wipes the stack and summons Kuruka.
#[test]
fn quest_q00414_green_blood_is_a_summon_meter() {
    let (mut world, _rx) = q414_world();

    // blood 0, forced roll 5 → `0 <= 5` → gain.
    let mob = NPC_OID + 100;
    add_test_npc(&mut world, mob, 20320, "Monster", 20, 30, 0, 0);
    world.force_roll(5);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 1, "gained a green blood");
    assert!(npcs_of(&mut world, 27045).is_empty(), "no summon yet");

    // blood 1, forced roll 0 → `1 <= 0` is false → wipe and summon.
    let mob2 = NPC_OID + 101;
    add_test_npc(&mut world, mob2, 20320, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, mob2, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 0, "the meter is wiped");
    let summoned = npcs_of(&mut world, 27045);
    assert_eq!(summoned.len(), 1, "Kuruka Ratman Leader was summoned");
    assert!(
        world
            .objects
            .get_component::<AggroList>(&summoned[0])
            .is_some_and(|a| a.0.contains_key(&3001)),
        "and set on the player"
    );
}

/// The tooth comes from Kuruka, never from the goblins — porting the blood as
/// a capped collection would make the quest unfinishable.
#[test]
fn quest_q00414_teeth_come_from_kuruka_and_reset_the_meter() {
    let (mut world, _rx) = q414_world();
    // Stock a little blood first.
    let mob = NPC_OID + 100;
    add_test_npc(&mut world, mob, 20320, "Monster", 20, 30, 0, 0);
    world.force_roll(19);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1578), 1);

    let kuruka = NPC_OID + 200;
    add_test_npc(&mut world, kuruka, 27045, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, kuruka, 3001);
    assert_eq!(
        item_count(&world, 3001, 1580),
        1,
        "the tooth comes from Kuruka"
    );
    assert_eq!(item_count(&world, 3001, 1578), 0, "and resets the meter");
}

/// Umbar Orcs spend one report per head (Zakan's first), 20% of the time.
#[test]
fn quest_q00414_umbar_heads_spend_the_reports() {
    let (mut world, _rx) = q414_world();
    for _ in 0..10 {
        inventory::add_inventory_item(&mut world, 3001, 1580, 1); // 10 teeth
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q414} 30570-07a.htm")),
    );
    assert_eq!(item_count(&world, 3001, 1589), 1, "Umbar's report");
    assert_eq!(item_count(&world, 3001, 1590), 1, "Zakan's report");
    assert_eq!(quest_cond(&world, 3001, Q414), Some(3));

    // A roll of 2 misses (`getRandom(10) < 2`).
    let miss = NPC_OID + 300;
    add_test_npc(&mut world, miss, 27054, "Monster", 20, 30, 0, 0);
    world.force_roll(2);
    npc::npc_do_die(&mut world, miss, 3001);
    assert_eq!(
        item_count(&world, 3001, 1591),
        0,
        "roll 2 is outside the 20%"
    );

    for i in 0..2 {
        let mob = NPC_OID + 310 + i;
        add_test_npc(&mut world, mob, 27054, "Monster", 20, 30, 0, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, 1591), 2, "two betrayer heads");
    assert_eq!(
        item_count(&world, 3001, 1590),
        0,
        "Zakan's report spent first"
    );
    assert_eq!(item_count(&world, 3001, 1589), 0, "then Umbar's");
    assert_eq!(quest_cond(&world, 3001, Q414), Some(4));

    // Kasman pays out.
    let kasman = NPC_OID + 20;
    add_test_npc(&mut world, kasman, 30501, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kasman}_Quest {Q414}")),
    );
    assert_eq!(item_count(&world, 3001, 1592), 1, "the Mark of Raider");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q414].is_completed());
    }
}

/// NPC 31978 ships five pages in this quest's directory but is registered
/// nowhere, and `30570-07.htm` offers only the `07a` button — so the whole
/// `07b` route is dead at both ends. Asserted so a future reader doesn't
/// "restore" one end without the other.
#[test]
fn orc_raider_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/"
    );
    // The orphaned pages really do ship.
    for p in ["01", "02", "03", "04", "05"] {
        assert!(
            std::path::Path::new(&format!("{DIST}31978-{p}.htm")).exists(),
            "31978-{p}.htm ships but is unreachable"
        );
    }
    // ...and nothing offers the 07b button.
    let fork = std::fs::read_to_string(format!("{DIST}30570-07.htm")).expect("the fork page");
    assert!(fork.contains("30570-07a.htm"), "07a is offered");
    assert!(
        !fork.contains("30570-07b.htm"),
        "07b is NOT offered — the route is unreachable"
    );
}

#[test]
fn orc_raider_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/"
    );
    for p in [
        "01", "02", "02a", "03", "04", "05", "06", "07", "07a", "07b", "08",
    ] {
        assert!(
            std::path::Path::new(&format!("{DIST}30570-{p}.htm")).exists(),
            "missing 30570-{p}.htm"
        );
    }
    for p in ["01", "02", "03"] {
        assert!(
            std::path::Path::new(&format!("{DIST}30501-{p}.htm")).exists(),
            "missing 30501-{p}.htm"
        );
    }
}

const Q415: &str = "Q00415_PathOfTheOrcMonk";

/// An Orc Fighter with Q00415 accepted (Gantaki at NPC_OID). `weapon` is put
/// straight into the RHand paperdoll when given.
fn q415_world(weapon: Option<i32>) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let ids = [
        1593, 1594, 1595, 1596, 1597, 1598, 1599, 1600, 1601, 1602, 1603, 1604, 1605, 1606, 1607,
        1608, 1609, 1610, 1611, 1612, 1613, 1614, 1615, 8545, 8546,
    ];
    let rows: Vec<(i32, &str, bool)> = ids.iter().map(|id| (*id, "Q415", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [
        20014, 20017, 20024, 20359, 20415, 20476, 20478, 20479, 21118,
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30587, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
        p.race = 3;
    }
    if let Some(w) = weapon {
        equip_weapon_row(&mut world, 3001, w);
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415} ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q415} 30587-06.htm")),
    );
    assert_eq!(item_count(&world, 3001, 1593), 1, "the pomegranate");
    drain(&mut rx);
    (world, rx)
}

/// The weapon gate is the **inverse** of quests 401/403: bare hands pass, a
/// sword fails, a fist weapon passes.
#[test]
fn quest_q00415_weapon_gate_wants_bare_hands_or_fists() {
    // (equipped weapon, is it a fist type, expected claws after one kill)
    let cases: [(Option<i32>, bool, i64); 3] = [
        (None, false, 1),       // bare-handed — the pass case
        (Some(7000), false, 0), // a sword — disqualifies
        (Some(7001), true, 1),  // a fist weapon — passes
    ];
    for (weapon, is_fist, expected) in cases {
        let (mut world, _rx) = q415_world(weapon);
        if let (Some(w), true) = (weapon, is_fist) {
            world
                .data
                .item_data
                .set_weapon_type_for_test(w, crate::data::item_data::kinds::WeaponType::Fist);
        }
        // Get pouch 1 from Rosheek.
        let rosheek = NPC_OID + 20;
        add_test_npc(&mut world, rosheek, 30590, "Folk", 5, 100, 0, 0);
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")),
        );
        assert_eq!(item_count(&world, 3001, 1594), 1, "first leather pouch");

        let bear = NPC_OID + 100;
        add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0, false);
        npc::npc_do_die(&mut world, bear, 3001);
        assert_eq!(
            item_count(&world, 3001, 1600),
            expected,
            "weapon {weapon:?} (fist={is_fist}): bare hands and fists pass, blades don't"
        );
    }
}

/// Each pouch takes **five** kills: four trophies, and the fifth converts.
#[test]
fn quest_q00415_pouch_takes_five_kills_not_four() {
    let (mut world, _rx) = q415_world(None);
    let rosheek = NPC_OID + 20;
    add_test_npc(&mut world, rosheek, 30590, "Folk", 5, 100, 0, 0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")),
    );

    for i in 1..=4 {
        let bear = NPC_OID + 100 + i;
        add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0, false);
        npc::npc_do_die(&mut world, bear, 3001);
        assert_eq!(item_count(&world, 3001, 1600), i as i64, "claw {i}");
        assert_eq!(item_count(&world, 3001, 1597), 0, "pouch not full yet");
    }
    // The fifth kill converts and consumes the four claws.
    let bear = NPC_OID + 200;
    add_test_npc(&mut world, bear, 20479, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, bear, 3001, 10.0, false);
    npc::npc_do_die(&mut world, bear, 3001);
    assert_eq!(
        item_count(&world, 3001, 1597),
        1,
        "the fifth kill fills the pouch"
    );
    assert_eq!(item_count(&world, 3001, 1600), 0, "claws consumed");
    assert_eq!(item_count(&world, 3001, 1594), 0, "empty pouch handed over");
    assert_eq!(quest_cond(&world, 3001, Q415), Some(3));

    // Rosheek swaps the full pouch for the next empty one.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{rosheek}_Quest {Q415}")),
    );
    assert_eq!(item_count(&world, 3001, 1595), 1, "second pouch");
    assert_eq!(quest_cond(&world, 3001, Q415), Some(4));
}

/// The fourth pouch spans four mobs at three trophies each and converts on the
/// twelfth kill.
#[test]
fn quest_q00415_fourth_pouch_converts_on_the_twelfth_kill() {
    let (mut world, _rx) = q415_world(None);
    inventory::add_inventory_item(&mut world, 3001, 1607, 1); // the 4th pouch
    let mut oid = NPC_OID + 300;
    let mobs = [(20014, 1612), (20017, 1609), (20024, 1611), (20359, 1610)];
    let mut killed = 0;
    for (mob, trophy) in mobs {
        for _ in 0..3 {
            oid += 1;
            add_test_npc(&mut world, oid, mob, "Monster", 20, 30, 0, 0);
            combat::npc_receive_damage(&mut world, oid, 3001, 10.0, false);
            npc::npc_do_die(&mut world, oid, 3001);
            killed += 1;
            if killed < 12 {
                assert_eq!(
                    item_count(&world, 3001, 1608),
                    0,
                    "not full at {killed} kills"
                );
            }
        }
        let _ = trophy;
    }
    assert_eq!(
        item_count(&world, 3001, 1608),
        1,
        "the twelfth kill fills the pouch"
    );
    for id in [1609, 1610, 1611, 1612] {
        assert_eq!(item_count(&world, 3001, id), 0, "trophy {id} consumed");
    }
    assert_eq!(quest_cond(&world, 3001, Q415), Some(12));
}

/// The alternate ending is dead at both ends: no page offers `09c`, and
/// neither 31979 nor 32056 is registered anywhere — 13 orphaned pages.
#[test]
fn orc_monk_alternate_ending_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/"
    );
    // The orphaned pages ship: 31979 x4, 32056 x9.
    for p in ["01", "02", "03", "04"] {
        assert!(
            std::path::Path::new(&format!("{DIST}31979-{p}.html")).exists(),
            "31979-{p} ships"
        );
    }
    for n in 1..=9 {
        assert!(
            std::path::Path::new(&format!("{DIST}32056-0{n}.html")).exists(),
            "32056-0{n} ships"
        );
    }
    // ...and the fork page offers only 09b.
    let fork = std::fs::read_to_string(format!("{DIST}30587-09a.html")).expect("the fork page");
    assert!(fork.contains("30587-09b.html"), "09b is offered");
    assert!(
        !fork.contains("30587-09c.html"),
        "09c is NOT offered — the route is unreachable"
    );
}

#[test]
fn orc_monk_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/"
    );
    for p in ["01", "02", "02a", "03", "04", "05", "06"] {
        assert!(
            std::path::Path::new(&format!("{DIST}30587-{p}.htm")).exists(),
            "missing 30587-{p}.htm"
        );
    }
    for p in ["07", "08", "09a", "09b", "09c", "10", "11"] {
        assert!(
            std::path::Path::new(&format!("{DIST}30587-{p}.html")).exists(),
            "missing 30587-{p}.html"
        );
    }
    for n in 1..=4 {
        assert!(
            std::path::Path::new(&format!("{DIST}30501-0{n}.html")).exists(),
            "missing 30501-0{n}"
        );
    }
    for n in 1..=9 {
        assert!(
            std::path::Path::new(&format!("{DIST}30590-0{n}.html")).exists(),
            "missing 30590-0{n}"
        );
    }
    for n in 1..=4 {
        assert!(
            std::path::Path::new(&format!("{DIST}30591-0{n}.html")).exists(),
            "missing 30591-0{n}"
        );
    }
}

const Q416: &str = "Q00416_PathOfTheOrcShaman";

/// An Orc Mage with Q00416 accepted (Tataru at NPC_OID).
fn q416_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1616..=1631).map(|id| (id, "Q416", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20038, 20043, 20335, 20415, 20478, 20479, 27056] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30585, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 49; // Orc Mage
        p.base_class_id = 49;
        p.race = 3;
    }
    drain_db(&mut db_rx);
    // Note the event name: START, not ACCEPT.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416} START")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416} 30585-07.htm")),
    );
    assert_eq!(item_count(&world, 3001, 1616), 1, "the fire charm");
    drain(&mut rx);
    (world, rx)
}

/// `ItemChanceHolder.count` is a **cond selector**, not a quantity: a grizzly
/// bear (gate cond 6) drops nothing at cond 1, and drops exactly one blood —
/// not six — once the cond matches.
#[test]
fn quest_q00416_holder_count_is_a_cond_gate_not_a_quantity() {
    let (mut world, _rx) = q416_world();
    // At cond 1 the grizzly is out of season.
    let early = NPC_OID + 100;
    add_test_npc(&mut world, early, 20335, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, early, 3001);
    assert_eq!(
        item_count(&world, 3001, 1625),
        0,
        "grizzly is gated to cond 6"
    );

    // Advance to cond 6 the short way: hand the player the flame charm and set
    // the cond, mirroring Umos' hand-over.
    inventory::add_inventory_item(&mut world, 3001, 1624, 1);
    set_quest_cond(&mut world, 3001, Q416, 6);
    let mob = NPC_OID + 101;
    add_test_npc(&mut world, mob, 20335, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(
        item_count(&world, 3001, 1625),
        1,
        "one blood per kill, not six"
    );
}

/// The first stage: three different mobs, one trophy each, cond 2 when all
/// three are in.
#[test]
fn quest_q00416_first_stage_needs_one_of_each_trophy() {
    let (mut world, _rx) = q416_world();
    let mut oid = NPC_OID + 200;
    for (mob, item) in [(20415, 1619), (20478, 1618)] {
        oid += 1;
        add_test_npc(&mut world, oid, mob, "Monster", 20, 30, 0, 0);
        npc::npc_do_die(&mut world, oid, 3001);
        assert_eq!(item_count(&world, 3001, item), 1, "trophy {item}");
        assert_eq!(quest_cond(&world, 3001, Q416), Some(1), "still collecting");
    }
    oid += 1;
    add_test_npc(&mut world, oid, 20479, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(
        quest_cond(&world, 3001, Q416),
        Some(2),
        "all three trophies"
    );

    // Tataru swaps them for the mask and the second egg.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q416}")),
    );
    assert_eq!(item_count(&world, 3001, 1620), 1, "Hestui mask");
    assert_eq!(item_count(&world, 3001, 1621), 1, "second fiery egg");
    assert_eq!(item_count(&world, 3001, 1616), 0, "fire charm consumed");
    assert_eq!(quest_cond(&world, 3001, Q416), Some(3));
}

/// The parasite meter escalates and, unlike quest 414's Kuruka, the conjured
/// Durka Spirit is **not** set on the player.
#[test]
fn quest_q00416_durka_meter_summons_without_aggro() {
    let (mut world, _rx) = q416_world();
    inventory::add_inventory_item(&mut world, 3001, 1627, 1); // spirit net
    set_quest_cond(&mut world, 3001, Q416, 9);

    // Below the threshold the kill just pays a parasite.
    let mob = NPC_OID + 300;
    add_test_npc(&mut world, mob, 20038, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob, 3001);
    assert_eq!(item_count(&world, 3001, 1629), 1, "a parasite");
    assert!(npcs_of(&mut world, 27056).is_empty(), "no spirit yet");

    // Eight parasites makes the summon certain.
    for _ in 0..7 {
        inventory::add_inventory_item(&mut world, 3001, 1629, 1);
    }
    assert_eq!(item_count(&world, 3001, 1629), 8);
    let mob2 = NPC_OID + 301;
    add_test_npc(&mut world, mob2, 20043, "Monster", 20, 30, 0, 0);
    npc::npc_do_die(&mut world, mob2, 3001);
    assert_eq!(item_count(&world, 3001, 1629), 0, "the meter is wiped");
    let spirits = npcs_of(&mut world, 27056);
    assert_eq!(spirits.len(), 1, "a Durka Spirit was conjured");
    assert!(
        world
            .objects
            .get_component::<AggroList>(&spirits[0])
            .is_none_or(|a| !a.0.contains_key(&3001)),
        "and is NOT set on the player, unlike quest 414's Kuruka"
    );

    // Killing it yields the bound spirit and consumes the net.
    npc::npc_do_die(&mut world, spirits[0], 3001);
    assert_eq!(item_count(&world, 3001, 1628), 1, "bound Durka spirit");
    assert_eq!(item_count(&world, 3001, 1627), 0, "the net is spent");
}

/// The tail: bound spirit → totem spirit blood → the Mask of Medium.
#[test]
fn quest_q00416_finish_awards_the_mask_of_medium() {
    let (mut world, mut rx) = q416_world();
    let (umos, duda) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, umos, 30502, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, duda, 30593, "Folk", 5, 100, 0, 0);
    inventory::add_inventory_item(&mut world, 3001, 1628, 1); // bound spirit
    set_quest_cond(&mut world, 3001, Q416, 9);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{duda}_Quest {Q416}")),
    );
    assert_eq!(item_count(&world, 3001, 1630), 1, "totem spirit blood");
    assert_eq!(
        quest_cond(&world, 3001, Q416),
        Some(11),
        "Java jumps 9 -> 11"
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{umos}_Quest {Q416} 30502-07.html")),
    );
    assert_eq!(item_count(&world, 3001, 1631), 1, "the Mask of Medium");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q416].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

/// The `memoState` 100+ branch is dead at both ends — third Orc quest running.
#[test]
fn orc_shaman_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/"
    );
    // The orphaned NPCs really do ship pages.
    for npc in ["31979", "32057", "32090"] {
        let any = (1..=9).any(|n| std::path::Path::new(&format!("{DIST}{npc}-0{n}.html")).exists());
        assert!(any, "{npc} ships pages but is registered nowhere");
    }
    // The only entry to memoState 100 is 30585-14, which nothing offers.
    assert!(
        std::path::Path::new(&format!("{DIST}30585-14.html")).exists(),
        "30585-14 ships"
    );
    for page in ["30585-11.html", "30585-12.html", "30585-13.html"] {
        let body = std::fs::read_to_string(format!("{DIST}{page}")).expect(page);
        assert!(
            !body.contains("30585-14"),
            "{page} must not offer the dead entry"
        );
    }
}

#[test]
fn orc_shaman_quest_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/"
    );
    for p in ["01", "02", "03", "04", "05", "06", "07"] {
        assert!(
            std::path::Path::new(&format!("{DIST}30585-{p}.htm")).exists(),
            "missing 30585-{p}.htm"
        );
    }
    for n in 8..=16 {
        assert!(
            std::path::Path::new(&format!("{DIST}30585-{n:02}.html")).exists(),
            "missing 30585-{n:02}.html"
        );
    }
    for n in 1..=7 {
        assert!(
            std::path::Path::new(&format!("{DIST}30502-0{n}.html")).exists(),
            "missing 30502-0{n}"
        );
    }
    for n in 1..=5 {
        assert!(
            std::path::Path::new(&format!("{DIST}30592-0{n}.html")).exists(),
            "missing 30592-0{n}"
        );
    }
    for n in 1..=6 {
        assert!(
            std::path::Path::new(&format!("{DIST}30593-0{n}.html")).exists(),
            "missing 30593-0{n}"
        );
    }
}

const Q418: &str = "Q00418_PathOfTheArtisan";

/// A Dwarven Fighter with Q00418 accepted (Silvera at NPC_OID).
fn q418_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1632..=1641).map(|id| (id, "Q418", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20017, 20389, 20390] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30527, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} ACCEPT")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-06.htm")),
    );
    assert_eq!(item_count(&world, 3001, 1632), 1, "Silvery's ring");
    drain(&mut rx);
    (world, rx)
}

/// The leader-tooth roll is lopsided: below 5 it pays **only** when one tooth
/// is already held, so the first tooth comes at 50% and the second at 100%.
#[test]
fn quest_q00418_leader_tooth_roll_has_a_hole_at_zero() {
    let (mut world, _rx) = q418_world();
    let mut oid = NPC_OID + 100;

    // Roll 0 with zero teeth: the `< 5` branch does nothing at all.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(
        item_count(&world, 3001, 1637),
        0,
        "roll<5 at zero teeth pays nothing"
    );

    // Roll 5 with zero teeth: the `else` branch always pays.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(5);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(item_count(&world, 3001, 1637), 1, "roll>=5 always pays");

    // Roll 0 with one tooth: now the `< 5` branch does pay.
    oid += 1;
    add_test_npc(&mut world, oid, 20390, "Monster", 20, 30, 0, 0);
    world.force_roll(0);
    npc::npc_do_die(&mut world, oid, 3001);
    assert_eq!(
        item_count(&world, 3001, 1637),
        2,
        "roll<5 pays the second tooth"
    );
}

/// Ratman teeth cap at 10 on a 70% roll; a roll of 7 misses.
#[test]
fn quest_q00418_ratman_teeth_roll_is_seventy_percent() {
    let (mut world, _rx) = q418_world();
    let miss = NPC_OID + 200;
    add_test_npc(&mut world, miss, 20389, "Monster", 20, 30, 0, 0);
    world.force_roll(7);
    npc::npc_do_die(&mut world, miss, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 0, "roll 7 is outside `< 7`");

    let hit = NPC_OID + 201;
    add_test_npc(&mut world, hit, 20389, "Monster", 20, 30, 0, 0);
    world.force_roll(6);
    npc::npc_do_die(&mut world, hit, 3001);
    assert_eq!(item_count(&world, 3001, 1636), 1, "roll 6 pays");
}

/// The whole chain: teeth → 1st pass → Kluto's letter → Pinter's footprint →
/// the stolen box → 2nd pass → the Final Pass Certificate.
#[test]
fn quest_q00418_full_chain_awards_the_final_pass() {
    let (mut world, mut rx) = q418_world();
    let (pinter, kluto) = (NPC_OID + 20, NPC_OID + 21);
    add_test_npc(&mut world, pinter, 30298, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, kluto, 30317, "Folk", 5, 100, 0, 0);
    for _ in 0..10 {
        inventory::add_inventory_item(&mut world, 3001, 1636, 1);
    }
    for _ in 0..2 {
        inventory::add_inventory_item(&mut world, 3001, 1637, 1);
    }

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q418} 30527-08b.html")),
    );
    assert_eq!(item_count(&world, 3001, 1633), 1, "first pass certificate");
    assert_eq!(item_count(&world, 3001, 1636), 0, "teeth handed over");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(3));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-04.html")),
    );
    assert_eq!(item_count(&world, 3001, 1638), 1, "Kluto's letter");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(4));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1639), 1, "footprint of thief");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(5));

    let orc = NPC_OID + 300;
    add_test_npc(&mut world, orc, 20017, "Monster", 20, 30, 0, 0);
    world.force_roll(0); // `getRandom(10) < 2`
    npc::npc_do_die(&mut world, orc, 3001);
    assert_eq!(item_count(&world, 3001, 1640), 1, "the stolen secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(6));

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{pinter}_Quest {Q418} 30298-06.html")),
    );
    assert_eq!(item_count(&world, 3001, 1634), 1, "second pass certificate");
    assert_eq!(item_count(&world, 3001, 1641), 1, "the secret box");
    assert_eq!(quest_cond(&world, 3001, Q418), Some(7));
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kluto}_Quest {Q418} 30317-10.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 1635),
        1,
        "the Final Pass Certificate"
    );
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q418].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

/// Fourth quest running with a route dead at both ends.
#[test]
fn artisan_dead_branch_is_dead_at_both_ends() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/quests/Q00418_PathOfTheArtisan/"
    );
    for npc in ["31956", "31963", "32052"] {
        let any = (1..=9).any(|n| std::path::Path::new(&format!("{DIST}{npc}-0{n}.html")).exists());
        assert!(any, "{npc} ships pages but is registered nowhere");
    }
    // Only 08b is offered; 08c (the memoState 10 entry) is not.
    // Pages only — the .java source naturally names `08c` as a case label,
    // which is exactly the handler we are proving is unreachable.
    let mut offers_08b = false;
    for entry in std::fs::read_dir(DIST).expect("quest dir") {
        let path = entry.expect("entry").path();
        let is_page = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "htm" || e == "html");
        if !is_page {
            continue;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            !body.contains("30527-08c"),
            "no page may offer the dead 08c entry"
        );
        offers_08b |= body.contains("30527-08b");
    }
    assert!(offers_08b, "08b is the live route and is offered");
}

const Q417: &str = "Q00417_PathOfTheScavenger";

/// A Dwarven Fighter with Q00417 accepted (Pipi at NPC_OID).
fn q417_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    let rows: Vec<(i32, &str, bool)> = (1642..=1657).map(|id| (id, "Q417", true)).collect();
    add_quest_items(&mut world, &rows);
    for id in [20403, 20508, 20777, 27058] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 20;
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    add_test_npc(&mut world, NPC_OID, 30524, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.class_id = 53; // Dwarven Fighter
        p.base_class_id = 53;
        p.race = 4;
    }
    drain_db(&mut db_rx);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {Q417} ACCEPT")),
    );
    assert_eq!(item_count(&world, 3001, 1643), 1, "Pipi's letter");
    drain(&mut rx);
    (world, rx)
}

/// Mark `npc_oid` as spoiled by `player`, the way the Spoil effect would.
fn mark_spoiled(world: &mut World, npc_oid: i32, player: i32) {
    if let Some(n) = world.objects.get_component_mut::<model::npc::Npc>(&npc_oid) {
        n.spoiler_object_id = player;
    }
}

/// The payout is gated on the corpse being **spoiled** — the Scavenger's own
/// mechanic. An unspoiled Honey Bear pays nothing.
#[test]
fn quest_q00417_payout_requires_a_spoiled_corpse() {
    for (spoil, expected) in [(false, 0), (true, 1)] {
        let (mut world, _rx) = q417_world();
        inventory::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture
        let bear = NPC_OID + 100;
        add_test_npc(&mut world, bear, 27058, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, bear, 3001, 10.0, false);
        if spoil {
            // Spoiled by someone else, so the attack-time disqualifier
            // (spoiler == attacker) does not fire.
            mark_spoiled(&mut world, bear, 9999);
        }
        npc::npc_do_die(&mut world, bear, 3001);
        assert_eq!(
            item_count(&world, 3001, 1655),
            expected,
            "spoiled={spoil}: honey only drops off a spoiled corpse"
        );
    }
}

/// `giveItemRandomly`'s chance is a 0..1 fraction and this quest passes **50**,
/// so every qualifying kill drops. No forced roll is used: if the port had
/// "corrected" it to 0.5 this would be flaky, and at 0.5 with the RNG it would
/// fail about half the time.
#[test]
fn quest_q00417_drop_chance_fifty_means_always() {
    let (mut world, _rx) = q417_world();
    inventory::add_inventory_item(&mut world, 3001, 1654, 1); // tarantula picture
    for i in 0..6 {
        let mob = NPC_OID + 200 + i;
        add_test_npc(&mut world, mob, 20403, "Monster", 20, 30, 0, 0);
        combat::npc_receive_damage(&mut world, mob, 3001, 10.0, false);
        mark_spoiled(&mut world, mob, 9999);
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(
        item_count(&world, 3001, 1656),
        6,
        "six kills, six beads — chance 50 is not 50%"
    );
}

/// The Honey Bear summon meter escalates at `20 * flag` percent and resets on
/// success.
#[test]
fn quest_q00417_honey_bear_summon_meter_escalates() {
    let (mut world, _rx) = q417_world();
    inventory::add_inventory_item(&mut world, 3001, 1653, 1); // bear picture

    // First kill: flag is 0, so no roll happens at all — it just rises.
    let b1 = NPC_OID + 300;
    add_test_npc(&mut world, b1, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b1, 3001, 10.0, false);
    npc::npc_do_die(&mut world, b1, 3001);
    assert!(
        npcs_of(&mut world, 27058).is_empty(),
        "flag 0 never summons"
    );

    // Second kill with the roll inside `20 * 1`: the bear appears.
    let b2 = NPC_OID + 301;
    add_test_npc(&mut world, b2, 20777, "Monster", 20, 30, 0, 0);
    combat::npc_receive_damage(&mut world, b2, 3001, 10.0, false);
    world.force_roll(5); // 5 < 20
    npc::npc_do_die(&mut world, b2, 3001);
    assert_eq!(
        npcs_of(&mut world, 27058).len(),
        1,
        "the Honey Bear was summoned"
    );
}

/// The delivery round-trip bumps the **tens** digit of `memoStateEx(1)`, and
/// the second hand-in promotes to cond 3.
#[test]
fn quest_q00417_deliveries_bump_the_tens_digit() {
    let (mut world, _rx) = q417_world();
    let shari = NPC_OID + 20;
    add_test_npc(&mut world, shari, 30517, "Folk", 5, 100, 0, 0);

    inventory::add_inventory_item(&mut world, 3001, 1648, 1); // Shari's axe
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(item_count(&world, 3001, 1651), 1, "Shari's pay");
    assert_eq!(
        quest_memo_ex(&world, 3001, Q417, 1),
        10,
        "tens digit bumped"
    );
    assert!(
        quest_cond(&world, 3001, Q417) != Some(3),
        "not promoted on the first"
    );

    inventory::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(quest_memo_ex(&world, 3001, Q417, 1), 20);
    inventory::add_inventory_item(&mut world, 3001, 1648, 1);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{shari}_Quest {Q417}")),
    );
    assert_eq!(
        quest_cond(&world, 3001, Q417),
        Some(3),
        "the third hand-in promotes"
    );
}

/// Torai hands over the undies and **deletes himself**; Raut then pays the
/// Ring of Raven.
#[test]
fn quest_q00417_torai_vanishes_and_raut_pays_the_ring() {
    let (mut world, mut rx) = q417_world();
    let (torai, raut) = (NPC_OID + 30, NPC_OID + 31);
    add_test_npc(&mut world, torai, 30557, "Folk", 5, 100, 0, 0);
    add_test_npc(&mut world, raut, 30316, "Folk", 5, 100, 0, 0);
    inventory::add_inventory_item(&mut world, 3001, 1644, 1); // teleport scroll

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{torai}_Quest {Q417} 30557-03.html")),
    );
    assert_eq!(item_count(&world, 3001, 1645), 1, "succubus undies");
    assert_eq!(quest_cond(&world, 3001, Q417), Some(11));
    assert!(
        world
            .objects
            .get_component::<model::npc::Npc>(&torai)
            .is_none(),
        "Torai deleted himself"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{raut}_Quest {Q417}")),
    );
    assert_eq!(item_count(&world, 3001, 1642), 1, "the Ring of Raven");
    {
        let quests = world
            .objects
            .get_component::<model::components::social::Quests>(&3001)
            .unwrap();
        assert!(quests.0[Q417].is_completed());
    }
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
    );
}

fn quest_memo_ex(world: &World, player: i32, quest: &str, slot: i32) -> i32 {
    world
        .objects
        .get_component::<model::components::social::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .and_then(|qs| qs.vars.get(&format!("memoStateEx{slot}")))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

/// Little Wing (420): the hatchling-pet quest, normal (plain Fairy Stone) path
/// end to end — forge the stone, brew Monkshood Juice, take Exarion's scale,
/// farm 20 eggs, hatch, and redeem a Dragonflute. Plus the Deluxe stone's
/// `onAttack` shatter risk as a separate check.
#[test]
fn quest_q00420_little_wing() {
    const COOPER: i32 = 30829;
    const CRONOS: i32 = 30610;
    const MARIA: i32 = 30608;
    const BYRON: i32 = 30711;
    const MIMYU: i32 = 30747;
    const EXARION: i32 = 30748;
    // Materials
    const COAL: i32 = 1870;
    const CHARCOAL: i32 = 1871;
    const SILVER_NUGGET: i32 = 1873;
    const GEMSTONE_D: i32 = 2130;
    const TOAD_SKIN: i32 = 3820;
    // Quest items
    const FAIRY_STONE_LIST: i32 = 3818;
    const FAIRY_STONE: i32 = 3816;
    const DELUXE_FAIRY_STONE: i32 = 3817;
    const MONKSHOOD_JUICE: i32 = 3821;
    const EXARION_SCALE: i32 = 3822;
    const EXARION_EGG: i32 = 3823;
    const DRAGONFLUTE_OF_WIND: i32 = 3500;
    // Monsters
    const LETO_WARRIOR: i32 = 20580;
    const FLINE: i32 = 20589; // a Deluxe-stone breaker

    let (mut world, _db, _l) = quest_test_world();
    let ids = [
        COAL,
        CHARCOAL,
        SILVER_NUGGET,
        GEMSTONE_D,
        TOAD_SKIN,
        FAIRY_STONE_LIST,
        FAIRY_STONE,
        DELUXE_FAIRY_STONE,
        MONKSHOOD_JUICE,
        EXARION_SCALE,
        EXARION_EGG,
        3499, // FAIRY_DUST
    ];
    let mut items: Vec<(i32, &str, bool)> = ids.iter().map(|&i| (i, "q", true)).collect();
    items.push((DRAGONFLUTE_OF_WIND, "flute", false));
    add_quest_items(&mut world, &items);
    for id in [LETO_WARRIOR, FLINE] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    let cooper = NPC_OID;
    let cronos = NPC_OID + 1;
    let maria = NPC_OID + 2;
    let byron = NPC_OID + 3;
    let mimyu = NPC_OID + 4;
    let exarion = NPC_OID + 5;
    for (oid, npc) in [
        (cooper, COOPER),
        (cronos, CRONOS),
        (maria, MARIA),
        (byron, BYRON),
        (mimyu, MIMYU),
        (exarion, EXARION),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 40;
    let q = "Q00420_LittleWing";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);

    // Accept → Cronos → pick the plain Fairy Stone (cond 2).
    talk(&mut world, cooper);
    ev(&mut world, cooper, "30829-02.htm");
    assert_eq!(cond(&world), Some(1));
    ev(&mut world, cronos, "30610-05.html");
    assert_eq!(cond(&world), Some(2), "plain stone chosen");
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE_LIST),
        1,
        "stone list given"
    );

    // Gather the materials, then Maria forges the Fairy Stone (cond 3).
    inject(&mut world, 3001, 0x0420_1000, COAL, 10);
    inject(&mut world, 3001, 0x0420_2000, CHARCOAL, 10);
    inject(&mut world, 3001, 0x0420_3000, GEMSTONE_D, 1);
    inject(&mut world, 3001, 0x0420_4000, SILVER_NUGGET, 3);
    inject(&mut world, 3001, 0x0420_5000, TOAD_SKIN, 10);
    ev(&mut world, maria, "30608-03.html");
    assert_eq!(cond(&world), Some(3));
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE),
        1,
        "Fairy Stone forged"
    );
    assert_eq!(item_count(&world, 3001, COAL), 0, "materials consumed");

    // Byron → Mimyu accepts the stone (cond 5) and brews Monkshood Juice.
    ev(&mut world, byron, "30711-03.html");
    assert_eq!(cond(&world), Some(4));
    ev(&mut world, mimyu, "30747-02.html");
    assert_eq!(cond(&world), Some(5));
    assert_eq!(
        item_count(&world, 3001, FAIRY_STONE),
        0,
        "stone handed to Mimyu"
    );
    ev(&mut world, mimyu, "30747-07.html");
    assert_eq!(
        item_count(&world, 3001, MONKSHOOD_JUICE),
        1,
        "Monkshood Juice"
    );

    // Exarion trades the juice for its Scale and a hunt (cond 6).
    ev(&mut world, exarion, "30748-02.html");
    assert_eq!(cond(&world), Some(6));
    assert_eq!(item_count(&world, 3001, EXARION_SCALE), 1, "Exarion Scale");
    assert_eq!(
        item_count(&world, 3001, MONKSHOOD_JUICE),
        0,
        "juice consumed"
    );

    // Farm 20 Exarion Eggs from Leto Warriors (drake_hunt).
    let mut mob = NPC_OID + 20;
    for _ in 0..20 {
        mob += 1;
        add_test_npc(&mut world, mob, LETO_WARRIOR, "Monster", 40, 110, 200, 0);
        world.force_roll(0); // give_item_randomly roll → drop
        npc::npc_do_die(&mut world, mob, 3001);
    }
    assert_eq!(item_count(&world, 3001, EXARION_EGG), 20, "20 eggs farmed");

    // Exarion hatches the egg (cond 7).
    talk(&mut world, exarion);
    assert_eq!(cond(&world), Some(7), "egg hatched → cond 7");
    assert_eq!(item_count(&world, 3001, EXARION_EGG), 1, "one hatched egg");
    assert_eq!(item_count(&world, 3001, EXARION_SCALE), 0, "scale consumed");

    // Mimyu redeems the egg for a Dragonflute (forced roll 0 → Wind), completing.
    world.force_roll(0); // give_reward roll(100) → Wind
    ev(&mut world, mimyu, "30747-12.html");
    assert_eq!(
        item_count(&world, 3001, DRAGONFLUTE_OF_WIND),
        1,
        "Dragonflute of Wind"
    );
    assert!(
        cond(&world).is_none(),
        "Little Wing is repeatable: the reward exit forgets the quest"
    );

    // --- Separately: a Deluxe Fairy Stone shatters when striking the fae. ---
    let (mut w2, _db2, _l2) = quest_test_world();
    add_quest_items(&mut w2, &[(DELUXE_FAIRY_STONE, "q", true)]);
    add_test_npc(&mut w2, NPC_OID, FLINE, "Monster", 40, 110, 200, 0);
    let _rx2 = ingame_player(&mut w2, 1, 3001, 100, 200, 0);
    w2.objects.get_component_mut::<Player>(&3001).unwrap().level = 40;
    {
        let quests = w2
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "6".to_string());
    }
    inject(&mut w2, 3001, 0x0420_9000, DELUXE_FAIRY_STONE, 1);
    w2.force_roll(0); // onAttack roll(100)==0 < 30 → shatter
    combat::npc_receive_damage(&mut w2, NPC_OID, 3001, 1.0, false);
    assert_eq!(
        item_count(&w2, 3001, DELUXE_FAIRY_STONE),
        0,
        "the Deluxe Fairy Stone shatters on the fae"
    );
}

/// Quest 421 — the full hatchling→strider arc, driven through the pet
/// infrastructure: the flute-enchant start gate, Mimyu binding the rite to the
/// flute's object id, the four-tree drink grind (only *the bound pet's* blows
/// count, and only past each tree's hit threshold), and redeeming the flute for
/// the Dragon Bugle once all four essences (`memoState == 15`) are drunk.
#[test]
fn quest_q00421_little_wings_big_adventure() {
    use crate::model::components::social::Quests;
    use crate::model::components::summons::{PetOf, SummonRef};
    use crate::model::inventory::Inventory;

    const CRONOS: i32 = 30610;
    const MIMYU: i32 = 30747;
    const FLUTE: i32 = 3500; // Dragonflute of Wind
    const BUGLE: i32 = 4422; // Dragon Bugle of Wind
    const LEAF: i32 = 4325; // Fairy Leaf
    const HATCHLING: i32 = 12311; // stand-in pet species
    // (tree npc id, min_hits, memo bit value)
    const TREES: [(i32, i32, i32); 4] = [
        (27185, 270, 1),
        (27186, 400, 2),
        (27187, 150, 4),
        (27188, 270, 8),
    ];
    const FLUTE_OID: i32 = 0x0042_1000;
    let q = "Q00421_LittleWingsBigAdventure";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (FLUTE, "Dragonflute of Wind", false),
            (BUGLE, "Dragon Bugle of Wind", false),
            (LEAF, "Fairy Leaf", true),
        ],
    );

    let cronos = NPC_OID;
    let mimyu = NPC_OID + 1;
    add_test_npc(&mut world, cronos, CRONOS, "Folk", 55, 100, 200, 0);
    add_test_npc(&mut world, mimyu, MIMYU, "Folk", 55, 120, 200, 0);
    let tree_oids: Vec<i32> = TREES
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| {
            let oid = NPC_OID + 2 + i as i32;
            add_test_npc(&mut world, oid, *id, "Monster", 60, 300, 300 + i as i32, 0);
            oid
        })
        .collect();

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 45;
    // One Dragonflute of Wind, enchant 60 (its enchant level is the hatchling's).
    inject(&mut world, 3001, FLUTE_OID, FLUTE, 1);

    let set_enchant = |w: &mut World, level: i32| {
        w.objects
            .get_component_mut::<Inventory>(&3001)
            .unwrap()
            .set_item_enchant_level(FLUTE, level);
    };
    let memo = |w: &World| -> i32 {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .and_then(|qs| qs.vars.get("memoState"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let started = |w: &World| -> bool {
        w.objects
            .get_component::<Quests>(&3001)
            .and_then(|qc| qc.0.get(q))
            .is_some_and(|qs| qs.state == model::quest::state::STARTED)
    };
    let set_hits = |w: &mut World, n: i32| {
        w.objects
            .get_component_mut::<Quests>(&3001)
            .unwrap()
            .0
            .get_mut(q)
            .unwrap()
            .vars
            .insert("hits".into(), n.to_string());
    };
    let event = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };
    let bind_pet = |w: &mut World, oid: i32, collar: i32| {
        add_test_npc(w, oid, HATCHLING, "Pet", 55, 130, 200, 0);
        w.objects.add_components(
            &oid,
            PetOf {
                collar_object_id: collar,
                fed: 100,
                max_fed: 100,
                level: 55,
                exp: 0,
                sp: 0,
                exp_before_death: 0,
            },
        );
        match w.objects.get_component_mut::<SummonRef>(&3001) {
            Some(s) => s.pet = Some(oid),
            None => w.objects.add_components(
                &3001,
                SummonRef {
                    servitor: None,
                    pet: Some(oid),
                },
            ),
        }
    };

    // --- Enchant gate: an under-enchanted flute (hatchling < 55) can't start. ---
    set_enchant(&mut world, 40);
    talk(&mut world, cronos); // creates the CREATED quest state (Java getQuestState(true))
    event(&mut world, cronos, "30610-05.htm");
    assert!(
        !started(&world),
        "under-enchanted flute cannot start the rite"
    );
    set_enchant(&mut world, 60);

    // --- Start: the rite binds to this flute's object id. ---
    event(&mut world, cronos, "30610-05.htm");
    assert!(
        started(&world),
        "the rite started with a level-60 hatchling"
    );
    assert_eq!(memo(&world), 100, "memoState 100 on start");
    assert_eq!(
        world.objects.get_component::<Quests>(&3001).unwrap().0[q]
            .vars
            .get("fluteObjectId")
            .map(|s| s.as_str()),
        Some(FLUTE_OID.to_string().as_str()),
        "rite bound to the flute's object id"
    );

    // --- Mimyu intro (100 → 200). ---
    talk(&mut world, mimyu);
    assert_eq!(memo(&world), 200, "Mimyu's intro advances memoState to 200");

    // Without the hatchling out, Mimyu withholds the Fairy Leaves.
    event(&mut world, mimyu, "30747-05.html");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        0,
        "no leaves without the pet"
    );
    assert_ne!(
        quest_cond(&world, 3001, q),
        Some(2),
        "no cond 2 without the pet"
    );

    // Summon the bound hatchling; now Mimyu hands over four leaves.
    bind_pet(&mut world, NPC_OID + 20, FLUTE_OID);
    event(&mut world, mimyu, "30747-05.html");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "four Fairy Leaves granted"
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(2), "cond 2 to drink");
    assert_eq!(memo(&world), 0, "memoState reset to the 4-bit drink field");

    // --- The player's own blow does not count. ---
    quests::notify_attack(&mut world, 3001, tree_oids[0], TREES[0].0, None, false);
    assert_eq!(memo(&world), 0, "a player blow drinks nothing");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "no leaf spent by the player"
    );

    // A pet blow below the hit threshold drinks nothing either.
    quests::notify_attack(&mut world, 3001, tree_oids[0], TREES[0].0, None, true);
    assert_eq!(memo(&world), 0, "below the threshold, no essence taken");
    assert_eq!(
        item_count(&world, 3001, LEAF),
        4,
        "no leaf spent below threshold"
    );

    // --- The four-tree grind: past each threshold, the bound pet drinks. ---
    for (i, (id, min_hits, value)) in TREES.iter().enumerate() {
        set_hits(&mut world, min_hits - 1); // next blow reaches the threshold
        world.force_roll(0); // the 2% essence roll → success
        let before = memo(&world);
        quests::notify_attack(&mut world, 3001, tree_oids[i], *id, None, true);
        assert_eq!(memo(&world), before + value, "tree {id} sets its memo bit");
    }
    assert_eq!(memo(&world), 15, "all four essences drunk");
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(3),
        "cond 3 once all drunk"
    );
    assert_eq!(
        item_count(&world, 3001, LEAF),
        0,
        "all four leaves consumed"
    );

    // --- Redemption: Mimyu grows the hatchling into a strider. ---
    talk(&mut world, mimyu); // memoState 15, pet present, no leaves → 16
    assert_eq!(memo(&world), 16, "Mimyu readies the transformation");
    world
        .objects
        .get_component_mut::<SummonRef>(&3001)
        .unwrap()
        .pet = None; // dismiss the hatchling
    talk(&mut world, mimyu); // memoState 16, no summon, bound flute → the Bugle
    assert_eq!(item_count(&world, 3001, BUGLE), 1, "Dragon Bugle awarded");
    assert_eq!(item_count(&world, 3001, FLUTE), 0, "the flute is consumed");
    assert!(
        world
            .objects
            .get_component::<Quests>(&3001)
            .is_none_or(|qc| !qc.0.contains_key(q)),
        "the repeatable quest is forgotten on completion"
    );
}

/// Quest 421 — killing a Tree of Vision (rather than drinking from it) summons a
/// 20-strong Guardian Ghost ambush that despawns after five minutes.
#[test]
fn quest_q00421_guardian_ambush_despawns() {
    use crate::model::components::social::Quests;

    const TREE: i32 = 27185; // Tree of Wind
    const GUARDIAN: i32 = 27189;

    let (mut world, _db, _l) = quest_test_world();
    {
        let mut t = crate::data::npc_data::default_template(GUARDIAN);
        t.type_name = "Monster".into();
        t.level = 40;
        world.data.npc_data.insert_for_test(t);
    }
    let tree = NPC_OID;
    add_test_npc(&mut world, tree, TREE, "Monster", 60, 300, 300, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 300, 300, 0);
    let q = "Q00421_LittleWingsBigAdventure";
    {
        let quests = world.objects.get_component_mut::<Quests>(&3001).unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "2".to_string());
    }

    // Fell the tree — the ambush spawns. Since the G22 `ai/others` sweep the
    // standalone `FairyTrees` script swarms the same trees with 20 more (Java
    // registers both scripts on this kill), so 40 appear; theirs last 30 s,
    // this quest's 5 minutes.
    drain(&mut rx);
    combat::npc_receive_damage(&mut world, tree, 3001, 10_000.0, false);
    assert_eq!(
        npcs_of(&mut world, GUARDIAN).len(),
        40,
        "20 Guardian Ghosts from the quest + 20 from ai/others/FairyTrees"
    );

    // The dying tree's parting shot: `npc.doCast(VICIOUS_POISON)` as the first
    // guardian appears. This is a *real* cast, so assert the wire rather than
    // trusting the call — `npc_cast` returns false and does nothing when the
    // skill id is absent or the use-conditions refuse, and a silent no-op here
    // would look exactly like a working port.
    let cast_skills: Vec<i32> = drain(&mut rx)
        .iter()
        .filter(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .map(|p| {
            let mut r = commons::network::PacketReader::new(&p[1..]);
            for _ in 0..3 {
                let _ = r.read_i32(); // cast bar, caster, target
            }
            r.read_i32().unwrap() // skill id
        })
        .collect();
    assert!(
        cast_skills.contains(&4243),
        "the tree casts Venomous Poison on its killer: {cast_skills:?}"
    );

    // After 30 s the FairyTrees half is gone and the quest's ambush remains.
    advance_ticks(&mut world, 301);
    assert_eq!(
        npcs_of(&mut world, GUARDIAN).len(),
        20,
        "the FairyTrees guardians (30 s) expire first"
    );

    // Five minutes later, they are gone.
    advance_ticks(&mut world, 2700); // the rest of the 300_000 ms
    assert!(
        npcs_of(&mut world, GUARDIAN).is_empty(),
        "the ambush despawns after 5 minutes"
    );
}
