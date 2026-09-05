//! The human mystic occupation quests: Q00404 Human Wizard and Q00405
//! Cleric, plus the quest-window tests that build on the Q00404 fixture.

use super::super::*;

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
