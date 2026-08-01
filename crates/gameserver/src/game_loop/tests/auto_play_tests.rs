//! Auto play (`Custom/AutoPlay.ini`) slice 1 — the panel's toggles and the
//! play loop: target acquisition, the mode filter, respectful hunting and loot.

use super::*;

use crate::model::components::{AutoPlaySettings, TargetRef, Vitals};

const PLAYER: i32 = 3001;

fn play_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
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
