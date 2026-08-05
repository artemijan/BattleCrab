//! Auto play (`Custom/AutoPlay.ini`) slice 1 — the panel's toggles and the
//! play loop: target acquisition, the mode filter, respectful hunting and loot.

use super::*;

use crate::model::components::{AutoPlaySettings, Casting, TargetRef, Vitals};

const PLAYER: i32 = 3001;

fn play_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.id_pool = 0x4D00_0000..0x4D00_0100;
    world.cfg.auto_play.enabled = true;
    // The dist gates the panel on premium; the loop itself does not, so the
    // tests drive the loop directly and leave that gate to its own test.
    world.cfg.auto_play.premium_only = false;
    let rx = ingame_player(&mut world, 1, PLAYER, 0, 0, 0);
    (world, rx)
}

fn add_mob(world: &mut World, oid: i32, npc_id: i32, x: i32, y: i32) {
    add_test_npc(world, oid, npc_id, "Monster", 10, x, y, 0);
}

fn settings_of(world: &World) -> AutoPlaySettings {
    crate::game_loop::auto_play::settings(world, PLAYER).unwrap()
}

fn set(world: &mut World, f: impl FnOnce(&mut AutoPlaySettings)) {
    let mut s = settings_of(world);
    f(&mut s);
    world.objects.add_components(&PLAYER, s);
}

/// `.play <toggle>` flips exactly the setting it names, and `percent` clamps.
#[test]
fn the_panel_toggles_each_setting() {
    let (mut world, mut rx) = play_world();
    let play = |world: &mut World, arg: &str| {
        on_packet(
            world,
            1,
            [vec![cop::SAY2], say2_body(&format!(".play {arg}"), 0, None)].concat(),
        );
    };

    assert!(!settings_of(&world).pickup, "loot starts off");
    play(&mut world, "loot");
    assert!(settings_of(&world).pickup, "and toggles on");
    play(&mut world, "loot");
    assert!(!settings_of(&world).pickup, "and back off");

    assert!(settings_of(&world).auto_attack, "attack starts on");
    play(&mut world, "attack");
    assert!(!settings_of(&world).auto_attack);

    play(&mut world, "mode2");
    assert_eq!(settings_of(&world).next_target_mode, 2);
    play(&mut world, "respect");
    assert!(settings_of(&world).respectful_hunting);
    play(&mut world, "range");
    assert!(settings_of(&world).short_range);

    play(&mut world, "percent 250");
    assert_eq!(settings_of(&world).potion_percent, 100, "clamped high");
    play(&mut world, "percent -5");
    assert_eq!(settings_of(&world).potion_percent, 0, "clamped low");

    play(&mut world, "start");
    assert!(settings_of(&world).active);
    play(&mut world, "stop");
    assert!(!settings_of(&world).active);
    drain(&mut rx);
}

/// The loop finds the **nearest** valid monster and attacks it.
#[test]
fn the_loop_targets_the_nearest_monster() {
    let (mut world, _rx) = play_world();
    add_mob(&mut world, 4001, 20001, 500, 0);
    add_mob(&mut world, 4002, 20002, 200, 0);
    set(&mut world, |s| s.active = true);

    crate::game_loop::auto_play::tick(&mut world);

    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4002),
        "the closer of the two"
    );
}

/// A dead target is dropped and replaced on the next pass.
#[test]
fn a_dead_target_is_released() {
    let (mut world, _rx) = play_world();
    add_mob(&mut world, 4001, 20001, 200, 0);
    set(&mut world, |s| s.active = true);
    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4001)
    );

    world
        .objects
        .get_component_mut::<Vitals>(&4001)
        .unwrap()
        .dead = true;
    add_mob(&mut world, 4002, 20002, 400, 0);
    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4002),
        "moved on to the live one"
    );
}

/// `isRespectfulHunting`: a mob already fighting someone else is skipped.
#[test]
fn respectful_hunting_skips_a_busy_mob() {
    let (mut world, _rx) = play_world();
    add_mob(&mut world, 4001, 20001, 200, 0);
    add_mob(&mut world, 4002, 20002, 400, 0);
    // The near mob is fighting somebody else.
    world.objects.add_components(&4001, TargetRef(Some(9999)));
    set(&mut world, |s| {
        s.active = true;
        s.respectful_hunting = true;
    });

    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4002),
        "the busy one is left alone"
    );

    // Without the setting, the nearer (busy) mob is fair game again. The
    // current target has to go first: a *valid* target ends the pass early, so
    // the loop never re-scans while one is held.
    world.objects.add_components(&PLAYER, TargetRef(None));
    set(&mut world, |s| {
        s.active = true;
        s.respectful_hunting = false;
    });
    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4001)
    );
}

