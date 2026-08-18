//! Scheduled restart — Java's `ServerRestartManager`.
//!
//! Writes into the same place as `//server_restart`: `pending_shutdown` plus a
//! `ServerShutdownTick`, so a scheduled restart and a GM one share one
//! countdown and one abort (`//server_abort`).
//!
//! **Java's `DeadLockDetector` is deliberately not ported**, and the reason is
//! structural rather than a deferral. It calls
//! `ThreadMXBean.findDeadlockedThreads()`, which reports cycles in monitor
//! ownership — a real hazard in Mobius, where game state is shared across a
//! thread pool behind `synchronized`. This port owns all mutable game state on
//! the single game thread and has **exactly one lock in the whole gameserver**
//! (`geo::GeoEngine::nswe_overrides`), acquired only as a leaf: every call site
//! takes it, reads or inserts, and drops it without calling out. A deadlock
//! needs either two locks ordered differently or a re-entrant acquisition, and
//! neither is expressible here. Porting the detector would add a thread to
//! search for a condition the architecture excludes.
//!
//! `DeadLockDetector`/`DeadLockCheckInterval`/`RestartOnDeadlock` are therefore
//! parsed and unread, which `config/server.rs` records at the fields. Re-check
//! the lock count before concluding this is still true.
//!
//! **A stall watchdog is also not being ported**, and that is a settled
//! decision rather than an open item. The tempting substitute — a thread
//! watching a heartbeat the game loop bumps each tick, restarting when it stops
//! advancing — was written and then removed on purpose. It detects a *different*
//! condition from the one the config keys name (a wedged loop, not a lock
//! cycle), so shipping it behind `DeadLockDetector` would make an operator who
//! enables that key believe they have Java's guarantee when they do not. This
//! codebase's most-repeated failure is exactly that gap between a flag being on
//! and the behaviour matching. If a stall watchdog is ever wanted it belongs
//! under its own config key, as its own feature, argued on its own merits — not
//! as a stand-in for a detector whose condition cannot arise here.

use crate::game_loop::time::{MILLIS_PER_DAY, MILLIS_PER_HOUR, MILLIS_PER_MINUTE};
use tracing::info;

use crate::game_loop::helpers::ms_to_ticks;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `ServerRestartManager`'s constructor: of every `HH:MM` in
/// `ServerRestartSchedule`, pick the **soonest** future occurrence — skipping
/// forward to a permitted weekday when `ServerRestartDays` is non-empty — and
/// return how many milliseconds away it is.
///
/// Java's day numbering is `Calendar.DAY_OF_WEEK`: Sunday = 1 … Saturday = 7.
/// `now_millis` is epoch-based and 1970-01-01 was a **Thursday**, so the
/// weekday is `((days + 4) % 7) + 1`.
///
/// Returns `None` when the schedule is empty or every entry is unparseable —
/// Java logs "the scheduled server restart config is not set properly" and
/// schedules nothing.
pub(crate) fn next_restart_delay_ms(
    now_millis: i64,
    schedule: &[String],
    days: &[i32],
) -> Option<i64> {
    let day_start = now_millis - now_millis.rem_euclid(MILLIS_PER_DAY);
    let mut best: Option<i64> = None;
    for entry in schedule {
        let mut parts = entry.trim().split(':');
        let (Some(h), Some(m)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(h), Ok(m)) = (h.trim().parse::<i64>(), m.trim().parse::<i64>()) else {
            continue;
        };
        if !(0..24).contains(&h) || !(0..60).contains(&m) {
            continue;
        }
        let mut at = day_start + h * MILLIS_PER_HOUR + m * MILLIS_PER_MINUTE;
        // `if (restartTime < currentTime) add(DAY_OF_WEEK, 1)` — today's slot
        // has passed, so take tomorrow's.
        if at < now_millis {
            at += MILLIS_PER_DAY;
        }
        if !days.is_empty() {
            // `while (!SERVER_RESTART_DAYS.contains(day)) add(DAY_OF_WEEK, 1)`.
            // Bounded at 7 hops: an unmatchable day list would spin forever in
            // Java, and there are only seven to try.
            let mut hops = 0;
            while !days.contains(&calendar_day_of_week(at)) && hops < 7 {
                at += MILLIS_PER_DAY;
                hops += 1;
            }
            if !days.contains(&calendar_day_of_week(at)) {
                continue; // no permitted day — this entry can never fire
            }
        }
        let delay = at - now_millis;
        if best.is_none_or(|b| delay < b) {
            best = Some(delay);
        }
    }
    best
}

/// `Calendar.DAY_OF_WEEK` for an epoch-millis instant: Sunday = 1 … Saturday
/// = 7.
fn calendar_day_of_week(millis: i64) -> i32 {
    let days = millis.div_euclid(MILLIS_PER_DAY);
    ((days + 4).rem_euclid(7) + 1) as i32
}

