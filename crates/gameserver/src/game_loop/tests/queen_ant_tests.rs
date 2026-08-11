//! Queen Ant — the larva and the nurse rotation.

use super::*;
use crate::game_loop::queen_ant;

use crate::game_loop::queen_ant::{LARVA, NURSE, QUEEN};

const QUEEN_OID: i32 = NPC_OID + 70;
/// Both heal skills are "Recovery"; the larva gets either, the Queen only 4020.
const HEAL1: i32 = 4020;
const HEAL2: i32 = 4024;

fn queen_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    for (id, kind, hp) in [
        (QUEEN, "GrandBoss", 100_000.0),
        (LARVA, "Monster", 50_000.0),
        (NURSE, "Monster", 5_000.0),
    ] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = kind.into();
        t.level = 40;
        t.base_hp_max = hp;
        t.base_mp_max = 10_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    for id in [HEAL1, HEAL2] {
        world
            .data
            .skill_data
            .insert_for_test(crate::model::skill::Skill {
                self_continuous: false,
                id,
                level: 1,
                magic_type: 1,
                effects: vec![crate::model::skill::SkillEffect::Heal { power: 1000.0 }],
                ..Default::default()
            });
    }
    (world, db, l)
}

fn spawn_nurse(world: &mut World, oid: i32, master: i32) {
    add_test_npc(world, oid, NURSE, "Monster", 40, 20, 0, 0);
    world
        .objects
        .add_components(&oid, crate::game_loop::minions::MinionOf(master));
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
    }
}

/// `add_test_npc` gives every NPC 100 HP regardless of its template, so a test
/// that sets an absolute `cur_hp` can silently set it **above** max — which
/// reads as "not wounded" and makes the whole assertion vacuous. Wound by
/// fraction instead.
fn wound_to_half(world: &mut World, oid: i32) -> f64 {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = v.max_hp as f64 / 2.0;
    v.cur_hp
}

fn hp(world: &World, oid: i32) -> f64 {
    world.objects.get_component::<Vitals>(&oid).unwrap().cur_hp
}

/// The Queen brings out her larva when she spawns — it is script-spawned, not
/// a minion, so nothing else would place it.
#[test]
fn the_queen_spawns_her_larva() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);

    crate::game_loop::queen_ant::on_queen_spawned(&mut world, QUEEN_OID);
    assert!(
        find_npc_object_id(&mut world, LARVA).is_some(),
        "the larva is out"
    );
}

/// Regression: the Queen must bring her **escort** — Java `spawnMinions
/// ("Privates")` = six nurses + eight royal guards. The grand-boss spawn path
/// skips a leader's `<minions>`, so without the script hook she stood alone
/// (the "only guards, no Queen fight" bug).
#[test]
fn the_queen_spawns_her_nurses_and_royal_guards() {
    const ROYAL_GUARD: i32 = 29005;
    let (mut world, _db, _l) = queen_world();
    // Declare the Queen's Privates and register the guard template.
    let mut queen = crate::data::npc_data::default_template(QUEEN);
    queen.type_name = "GrandBoss".into();
    queen.base_hp_max = 100_000.0;
    queen.minions = vec![
        crate::data::npc_data::MinionHolder {
            npc_id: NURSE,
            count: 6,
            group: "Privates".into(),
        },
        crate::data::npc_data::MinionHolder {
            npc_id: ROYAL_GUARD,
            count: 8,
            group: "Privates".into(),
        },
    ];
    world.data.npc_data.insert_for_test(queen);
    let mut guard = crate::data::npc_data::default_template(ROYAL_GUARD);
    guard.type_name = "Monster".into();
    guard.base_hp_max = 5_000.0;
    world.data.npc_data.insert_for_test(guard);

    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    crate::game_loop::queen_ant::on_queen_spawned(&mut world, QUEEN_OID);

    let count = |world: &mut World, npc_id: i32| {
        let mut n = 0;
        world.objects.for_each_mut::<&crate::model::npc::Npc>(|x| {
            if x.npc_id == npc_id {
                n += 1;
            }
        });
        n
    };
    assert_eq!(count(&mut world, NURSE), 6, "six nurses at her side");
    assert_eq!(count(&mut world, ROYAL_GUARD), 8, "eight royal guards");

    // The nurses are the Queen's minions, so the heal rotation finds them.
    let a_nurse = find_npc_object_id(&mut world, NURSE).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<crate::game_loop::minions::MinionOf>(&a_nurse)
            .map(|m| m.0),
        Some(QUEEN_OID),
        "the nurses belong to the Queen"
    );
}

