//! Antharas — the escalating, capped minion waves.

use super::*;

use crate::game_loop::antharas::{AntharasMinions, ANTHARAS};

const ANTHARAS_OID: i32 = NPC_OID + 120;
const BEHEMOTH: i32 = 29069;
const TERASQUE: i32 = 29190;

fn antharas_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(ANTHARAS, "GrandBoss"), (BEHEMOTH, "Monster"), (TERASQUE, "Monster")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 85;
        t.base_hp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

fn spawned(world: &mut World) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == BEHEMOTH || x.npc_id == TERASQUE {
            n += 1;
        }
    });
    n
}

fn state(world: &World) -> AntharasMinions {
    *world.objects.get_component::<AntharasMinions>(&ANTHARAS_OID).unwrap()
}

fn set_state(world: &mut World, count: i32, multiplier: i32) {
    world.objects.add_components(&ANTHARAS_OID, AntharasMinions { count, multiplier });
}

/// The first wave is a **single pair** — the multiplier starts at 1, so
/// Antharas opens gently and escalates.
#[test]
fn the_first_wave_is_one_pair() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    crate::game_loop::antharas::begin_waves(&mut world, ANTHARAS_OID);

    world.forced_rolls.push_back(50); // > 10: the multiplier grows
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 2, "one Behemoth and one Tarask");
    assert_eq!(state(&world).multiplier, 2, "and the next wave will be bigger");
}

/// **Waves grow to a cap of 4** (eight adds), not without bound.
#[test]
fn the_multiplier_stops_at_four() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 0, 4);

    world.forced_rolls.push_back(50); // would grow, if it could
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(state(&world).multiplier, 4, "capped");
    assert_eq!(spawned(&mut world), 8, "four pairs is the largest wave");
}

/// A low roll leaves the multiplier alone — growth is ~89% per wave, not
/// guaranteed.
#[test]
fn a_low_roll_does_not_grow_the_wave() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 0, 1);

    world.forced_rolls.push_back(5); // not > 10
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(state(&world).multiplier, 1, "still one pair next time");
}

/// **Near the cap, a full wave gives way to a single pair** — the ladder's
/// second step, which keeps the population from overshooting 100.
#[test]
fn a_full_wave_gives_way_to_a_pair_near_the_cap() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    // multiplier 4 would want 8, but 100 - 8 = 92 and we are past it.
    set_state(&mut world, 95, 4);

    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 2, "one pair, not eight");
    assert_eq!(state(&world).count, 97);
}

/// **At 98 the last slot is filled by a single, randomly chosen dragon** —
/// the ladder's third step. Collapsing the ladder to "a pair if there's room
/// for two" would stall the lair at 98 and lose this.
#[test]
fn the_last_slot_takes_one_random_dragon() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 98, 1);

    world.forced_rolls.push_back(0); // picks Behemoth
    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 1, "exactly one more");
    assert_eq!(state(&world).count, 99, "filling the lair to 99");
}

/// At the cap, a wave adds nothing — but still rearms, so the fight recovers
/// as adds are killed.
#[test]
fn a_full_lair_spawns_nothing_but_keeps_ticking() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    set_state(&mut world, 99, 1);

    let before = world.scheduler.len();
    world.forced_rolls.push_back(5);
    crate::game_loop::antharas::handle_wave(&mut world, ANTHARAS_OID);
    assert_eq!(spawned(&mut world), 0, "the lair is full");
    assert_eq!(world.scheduler.len(), before + 1, "and the next wave is still armed");
}
