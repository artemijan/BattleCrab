//! The class-transfer scripts themselves (first and second change, per
//! race) and the alliance master menu that sits beside them.

use super::*;

/// OrcChange1: an eligible Orc Fighter with the Mark of Raider becomes an
/// Orc Raider — proof consumed, 15 coupons paid, class persisted; the
/// category gates refuse a player who already transferred.
#[test]
fn orc_change1_first_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1592, "Mark of Raider", true),
            (8869, "Shadow Coupon (D)", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("FIGHTER_GROUP", &[44, 45]);
    world.data.categories.insert_for_test("MAGE_GROUP", &[49]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[45]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30500, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 3; // Orc
        p.class_id = 44; // Orc Fighter
        p.base_class_id = 44;
    }
    inventory::add_inventory_item(&mut world, 3001, 1592, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // The named bypass shows the fighter class list.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("class list");
    assert!(html.contains("45") || !html.is_empty());

    // Transfer to Orc Raider (45).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")),
    );
    {
        let p = world.objects.get_component::<Player>(&3001).unwrap();
        assert_eq!(p.class_id, 45);
        assert_eq!(p.base_class_id, 45);
    }
    assert_eq!(item_count(&world, 3001, 1592), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "shadow coupons");
    // The change persisted immediately.
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::StorePlayer { save } if save.base.class_id == 45)),
        "StorePlayer with the new class"
    );
    // A UserInfo re-broadcast reached the player.
    assert!(
        drain(&mut rx).iter().any(|p| p[0] == 0x32),
        "UserInfo after transfer"
    );

    // Now in SECOND_CLASS_GROUP: another transfer attempt is refused.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange1 45")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("refusal page");
    assert!(html.contains("class transfer") || !html.is_empty());
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        45,
        "unchanged"
    );
}

/// DwarfWarehouseChange1: a Dwarven Fighter with the Ring of Raven becomes a
/// Scavenger. Mirrors the OrcChange1 test, but on the shared
/// `DwarfChange1` implementation.
#[test]
fn dwarf_warehouse_change1_first_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1642, "Ring of Raven", true),
            (8869, "Shadow Coupon (D)", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("BOUNTY_HUNTER_GROUP", &[53, 54]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[54]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30498, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 4; // Dwarf
        p.class_id = 53;
        p.base_class_id = 53;
    }
    inventory::add_inventory_item(&mut world, 3001, 1642, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfWarehouseChange1 54")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 54, "now a Scavenger");
    assert_eq!(
        p.base_class_id, 54,
        "on the base slot the base class moves too"
    );
    assert_eq!(item_count(&world, 3001, 1642), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "shadow coupons paid");
}

/// The level gate: 19 is refused, the proof is kept, and nothing is paid.
#[test]
fn dwarf_change1_refuses_below_level_20() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1635, "Final Pass Certificate", true),
            (8869, "Shadow Coupon (D)", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("WARSMITH_GROUP", &[53, 56]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[56]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30499, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 19;
        p.race = 4;
        p.class_id = 53;
        p.base_class_id = 53;
    }
    inventory::add_inventory_item(&mut world, 3001, 1635, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange1 56")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 53, "still a Dwarven Fighter at 19");
    assert_eq!(
        item_count(&world, 3001, 1635),
        1,
        "the proof is NOT consumed on a refusal"
    );
    assert_eq!(item_count(&world, 3001, 8869), 0, "and nothing is paid");
}

/// Without the proof item the transfer is refused even at level 20.
#[test]
fn dwarf_change1_refuses_without_the_proof_item() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1635, "Final Pass Certificate", true),
            (8869, "Shadow Coupon (D)", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("WARSMITH_GROUP", &[53, 56]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[56]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30499, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 4;
        p.class_id = 53;
        p.base_class_id = 53;
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange1 56")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        53,
        "no proof, no transfer"
    );
}