/// `isShortRange` halves the scan: a mob at 900 units is out of reach.
#[test]
fn short_range_limits_the_scan() {
    let (mut world, _rx) = play_world();
    add_mob(&mut world, 4001, 20001, 900, 0);
    set(&mut world, |s| {
        s.active = true;
        s.short_range = true;
    });

    crate::game_loop::auto_play::tick(&mut world);
    assert!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0)
            .is_none(),
        "900 > the 600-unit short range"
    );

    set(&mut world, |s| s.short_range = false);
    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4001),
        "and inside the 1400-unit long range"
    );
}

/// Mode 3 wants NPCs, so a monster no longer qualifies.
#[test]
fn the_target_mode_filters_what_counts() {
    let (mut world, _rx) = play_world();
    add_mob(&mut world, 4001, 20001, 200, 0);
    add_test_npc(&mut world, 4002, 30001, "Folk", 10, 300, 0, 0);
    set(&mut world, |s| {
        s.active = true;
        s.next_target_mode = 3;
    });

    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&PLAYER)
            .and_then(|t| t.0),
        Some(4002),
        "mode 3 takes the Folk, not the nearer monster"
    );
}

/// With `doPickup`, loot in reach is taken instead of a mob being chased.
#[test]
fn loot_in_reach_is_picked_up() {
    use crate::game_loop::ground_items::DropSource;
    use crate::model::inventory::Inventory;

    let (mut world, _rx) = play_world();
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = 57;
    t.name = "Adena".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    // Unowned loot 30 units away — inside the 70-unit reach.
    crate::game_loop::ground_items::spawn_ground_item(
        &mut world,
        57,
        100,
        0,
        30,
        0,
        0,
        0,
        DropSource::Npc,
    );
    set(&mut world, |s| {
        s.active = true;
        s.pickup = true;
    });

    crate::game_loop::auto_play::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .map_or(0, |i| i.count_of(57)),
        100,
        "the loop looted it"
    );
}

// ---------------------------------------------------------------------------
// Auto use (slice 2)
// ---------------------------------------------------------------------------

use crate::model::components::{AutoUseSettings, Buffs, SkillBook};
use crate::model::inventory::Inventory;

const SHOT: i32 = 1835;
const POTION: i32 = 1540;
const BUFF_SKILL: i32 = 1204;
const ATTACK_SKILL: i32 = 3;

fn use_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, rx) = play_world();
    for (item_id, skill_id) in [(SHOT, 9201), (POTION, 9202)] {
        let mut t = crate::data::item_data::ItemTemplate::default();
        t.item_id = item_id;
        t.name = format!("Item {item_id}");
        t.is_stackable = true;
        t.handler = crate::data::item_data::ItemHandler::ItemSkills;
        t.item_skills = vec![(skill_id, 1)];
        t.default_action = crate::data::item_data::ActionType::SkillReduce;
        t.immediate_effect = true;
        world.data.item_data.insert_for_test(t);
        world
            .data
            .skill_data
            .insert_for_test(crate::model::skill::Skill {
                self_continuous: false,
                id: skill_id,
                level: 1,
                name: format!("Effect {skill_id}"),
                ..Default::default()
            });
    }
    {
        let v = world.objects.get_component_mut::<Vitals>(&PLAYER).unwrap();
        v.max_hp = 1000;
        v.cur_hp = 1000.0;
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

fn auto_use(world: &World) -> AutoUseSettings {
    crate::game_loop::auto_use::settings(world, PLAYER)
}

/// A supply item is used; one the player no longer carries is **dropped from
/// the list** rather than retried forever.
#[test]
fn supply_items_are_used_and_missing_ones_forgotten() {
    let (mut world, _rx) = use_world();
    give(&mut world, SHOT, 2, 0x4D00_0010);
    set(&mut world, |s| s.active = true);
    world.objects.add_components(
        &PLAYER,
        AutoUseSettings {
            supply_items: vec![SHOT],
            ..Default::default()
        },
    );

    crate::game_loop::auto_use::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .map_or(0, |i| i.count_of(SHOT)),
        1,
        "one was used"
    );

    // Run the bag dry, then tick again: the id leaves the list.
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .remove_item(SHOT, 1);
    crate::game_loop::auto_use::tick(&mut world);
    assert!(
        auto_use(&world).supply_items.is_empty(),
        "a vanished item is forgotten, not retried"
    );
}

/// The potion drinks below the configured percentage, and the slot clears when
/// the potion runs out.
#[test]
fn the_potion_drinks_below_the_threshold() {
    let (mut world, _rx) = use_world();
    give(&mut world, POTION, 1, 0x4D00_0020);
    set(&mut world, |s| {
        s.active = true;
        s.potion_percent = 70;
    });
    world.objects.add_components(
        &PLAYER,
        AutoUseSettings {
            potion_item: POTION,
            ..Default::default()
        },
    );

    // Full HP: nothing happens.
    crate::game_loop::auto_use::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .map_or(0, |i| i.count_of(POTION)),
        1
    );

    world
        .objects
        .get_component_mut::<Vitals>(&PLAYER)
        .unwrap()
        .cur_hp = 500.0;
    crate::game_loop::auto_use::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .map_or(0, |i| i.count_of(POTION)),
        0,
        "drunk at 50 % of 1000"
    );

    // With none left, the slot empties itself.
    crate::game_loop::auto_use::tick(&mut world);
    assert_eq!(auto_use(&world).potion_item, 0);
}

