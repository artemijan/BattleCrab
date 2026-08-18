//! The manor period scheduler: ModeTimes, the boot mode, and the daily
//! Maintenance → Modifiable → Approved transitions.

// ---------------------------------------------------------------------------
// Period scheduler — port of `CastleManorManager`'s wall-clock mode machine.
// ---------------------------------------------------------------------------

use super::arm_autosave;
use super::persist::notify_leader;
use super::persist::store_manor;
use super::settlement::charge_next_period;
use super::settlement::gate_next_period_on_treasury;
use super::settlement::owned_manor_castles;
use super::settlement::settle_closing_period;
/// The five daily cutover times (from `General.ini`), pulled off config.
use crate::game_loop::time::MILLIS_PER_DAY;
use crate::game_loop::time::MILLIS_PER_HOUR;
use crate::game_loop::time::MILLIS_PER_MINUTE;
use crate::model::manor::ManorMode;
use crate::network::server_packets::sm_ids;
use crate::scheduler::ScheduledTask;
use crate::world::World;
#[derive(Debug, Clone, Copy)]
pub(super) struct ModeTimes {
    pub(super) refresh_h: i32,
    pub(super) refresh_m: i32,
    pub(super) maintenance_m: i32,
    pub(super) approve_h: i32,
    pub(super) approve_m: i32,
}

pub(super) fn mode_times(world: &World) -> ModeTimes {
    let g = &world.cfg.general;
    ModeTimes {
        refresh_h: g.alt_manor_refresh_time,
        refresh_m: g.alt_manor_refresh_min,
        maintenance_m: g.alt_manor_maintenance_min,
        approve_h: g.alt_manor_approve_time,
        approve_m: g.alt_manor_approve_min,
    }
}

fn daily_millis(now_millis: i64, hour: i32, minute: i32) -> i64 {
    let day = now_millis.div_euclid(MILLIS_PER_DAY);
    day * MILLIS_PER_DAY + hour as i64 * MILLIS_PER_HOUR + minute as i64 * MILLIS_PER_MINUTE
}

/// Port of `CastleManorManager` init's wall-clock mode guess. The `refresh`
/// clause's `min >= maintenanceMin` check ignores the hour (a Java quirk kept
/// verbatim — the immediate-fire cascade in [`arm_next_mode_change`] corrects
/// any wrong guess within a tick or two).
pub(super) fn boot_mode(now_millis: i64, t: ModeTimes) -> ManorMode {
    let day = now_millis.div_euclid(MILLIS_PER_DAY);
    let mins_into_day = (now_millis - day * MILLIS_PER_DAY) / MILLIS_PER_MINUTE;
    let hour = (mins_into_day / 60) as i32;
    let min = (mins_into_day % 60) as i32;
    let maintenance_min = t.refresh_m + t.maintenance_m;
    if (hour >= t.refresh_h && min >= maintenance_min)
        || hour < t.approve_h
        || (hour == t.approve_h && min <= t.approve_m)
    {
        ManorMode::Modifiable
    } else if hour == t.refresh_h && min >= t.refresh_m && min < maintenance_min {
        ManorMode::Maintenance
    } else {
        ManorMode::Approved
    }
}

/// Port of `scheduleModeChange`'s next-change time for the *current* mode. Only
/// `MODIFIABLE` gets Java's "+1 day if already past" guard; `APPROVED`/
/// `MAINTENANCE` return today's time even when past, so a stale boot mode
/// fires immediately and cascades to the right one (Java's `Math.max(0, …)`).
pub(super) fn next_mode_change_millis(mode: ManorMode, now_millis: i64, t: ModeTimes) -> i64 {
    match mode {
        ManorMode::Modifiable => {
            let at = daily_millis(now_millis, t.approve_h, t.approve_m);
            if at < now_millis {
                at + MILLIS_PER_DAY
            } else {
                at
            }
        }
        ManorMode::Maintenance => {
            daily_millis(now_millis, t.refresh_h, t.refresh_m + t.maintenance_m)
        }
        // APPROVED (and the DISABLED fallback, which is never scheduled).
        _ => daily_millis(now_millis, t.refresh_h, t.refresh_m),
    }
}

/// Set the initial mode from the wall clock and arm the first change — the data
/// half of `CastleManorManager` init. When the manor is disabled the mode is
/// `DISABLED` and nothing is scheduled (Java's `else` branch). Called from the
/// `ManorLoaded` boot handler.
pub(crate) fn schedule_manor_at_boot(world: &mut World) {
    if !world.cfg.general.allow_manor {
        world.manor.set_mode(ManorMode::Disabled);
        return;
    }
    let now = commons::util::now_millis();
    let mode = boot_mode(now, mode_times(world));
    world.manor.set_mode(mode);
    arm_next_mode_change(world, now);
    // Java arms the autosave in the same `load()` that sets the mode, and only
    // when per-action saving is off.
    arm_autosave(world);
}

/// The wall-clock instant the current mode is scheduled to end — Java
/// `CastleManorManager._nextModeChange`, which it keeps as a field. The port
/// derives it instead, from the same function that arms the timer, so the two
/// cannot drift apart.
pub(crate) fn next_mode_change_at(world: &World, now_millis: i64) -> i64 {
    next_mode_change_millis(world.manor.mode(), now_millis, mode_times(world))
}

fn arm_next_mode_change(world: &mut World, now_millis: i64) {
    let at = next_mode_change_millis(world.manor.mode(), now_millis, mode_times(world));
    crate::game_loop::time::schedule_in_ms(world, at - now_millis, ScheduledTask::ManorModeChange);
}

/// Port of `CastleManorManager.changeMode` — advance the period, run the
/// settlement that rides on each transition, and re-arm the next change.
///
/// - **APPROVED → MAINTENANCE**: settle the closing period (crops bought get
///   paid into the owner's clan warehouse, unspent crop reservations go back to
///   the treasury), roll next → current, then gate the *new* next period on the
///   treasury covering it. Java `storeMe()`s afterwards; so does the port.
/// - **MAINTENANCE → MODIFIABLE**: tell each owner's online leader the manor
///   information was updated.
/// - **MODIFIABLE → APPROVED**: charge the next period's manor cost, or clear
///   the setup and warn the leader when it can't be afforded *and* the
///   warehouse has no room.
pub(crate) fn advance_manor_mode(world: &mut World) {
    let next_mode = match world.manor.mode() {
        ManorMode::Approved => {
            for castle_id in owned_manor_castles(world) {
                settle_closing_period(world, castle_id);
                world.manor.roll_period(castle_id);
                gate_next_period_on_treasury(world, castle_id);
                store_manor(world, castle_id);
            }
            ManorMode::Maintenance
        }
        ManorMode::Maintenance => {
            for castle_id in owned_manor_castles(world) {
                notify_leader(
                    world,
                    castle_id,
                    sm_ids::THE_MANOR_INFORMATION_HAS_BEEN_UPDATED,
                );
            }
            ManorMode::Modifiable
        }
        ManorMode::Modifiable => {
            for castle_id in owned_manor_castles(world) {
                charge_next_period(world, castle_id);
            }
            // Java only `storeMe()`s here under `ALT_MANOR_SAVE_ALL_ACTIONS`
            // (off on this dist), so nothing is written.
            ManorMode::Approved
        }
        // A disabled manor never scheduled a change; nothing to do.
        ManorMode::Disabled => return,
    };
    world.manor.set_mode(next_mode);
    arm_next_mode_change(world, commons::util::now_millis());
}