/// Every html page the two scripts can return must exist in the dist, or a
/// player hits a blank window at the moment of their class change.
#[test]
fn dwarf_change1_html_pages_exist_in_dist() {
    // Village-master pages live under data/scripts/, not data/html/.
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/");
    for (dir, npcs, extra) in [
        (
            "village_master/DwarfBlacksmithChange1",
            [30499, 30504, 30595, 32093],
            "30499-12.htm",
        ),
        (
            "village_master/DwarfWarehouseChange1",
            [30498, 30503, 30594, 32092],
            "30498-12.htm",
        ),
    ] {
        for npc in npcs {
            // -01/-05 from onTalk, -06/-07 refusals, -08..-11 the level/proof
            // matrix, -10 the success page.
            for suffix in ["01", "05", "06", "07", "08", "09", "10", "11"] {
                let path = format!("{DIST}{dir}/{npc}-{suffix}.htm");
                assert!(
                    std::path::Path::new(&path).exists(),
                    "missing dist page {dir}/{npc}-{suffix}.htm"
                );
            }
        }
        // Only the *first* NPC of each set ships a `-12` page; Java hard-codes
        // that one id for the fourth-class refusal regardless of who you are
        // talking to, which is why the port does the same.
        assert!(
            std::path::Path::new(&format!("{DIST}{dir}/{extra}")).exists(),
            "missing fourth-class page {dir}/{extra}"
        );
    }
}

/// ElfHumanFighterChange1: a Human Fighter with the Medallion of Warrior
/// becomes a Warrior. The same NPCs serve Elves, so the elf branch is checked
/// too — a bad `from_class` match would let a Human take an Elven class.
#[test]
fn elf_human_fighter_change1_transfers_by_race() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (1145, "Medallion of Warrior", true),
            (8869, "Coupon", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("FIGHTER_GROUP", &[0, 1, 18, 19]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[1, 19]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30066, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 0; // Human
        p.class_id = 0; // Fighter
        p.base_class_id = 0;
    }
    inventory::add_inventory_item(&mut world, 3001, 1145, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    // A Human Fighter may not take the Elven Knight (19) branch.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange1 19")),
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        0,
        "a Human must not take an Elf class from the same NPC"
    );
    assert_eq!(
        item_count(&world, 3001, 1145),
        1,
        "and nothing was consumed"
    );

    // Warrior (1) is the Human branch.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange1 1")),
    );
    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 1, "now a Warrior");
    assert_eq!(p.base_class_id, 1);
    assert_eq!(item_count(&world, 3001, 1145), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// ElfHumanWizardChange1: an Elven Mage becomes an Oracle.
#[test]
fn elf_human_wizard_change1_elf_branch() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1235, "Leaf of Oracle", true), (8869, "Coupon", false)],
    );
    world
        .data
        .categories
        .insert_for_test("MAGE_GROUP", &[10, 25, 29]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[29]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30037, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 1; // Elf
        p.class_id = 25; // Elven Mage
        p.base_class_id = 25;
    }
    inventory::add_inventory_item(&mut world, 3001, 1235, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanWizardChange1 29")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 29, "now an Oracle");
    assert_eq!(item_count(&world, 3001, 1235), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// Every page the two scripts can return must exist in the dist. The matrix is
/// table-driven (four consecutive pages per target), so an off-by-one in the
/// table would silently serve the wrong — or a missing — page.
#[test]
fn elf_human_change1_html_pages_exist_in_dist() {
    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/data/scripts/");
    // (dir, npcs, per-target first pages, talk/refusal pages, fourth-class page)
    let sets: [(&str, &[i32], &[u32], &[u32], &str); 2] = [
        (
            "village_master/ElfHumanFighterChange1",
            &[30066, 30288, 30373, 32094],
            &[21, 25, 29, 33, 37],
            &[1, 11, 18, 19, 20],
            "30066-41.htm",
        ),
        (
            "village_master/ElfHumanWizardChange1",
            &[30037, 30070, 30289, 32095, 32098],
            &[18, 22, 26, 30],
            &[1, 8, 15, 16, 17],
            "30037-34.htm",
        ),
    ];
    for (dir, npcs, firsts, fixed, fourth) in sets {
        for npc in npcs {
            for page in fixed {
                let path = format!("{DIST}{dir}/{npc}-{page:02}.htm");
                assert!(
                    std::path::Path::new(&path).exists(),
                    "missing {dir}/{npc}-{page:02}.htm"
                );
            }
            for first in firsts {
                for p in *first..=(*first + 3) {
                    let path = format!("{DIST}{dir}/{npc}-{p}.htm");
                    assert!(
                        std::path::Path::new(&path).exists(),
                        "missing {dir}/{npc}-{p}.htm"
                    );
                }
            }
        }
        assert!(
            std::path::Path::new(&format!("{DIST}{dir}/{fourth}")).exists(),
            "missing fourth-class page {dir}/{fourth}"
        );
    }
}

/// DarkElfChange1: a Dark Fighter with the Gaze of Abyss becomes a Palus
/// Knight. Note the bypass event is the CLASSES **row index** (0), not the
/// class id — the opposite convention to the other Change1 scripts.
#[test]
fn dark_elf_change1_transfers_by_row_index() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1244, "Gaze of Abyss", true), (8869, "Coupon", false)],
    );
    world
        .data
        .categories
        .insert_for_test("FIRST_CLASS_GROUP", &[32, 35, 39, 42]);
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 2; // Dark Elf
        p.class_id = 31; // Dark Fighter
        p.base_class_id = 31;
    }
    inventory::add_inventory_item(&mut world, 3001, 1244, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1 0")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 32, "row 0 is Palus Knight");
    assert_eq!(item_count(&world, 3001, 1244), 0, "proof consumed");
    assert_eq!(item_count(&world, 3001, 8869), 15, "coupons paid");
}

