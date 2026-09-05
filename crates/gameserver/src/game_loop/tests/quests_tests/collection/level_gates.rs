//! The `addCondMaxLevel` refusals, gathered from every quest in this block.
//!
//! Five of the seven are identical apart from the quest, NPC, item, level cap
//! and html page — they are kept as separate tests, but sitting together makes
//! that plain and a table-driven merge a single-file change.

use super::super::*;

/// Q00261 refuses a starter above level 21 (`addCondMaxLevel(21)`): the quest
/// never starts.
#[test]
fn quest_q00261_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1087, "Spider Leg", true)]);
    add_test_npc(&mut world, NPC_OID, 30222, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;

    let q = "Q00261_CollectorsDream";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30222-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "the level-22 starter never begins the quest"
    );
}

/// Q00257 refuses a starter above level 16 (`addCondMaxLevel(16)`).
#[test]
fn quest_q00257_refused_above_level_16() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1084, "Gludio Lord's Mark", false)]);
    add_test_npc(&mut world, NPC_OID, 30039, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 17;

    let q = "Q00257_TheGuardIsBusy";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30039-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-17 starter never begins"
    );
    assert_eq!(
        item_count(&world, 3001, 1084),
        0,
        "no Lord's Mark handed out"
    );
}

/// Q00259 refuses a starter above level 21 (`addCondMaxLevel(21)`).
#[test]
fn quest_q00259_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1495, "Spider Skin", true)]);
    add_test_npc(&mut world, NPC_OID, 30497, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;

    let q = "Q00259_RequestFromTheFarmOwner";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30497-03.html")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-22 starter never begins"
    );
}

/// Q00293 refuses a starter above level 15 (`addCondMaxLevel(15)`).
#[test]
fn quest_q00293_refused_above_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1488, "Chrysolite Ore", true)]);
    add_test_npc(&mut world, NPC_OID, 30535, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 16;
        p.race = 4;
    }
    let q = "Q00293_TheHiddenVeins";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30535-04.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-16 starter never begins"
    );
}

/// Q00296 refuses a starter above level 21 (`addCondMaxLevel(21)`).
#[test]
fn quest_q00296_refused_above_level_21() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1493, "Silk", true)]);
    add_test_npc(&mut world, NPC_OID, 30519, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 22;
    let q = "Q00296_TarantulasSpiderSilk";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30519-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-22 starter never begins"
    );
}

/// Q00295 refuses a starter above level 15 (`addCondMaxLevel(15)`).
#[test]
fn quest_q00295_refused_above_level_15() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(1492, "Floating Stone", true)]);
    add_test_npc(&mut world, NPC_OID, 30536, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 16;
    let q = "Q00295_DreamingOfTheSkies";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30536-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-16 starter never begins"
    );
}

/// Q00262 refuses a starter above level 16 (`addCondMaxLevel(16)`).
#[test]
fn quest_q00262_refused_above_level_16() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_quest_items(&mut world, &[(707, "Spore Sac", true)]);
    add_test_npc(&mut world, NPC_OID, 30137, "Folk", 5, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 17;
    let q = "Q00262_TradeWithTheIvoryTower";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {q} 30137-03.htm")),
    );
    assert!(
        quest_cond(&world, 3001, q).is_none_or(|c| c == 0),
        "level-17 starter never begins"
    );
}
