//! Q00300-Q00399 — the hunting and collection quests.

use super::*;

#[test]
fn quest_q00344_1000_years_the_end_of_lamentation() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (4269, "Articles of Sacrifice", true),
            (4271, "Old Hilt", true),
            (1874, "Oriharukon Ore", false),
        ],
    );
    register_npc(&mut world, 20236, "Monster", 50);
    let gilmore = NPC_OID;
    let kaien = NPC_OID + 1;
    add_test_npc(&mut world, gilmore, 30754, "Folk", 50, 100, 0, 0);
    add_test_npc(&mut world, kaien, 30623, "Folk", 50, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 50;
    let q = "Q00344_1000YearsTheEndOfLamentation";
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Script {q}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Script {q} 30754-04.htm")),
    );
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    // Kill a Cave Servant → an Article.
    kill_mob(&mut world, NPC_OID + 10, 20236, 50, 0); // give_item_randomly roll_f64 (0.0 < 0.58)
    assert_eq!(
        item_count(&world, 3001, 4269),
        1,
        "Cave Servant drops an Article"
    );
    // Turn-in gamble: 5 articles, roll(1000)=0 < 5 → relic; roll(4)=0 → Old Hilt (memo 1, cond 2).
    inject(&mut world, 3001, 0x0344_0000, 4269, 4); // 5 total
    world.force_roll(0); // roll(1000) < count → relic path
    world.force_roll(0); // roll(4) → Old Hilt
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Script {q} 30754-08.html")),
    );
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(2),
        "relic jackpot → cond 2"
    );
    assert_eq!(
        item_count(&world, 3001, 4271),
        1,
        "received the Old Hilt relic"
    );
    assert_eq!(
        item_count(&world, 3001, 4269),
        0,
        "articles consumed by the turn-in"
    );
    // Kaien exchanges the Old Hilt: roll(100)=0 ≤ 52 → 25 Oriharukon Ore, back to cond 1.
    world.force_roll(0);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{kaien}_Script {q}")),
    );
    assert_eq!(
        item_count(&world, 3001, 1874),
        25,
        "Kaien pays 25 Oriharukon Ore for the hilt"
    );
    assert_eq!(item_count(&world, 3001, 4271), 0, "hilt consumed");
    assert_eq!(quest_cond(&world, 3001, q), Some(1), "back to collecting");
    // Adena path: 3 articles, roll(1000)=500 ≥ 3 → 3*60 = 180 adena.
    inject(&mut world, 3001, 0x0344_0001, 4269, 3);
    let a = item_count(&world, 3001, 57);
    world.force_roll(500);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{gilmore}_Script {q} 30754-08.html")),
    );
    assert_eq!(
        item_count(&world, 3001, 57),
        a + 180,
        "3 articles → 180 adena"
    );
    assert_eq!(item_count(&world, 3001, 4269), 0, "articles consumed");
}