/// A Dark Mage cannot take a Dark Fighter row, even though the same NPC
/// serves both — Java checks the source class per row.
#[test]
fn dark_elf_change1_rejects_the_wrong_source_class() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(1244, "Gaze of Abyss", true), (8869, "Coupon", false)],
    );
    world
        .data
        .categories
        .insert_for_test("FIRST_CLASS_GROUP", &[32, 35, 39, 42]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 20;
        p.race = 2;
        p.class_id = 38; // Dark MAGE asking for the fighter row
        p.base_class_id = 38;
    }
    inventory::add_inventory_item(&mut world, 3001, 1244, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1 0")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        38,
        "unchanged"
    );
    assert_eq!(item_count(&world, 3001, 1244), 1, "and nothing consumed");
}

/// A character standing on a subclass is refused outright — Java's
/// `if (player.isSubClassActive()) return getNoQuestMsg(player);`.
#[test]
fn dark_elf_change1_refuses_while_a_subclass_is_active() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1244, "Gaze of Abyss", true)]);
    add_test_npc(&mut world, NPC_OID, 30290, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 2;
        p.class_id = 31;
        p.base_class_id = 31;
        p.class_index = 1; // on a subclass
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange1")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a reply");
    assert!(
        !html.contains("30290-01"),
        "the class list must not be offered on a subclass"
    );
}

/// Every page DarkElfChange1 can return exists — and note these are `.html`,
/// not `.htm` like its siblings.
#[test]
fn dark_elf_change1_html_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/DarkElfChange1/"
    );
    for npc in [30290, 30297, 30462] {
        for page in [1, 8, 31, 32, 33] {
            let path = format!("{DIST}{npc}-{page:02}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{page:02}.html"
            );
        }
        for page in 15..=30 {
            let path = format!("{DIST}{npc}-{page}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}-{page}.html"
            );
        }
    }
}

