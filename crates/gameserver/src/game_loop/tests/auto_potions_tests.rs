//! Auto potions (`Custom/AutoPotions.ini`) — the `.apon` loop: who joins it,
//! when it drinks, which potion it picks, and who it drops.

use super::*;

use crate::model::components::{PlayerVitals, Vitals};
use crate::model::inventory::Inventory;

const PLAYER: i32 = 3001;
/// Two HP potions, in preference order, plus one MP potion.
const HP_GOOD: i32 = 1540;
const HP_CHEAP: i32 = 1061;
const MP_POTION: i32 = 728;

/// A world with the loop configured and the player holding potions.
fn potion_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.id_pool = 0x4B00_0000..0x4B00_0100;
    let cfg = &mut world.cfg.auto_potions;
    cfg.enabled = true;
    cfg.minimum_level = 1;
    cfg.hp.enabled = true;
    cfg.hp.percentage = 70;
    cfg.hp.item_ids = vec![HP_GOOD, HP_CHEAP];
    cfg.mp.enabled = true;
    cfg.mp.percentage = 30;
    cfg.mp.item_ids = vec![MP_POTION];
    cfg.cp.enabled = false;

    // Potions that restore through an item skill, like the real ones.
    for (item_id, skill_id) in [(HP_GOOD, 9101), (HP_CHEAP, 9102), (MP_POTION, 9103)] {
        let mut t = crate::data::item_data::ItemTemplate::default();
        t.item_id = item_id;
        t.name = format!("Potion {item_id}");
        t.is_stackable = true;
        t.handler = crate::data::item_data::ItemHandler::ItemSkills;
        t.item_skills = vec![(skill_id, 1)];
        // The dist's potions: `default_action = SKILL_REDUCE` +
        // `immediate_effect`, which is what makes the handler consume one.
        t.default_action = crate::data::item_data::ActionType::SkillReduce;
        t.immediate_effect = true;
        world.data.item_data.insert_for_test(t);
        world
            .data
            .skill_data
            .insert_for_test(crate::model::skill::Skill {
                id: skill_id,
                level: 1,
                name: format!("Restore {item_id}"),
                ..Default::default()
            });
    }

    let rx = ingame_player(&mut world, 1, PLAYER, 0, 0, 0);
    {
        let v = world.objects.get_component_mut::<Vitals>(&PLAYER).unwrap();
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
        v.max_mp = 500;
        v.cur_mp = 500.0;
    }
    (world, rx)
}

fn give(world: &mut World, item_id: i32, count: i64, obj_id: i32) {
    let World { objects, data, .. } = world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, obj_id, item_id, count);
}

fn count_of(world: &World, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .map_or(0, |i| i.count_of(item_id))
}

/// `.apon` joins the loop, `.apoff` leaves it, and the level gate refuses.
#[test]
fn the_voiced_commands_toggle_membership() {
    let (mut world, mut rx) = potion_world();
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".apon", 0, None)].concat(),
    );
    assert!(world.auto_potion_players.contains(&PLAYER), "joined");
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("Auto potions is enabled")),
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".apoff", 0, None)].concat(),
    );
    assert!(!world.auto_potion_players.contains(&PLAYER), "left");

    // Below the minimum level, the command is refused.
    world.cfg.auto_potions.minimum_level = 50;
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [vec![cop::SAY2], say2_body(".apon", 0, None)].concat(),
    );
    assert!(!world.auto_potion_players.contains(&PLAYER), "too low");
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("need to be at least 50")),
    );
}

/// The tick drinks only when the pool is actually low, and takes the **first**
/// carried item from the preference list.
#[test]
fn the_tick_drinks_the_preferred_potion_when_low() {
    let (mut world, mut rx) = potion_world();
    give(&mut world, HP_GOOD, 5, 0x4B00_0010);
    give(&mut world, HP_CHEAP, 5, 0x4B00_0011);
    give(&mut world, MP_POTION, 5, 0x4B00_0012);
    world.auto_potion_players.insert(PLAYER);
    drain(&mut rx);

    // Full pools: nothing is drunk, and the "out of potions" line stays quiet
    // because the player *carries* some.
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(count_of(&world, HP_GOOD), 5, "no drink at full HP");
    assert!(
        !drain(&mut rx)
            .iter()
            .filter_map(|p| sysmsg_text(p))
            .any(|t| t.contains("out of potions")),
        "carrying potions keeps it quiet"
    );

    // Drop below 70 % HP: the preferred potion goes first.
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp = 500.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(count_of(&world, HP_GOOD), 4, "the preferred potion");
    assert_eq!(count_of(&world, HP_CHEAP), 5, "the fallback is untouched");

    // With the preferred one gone, the fallback is used.
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .remove_item(HP_GOOD, 4);
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp = 500.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(count_of(&world, HP_CHEAP), 4, "falls through the list");

    // MP has its own, lower threshold: 50 % is fine, 20 % is not.
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_mp = 250.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(
        count_of(&world, MP_POTION),
        5,
        "50 % is above the 30 % mark"
    );
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_mp = 100.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(count_of(&world, MP_POTION), 4, "20 % is below it");
}

