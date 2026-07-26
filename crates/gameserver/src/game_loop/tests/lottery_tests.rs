//! Lucky Lottery (G26.5) — slice 1: the round lifecycle + `lottery`-table
//! persistence (fresh boot, resume, finished-row carry-over, draw rollover).
//! Ticket purchase + the prize draw are slice 2.

use super::*;

use crate::game_loop::lottery;
use crate::model::lottery::LotteryRow;
use crate::scheduler::ScheduledTask;

/// A test world with the lottery enabled (dist ships it off).
fn enabled_world() -> (World, db::CmdRx) {
    let (mut world, _tx, db_rx, _link) = test_world();
    world.cfg.general.allow_lottery = true;
    world.cfg.general.alt_lottery_prize = 50000;
    (world, db_rx)
}

#[test]
fn fresh_boot_opens_round_one() {
    let (mut world, mut db_rx) = enabled_world();

    lottery::on_loaded(&mut world, None);

    assert_eq!(world.lottery.number, 1);
    assert_eq!(world.lottery.prize, 50000);
    assert!(world.lottery.selling && world.lottery.started);
    let pending = world.scheduler.pending_tasks_for_test();
    assert!(pending.contains(&ScheduledTask::LotteryFinish));
    assert!(pending.contains(&ScheduledTask::LotteryStopSelling));
    // The new round was persisted.
    assert!(drain_db(&mut db_rx)
        .iter()
        .any(|c| matches!(c, crate::db::DbCommand::StoreLottery { idnr: 1, .. })));
}

#[test]
fn a_disabled_lottery_stays_inert() {
    let (mut world, _tx, _db, _l) = test_world(); // AllowLottery defaults false
    lottery::on_loaded(&mut world, None);
    assert_eq!(world.lottery.number, 0);
    assert!(!world.lottery.started && !world.lottery.selling);
    assert!(world.scheduler.pending_tasks_for_test().is_empty());
}

#[test]
fn a_finished_row_carries_the_pot_into_the_next_round() {
    let (mut world, _db) = enabled_world();

    lottery::on_loaded(
        &mut world,
        Some(LotteryRow {
            idnr: 7,
            prize: 999,
            newprize: 123_456,
            enddate: 0,
            finished: true,
        }),
    );

    assert_eq!(world.lottery.number, 8); // idnr + 1
    assert_eq!(world.lottery.prize, 123_456); // newprize carried forward
    assert!(world.lottery.started);
}

#[test]
fn a_live_round_resumes_with_its_draw_armed() {
    let (mut world, _db) = enabled_world();
    let far = commons::util::now_millis() + 7 * 24 * 3600 * 1000; // a week out

    lottery::on_loaded(
        &mut world,
        Some(LotteryRow {
            idnr: 3,
            prize: 777,
            newprize: 777,
            enddate: far,
            finished: false,
        }),
    );

    assert_eq!(world.lottery.number, 3);
    assert_eq!(world.lottery.prize, 777);
    assert!(world.lottery.started && world.lottery.selling);
    let pending = world.scheduler.pending_tasks_for_test();
    assert!(pending.contains(&ScheduledTask::LotteryFinish));
    assert!(pending.contains(&ScheduledTask::LotteryStopSelling));
}

#[test]
fn finish_rolls_the_round_over_and_carries_the_pot() {
    let (mut world, mut db_rx) = enabled_world();
    lottery::on_loaded(&mut world, None); // round 1, pot 50000
    drain_db(&mut db_rx);

    lottery::finish_lottery(&mut world);

    assert_eq!(world.lottery.number, 2); // number++
    assert_eq!(world.lottery.prize, 50000); // no tickets sold → whole pot carries
    assert!(!world.lottery.started);
    let cmds = drain_db(&mut db_rx);
    assert!(cmds
        .iter()
        .any(|c| matches!(c, crate::db::DbCommand::FinishLottery { idnr: 1, .. })));
    // A fresh round is armed to open a minute later.
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::LotteryStart));
}