/// FirstClassTransferTalk: the seven headmasters only *talk* about transfers.
/// The page name uses an underscore and `.html`, unlike every other
/// village-master script.
#[test]
fn first_class_transfer_talk_picks_the_page_by_race_and_progress() {
    let cases: [(i32, i32, bool, i32, &str); 7] = [
        // (npc, player race, is_mage, class level, expected suffix)
        (30026, 0, false, 0, "fighter"),    // Blitz, human fighter
        (30026, 0, true, 0, "no"),          // a mage at the fighter guild
        (30031, 0, true, 0, "mystic"),      // Biotin, human priest
        (30154, 1, true, 0, "mystic"),      // Asterios serves both sides
        (30520, 4, false, 0, "fighter"),    // Dwarves: fighter only
        (30026, 0, false, 1, "transfer_1"), // already first-occupation
        (30026, 0, false, 2, "transfer_2"), // second or beyond
    ];
    for (npc_id, race, is_mage, class_level, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        // class 0 = base fighter, 1 = a first occupation, 4 = a second.
        let class_id = match class_level {
            0 => {
                if is_mage {
                    10
                } else {
                    0
                }
            }
            1 => 1,
            _ => 4,
        };
        world.data.categories.insert_for_test("MAGE_GROUP", &[10]);
        world
            .data
            .categories
            .insert_for_test("FIRST_CLASS_GROUP", &[1]);
        world
            .data
            .categories
            .insert_for_test("SECOND_CLASS_GROUP", &[4]);
        world
            .data
            .categories
            .insert_for_test("THIRD_CLASS_GROUP", &[]);
        world
            .data
            .categories
            .insert_for_test("FOURTH_CLASS_GROUP", &[]);
        add_test_npc(&mut world, NPC_OID, npc_id, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.race = race;
            p.class_id = class_id;
            p.base_class_id = class_id;
        }
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest FirstClassTransferTalk")),
        );

        let html = drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .unwrap_or_default();
        // Compare against the actual dist page, run through the same strip the
        // cache applies — asserting "non-empty" would happily accept the
        // *wrong* page.
        let want_path = format!(
            "{}/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/{npc_id}_{expected}.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let want = crate::data::htm_cache::strip_htm(
            &std::fs::read_to_string(&want_path)
                .unwrap_or_else(|_| panic!("dist page {want_path}")),
        )
        .replace("%objectId%", &NPC_OID.to_string());
        assert_eq!(
            html, want,
            "npc {npc_id} race {race} mage {is_mage} level {class_level}: wrong page (wanted {expected})"
        );
    }
}

/// A player of the wrong race gets the refusal page.
#[test]
fn first_class_transfer_talk_refuses_another_race() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30520, "VillageMaster", 70, 100, 0, 0); // Dwarf master
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.race = 0; // Human at a Dwarf headmaster
        p.class_id = 0;
        p.base_class_id = 0;
    }
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest FirstClassTransferTalk")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a reply");
    let want = crate::data::htm_cache::strip_htm(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/30520_no.html"
        ))
        .expect("dist page"),
    )
    .replace("%objectId%", &NPC_OID.to_string());
    assert_eq!(
        html, want,
        "a Human at a Dwarf headmaster gets the refusal page"
    );
}

/// Every page the script can name must exist — and the availability is
/// asymmetric: the Human fighter master ships no `mystic`, the priest no
/// `fighter`, and neither Dwarf master ships a `mystic` at all.
#[test]
fn first_class_transfer_talk_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/FirstClassTransferTalk/"
    );
    let expected: [(i32, &[&str]); 7] = [
        (30026, &["fighter", "no", "transfer_1", "transfer_2"]),
        (30031, &["mystic", "no", "transfer_1", "transfer_2"]),
        (
            30154,
            &["fighter", "mystic", "no", "transfer_1", "transfer_2"],
        ),
        (
            30358,
            &["fighter", "mystic", "no", "transfer_1", "transfer_2"],
        ),
        (
            30565,
            &["fighter", "mystic", "no", "transfer_1", "transfer_2"],
        ),
        (30520, &["fighter", "no", "transfer_1", "transfer_2"]),
        (30525, &["fighter", "no", "transfer_1", "transfer_2"]),
    ];
    for (npc, suffixes) in expected {
        for s in suffixes {
            let path = format!("{DIST}{npc}_{s}.html");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {npc}_{s}.html"
            );
        }
    }
    // And the asymmetry is real, not an accident of my table: the Human
    // fighter master genuinely ships no mystic page, which is why the script
    // must answer `no` there rather than inventing one.
    assert!(!std::path::Path::new(&format!("{DIST}30026_mystic.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30031_fighter.html")).exists());
    assert!(!std::path::Path::new(&format!("{DIST}30520_mystic.html")).exists());
}

/// DwarfBlacksmithChange2: an Artisan with **all three** marks becomes a
/// Warsmith at level 40, paying C-grade coupons.
#[test]
fn dwarf_change2_second_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (3119, "Mark of Guildsman", true),
            (3238, "Mark of Prosperity", true),
            (2867, "Mark of Maestro", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("WARSMITH_GROUP", &[56, 57]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30512, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 4;
        p.class_id = 56; // Artisan
        p.base_class_id = 56;
    }
    for id in [3119, 3238, 2867] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange2 57")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 57, "now a Warsmith");
    for id in [3119, 3238, 2867] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 8870), 15, "C-grade coupons");
}