/// A wounded Queen is healed by her nurses.
#[test]
fn nurses_heal_a_wounded_queen() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    spawn_nurse(&mut world, QUEEN_OID + 1, QUEEN_OID);
    let wounded = wound_to_half(&mut world, QUEEN_OID);

    crate::game_loop::queen_ant::handle_heal_tick(&mut world, QUEEN_OID);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }

    assert!(hp(&world, QUEEN_OID) > wounded, "the Queen was healed");
}

/// **The larva takes priority.** With both wounded, the nurses heal the larva
/// and the Queen goes untended — which is the fight: leave the larva up and
/// you are fighting a Queen whose healers are busy.
#[test]
fn the_larva_takes_priority_over_the_queen() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    spawn_nurse(&mut world, QUEEN_OID + 1, QUEEN_OID);
    queen_ant::on_queen_spawned(&mut world, QUEEN_OID);
    let larva = find_npc_object_id(&mut world, LARVA).unwrap();

    let queen_before = wound_to_half(&mut world, QUEEN_OID);
    let larva_before = wound_to_half(&mut world, larva);

    queen_ant::handle_heal_tick(&mut world, QUEEN_OID);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }

    assert!(hp(&world, larva) > larva_before, "the larva was healed");
    assert_eq!(hp(&world, QUEEN_OID), queen_before, "and the Queen was not");
}

/// With the larva dead, the nurses switch to the Queen — the same rotation,
/// different target, which is what killing the larva buys the party.
#[test]
fn killing_the_larva_frees_the_nurses_for_the_queen() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    spawn_nurse(&mut world, QUEEN_OID + 1, QUEEN_OID);
    queen_ant::on_queen_spawned(&mut world, QUEEN_OID);
    let larva = find_npc_object_id(&mut world, LARVA).unwrap();

    let wounded = wound_to_half(&mut world, QUEEN_OID);
    world
        .objects
        .get_component_mut::<Vitals>(&larva)
        .unwrap()
        .dead = true;

    queen_ant::handle_heal_tick(&mut world, QUEEN_OID);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }

    assert!(
        hp(&world, QUEEN_OID) > wounded,
        "the nurses turned to the Queen"
    );
}

/// A nurse belonging to a *different* master is not part of this Queen's
/// rotation — the lookup is by master, not by npc id.
#[test]
fn a_nurse_of_another_master_does_not_heal_this_queen() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    // Nurse owned by someone else entirely.
    spawn_nurse(&mut world, QUEEN_OID + 1, QUEEN_OID + 999);
    let wounded = wound_to_half(&mut world, QUEEN_OID);

    crate::game_loop::queen_ant::handle_heal_tick(&mut world, QUEEN_OID);
    for _ in 0..60 {
        advance_ticks(&mut world, 1);
    }

    assert_eq!(
        hp(&world, QUEEN_OID),
        wounded,
        "not this Queen's nurse, no heal"
    );
}

/// The larva is spawned immortal and rooted — you cannot kill it or move it, so
/// the nurses always have it to heal. That is the fight.
#[test]
fn the_larva_is_immobilized_and_undying() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    queen_ant::on_queen_spawned(&mut world, QUEEN_OID);
    let larva = find_npc_object_id(&mut world, LARVA).unwrap();
    let flags = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&larva)
        .expect("larva has admin flags");
    assert!(flags.undying, "the larva cannot be killed");
    assert!(flags.paralyzed, "the larva cannot move");
}