/// Arm the scheduled restart at boot (Java builds `ServerRestartManager` when
/// `ServerRestartScheduleEnabled`).
///
/// Java schedules the *task* `countdown` seconds before the restart moment and
/// the task then runs a `countdown`-second shutdown, so the server actually
/// goes down at the configured time rather than `countdown` past it. The same
/// arithmetic is kept here, including the case where the countdown is longer
/// than the delay: Java would schedule at a negative delay (firing at once),
/// which `saturating_sub` reproduces as "fire on the next tick".
pub(crate) fn schedule_server_restart(world: &mut World) {
    let cfg = &world.cfg.server;
    if !cfg.server_restart_schedule_enabled {
        return;
    }
    let now = commons::util::now_millis();
    let Some(delay_ms) =
        next_restart_delay_ms(now, &cfg.server_restart_schedule, &cfg.server_restart_days)
    else {
        info!(
            "ServerRestartManager: the scheduled server restart config is not set properly, please correct it!"
        );
        return;
    };
    let countdown_ms = cfg.server_restart_schedule_countdown as i64 * 1000;
    let fire_in_ms = (delay_ms - countdown_ms).max(0);
    let at = world.tick + ms_to_ticks(fire_in_ms);
    world
        .scheduler
        .schedule(at, ScheduledTask::ServerRestartSchedule);
    info!(
        "ServerRestartManager: scheduled server restart in {} minutes.",
        delay_ms / MILLIS_PER_MINUTE
    );
}

/// The scheduled moment arrived: start the countdown, then re-arm for the next
/// slot so the schedule repeats (Java re-creates the manager on restart; here
/// the process may outlive an aborted countdown, so re-arming keeps the
/// schedule alive either way).
pub(crate) fn handle_server_restart_schedule(world: &mut World) {
    let countdown = world.cfg.server.server_restart_schedule_countdown;
    crate::game_loop::admin::begin_shutdown(world, countdown, true);
    schedule_server_restart(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1970-01-01 was a Thursday, which `Calendar.DAY_OF_WEEK` numbers 5.
    #[test]
    fn day_of_week_matches_javas_calendar_numbering() {
        assert_eq!(calendar_day_of_week(0), 5); // Thu
        assert_eq!(calendar_day_of_week(3 * MILLIS_PER_DAY), 1); // Sun
        assert_eq!(calendar_day_of_week(4 * MILLIS_PER_DAY), 2); // Mon
        assert_eq!(calendar_day_of_week(9 * MILLIS_PER_DAY), 7); // Sat
    }

    /// The **soonest** entry wins, not the first one listed.
    #[test]
    fn the_soonest_scheduled_time_is_chosen() {
        // 1970-01-01 06:00 UTC.
        let now = 6 * MILLIS_PER_HOUR;
        let sched: Vec<String> = ["23:00", "08:00", "12:00"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let delay = next_restart_delay_ms(now, &sched, &[]).unwrap();
        assert_eq!(delay, 2 * MILLIS_PER_HOUR, "08:00 is two hours out");
    }

    /// A time already past today rolls to tomorrow rather than reading as
    /// "overdue" — Java's `add(DAY_OF_WEEK, 1)`.
    #[test]
    fn a_time_already_past_rolls_to_tomorrow() {
        let now = 20 * MILLIS_PER_HOUR; // 20:00
        let sched = vec!["08:00".to_string()];
        let delay = next_restart_delay_ms(now, &sched, &[]).unwrap();
        assert_eq!(delay, 12 * MILLIS_PER_HOUR, "tomorrow 08:00");
    }

    /// With `ServerRestartDays` set, the slot skips forward to a permitted
    /// weekday. From Thursday 06:00, a Monday-only schedule is four days out.
    #[test]
    fn restart_days_skip_forward_to_a_permitted_weekday() {
        let now = 6 * MILLIS_PER_HOUR; // Thu 06:00
        let sched = vec!["08:00".to_string()];
        let delay = next_restart_delay_ms(now, &sched, &[2]).unwrap(); // Monday
        assert_eq!(delay, 4 * MILLIS_PER_DAY + 2 * MILLIS_PER_HOUR);
    }

    /// An empty or malformed schedule schedules nothing, which is Java's
    /// "config is not set properly" path rather than a restart at midnight.
    #[test]
    fn a_malformed_schedule_arms_nothing() {
        assert!(next_restart_delay_ms(0, &[], &[]).is_none());
        assert!(next_restart_delay_ms(0, &["nonsense".to_string()], &[]).is_none());
        assert!(next_restart_delay_ms(0, &["25:00".to_string()], &[]).is_none());
        // A day list that no weekday satisfies cannot fire either.
        assert!(next_restart_delay_ms(0, &["08:00".to_string()], &[99]).is_none());
    }
}