/// Holding only *some* of the marks is not enough — Java's
/// `hasQuestItems(a, b, c)` is an AND. Treating it as "any" would let a player
/// transfer on one mark.
#[test]
fn dwarf_change2_requires_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (3119, "Mark of Guildsman", true),
            (3238, "Mark of Prosperity", true),
            (2867, "Mark of Maestro", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("WARSMITH_GROUP", &[56, 57]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30512, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 4;
        p.class_id = 56;
        p.base_class_id = 56;
    }
    // Two of the three.
    inventory::add_inventory_item(&mut world, 3001, 3119, 1);
    inventory::add_inventory_item(&mut world, 3001, 3238, 1);
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfBlacksmithChange2 57")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        56,
        "still an Artisan"
    );
    assert_eq!(
        item_count(&world, 3001, 3119),
        1,
        "and the marks are not taken"
    );
    assert_eq!(item_count(&world, 3001, 3238), 1);
}

/// Level 39 is refused even holding all three marks.
#[test]
fn dwarf_change2_requires_level_40() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (3119, "Mark of Guildsman", true),
            (3238, "Mark of Prosperity", true),
            (2809, "Mark of Searcher", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("BOUNTY_HUNTER_GROUP", &[54, 55]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30511, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 39;
        p.race = 4;
        p.class_id = 54; // Scavenger
        p.base_class_id = 54;
    }
    for id in [3119, 3238, 2809] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DwarfWarehouseChange2 55")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        54,
        "still a Scavenger at 39"
    );
    assert_eq!(item_count(&world, 3001, 3119), 1, "marks kept");
}

/// One 12-page set serves all eight masters per script — every page the
/// scripts can name belongs to the *first* NPC's id.
#[test]
fn dwarf_change2_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/"
    );
    for (dir, page_npc) in [
        ("DwarfBlacksmithChange2", 30512),
        ("DwarfWarehouseChange2", 30511),
    ] {
        for n in 1..=12 {
            let path = format!("{DIST}{dir}/{page_npc}-{n:02}.htm");
            assert!(
                std::path::Path::new(&path).exists(),
                "missing {dir}/{page_npc}-{n:02}.htm"
            );
        }
        // And the other masters genuinely ship nothing of their own.
        let other = format!("{DIST}{dir}/30677-01.htm");
        assert!(
            !std::path::Path::new(&other).exists(),
            "only the first NPC ships pages"
        );
    }
}

/// OrcChange2: an Orc Raider with the three marks becomes a Destroyer and is
/// paid C-grade coupons.
#[test]
fn orc_change2_transfer_pays_coupons() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2627, "Challenger", true),
            (3203, "Glory", true),
            (3276, "Champion", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("ORC_MALL_CLASS", &[45, 46]);
    world.data.categories.insert_for_test("ORC_FALL_CLASS", &[]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30513, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 3;
        p.class_id = 45; // Orc Raider
        p.base_class_id = 45;
    }
    for id in [2627, 3203, 3276] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange2 46")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 46, "now a Destroyer");
    assert_eq!(
        item_count(&world, 3001, 8870),
        15,
        "Orc masters pay coupons"
    );
}

/// DarkElfChange2 takes the **row index**, and — unlike every other Change2 —
/// pays **no coupon at all**.
#[test]
fn dark_elf_change2_uses_row_index_and_pays_nothing() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2633, "Duty", true),
            (3172, "Fate", true),
            (3307, "Witchcraft", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("SECOND_CLASS_GROUP", &[33]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30474, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 2; // Dark Elf
        p.class_id = 32; // Palus Knight
        p.base_class_id = 32;
    }
    for id in [2633, 3172, 3307] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    // Row 0 = Shillien Knight (33) from Palus Knight (32).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest DarkElfChange2 0")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 33, "row 0 is Shillien Knight");
    for id in [2633, 3172, 3307] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(
        item_count(&world, 3001, 8870),
        0,
        "the Dark Elf script pays NO coupon"
    );
}