/// When the Queen dies, her immortal larva is finally removed with her.
#[test]
fn the_larva_is_removed_when_the_queen_dies() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    queen_ant::on_queen_spawned(&mut world, QUEEN_OID);
    assert!(
        find_npc_object_id(&mut world, LARVA).is_some(),
        "larva out during the fight"
    );

    queen_ant::on_queen_killed(&mut world);
    assert!(
        find_npc_object_id(&mut world, LARVA).is_none(),
        "the larva fell with its mistress"
    );
}

/// Drag the Queen far from home and the leash check drops her hate (and sends
/// her back); keep her near and it leaves her alone.
#[test]
fn the_leash_resets_a_dragged_queen() {
    use crate::model::npc::{AggroInfo, AggroList};

    let add_hate = |world: &mut World, oid: i32| {
        world
            .objects
            .get_component_mut::<AggroList>(&oid)
            .unwrap()
            .0
            .insert(
                500,
                AggroInfo {
                    hate: 100.0,
                    damage: 0.0,
                },
            );
    };
    let has_hate = |world: &World, oid: i32| {
        !world
            .objects
            .get_component::<AggroList>(&oid)
            .unwrap()
            .0
            .is_empty()
    };

    // Far from home (0,0,0 vs ~-21610,181594): the leash fires.
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    add_hate(&mut world, QUEEN_OID);
    crate::game_loop::queen_ant::handle_distance_check(&mut world, QUEEN_OID);
    assert!(
        !has_hate(&world, QUEEN_OID),
        "a dragged Queen drops her hate"
    );

    // At home: the leash leaves her be.
    let (mut world, _db, _l) = queen_world();
    add_test_npc(
        &mut world,
        QUEEN_OID,
        QUEEN,
        "GrandBoss",
        40,
        -21610,
        181594,
        -5734,
    );
    add_hate(&mut world, QUEEN_OID);
    crate::game_loop::queen_ant::handle_distance_check(&mut world, QUEEN_OID);
    assert!(
        has_hate(&world, QUEEN_OID),
        "a Queen at home keeps fighting"
    );
}

/// A dead Queen ends the beat rather than rescheduling forever.
#[test]
fn the_heal_beat_stops_when_the_queen_dies() {
    let (mut world, _db, _l) = queen_world();
    add_test_npc(&mut world, QUEEN_OID, QUEEN, "GrandBoss", 40, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&QUEEN_OID)
        .unwrap()
        .dead = true;

    let before = world.scheduler.len();
    crate::game_loop::queen_ant::handle_heal_tick(&mut world, QUEEN_OID);
    assert_eq!(world.scheduler.len(), before, "nothing rescheduled");
}

/// `OnAttackableFactionCall`'s Queen Ant listener: a nurse recruited by a
/// faction call heals the hurt caller at once (Recovery 4020) and ignores a
/// caller at full health — the opportunistic heal on top of the periodic
/// rotation.
#[test]
fn nurse_faction_call_heals_the_hurt_caller() {
    use crate::model::components::Casting;

    let (mut world, _db, _l) = queen_world();
    let queen = NPC_OID;
    add_test_npc(&mut world, queen, QUEEN, "GrandBoss", 40, 0, 0, 0);
    let nurse = NPC_OID + 1;
    spawn_nurse(&mut world, nurse, queen);
    let _rx = ingame_caster(&mut world, 1, 3001, 200, 0);

    // Caller at full HP: the listener does nothing.
    crate::game_loop::ai::on_faction_call_script_for_test(&mut world, nurse, queen, 3001);
    assert!(
        !world.objects.has_component::<Casting>(&nurse),
        "a healthy caller gets no heal"
    );

    // Wound the queen: the recruited nurse opens with Recovery.
    wound_to_half(&mut world, queen);
    crate::game_loop::ai::on_faction_call_script_for_test(&mut world, nurse, queen, 3001);
    assert!(
        world.objects.has_component::<Casting>(&nurse),
        "the faction-called nurse heals the hurt queen"
    );
}
