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

// ---------------------------------------------------------------------------
// The entry cinematic (slice 17)
// ---------------------------------------------------------------------------

fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, opcode: u8) -> usize {
    let mut n = 0;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&opcode) {
            n += 1;
        }
    }
    n
}

/// **Antharas chains, Valakas does not.** Each beat schedules exactly the next
/// one, so at any moment only a single cinematic timer is pending — unlike
/// Valakas, which arms all ten up front. Reusing the Valakas shape here would
/// have quietly changed the timing model, so the difference is pinned.
#[test]
fn the_cinematic_is_a_chain_not_a_batch() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);

    let before = world.scheduler.len();
    crate::game_loop::antharas::begin_cinematic(&mut world, ANTHARAS_OID);
    assert_eq!(world.scheduler.len() - before, 1, "one beat armed, not five");
}

/// Each beat sends its camera shot and arms the next.
#[test]
fn each_beat_sends_a_shot_and_arms_the_next() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    let before = world.scheduler.len();
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 0);
    assert_eq!(drain(&mut rx, 0xD6), 1, "one camera shot");
    assert_eq!(world.scheduler.len(), before + 1, "and the next beat armed");
}

/// **Beat 3 forks**: it roars *and* schedules a second social action 5.2 s
/// later, independent of the camera chain — the only beat that arms two
/// timers, which a uniform "each beat arms one" port would lose.
#[test]
fn the_third_beat_forks_a_second_social() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    let before = world.scheduler.len();
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 2);
    assert_eq!(world.scheduler.len(), before + 2, "the next beat *and* the forked social");
    // `SocialAction` is 0x27 — the roar goes out with the shot, not only the
    // deferred one 5.2 s later.
    assert_eq!(drain(&mut rx, 0x27), 1, "the roar accompanied the camera shot");
}

/// The forked social fires on its own, after the chain has moved on.
#[test]
fn the_forked_social_fires_independently() {
    let (mut world, _db, _l) = antharas_world();
    let mut rx = ingame_caster(&mut world, 1, 9960, 0, 0);
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    while rx.try_recv().is_ok() {}

    crate::game_loop::antharas::handle_social(&mut world, ANTHARAS_OID);
    assert_eq!(drain(&mut rx, 0x27), 1, "the second social went out by itself");
}

/// The tail hands Antharas his AI back and **starts the minion waves** — so a
/// boss standing in its lair un-engaged is not already spawning adds.
#[test]
fn the_cinematic_tail_starts_the_waves() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);

    // One past the last camera beat is `START_MOVE`.
    crate::game_loop::antharas::handle_cinematic_step(&mut world, ANTHARAS_OID, 5);
    assert!(
        world.objects.get_component::<AntharasMinions>(&ANTHARAS_OID).is_some(),
        "the wave state exists, so the waves are running"
    );
}

/// Spawning Antharas runs the cinematic rather than going straight to waves —
/// the ordering that keeps an un-engaged boss quiet.
#[test]
fn spawning_starts_the_cinematic_not_the_waves() {
    let (mut world, _db, _l) = antharas_world();
    add_test_npc(&mut world, ANTHARAS_OID, ANTHARAS, "GrandBoss", 85, 0, 0, 0);
    crate::game_loop::antharas::begin_cinematic(&mut world, ANTHARAS_OID);

    assert!(
        world.objects.get_component::<AntharasMinions>(&ANTHARAS_OID).is_none(),
        "no adds before the fight begins"
    );
}
