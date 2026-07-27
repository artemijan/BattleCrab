//! Daily task manager (G33 slice 1): the vitality daily/weekly refill and the
//! self-rescheduling daily reset.

use super::*;

use crate::db::DbCommand;
use crate::game_loop::{daily_tasks, vitality};
use crate::model::{Player, MAX_VITALITY_POINTS};
use crate::scheduler::ScheduledTask;

fn set_vitality(world: &mut World, oid: i32, points: i32) {
    world
        .objects
        .get_component_mut::<Player>(&oid)
        .unwrap()
        .vitality_points = points;
}

fn vitality_of(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<Player>(&oid)
        .unwrap()
        .vitality_points
}

#[test]
fn daily_refill_adds_a_quarter_and_updates_offline_rows() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 50_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, false);

    assert_eq!(
        vitality_of(&world, 3001),
        50_000 + MAX_VITALITY_POINTS / 4,
        "online pool gains MAX/4"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::ResetVitality { weekly: false })),
        "offline daily reset queued"
    );
}

#[test]
fn daily_refill_clamps_at_the_maximum() {
    let (mut world, ..) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, MAX_VITALITY_POINTS - 1000);

    vitality::reset_vitality(&mut world, false);
    assert_eq!(vitality_of(&world, 3001), MAX_VITALITY_POINTS, "capped");
}

#[test]
fn weekly_refill_sets_the_pool_to_full() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 1_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, true);

    assert_eq!(
        vitality_of(&world, 3001),
        MAX_VITALITY_POINTS,
        "full refill"
    );
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, DbCommand::ResetVitality { weekly: true })));
}

#[test]
fn a_disabled_vitality_system_does_not_refill() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = false;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 10_000);
    drain_db(&mut db_rx);

    vitality::reset_vitality(&mut world, false);
    assert_eq!(vitality_of(&world, 3001), 10_000, "unchanged");
    assert!(
        drain_db(&mut db_rx).is_empty(),
        "no DB reset when vitality is off"
    );
}

#[test]
fn the_daily_reset_runs_both_sub_resets_and_reschedules_itself() {
    let (mut world, _tx, mut db_rx, _link) = test_world();
    world.cfg.character.enable_vitality = true;
    let _p = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    set_vitality(&mut world, 3001, 20_000);
    drain_db(&mut db_rx);

    daily_tasks::handle_daily_reset(&mut world);

    // Vitality moved (its sub-reset ran) and both offline UPDATEs were queued.
    assert!(vitality_of(&world, 3001) > 20_000, "vitality refilled");
    let cmds = drain_db(&mut db_rx);
    assert!(cmds
        .iter()
        .any(|c| matches!(c, DbCommand::ResetVitality { .. })));
    assert!(cmds.iter().any(|c| matches!(c, DbCommand::ResetRecommends)));
    // The task re-armed itself for the next day.
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .any(|t| matches!(t, ScheduledTask::DailyReset)));
}