/// **Java's noisy message, kept verbatim:** carrying no configured potion at
/// all is reported every tick, whether or not anything needed restoring.
#[test]
fn an_empty_bag_is_reported_every_tick() {
    let (mut world, mut rx) = potion_world();
    world.auto_potion_players.insert(PLAYER);
    drain(&mut rx);

    for _ in 0..2 {
        crate::game_loop::auto_potions::tick(&mut world);
        assert!(
            drain(&mut rx)
                .iter()
                .filter_map(|p| sysmsg_text(p))
                .any(|t| t.contains("out of potions")),
            "told again, at full health, once per tick"
        );
    }
}

/// The sweep **drops** a dead player rather than skipping them, so reviving
/// does not silently resume the loop.
#[test]
fn death_removes_the_player_from_the_loop() {
    let (mut world, _rx) = potion_world();
    give(&mut world, HP_GOOD, 5, 0x4B00_0013);
    world.auto_potion_players.insert(PLAYER);

    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .dead = true;
    crate::game_loop::auto_potions::tick(&mut world);
    assert!(
        !world.auto_potion_players.contains(&PLAYER),
        "dropped, not skipped"
    );

    // Reviving does not put them back.
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .dead = false;
    crate::game_loop::auto_potions::tick(&mut world);
    assert!(!world.auto_potion_players.contains(&PLAYER));
}

/// `AutoPotionsInOlympiad = false` here: a player entering a match is dropped.
#[test]
fn an_olympiad_competitor_is_dropped() {
    let (mut world, _rx) = potion_world();
    give(&mut world, HP_GOOD, 5, 0x4B00_0014);
    world.cfg.auto_potions.in_olympiad = false;
    world.auto_potion_players.insert(PLAYER);
    world.olympiad.in_competition.insert(PLAYER);

    crate::game_loop::auto_potions::tick(&mut world);
    assert!(!world.auto_potion_players.contains(&PLAYER));

    // With the flag on, the same player stays and is topped up.
    world.cfg.auto_potions.in_olympiad = true;
    world.auto_potion_players.insert(PLAYER);
    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp = 100.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert!(world.auto_potion_players.contains(&PLAYER), "kept");
    assert_eq!(count_of(&world, HP_GOOD), 4, "and drinks in the arena");
}

/// CP is watched too, through its own component and its own threshold.
#[test]
fn cp_has_its_own_pool() {
    const CP_POTION: i32 = 5592;
    let (mut world, _rx) = potion_world();
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = CP_POTION;
    t.name = "CP Potion".into();
    t.is_stackable = true;
    t.handler = crate::data::item_data::ItemHandler::ItemSkills;
    t.item_skills = vec![(9104, 1)];
    t.default_action = crate::data::item_data::ActionType::SkillReduce;
    t.immediate_effect = true;
    world.data.item_data.insert_for_test(t);
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            id: 9104,
            level: 1,
            name: "Restore CP".into(),
            ..Default::default()
        });
    world.cfg.auto_potions.cp.enabled = true;
    world.cfg.auto_potions.cp.percentage = 70;
    world.cfg.auto_potions.cp.item_ids = vec![CP_POTION];
    give(&mut world, CP_POTION, 5, 0x4B00_0015);
    world.auto_potion_players.insert(PLAYER);
    {
        let pv = world
            .objects
            .get_component_mut::<PlayerVitals>(&PLAYER)
            .unwrap();
        pv.max_cp = 200;
        pv.cur_cp = 200.0;
    }

    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(count_of(&world, CP_POTION), 5, "full CP, no drink");

    world
        .objects
        .get_component_mut::<PlayerVitals>(&PLAYER)
        .unwrap()
        .cur_cp = 50.0;
    crate::game_loop::auto_potions::tick(&mut world);
    assert_eq!(
        count_of(&world, CP_POTION),
        4,
        "25 % is below the 70 % mark"
    );
}
