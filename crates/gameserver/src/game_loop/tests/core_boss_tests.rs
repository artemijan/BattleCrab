//! Core — the script-spawned minions.

use super::*;

use crate::game_loop::core_boss::CORE;
use crate::game_loop::grand_boss::{ALIVE, DEAD};

const DEATH_KNIGHT: i32 = 29007;
const CORE_OID: i32 = NPC_OID + 80;

fn core_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind) in [(CORE, "GrandBoss"), (29007, "Monster"), (29008, "Monster"), (29011, "Monster")] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 50;
        t.base_hp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world.grand_bosses.insert(
        CORE,
        crate::model::grand_boss::GrandBoss {
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
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
        if x.npc_id == npc_id {
            n += 1;
        }
    });
    n
}

fn total_minions(world: &mut World) -> usize {
    let mut n = 0;
    world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
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
    crate::game_loop::core_boss::on_core_spawned(&mut world);

    assert_eq!(total_minions(&mut world), 3, "one of each type, not 19");
    for id in [29007, 29008, 29011] {
        assert_eq!(count_of(&mut world, id), 1, "exactly one npc {id}");
    }
}

/// A minion killed while Core lives comes back after its timer.
#[test]
fn a_minion_killed_while_core_lives_respawns() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world);
    assert_eq!(count_of(&mut world, DEATH_KNIGHT), 1);

    crate::game_loop::core_boss::on_minion_killed(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::handle_minion_respawn(&mut world, DEATH_KNIGHT);

    assert_eq!(count_of(&mut world, DEATH_KNIGHT), 2, "a replacement was placed");
}

/// **With Core dead, minions stop coming back** — Java guards the respawn on
/// `getStatus(CORE) == ALIVE`, so a cleared lair stays cleared.
#[test]
fn minions_do_not_respawn_once_core_is_dead() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world);
    world.grand_bosses.get_mut(&CORE).unwrap().status = DEAD;

    let before = count_of(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::on_minion_killed(&mut world, DEATH_KNIGHT);
    crate::game_loop::core_boss::handle_minion_respawn(&mut world, DEATH_KNIGHT);

    assert_eq!(count_of(&mut world, DEATH_KNIGHT), before, "no repopulating an empty lair");
}

/// Core dying clears its minions — otherwise the adds outlive the boss.
#[test]
fn core_dying_despawns_its_minions() {
    let (mut world, _db, _l) = core_world();
    crate::game_loop::core_boss::on_core_spawned(&mut world);
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
    crate::game_loop::core_boss::on_core_spawned(&mut world);

    crate::game_loop::core_boss::on_core_killed(&mut world);
    assert_eq!(total_minions(&mut world), 3, "still standing right after the kill");
}

/// An unrelated NPC is not treated as one of Core's minions.
#[test]
fn an_unrelated_npc_is_not_a_core_minion() {
    assert!(!crate::game_loop::core_boss::is_core_minion(12077), "a Wolf is not Core's add");
    assert!(crate::game_loop::core_boss::is_core_minion(DEATH_KNIGHT));
}
