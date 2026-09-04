//! The wall-clock daily reset (Java `instancemanager/DailyTaskManager.onReset`,
//! G33): fired at 06:30 each day, it runs the recommends reset and the vitality
//! refill (weekly-full on Wednesday, else the daily add), for online **and**
//! offline characters, then reschedules itself 24 h out.
//!
//! The port computes 06:30 in **UTC** (not server-local like Java's `Calendar`),
//! so "Wednesday" here is UTC-Wednesday — consistent with the reco reset this
//! generalises.

use crate::game_loop::time::MILLIS_PER_DAY;
use crate::scheduler::ScheduledTask;
use crate::scheduler::ms_to_ticks;
use crate::world::World;

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
    // Wednesday → clan-leader transfers + full vitality; any other day → the
    // daily vitality add (Java's `Calendar.DAY_OF_WEEK == WEDNESDAY` branch).
    run_reset(world, is_weekly_reset_day(commons::util::now_millis()));

    world
        .scheduler
        .schedule(world.tick + DAILY_RESET_PERIOD, ScheduledTask::DailyReset);
}

/// The sub-resets themselves, with the weekday decided by the caller — split
/// out so a test can run either branch without moving the wall clock.
pub(crate) fn run_reset(world: &mut World, weekly: bool) {
    // Java `onReset`'s first line: stamp the reset. Nothing in this chronicle
    // reads it back (see `schedule_initial_daily_reset`), but writing it keeps
    // the stored state honest and is what a fixed catch-up would need.
    super::global_vars::set(
        world,
        super::global_vars::DAILY_TASK_RESET,
        commons::util::now_millis(),
    );
    if weekly {
        clan_leader_apply(world);
    }
    crate::game_loop::character::vitality::reset_vitality(world, weekly);
    crate::game_loop::character::reco::reset_recommends(world);
    reset_world_chat_points(world);
    // `TaskBirthday` is a separate `TYPE_GLOBAL_TASK` in Java, registered at
    // the same "06:30:00" this beat runs on. It reads the stamp set above, so
    // it goes after it: a run that was missed is caught up from there.
    super::birthday::check_birthdays(world);
}

/// Java `DailyTaskManager.resetWorldChatPoints`: zero every character's spent
/// world-chat quota — the offline population through one unfiltered UPDATE,
/// then the online one in memory, each with a fresh `ExWorldChatCnt`.
///
/// Gated on `WorldChatEnabled` as upstream, so with the channel off neither
/// half runs and no stored counter is disturbed.
///
/// **The reset clock and the quota message disagree, in Java too.** The
/// client's line reads "a new day starts every day at 18:30" while
/// `DailyTaskManager` fires at 06:30. The string is the client's and the
/// schedule is the server's; the port keeps both rather than "fixing" either.
fn reset_world_chat_points(world: &mut World) {
    if !world.cfg.general.world_chat_enabled {
        return;
    }

    // Offline population (Java's single UPDATE).
    let _ = world.db.send(crate::db::DbCommand::ResetWorldChatPoints);

    // Online players: `setWorldChatUsed(0)` + `ExWorldChatCnt`. Java also calls
    // `getVariables().storeMe()`; here the memory-first autosave carries the
    // variable map with the rest of the character, so the in-memory write above
    // is the whole persistence story.
    let online: Vec<i32> = world.in_game_player_oids().collect();
    for oid in online {
        crate::game_loop::helpers::set_player_var_int(
            world,
            oid,
            crate::model::components::player::WORLD_CHAT_USED,
            0,
        );
        let left = crate::game_loop::social::chat::world_chat_points_left(world, oid);
        crate::game_loop::helpers::send_to_player(
            world,
            oid,
            crate::network::server_packets::ex_world_chat_cnt(left),
        );
    }
}

/// Java `DailyTaskManager.clanLeaderApply`: every clan carrying a pending
/// `new_leader_id` hands leadership over, provided the nominee is still a
/// member (Java `getClanMember(newLeaderId) == null → continue`, so a nominee
/// who left keeps the clan's stamp rather than clearing it — the transfer just
/// never fires).
///
/// This is the delivery half of the delegated transfer stamped by the village
/// master's `change_clan_leader` bypass; `clans::force_new_leader` is the port's
/// `Clan.setNewLeader`, so it also clears the stamp and persists the row.
fn clan_leader_apply(world: &mut World) {
    let pending: Vec<(i32, i32)> = world
        .clans
        .values()
        .filter(|c| c.new_leader_id != 0)
        .filter(|c| c.members.iter().any(|m| m.char_id == c.new_leader_id))
        .map(|c| (c.id, c.new_leader_id))
        .collect();
    let applied = pending.len();
    for (clan_id, new_leader) in pending {
        crate::game_loop::clans::force_new_leader(world, clan_id, new_leader);
    }
    if applied > 0 {
        tracing::info!("DailyTaskManager: {applied} clan leader(s) have been updated.");
    }
}

/// Schedule the first `DailyReset` for the next 06:30 UTC (Java
/// `DailyTaskManager`'s constructor). Called once at game-loop start.
///
/// Java's boot "catch-up" is **not** ported, because as written it cannot fire.
/// `DailyTaskManager`'s constructor computes `calendarTime` = the *next* 06:30
/// (today's if still ahead, else tomorrow's) and then runs `onReset()` only
/// when the stored `DAILY_TASK_RESET` stamp is **not** less than it. The stamp
/// is always a past timestamp and `calendarTime` is always in the future, so
/// the comparison is true and the catch-up branch is dead — despite the comment
/// above it reading "Check if 24 hours have passed since the last daily reset",
/// which would need a comparison against the *previous* occurrence.
///
/// So a reset missed while the server was down runs at the next 06:30 in Java
/// too, which is what this port already did. The stamp itself **is** written
/// (below) so the value is there if a later chronicle fixes the comparison.
pub(crate) fn schedule_initial_daily_reset(world: &mut World) {
    let now = commons::util::now_millis();
    let ms_of_day = now.rem_euclid(MILLIS_PER_DAY);
    let mut delay_ms = DAILY_RESET_MS_OF_DAY - ms_of_day;
    if delay_ms < 0 {
        delay_ms += MILLIS_PER_DAY;
    }
    let delay_ticks = ms_to_ticks(delay_ms);
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