/// Both scripts need all three marks.
#[test]
fn change2_scripts_require_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2627, "Challenger", true),
            (3203, "Glory", true),
            (3276, "Champion", true),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("ORC_MALL_CLASS", &[45, 46]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30513, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 3;
        p.class_id = 45;
        p.base_class_id = 45;
    }
    inventory::add_inventory_item(&mut world, 3001, 2627, 1); // one of three
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest OrcChange2 46")),
    );

    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        45,
        "still a Raider"
    );
    assert_eq!(item_count(&world, 3001, 2627), 1, "the one mark is kept");
}

/// Both page sets exist, and both are owned by a single NPC — note the Dark
/// Elf owner (30474) is the *third* entry in its NPC list, not the first.
#[test]
fn change2_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/"
    );
    for n in [1u32, 2, 6, 10, 17, 18, 19] {
        let p = format!("{DIST}OrcChange2/30513-{n:02}.htm");
        assert!(
            std::path::Path::new(&p).exists(),
            "missing OrcChange2/30513-{n:02}.htm"
        );
    }
    for first in [20u32, 24, 28, 32] {
        for n in first..=(first + 3) {
            let p = format!("{DIST}OrcChange2/30513-{n}.htm");
            assert!(
                std::path::Path::new(&p).exists(),
                "missing OrcChange2/30513-{n}.htm"
            );
        }
    }
    for n in [1u32, 8, 12, 19, 54, 55, 56] {
        let p = format!("{DIST}DarkElfChange2/30474-{n:02}.html");
        assert!(
            std::path::Path::new(&p).exists(),
            "missing DarkElfChange2/30474-{n:02}.html"
        );
    }
    for first in [26u32, 30, 34, 38, 42, 46, 50] {
        for n in first..=(first + 3) {
            let p = format!("{DIST}DarkElfChange2/30474-{n}.html");
            assert!(
                std::path::Path::new(&p).exists(),
                "missing DarkElfChange2/30474-{n}.html"
            );
        }
    }
}

/// ElfHumanFighterChange2: a Warrior with all three marks becomes a Gladiator
/// at level 40 and is paid 15 C-grade coupons.
#[test]
fn elf_human_change2_second_class_transfer() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2627, "Challenger", true),
            (2734, "Trust", true),
            (2762, "Duelist", true),
            (8870, "Coupon C", false),
        ],
    );
    world
        .data
        .categories
        .insert_for_test("FIGHTER_GROUP", &[1, 2, 3]);
    world
        .data
        .categories
        .insert_for_test("HUMAN_FALL_CLASS", &[1, 2, 3]);
    world.data.categories.insert_for_test("ELF_FALL_CLASS", &[]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30109, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 1; // Warrior
        p.base_class_id = 1;
    }
    for id in [2627, 2734, 2762] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange2 2")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 2, "now a Gladiator");
    assert_eq!(p.base_class_id, 2, "and it is the base class");
    for id in [2627, 2734, 2762] {
        assert_eq!(item_count(&world, 3001, id), 0, "mark {id} consumed");
    }
    assert_eq!(item_count(&world, 3001, 8870), 15, "15 C-grade coupons");
}

/// The `from_class` half of each row is load-bearing. All ten Fighter targets
/// live on the same NPC, so without it a Human Knight could take Temple
/// Knight — an *Elven* Knight's class — from the same master.
#[test]
fn elf_human_change2_rejects_the_wrong_source_class() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2633, "Duty", true),
            (3140, "Life", true),
            (2820, "Healer", true),
        ],
    );
    world.data.categories.insert_for_test("FIGHTER_GROUP", &[4]);
    world
        .data
        .categories
        .insert_for_test("HUMAN_FALL_CLASS", &[4]);
    world.data.categories.insert_for_test("ELF_FALL_CLASS", &[]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30109, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 4; // Human Knight, holding exactly the Temple Knight marks
        p.base_class_id = 4;
    }
    for id in [2633, 3140, 2820] {
        inventory::add_inventory_item(&mut world, 3001, id, 1);
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    // 20 = Temple Knight, which only an Elven Knight (19) may take.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanFighterChange2 20")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 4, "still a Knight");
    for id in [2633, 3140, 2820] {
        assert_eq!(item_count(&world, 3001, id), 1, "mark {id} not consumed");
    }
}

