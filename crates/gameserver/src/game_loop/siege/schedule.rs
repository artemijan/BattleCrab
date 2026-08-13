//! The automatic weekly schedule (`SiegeSchedule.xml`): boot scheduling,
//! `Siege.startAutoTask` countdown chain, and the owner hour-picking window.

use super::*;

const MILLIS_PER_DAY: i64 = 86_400_000;
const MILLIS_PER_HOUR: i64 = 3_600_000;
const TICKS_PER_SECOND: u64 = 10;

/// The next `weekday`@`hour`:00 **UTC** strictly after `now_millis` (Java
/// `SiegeScheduleDate` + `Calendar` next-occurrence, computed in UTC — Rust std
/// has no timezone, so this differs from Java's server-local time by the
/// deployment's UTC offset; the weekly cadence itself is exact).
///
/// `weekday` is `Mon=0..Sun=6`. 1970-01-01 (epoch day 0) was a Thursday, so
/// `weekday_of(day) = (day + 3) % 7`.
pub(crate) fn next_siege_millis(now_millis: i64, weekday: u32, hour: u32) -> i64 {
    let now_day = now_millis.div_euclid(MILLIS_PER_DAY);
    let now_weekday = (now_day + 3).rem_euclid(7) as u32;
    let mut delta = (weekday as i64 - now_weekday as i64).rem_euclid(7);
    let mut candidate = (now_day + delta) * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR;
    if candidate <= now_millis {
        delta += 7;
        candidate = (now_day + delta) * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR;
    }
    candidate
}

/// Arm each enabled castle's auto-task. Called once the per-castle `Siege`s
/// exist (the `SiegesLoaded` boot handler).
pub(crate) fn schedule_all_at_boot(world: &mut World) {
    let castle_ids: Vec<i32> = world
        .data
        .siege_schedule
        .iter()
        .filter(|(_, e)| e.enabled)
        .map(|(&id, _)| id)
        .collect();
    let now = commons::util::now_millis();
    for castle_id in castle_ids {
        // Java keeps `castle.siegeDate` as a **stored** moment that the clock
        // eventually passes. Deriving it fresh each time cannot work for a
        // re-reading chain: `next_siege_millis` is strictly future by
        // construction, so "time remaining" would never reach zero and the
        // siege would never fire. Stamp it once here, then let it age.
        set_next_siege_date(world, castle_id, now);
        arm_auto_task(world, castle_id, 0);
    }
}

/// Java `Siege.setNextSiegeDate()` — store the castle's next siege moment.
///
/// Java pushes it two weeks out from the last siege; this dist is
/// schedule-driven (`SiegeSchedule.xml` weekday + hour per castle), so the next
/// matching slot is the equivalent. Only stamps when the stored date is absent
/// or already spent, so an hour the owner picked is never overwritten.
fn set_next_siege_date(world: &mut World, castle_id: i32, now: i64) {
    let Some(entry) = world
        .data
        .siege_schedule
        .get(&castle_id)
        .copied()
        .filter(|e| e.enabled)
    else {
        return;
    };
    let stored = world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map(|c| c.siege_date)
        .unwrap_or(0);
    if stored > now {
        return; // a future date is already set — the owner's choice included
    }
    let next = next_siege_millis(now, entry.weekday, entry.hour);
    let Some(c) = world.castles.iter_mut().find(|c| c.id == castle_id) else {
        return;
    };
    c.siege_date = next;
    let time_registration_over = c.time_registration_over;
    let _ = world.db.send(DbCommand::UpdateCastleSiegeTime {
        castle_id,
        siege_date: next,
        time_registration_over,
        siege_time_registration_end: None,
    });
}

// --- `Siege.startAutoTask` / `ScheduleStartSiegeTask` -----------------------
//
// Java does **not** arm one timer for the siege moment. It arms a chain that
// re-reads `getSiegeDate()` every time it fires and re-arms itself closer and
// closer, only calling `startSiege()` once the date has actually passed.
//
// That design is why the owner's chosen hour is honored without any timer
// cancellation: a hop armed against the old date simply re-reads the new one
// when it wakes and re-arms accordingly. This port previously armed a single
// fire-at-the-computed-tick task, which is what made the chosen hour unreachable
// — the fix is the chain, not a cancellable scheduler.

/// Java's ladder rungs, in milliseconds remaining before the siege.
const AUTO_TASK_DAY_MS: i64 = 86_400_000;
/// Java's second rung. Its comment reads "Prepare task for 1 hr left before
/// siege start", but the literal is `13600000` — 3 h 46 m 40 s, an apparent
/// stray digit in `3600000`. The **value** is what the server runs on, so the
/// value is what is ported: this is the moment attacker/defender registration
/// closes and the waiting list is cleared, and moving it to a true hour would
/// silently give clans 2 h 46 m more to register than retail does.
const AUTO_TASK_REG_END_MS: i64 = 13_600_000;
const AUTO_TASK_10_MIN_MS: i64 = 600_000;
const AUTO_TASK_5_MIN_MS: i64 = 300_000;
const AUTO_TASK_10_SEC_MS: i64 = 10_000;

/// Schedule the next hop of the auto-task chain, `delay_ms` from now.
fn arm_auto_task(world: &mut World, castle_id: i32, delay_ms: i64) {
    let delay_ticks = (delay_ms.max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::SiegeStart { castle_id },
    );
}

