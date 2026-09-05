//! The orc occupation quests: Q00414 Raider, Q00415 Monk and Q00416 Shaman,
//! their page guards, and the branches that are dead at both ends.

use super::super::*;

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