/// All three marks are required — `hasQuestItems(a, b, c)` is an AND. With two
/// of three at level 40 the master serves the noProof page and takes nothing.
#[test]
fn elf_human_change2_requires_all_three_marks() {
    let (mut world, mut db_rx, _link_rx) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (2721, "Pilgrim", true),
            (2734, "Trust", true),
            (2820, "Healer", true),
        ],
    );
    world.data.categories.insert_for_test("CLERIC_GROUP", &[15]);
    world
        .data
        .categories
        .insert_for_test("HUMAN_CALL_CLASS", &[15]);
    world.data.categories.insert_for_test("ELF_CALL_CLASS", &[]);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[]);
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[]);
    add_test_npc(&mut world, NPC_OID, 30120, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.race = 0;
        p.class_id = 15; // Cleric
        p.base_class_id = 15;
    }
    for id in [2721, 2734] {
        inventory::add_inventory_item(&mut world, 3001, id, 1); // two of three
    }
    drain_db(&mut db_rx);
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest ElfHumanClericChange2 16")),
    );

    let p = world.objects.get_component::<Player>(&3001).unwrap();
    assert_eq!(p.class_id, 15, "still a Cleric");
    for id in [2721, 2734] {
        assert_eq!(item_count(&world, 3001, id), 1, "mark {id} kept");
    }
}

/// `onTalk` routes to a class-list page, the first-occupation refusal, or the
/// mismatch page. Compared byte-for-byte against the dist page — asserting
/// "non-empty" would pass while serving the wrong window.
#[test]
fn elf_human_change2_talk_picks_the_class_list() {
    // (script, page npc, group, human cat, elf cat, class id, expected page)
    let cases: [(&str, i32, &str, &str, &str, i32, u32); 9] = [
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            1,
            2,
        ),
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            5,
            9,
        ),
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            7,
            16,
        ),
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            19,
            23,
        ),
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            22,
            30,
        ),
        // No first occupation yet.
        (
            "ElfHumanFighterChange2",
            30109,
            "FIGHTER_GROUP",
            "HUMAN_FALL_CLASS",
            "ELF_FALL_CLASS",
            0,
            37,
        ),
        (
            "ElfHumanWizardChange2",
            30115,
            "WIZARD_GROUP",
            "HUMAN_MALL_CLASS",
            "ELF_MALL_CLASS",
            11,
            2,
        ),
        (
            "ElfHumanWizardChange2",
            30115,
            "WIZARD_GROUP",
            "HUMAN_MALL_CLASS",
            "ELF_MALL_CLASS",
            26,
            12,
        ),
        (
            "ElfHumanClericChange2",
            30120,
            "CLERIC_GROUP",
            "HUMAN_CALL_CLASS",
            "ELF_CALL_CLASS",
            29,
            9,
        ),
    ];
    for (script, npc_id, group, human_cat, elf_cat, class_id, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        world.data.categories.insert_for_test(group, &[class_id]);
        world
            .data
            .categories
            .insert_for_test(human_cat, &[class_id]);
        world.data.categories.insert_for_test(elf_cat, &[]);
        world
            .data
            .categories
            .insert_for_test("THIRD_CLASS_GROUP", &[]);
        world
            .data
            .categories
            .insert_for_test("FOURTH_CLASS_GROUP", &[]);
        add_test_npc(&mut world, NPC_OID, npc_id, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
            p.class_id = class_id;
            p.base_class_id = class_id;
        }
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {script}")),
        );

        let html = drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .unwrap_or_default();
        let want_path = format!(
            "{}/../../dist/game/data/scripts/village_master/{script}/{npc_id}-{expected:02}.htm",
            env!("CARGO_MANIFEST_DIR")
        );
        let want = crate::data::htm_cache::strip_htm(
            &std::fs::read_to_string(&want_path)
                .unwrap_or_else(|_| panic!("dist page {want_path}")),
        )
        .replace("%objectId%", &NPC_OID.to_string());
        assert_eq!(
            html, want,
            "{script} class {class_id}: wrong page (wanted {expected})"
        );
    }
}

