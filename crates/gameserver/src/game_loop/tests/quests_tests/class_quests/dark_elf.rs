//! The dark elf occupation quests: Q00410 Palus Knight, Q00411 Assassin,
//! Q00412 Dark Wizard and Q00413 Shillien Oracle.

use super::super::*;

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
