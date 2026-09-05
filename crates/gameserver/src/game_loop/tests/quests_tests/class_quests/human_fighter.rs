//! The human fighter occupation quests: Q00401 Warrior, Q00402 Human Knight
//! and Q00403 Rogue.

use super::super::*;

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