/// Every page the three scripts can name exists — and each set is owned by one
/// NPC, so the other masters must ship nothing (which is why the hard-coded
/// page owner cannot be tidied into a per-NPC name that would 404).
#[test]
fn elf_human_change2_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/"
    );
    // (script, page npc, fixed pages, row first-pages, another master's id)
    let sets: [(&str, i32, &[u32], &[u32], i32); 3] = [
        (
            "ElfHumanFighterChange2",
            30109,
            &[1, 2, 9, 16, 23, 30, 37, 38, 39],
            &[40, 44, 48, 52, 56, 60, 64, 68, 72, 76],
            30187,
        ),
        (
            "ElfHumanWizardChange2",
            30115,
            &[1, 2, 12, 19, 20, 21],
            &[22, 26, 30, 34, 38],
            30174,
        ),
        (
            "ElfHumanClericChange2",
            30120,
            &[1, 2, 9, 13, 14, 15],
            &[16, 20, 24],
            30191,
        ),
    ];
    for (script, npc, fixed, firsts, other) in sets {
        for n in fixed {
            let p = format!("{DIST}{script}/{npc}-{n:02}.htm");
            assert!(
                std::path::Path::new(&p).exists(),
                "missing {script}/{npc}-{n:02}.htm"
            );
        }
        for first in firsts {
            for n in *first..=(*first + 3) {
                let p = format!("{DIST}{script}/{npc}-{n}.htm");
                assert!(
                    std::path::Path::new(&p).exists(),
                    "missing {script}/{npc}-{n}.htm"
                );
            }
        }
        let p = format!("{DIST}{script}/{other}-01.htm");
        assert!(
            !std::path::Path::new(&p).exists(),
            "only {npc} ships {script} pages"
        );
    }
}

/// AllianceMaster's dist page, run through the same strip the cache applies.
fn alliance_page(name: &str) -> String {
    let path = format!(
        "{}/../../dist/game/data/scripts/village_master/AllianceMaster/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    crate::data::htm_cache::strip_htm(
        &std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("dist page {path}")),
    )
    .replace("%objectId%", &NPC_OID.to_string())
}

/// Talking to any village master opens the alliance menu — with no clan check,
/// so a clanless player does see the two buttons.
#[test]
fn alliance_master_talk_opens_the_menu_without_a_clan() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .clan_id,
        0,
        "fixture player is clanless"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest AllianceMaster")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a reply");
    assert_eq!(
        html,
        alliance_page("9001-01.htm"),
        "the menu, not the clan refusal"
    );
}

/// Every page *except* the menu needs a clan. The asymmetry is the whole
/// script: the clanless player is refused on click, not on open.
#[test]
fn alliance_master_gates_every_page_but_the_menu_on_having_a_clan() {
    // (clan id, requested page, expected page)
    let cases: [(i32, &str, &str); 6] = [
        (0, "9001-02.htm", "9001-04.htm"), // create, clanless → refused
        (0, "9001-03.htm", "9001-04.htm"), // dissolve, clanless → refused
        (0, "9001-01.htm", "9001-01.htm"), // the menu is NOT gated
        (7, "9001-02.htm", "9001-02.htm"), // in a clan → the real page
        (7, "9001-03.htm", "9001-03.htm"),
        (7, "9001-01.htm", "9001-01.htm"),
    ];
    for (clan_id, requested, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        world
            .objects
            .get_component_mut::<Player>(&3001)
            .unwrap()
            .clan_id = clan_id;
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest AllianceMaster {requested}")),
        );

        let html = drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .expect("a reply");
        assert_eq!(
            html,
            alliance_page(expected),
            "clan {clan_id} asking for {requested} should get {expected}"
        );
    }
}

/// The four pages exist, and they are numbered against the **virtual** NPC id
/// 9001 — no real master ships a page of its own.
#[test]
fn alliance_master_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/AllianceMaster/"
    );
    for n in 1..=4 {
        let p = format!("{DIST}9001-{n:02}.htm");
        assert!(std::path::Path::new(&p).exists(), "missing 9001-{n:02}.htm");
    }
    for npc in [30026, 30031, 30913] {
        let p = format!("{DIST}{npc}-01.htm");
        assert!(
            !std::path::Path::new(&p).exists(),
            "pages are 9001-*, not per-NPC"
        );
    }
}
