//! Punishment / jail (G31 slice 1): the jail effect (teleport + persist), the
//! release path (`//unjail` + timed expiry), the login re-apply, and the
//! JailZone keep-in.

use super::*;

use crate::data::spawn_data::{Territory, ZoneForm};
use crate::data::zone_data::{Zone, ZoneKind};
use crate::db::DbCommand;
use crate::game_loop::punishment;
use crate::model::components::Position;
use crate::model::punishment::{PunishmentAffect, PunishmentType};
use crate::scheduler::ScheduledTask;

// Java `JailZone` locations.
const JAIL_IN: (i32, i32) = (-114356, -249645);
const JAIL_OUT: (i32, i32) = (17836, 170178);

/// Register a jail zone around the jail-in point so `in_jail_zone` is meaningful
/// (the test `GameData` ships no zones).
fn add_jail_zone(world: &mut World) {
    world.data.zone_data.insert(Zone {
        id: 0,
        name: "test_jail".into(),
        kind: ZoneKind::Jail,
        territory: Territory {
            form: ZoneForm::Cuboid {
                x1: JAIL_IN.0 - 2000,
                x2: JAIL_IN.0 + 2000,
                y1: JAIL_IN.1 - 2000,
                y2: JAIL_IN.1 + 2000,
            },
            min_z: -4000,
            max_z: 0,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
    });
}

fn pos_xy(world: &World, oid: i32) -> (i32, i32) {
    let p = world.objects.get_component::<Position>(&oid).unwrap();
    (p.x, p.y)
}

fn store_punishment_cmds(cmds: &[DbCommand]) -> usize {
    cmds.iter()
        .filter(|c| matches!(c, DbCommand::StorePunishment { .. }))
        .count()
}

#[test]
fn jail_teleports_marks_and_persists() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _rx = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    let applied = punishment::jail_character(&mut world, 3001, 0, "r".into(), "gm".into());
    assert!(applied);

    // Flag set, teleported into the prison.
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_IN);

    // Registered and persisted.
    assert!(world.punishments.has_punishment(
        "3001",
        PunishmentAffect::Character,
        PunishmentType::Jail
    ));
    let cmds = drain_db(&mut db_rx);
    assert_eq!(store_punishment_cmds(&cmds), 1, "one StorePunishment sent");
    assert!(cmds.iter().any(|c| matches!(
        c,
        DbCommand::StorePunishment { key, affect, ptype, .. }
            if key == "3001" && affect == "CHARACTER" && ptype == "JAIL"
    )));
}

#[test]
fn jailing_an_already_jailed_player_is_rejected() {
    let (mut world, _tx, _rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    assert!(punishment::jail_character(
        &mut world,
        3001,
        0,
        "r".into(),
        "gm".into()
    ));
    // Second jail on the same character → Java's "already affected" guard.
    assert!(!punishment::jail_character(
        &mut world,
        3001,
        0,
        "r".into(),
        "gm".into()
    ));
}

#[test]
fn unjail_releases_teleports_out_and_deletes() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    punishment::jail_character(&mut world, 3001, 0, "r".into(), "gm".into());
    drain_db(&mut db_rx);

    assert!(punishment::unjail_character(&mut world, 3001));
    assert!(!world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_OUT);
    assert!(!world.punishments.has_punishment(
        "3001",
        PunishmentAffect::Character,
        PunishmentType::Jail
    ));
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::DeletePunishment { .. })),
        "DeletePunishment sent on release"
    );

    // Releasing a non-jailed player is a no-op false.
    assert!(!punishment::unjail_character(&mut world, 3001));
}

#[test]
fn a_timed_jail_expires_and_releases_the_player() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    // One minute → an expiry timer is armed.
    punishment::jail_character(&mut world, 3001, 1, "r".into(), "gm".into());
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    drain_db(&mut db_rx);

    // 60 s = 600 ticks, plus a margin, firing tasks each tick.
    advance_ticks(&mut world, 700);

    assert!(!world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_OUT);
    assert!(!world.punishments.has_punishment(
        "3001",
        PunishmentAffect::Character,
        PunishmentType::Jail
    ));
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, DbCommand::DeletePunishment { .. })));
}

#[test]
fn keep_in_teleports_a_wanderer_back_but_leaves_an_inmate() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    let _out = ingame_player(&mut world, 1, 3001, JAIL_IN.0, JAIL_IN.1, -2984);
    // Mark jailed directly (jail_character would teleport; we want to place them).
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .jailed = true;

    // Standing inside the jail zone: keep-in leaves them put.
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_eq!(pos_xy(&world, 3001), JAIL_IN);

    // Wander far outside the zone, then re-check: teleported straight back.
    {
        let p = world.objects.get_component_mut::<Position>(&3001).unwrap();
        p.x = 50_000;
        p.y = 50_000;
    }
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_eq!(pos_xy(&world, 3001), JAIL_IN);
}

#[test]
fn keep_in_ignores_a_free_player() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);
    // Not jailed, standing outside the zone → keep-in does nothing.
    punishment::enforce_jail_keep_in(&mut world, 3001);
    assert_eq!(pos_xy(&world, 3001), (50_000, 50_000));
}

#[test]
fn boot_load_registers_and_re_arms_a_timed_jail() {
    let (mut world, _tx, _rx, _link) = test_world();
    let now = commons::util::now_millis();
    let task = crate::model::punishment::Punishment {
        id: 7,
        key: "3001".into(),
        affect: PunishmentAffect::Character,
        ptype: PunishmentType::Jail,
        expiration: now + 60_000,
        reason: "r".into(),
        punished_by: "gm".into(),
    };
    punishment::on_loaded(&mut world, 8, vec![task]);

    // Registered, allocator seeded, and an expiry timer queued.
    assert!(world.punishments.has_punishment(
        "3001",
        PunishmentAffect::Character,
        PunishmentType::Jail
    ));
    assert_eq!(world.punishments.next_id, 8);
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .any(|t| matches!(t, ScheduledTask::PunishmentExpire { punishment_id: 7 })));
}

#[test]
fn on_enter_world_reapplies_jail_to_a_returning_inmate() {
    let (mut world, _tx, _rx, _link) = test_world();
    add_jail_zone(&mut world);
    // A persisted jail for char 3001, but the player logs in out in the world.
    let now = commons::util::now_millis();
    punishment::on_loaded(
        &mut world,
        1,
        vec![crate::model::punishment::Punishment {
            id: 1,
            key: "3001".into(),
            affect: PunishmentAffect::Character,
            ptype: PunishmentType::Jail,
            expiration: now + 3_600_000,
            reason: "r".into(),
            punished_by: "gm".into(),
        }],
    );
    let _out = ingame_player(&mut world, 1, 3001, 50_000, 50_000, -3000);

    punishment::on_enter_world(&mut world, 1, 3001);
    assert!(world.objects.get_component::<Player>(&3001).unwrap().jailed);
    assert_eq!(pos_xy(&world, 3001), JAIL_IN);
}
