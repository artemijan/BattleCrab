//! Core — the script-spawned minions.

use super::*;

use crate::game_loop::core_boss::CORE;
use crate::game_loop::grand_boss::{ALIVE, DEAD};

const DEATH_KNIGHT: i32 = 29007;
const CORE_OID: i32 = NPC_OID + 80;

fn core_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [
        (CORE, "GrandBoss"),
        (29007, "Monster"),
        (29008, "Monster"),
        (29011, "Monster"),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 50;
        t.base_hp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.grand_bosses.insert(
        CORE,
        model::grand_boss::GrandBoss {
            boss_id: CORE,
            loc_x: 17726,
            loc_y: 108299,
            loc_z: -6488,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: ALIVE,
        },
    );
    (world, db, l)
}

fn count_of(world: &mut World, npc_id: i32) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&model::npc::Npc>(|x| {
        if x.npc_id == npc_id {
            n += 1;
        }
    });
    n
}

fn total_minions(world: &mut World) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&model::npc::Npc>(|x| {
        if crate::game_loop::core_boss::is_core_minion(x.npc_id) {
            n += 1;
        }
    });
    n
}

/// **Core spawns three minions, not nineteen.** Java's `MINNION_SPAWNS` is a
/// `Map<Integer, Location>` with 19 `put`s keyed by npc id, so each type keeps
/// only its last location. Reading the 19 as a list would give Core six times
/// the adds and a different fight entirely.
#[test]
fn core_spawns_three_minions_not_nineteen() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);

    assert_eq!(total_minions(&mut world), 3, "one of each type, not 19");
    for id in [29007, 29008, 29011] {
        assert_eq!(count_of(&mut world, id), 1, "exactly one npc {id}");
    }
}

/// Core is a stationary generator: it may not move (so it never chases), but
/// its actions aren't blocked — it still melees adjacent attackers.
#[test]
fn core_is_immobilized_but_can_still_act() {
    let (mut world, _db, _l) = core_world();
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 0, 0, 0);
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);

    assert!(
        abnormal::is_movement_disabled(&world, CORE_OID),
        "Core is rooted to its spawn"
    );
    assert!(
        !abnormal::is_control_blocked(&world, CORE_OID),
        "but it can still fight"
    );
}

/// A minion killed while Core lives comes back after its timer.
#[test]
fn a_minion_killed_while_core_lives_respawns() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);
    assert_eq!(count_of(&mut world, DEATH_KNIGHT), 1);

    crate::game_loop::core_boss::on_minion_killed(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::handle_minion_respawn(&mut world, DEATH_KNIGHT);

    assert_eq!(
        count_of(&mut world, DEATH_KNIGHT),
        2,
        "a replacement was placed"
    );
}

/// **With Core dead, minions stop coming back** — Java guards the respawn on
/// `getStatus(CORE) == ALIVE`, so a cleared lair stays cleared.
#[test]
fn minions_do_not_respawn_once_core_is_dead() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);
    world.grand_bosses.get_mut(&CORE).unwrap().status = DEAD;

    let before = count_of(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::on_minion_killed(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::handle_minion_respawn(&mut world, DEATH_KNIGHT);

    assert_eq!(
        count_of(&mut world, DEATH_KNIGHT),
        before,
        "no repopulating an empty lair"
    );
}

/// Core dying clears its minions — otherwise the adds outlive the boss.
#[test]
fn core_dying_despawns_its_minions() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 0, 0, 0);
    assert_eq!(total_minions(&mut world), 3);

    crate::game_loop::core_boss::handle_despawn_minions(&mut world);
    assert_eq!(total_minions(&mut world), 0, "the lair is cleared");
}

/// The despawn is **delayed**, not immediate: Java gives it 20 s, so the adds
/// linger briefly after the kill rather than vanishing mid-animation.
#[test]
fn the_despawn_is_scheduled_rather_than_immediate() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);

    crate::game_loop::core_boss::on_core_killed(&mut world);
    assert_eq!(
        total_minions(&mut world),
        3,
        "still standing right after the kill"
    );
}

/// An unrelated NPC is not treated as one of Core's minions.
#[test]
fn an_unrelated_npc_is_not_a_core_minion() {
    assert!(
        !crate::game_loop::core_boss::is_core_minion(12077),
        "a Wolf is not Core's add"
    );
    assert!(crate::game_loop::core_boss::is_core_minion(DEATH_KNIGHT));
}

// ---------------------------------------------------------------------------
// Barks (slice 9)
// ---------------------------------------------------------------------------

/// `NpcSay` is opcode 0x30 — the packet Core's lines ride on.
fn count_npc_say(rx: &mut UnboundedReceiver<bytes::Bytes>) -> usize {
    let mut n = 0;
    while let Ok(p) = rx.try_recv() {
        if p.first() == Some(&0x30) {
            n += 1;
        }
    }
    n
}

