//! The in-game clock (Java `taskmanager/GameTimeTaskManager`, G33).
//!
//! One in-game day is 4 real hours (`IG_DAYS_PER_DAY = 6`), so a game-minute is
//! 10 real seconds. The client tracks time on its own from the `game_time`
//! (game-minutes since in-game midnight, 0..1439) that `CharSelected` carries —
//! this milestone just stops sending it as a permanent `0` (midnight).
//!
//! Java anchors its reference to **local midnight of the boot day**; the port
//! needs no stored anchor. A real-day (86 400 000 ms) is exactly `IG_DAYS_PER_DAY`
//! in-game days (6 × 14 400 000 ms), so any midnight is a whole number of
//! in-game days past the Unix epoch — i.e. `≡ 0 (mod MILLIS_PER_IG_DAY)`. The
//! `% MILLIS_PER_IG_DAY` phase is therefore identical whether measured from
//! boot-day midnight or from the epoch, so the clock is a pure function of
//! `System.currentTimeMillis()`.

/// Real milliseconds per game-minute (`MILLIS_IN_TICK * 100`, i.e. 10 s).
const MILLIS_PER_GAME_MINUTE: i64 = 10_000;
/// Game-minutes in an in-game day (24 h × 60).
const GAME_MINUTES_PER_DAY: i64 = 1440;
/// The in-game hour before which it is night (Java `_gameHour < 6`).
const NIGHT_UNTIL_HOUR: i32 = 6;

/// Game-minutes since in-game midnight (0..1439) at `now` unix-millis — the
/// value `CharSelected` sends. Pure, so the caller's clock is the only input.
pub(crate) fn game_time_minutes_at(now_millis: i64) -> i32 {
    ((now_millis / MILLIS_PER_GAME_MINUTE).rem_euclid(GAME_MINUTES_PER_DAY)) as i32
}

/// The current in-game time in game-minutes (Java `getGameTime`).
pub(crate) fn game_time_minutes() -> i32 {
    game_time_minutes_at(commons::util::now_millis())
}

/// Whether it is currently night in-game (Java `isNight` — hour < 6). The
/// day/night state the spawn/effect scripts (`DayNightSpawns`, `NightStatModify`)
/// will read once ported.
#[allow(
    dead_code,
    reason = "day/night query wired when the day/night scripts land"
)]
pub(crate) fn is_night_at(now_millis: i64) -> bool {
    game_time_minutes_at(now_millis) / 60 < NIGHT_UNTIL_HOUR
}

#[cfg(test)]
mod tests {
    use super::*;

    const IG_DAY_MS: i64 = 14_400_000; // 4 real hours

    #[test]
    fn midnight_is_zero_and_the_day_wraps() {
        // Any exact in-game-day boundary reads as game-midnight.
        assert_eq!(game_time_minutes_at(0), 0);
        assert_eq!(game_time_minutes_at(IG_DAY_MS), 0);
        assert_eq!(game_time_minutes_at(6 * IG_DAY_MS), 0); // a real-day midnight
                                                            // 10 real seconds = one game-minute; 600 s = 60 game-min = 01:00.
        assert_eq!(game_time_minutes_at(10_000), 1);
        assert_eq!(game_time_minutes_at(600_000), 60);
        // Just before the wrap: 1439 game-min (23:59), then back to 0.
        assert_eq!(
            game_time_minutes_at(IG_DAY_MS - MILLIS_PER_GAME_MINUTE),
            1439
        );
    }

    #[test]
    fn night_is_before_the_sixth_in_game_hour() {
        // 00:00..05:59 night (game-min 0..359); 06:00 (game-min 360) day.
        assert!(is_night_at(0));
        assert!(is_night_at(359 * MILLIS_PER_GAME_MINUTE));
        assert!(!is_night_at(360 * MILLIS_PER_GAME_MINUTE));
        assert!(!is_night_at(720 * MILLIS_PER_GAME_MINUTE)); // noon
    }
}