/// **Buffs run in town; supply items do not.** That asymmetry is the point of
/// the peace-zone gate — pre-buff at the fountain, shots only in the field.
#[test]
fn a_peace_zone_stops_items_but_not_buffs() {
    let (mut world, _rx) = use_world();
    give(&mut world, SHOT, 2, 0x4D00_0030);
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: BUFF_SKILL,
            level: 1,
            name: "Wind Walk".into(),
            target_type: crate::model::skill::TargetType::Self_,
            ..Default::default()
        });
    world
        .objects
        .get_component_mut::<SkillBook>(&PLAYER)
        .unwrap()
        .0
        .insert(BUFF_SKILL, 1);
    set(&mut world, |s| s.active = true);
    world.objects.add_components(
        &PLAYER,
        AutoUseSettings {
            buffs: vec![BUFF_SKILL],
            supply_items: vec![SHOT],
            ..Default::default()
        },
    );
    world
        .objects
        .get_component_mut::<crate::model::components::ZoneFlags>(&PLAYER)
        .unwrap()
        .mask |= crate::data::zone_data::ZoneKind::Peace.bit();

    crate::game_loop::auto_use::tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .map_or(0, |i| i.count_of(SHOT)),
        2,
        "no shots in town"
    );
    assert!(
        world.objects.has_component::<Casting>(&PLAYER)
            || world
                .objects
                .get_component::<Buffs>(&PLAYER)
                .is_some_and(|b| !b.0.is_empty()),
        "but the buff still goes up"
    );
}

/// A buff already up is skipped, and a skill the player has forgotten leaves
/// the list.
#[test]
fn a_running_buff_is_skipped_and_unknown_skills_forgotten() {
    let (mut world, _rx) = use_world();
    set(&mut world, |s| s.active = true);
    world.objects.add_components(
        &PLAYER,
        AutoUseSettings {
            buffs: vec![BUFF_SKILL],
            skills: vec![ATTACK_SKILL],
            ..Default::default()
        },
    );

    // Neither skill is known → both lists self-clean.
    crate::game_loop::auto_use::tick(&mut world);
    assert!(auto_use(&world).buffs.is_empty(), "unknown buff forgotten");
}

/// `.playskills <id>` files a **self-target** skill under buffs and everything
/// else under attack skills, and a second press removes it.
#[test]
fn the_skill_page_sorts_buffs_from_attack_skills() {
    let (mut world, _rx) = use_world();
    for (id, target) in [
        (BUFF_SKILL, crate::model::skill::TargetType::Self_),
        (ATTACK_SKILL, crate::model::skill::TargetType::Enemy),
    ] {
        world
            .data
            .skill_data
            .insert_for_test(crate::model::skill::Skill {
                self_continuous: false,
                id,
                level: 1,
                name: format!("Skill {id}"),
                target_type: target,
                ..Default::default()
            });
        world
            .objects
            .get_component_mut::<SkillBook>(&PLAYER)
            .unwrap()
            .0
            .insert(id, 1);
    }
    let play = |world: &mut World, text: &str| {
        on_packet(
            world,
            1,
            [vec![cop::SAY2], say2_body(text, 0, None)].concat(),
        );
    };

    play(&mut world, &format!(".playskills {BUFF_SKILL}"));
    assert_eq!(
        auto_use(&world).buffs,
        vec![BUFF_SKILL],
        "self-target → buff"
    );
    assert!(auto_use(&world).skills.is_empty());

    play(&mut world, &format!(".playskills {ATTACK_SKILL}"));
    assert_eq!(
        auto_use(&world).skills,
        vec![ATTACK_SKILL],
        "→ attack skill"
    );

    // Pressing the same row again removes it.
    play(&mut world, &format!(".playskills {BUFF_SKILL}"));
    assert!(auto_use(&world).buffs.is_empty(), "toggled back off");
}

/// The potion page is a single slot: choosing a second potion replaces the
/// first, and choosing the current one clears it.
#[test]
fn the_potion_page_is_one_slot() {
    let (mut world, _rx) = use_world();
    give(&mut world, POTION, 1, 0x4D00_0040);
    give(&mut world, SHOT, 1, 0x4D00_0041);
    let play = |world: &mut World, text: &str| {
        on_packet(
            world,
            1,
            [vec![cop::SAY2], say2_body(text, 0, None)].concat(),
        );
    };

    play(&mut world, &format!(".playpotion {POTION}"));
    assert_eq!(auto_use(&world).potion_item, POTION);
    play(&mut world, &format!(".playpotion {SHOT}"));
    assert_eq!(auto_use(&world).potion_item, SHOT, "replaced, not added");
    play(&mut world, &format!(".playpotion {SHOT}"));
    assert_eq!(auto_use(&world).potion_item, 0, "same row clears it");
}