/// The **first** hit of a life plays both intro lines; later hits do not
/// replay them.
#[test]
fn core_says_its_intro_once_per_life() {
    let (mut world, _db, _l) = core_world();
    let mut rx = ingame_caster(&mut world, 1, 9980, 0, 0);
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 20, 0, 0);
    while rx.try_recv().is_ok() {}

    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert_eq!(
        count_npc_say(&mut rx),
        2,
        "both intro lines on the first hit"
    );

    // A later hit: force the taunt roll to fail so only the intro could speak.
    world.force_roll(7);
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert_eq!(count_npc_say(&mut rx), 0, "the intro does not replay");
}

/// The taunt is 1-in-100, not every swing — forced both ways so the mechanic
/// rather than the RNG is under test.
#[test]
fn the_taunt_is_rare() {
    let (mut world, _db, _l) = core_world();
    let mut rx = ingame_caster(&mut world, 1, 9980, 0, 0);
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 20, 0, 0);
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID); // consume the intro
    while rx.try_recv().is_ok() {}

    world.force_roll(0); // the 1-in-100 hit
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert_eq!(count_npc_say(&mut rx), 1, "taunted");

    world.force_roll(50); // a miss
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert_eq!(count_npc_say(&mut rx), 0, "silent");
}

/// Dying resets the intro, so the next Core greets its killers afresh rather
/// than staying quiet forever after the first pull of the server's life.
#[test]
fn dying_resets_the_intro() {
    let (mut world, _db, _l) = core_world();
    let mut rx = ingame_caster(&mut world, 1, 9980, 0, 0);
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 20, 0, 0);
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    while rx.try_recv().is_ok() {}

    crate::game_loop::core_boss::on_core_killed(&mut world);
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert_eq!(count_npc_say(&mut rx), 2, "the intro plays again next life");
}

/// Core's death lines are said.
#[test]
fn core_says_its_death_lines() {
    let (mut world, _db, _l) = core_world();
    let mut rx = ingame_caster(&mut world, 1, 9980, 0, 0);
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 20, 0, 0);
    while rx.try_recv().is_ok() {}

    crate::game_loop::core_boss::say_death_lines(&mut world, CORE_OID);
    assert_eq!(count_npc_say(&mut rx), 2, "both death lines");
}

// ---------------------------------------------------------------------------
// `Core_Attacked` persistence (Java `Core.onSave` / spawn restore)
// ---------------------------------------------------------------------------

use crate::game_loop::global_vars;

/// The first hit stamps `Core_Attacked`, and the value is written through to
/// the DB rather than kept only in memory.
#[test]
fn the_first_attack_persists_core_attacked() {
    let (mut world, mut db_rx, _l) = core_world();
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 0, 0, 0);
    drain_db(&mut db_rx);

    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);

    assert!(
        global_vars::get_bool(
            &world,
            crate::game_loop::core_boss::CORE_ATTACKED_VAR,
            false
        ),
        "the flag is set in memory"
    );
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::SaveGlobalVariable { var, value }
                if var == crate::game_loop::core_boss::CORE_ATTACKED_VAR && value == "true"
        )),
        "and written through to global_variables"
    );
}

/// Spawning restores it, so a restart between the intro and the kill does not
/// replay the intro lines. This is the whole point of persisting the flag —
/// without the restore the variable would be written and never read.
#[test]
fn spawning_restores_core_attacked_from_the_stored_variable() {
    let (mut world, _db, _l) = core_world();
    global_vars::set(
        &mut world,
        crate::game_loop::core_boss::CORE_ATTACKED_VAR,
        true,
    );
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 0, 0, 0);

    crate::game_loop::core_boss::on_core_spawned(&mut world, CORE_OID);

    assert!(
        world
            .objects
            .get_component::<crate::game_loop::core_boss::CoreState>(&CORE_OID)
            .is_some_and(|s| s.first_attacked),
        "a Core that was already provoked stays provoked across a restart"
    );
}

/// Dying clears it — the intro plays again next life, and the *stored* value
/// has to follow or the next Core would spawn permanently silent.
#[test]
fn dying_clears_the_stored_flag() {
    let (mut world, _db, _l) = core_world();
    add_test_npc(&mut world, CORE_OID, CORE, "GrandBoss", 50, 0, 0, 0);
    crate::game_loop::core_boss::on_core_attacked(&mut world, CORE_OID);
    assert!(global_vars::get_bool(
        &world,
        crate::game_loop::core_boss::CORE_ATTACKED_VAR,
        false
    ));

    crate::game_loop::core_boss::on_core_killed(&mut world);

    assert!(
        !global_vars::get_bool(
            &world,
            crate::game_loop::core_boss::CORE_ATTACKED_VAR,
            false
        ),
        "the stored flag is cleared, not just the in-memory one"
    );
}
