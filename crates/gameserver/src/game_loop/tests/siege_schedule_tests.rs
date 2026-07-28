//! The automatic weekly siege schedule (G24 slice 1).

use super::*;

use crate::data::siege_data::{SiegeScheduleEntry, load_siege_schedule};
use crate::game_loop::siege::next_siege_millis;
use crate::model::castle::{Castle, CastleSide};
use crate::model::siege::Siege;
use crate::scheduler::ScheduledTask;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

fn weekday_of(millis: i64) -> u32 {
    (millis.div_euclid(DAY_MS) + 3).rem_euclid(7) as u32
}

/// `next_siege_millis` lands strictly in the future, on the target weekday, at
/// `hour`:00 UTC, within a week.
#[test]
fn next_siege_is_the_next_matching_weekday_and_hour() {
    // A fixed reference: 1970-01-01 00:00 UTC was a Thursday (weekday 3).
    let thursday_midnight = 0i64;
    assert_eq!(weekday_of(thursday_midnight), 3, "epoch is Thursday");

    for now in [
        0i64,
        5 * DAY_MS + 3 * HOUR_MS,
        123_456_789_000,
        1_700_000_000_000,
    ] {
        for weekday in 0..7u32 {
            for hour in [0u32, 16, 20, 23] {
                let at = next_siege_millis(now, weekday, hour);
                assert!(at > now, "strictly future: now={now} -> {at}");
                assert_eq!(weekday_of(at), weekday, "on the target weekday");
                assert_eq!(at.rem_euclid(DAY_MS), hour as i64 * HOUR_MS, "at hour:00");
                assert!(at - now <= 7 * DAY_MS, "within a week");
            }
        }
    }
}

/// A slot earlier *today* rolls to next week, not today.
#[test]
fn a_passed_slot_today_rolls_a_week_forward() {
    // Epoch is Thursday(3) 00:00. Asking for Thursday@0 must skip to next week.
    let at = next_siege_millis(0, 3, 0);
    assert_eq!(at, 7 * DAY_MS, "next Thursday, not today");
    // Thursday@16 the same day is still ahead → today.
    let at = next_siege_millis(0, 3, 16);
    assert_eq!(at, 16 * HOUR_MS, "later today");
}

/// The dist schedule loads: all nine castles, Sunday, enabled, hours 16/20.
#[test]
fn the_dist_schedule_loads_all_nine_castles() {
    let sched = load_siege_schedule(DIST);
    assert_eq!(sched.len(), 9, "nine castles");
    for id in 1..=9 {
        let e = sched.get(&id).unwrap_or_else(|| panic!("castle {id}"));
        assert_eq!(e.weekday, 6, "Sunday");
        assert!(e.enabled);
        assert!(e.hour == 16 || e.hour == 20, "hour {}", e.hour);
    }
    // Gludio 16:00, Dion 20:00 (transcribed from the file).
    assert_eq!(sched.get(&1).unwrap().hour, 16);
    assert_eq!(sched.get(&2).unwrap().hour, 20);
}

fn schedule_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    world.castles = vec![
        Castle {
            id: 1,
            name: "Gludio".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            time_registration_over: true,
            siege_date: 0,
        },
        Castle {
            id: 2,
            name: "Dion".into(),
            side: CastleSide::Neutral,
            ticket_buy_count: 0,
            time_registration_over: true,
            siege_date: 0,
        },
    ];
    world.sieges.insert(1, Siege::new(1));
    world.sieges.insert(2, Siege::new(2));
    world.data.siege_schedule.insert(
        1,
        SiegeScheduleEntry {
            weekday: 6,
            hour: 16,
            enabled: true,
        },
    );
    // Castle 2 is disabled — it must not be armed.
    world.data.siege_schedule.insert(
        2,
        SiegeScheduleEntry {
            weekday: 6,
            hour: 20,
            enabled: false,
        },
    );
    (world, db, l)
}

fn pending_siege_starts(world: &World) -> usize {
    world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .filter(|t| matches!(t, ScheduledTask::SiegeStart { .. }))
        .count()
}

/// Boot arms exactly one `SiegeStart` per *enabled* castle (the disabled one is
/// skipped); firing it starts that castle's siege and re-arms next week's
/// timer — the self-perpetuating weekly schedule.
#[test]
fn boot_arms_enabled_castles_and_the_start_re_arms_next_week() {
    let (mut world, _db, _l) = schedule_world();

    crate::game_loop::siege::schedule_all_at_boot(&mut world);
    assert_eq!(
        pending_siege_starts(&world),
        1,
        "only the enabled castle is armed"
    );

    // Fire castle 1's start. (Calling the handler directly leaves the
    // boot-armed task in the heap — production drains it first — so measure the
    // *delta* the firing adds.)
    assert!(!world.sieges[&1].in_progress);
    let before = pending_siege_starts(&world);
    crate::game_loop::siege::handle_scheduled_siege_start(&mut world, 1);
    assert!(world.sieges[&1].in_progress, "the scheduled siege began");
    assert_eq!(
        pending_siege_starts(&world) - before,
        1,
        "firing re-arms exactly one next-week start"
    );
}