/// The castle's siege time (epoch-millis): the owner-chosen date when one is set
/// for the future, else the next fixed `SiegeSchedule.xml` slot. 0 when neither.
pub(super) fn effective_siege_millis(world: &World, castle_id: i32, now_millis: i64) -> i64 {
    let stored = world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map(|c| c.siege_date)
        .unwrap_or(0);
    if stored != 0 {
        // The stored date wins even once it is **in the past** — that is how the
        // auto-task chain detects "the moment has arrived". Returning a derived
        // future slot instead would make `remaining` permanently positive.
        return stored;
    }
    world
        .data
        .siege_schedule
        .get(&castle_id)
        .filter(|e| e.enabled)
        .map(|e| next_siege_millis(now_millis, e.weekday, e.hour))
        .unwrap_or(0)
}

/// Whether the castle owner may still pick the siege hour (Java
/// `!isTimeRegistrationOver`).
pub(super) fn can_pick_siege_time(world: &World, castle_id: i32) -> bool {
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .is_some_and(|c| !c.time_registration_over)
}

/// One hop of Java's `ScheduleStartSiegeTask.run()`.
///
/// Every wake-up **re-reads the siege date** and decides afresh: re-arm closer,
/// or start. That is what makes the owner's chosen hour work — a hop armed
/// against the fixed schedule wakes up, sees the new date, and re-arms for it.
///
/// The ladder is Java's, rung for rung. Each rung wakes at the moment the
/// *next* rung's threshold is reached, so the chain converges on the siege
/// instead of spinning.
pub(crate) fn handle_scheduled_siege_start(world: &mut World, castle_id: i32) {
    run_auto_task(world, castle_id, commons::util::now_millis());
}

/// [`handle_scheduled_siege_start`] with the clock passed in.
///
/// The seam is load-bearing for tests, not cosmetic: the chain only converges
/// because real time passes between hops. Driving it with a fixed wall clock
/// re-computes the same rung forever, so a test that "fires the handler N
/// times" would spin rather than reach the siege.
pub(crate) fn run_auto_task(world: &mut World, castle_id: i32, now: i64) {
    // Java's first line: a siege already running owns the castle's state.
    if world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }

    // The owner's hour-picking window, which closes 24 h after the last siege
    // ended. While it is open the chain just waits it out.
    if can_pick_siege_time(world, castle_id) {
        let reg_remaining = time_registration_end_millis(world, castle_id) - now;
        if reg_remaining > 0 {
            arm_auto_task(world, castle_id, reg_remaining);
            return;
        }
        // Java `endTimeRegistration(true)` — the automatic close does **not**
        // save; the flag is re-derived at the next boot from the siege date.
        if let Some(c) = world.castles.iter_mut().find(|c| c.id == castle_id) {
            c.time_registration_over = true;
        }
    }

    let siege_at = effective_siege_millis(world, castle_id, now);
    if siege_at == 0 {
        // No schedule and no chosen date: GM-driven only, so there is nothing
        // to converge on. Java never arms a chain for such a castle either.
        return;
    }
    let remaining = siege_at - now;

    if remaining > AUTO_TASK_DAY_MS {
        arm_auto_task(world, castle_id, remaining - AUTO_TASK_DAY_MS);
    } else if remaining > AUTO_TASK_REG_END_MS {
        // The 24 h rung is where attacker/defender registration closes.
        broadcast_sm(
            world,
            sm_ids::THE_REGISTRATION_TERM_FOR_S1_HAS_ENDED,
            castle_id,
        );
        clear_siege_waiting_clans(world, castle_id);
        arm_auto_task(world, castle_id, remaining - AUTO_TASK_REG_END_MS);
    } else if remaining > AUTO_TASK_10_MIN_MS {
        arm_auto_task(world, castle_id, remaining - AUTO_TASK_10_MIN_MS);
    } else if remaining > AUTO_TASK_5_MIN_MS {
        arm_auto_task(world, castle_id, remaining - AUTO_TASK_5_MIN_MS);
    } else if remaining > AUTO_TASK_10_SEC_MS {
        arm_auto_task(world, castle_id, remaining - AUTO_TASK_10_SEC_MS);
    } else if remaining > 0 {
        arm_auto_task(world, castle_id, remaining);
    } else {
        start_siege(world, castle_id);
        // The owner's one-off chosen time is spent. Roll the stored date to the
        // next scheduled slot so the SiegeInfo window and the registration
        // cut-off describe the *next* siege rather than the one just begun.
        set_next_siege_date(world, castle_id, now);
        // Restart the chain for the next cycle. It re-reads, so this one call
        // covers both the fixed schedule and any hour the owner picks later.
        arm_auto_task(world, castle_id, 0);
    }
}

/// Java `Siege.getTimeRegistrationOverDate()` — the deadline for the owner to
/// pick the siege hour, stamped `now + 1 day` when the previous siege ended
/// (`castle.regTimeEnd`).
fn time_registration_end_millis(world: &World, castle_id: i32) -> i64 {
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map(|c| c.siege_time_registration_end)
        .unwrap_or(0)
}

/// Java `Siege.clearSiegeWaitingClan()` — drop every defender still awaiting the
/// owner's approval (`siege_clans.type = 2`) once registration closes. An
/// unapproved defender does not become one by default.
fn clear_siege_waiting_clans(world: &mut World, castle_id: i32) {
    let waiting: Vec<i32> = world
        .sieges
        .get(&castle_id)
        .map(|s| {
            s.clans
                .iter()
                .filter(|c| c.kind == SiegeClanType::DefenderPending)
                .map(|c| c.clan_id)
                .collect()
        })
        .unwrap_or_default();
    if waiting.is_empty() {
        return;
    }
    if let Some(s) = world.sieges.get_mut(&castle_id) {
        s.clans.retain(|c| c.kind != SiegeClanType::DefenderPending);
    }
    for clan_id in waiting {
        let _ = world
            .db
            .send(DbCommand::RemoveSiegeClan { castle_id, clan_id });
    }
}
