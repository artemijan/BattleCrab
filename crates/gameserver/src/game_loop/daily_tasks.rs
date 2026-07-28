//! The wall-clock daily reset (Java `instancemanager/DailyTaskManager.onReset`,
//! G33): fired at 06:30 each day, it runs the recommends reset and the vitality
//! refill (weekly-full on Wednesday, else the daily add), for online **and**
//! offline characters, then reschedules itself 24 h out.
//!
//! The port computes 06:30 in **UTC** (not server-local like Java's `Calendar`),
//! so "Wednesday" here is UTC-Wednesday — consistent with the reco reset this
//! generalises.

use crate::scheduler::ScheduledTask;
use crate::world::World;

const MILLIS_PER_DAY: i64 = 86_400_000;
/// 24 h between resets, in 100 ms ticks.
const DAILY_RESET_PERIOD: u64 = 864_000;
/// 06:30, as milliseconds past midnight.
const DAILY_RESET_MS_OF_DAY: i64 = (6 * 3600 + 30 * 60) * 1000;
/// UTC weekday of the weekly (full) vitality refill. Epoch day 0 (1970-01-01)
/// was a Thursday, so `(epoch_day + 4) % 7` yields 0=Sun..6=Sat; Wednesday = 3
/// (Java `Calendar.WEDNESDAY`).
const WEEKLY_VITALITY_WEEKDAY: i64 = 3;

/// Whether `now` (unix millis) falls on a UTC Wednesday.
fn is_weekly_reset_day(now: i64) -> bool {
    ((now / MILLIS_PER_DAY) + 4).rem_euclid(7) == WEEKLY_VITALITY_WEEKDAY
}

/// `ScheduledTask::DailyReset` → Java `DailyTaskManager.onReset`: run each daily
/// sub-reset, then re-arm 24 h out.
pub(crate) fn handle_daily_reset(world: &mut World) {
    // Wednesday → full vitality; any other day → the daily add (Java's
    // `Calendar.DAY_OF_WEEK == WEDNESDAY` branch).
    let weekly = is_weekly_reset_day(commons::util::now_millis());
    super::vitality::reset_vitality(world, weekly);
    super::reco::reset_recommends(world);

    world
        .scheduler
        .schedule(world.tick + DAILY_RESET_PERIOD, ScheduledTask::DailyReset);
}

/// Schedule the first `DailyReset` for the next 06:30 UTC (Java
/// `DailyTaskManager`'s constructor). Called once at game-loop start.
///
/// TODO(G33): no `GlobalVariablesManager.DAILY_TASK_RESET` catch-up yet — a
/// reset missed while the server was down runs at the next 06:30 rather than
/// immediately on boot. Needs a persisted last-reset stamp (no GlobalVariables
/// table in the port).
pub(crate) fn schedule_initial_daily_reset(world: &mut World) {
    let now = commons::util::now_millis();
    let ms_of_day = now.rem_euclid(MILLIS_PER_DAY);
    let mut delay_ms = DAILY_RESET_MS_OF_DAY - ms_of_day;
    if delay_ms < 0 {
        delay_ms += MILLIS_PER_DAY;
    }
    let delay_ticks = (delay_ms / 100) as u64;
    world
        .scheduler
        .schedule(world.tick + delay_ticks, ScheduledTask::DailyReset);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wednesday_is_the_weekly_reset_day() {
        // 1970-01-07 was a Wednesday (epoch day 6).
        assert!(is_weekly_reset_day(6 * MILLIS_PER_DAY));
        // The surrounding days are not.
        assert!(!is_weekly_reset_day(5 * MILLIS_PER_DAY)); // Tue
        assert!(!is_weekly_reset_day(7 * MILLIS_PER_DAY)); // Thu
        // A modern Wednesday: 2024-01-03.
        assert!(is_weekly_reset_day(19725 * MILLIS_PER_DAY));
        assert!(!is_weekly_reset_day(19726 * MILLIS_PER_DAY));
    }
}
