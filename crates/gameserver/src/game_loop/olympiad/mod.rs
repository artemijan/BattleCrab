//! Grand Olympiad (G25) — registration, persistence and the period state
//! machine. Java `model/olympiad/{Olympiad, OlympiadManager}`.
//!
//! Covers: `register`/`unregister` into the waiting lists with the eligibility
//! and timing gates; boot load / shutdown save of `olympiad_data` +
//! `olympiad_nobles`; the competition-window schedule (18:00 for 6 h on the
//! weekend competition days) plus the weekly point/match refresh; and the
//! match-making sweep that pairs waiting nobles into stadium matches; and the
//! match run itself — the fighters are ported to the arena, the bout is polled
//! to a result, points transferred and win/loss/draw recorded, and everyone
//! ported back; and the monthly round transitions — at the month end the
//! period flips to validation, the class leaders are crowned heroes, and after
//! the validation day a fresh cycle begins with a clean noble table. The crown
//! persists to the `heroes` table and re-applies on login (so it survives relogs
//! and reaches offline heroes). A match runs the full pre-fight ceremony (the
//! teleport + battle countdowns with their announcements), strips the fighters'
//! buffs on entry, and announces the round's end to everyone online. Only the
//! stadium instancing (needs G27) remains a follow-up.

use crate::config::OlympiadConfig;
use crate::db::{DbCommand, HeroRow, OlympiadEomRow, OlympiadNobleRow};
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::ms_to_ticks;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::send_sm_bare_to_player as send_sm;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::time::MILLIS_PER_DAY;
use crate::model::Player;
use crate::model::components::OlympiadObserver;
use crate::model::olympiad::{
    CompetitionType, NobleStats, OlympiadMatch, OlympiadState, REG_CLOSE_BEFORE_END_MS,
};
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

// --- the competition-period state machine (dist `config/Olympiad.ini`) ---

/// How often the match-making sweep runs while the window is open (Java
/// `OlympiadGameManager` fixed rate).
const GAME_MANAGER_PERIOD_MS: i64 = 30_000;
/// Stadiums available for concurrent matches (Java one `OlympiadGameTask` per
/// `OlympiadStadiumZone`; `zones/olympiad_stadium.xml` defines four).
const NUM_ARENAS: usize = 4;

/// Java's `LIMIT 10` on every class-leaderboard query. The page has fifteen
/// rows; the rest are blanked.
const LEADER_BOARD_LIMIT: usize = 10;
/// The hero animation played on claiming (Java `new SocialAction(id, 20016)`).
const HERO_SOCIAL_ACTION: i32 = 20016;
/// Java `Hero.ACTION_HERO_GAINED` — the diary entry written by `setHeroGained`.
const HERO_ACTION_GAINED_HERO: i32 = 2;
/// Noon, as milliseconds past midnight (Java `setNewOlympiadEnd` anchors the
/// end at `HOUR_OF_DAY 12`).
const NOON_MS_OF_DAY: i64 = 12 * 3600 * 1000;

/// `Olympiad.setNewOlympiadEnd`'s `DAY` branch: noon today plus
/// `(multiplier - 1)` days (the final day is reserved for validation).
pub(crate) fn next_olympiad_end(cfg: &OlympiadConfig, now_ms: i64) -> i64 {
    let noon_today = now_ms - ms_of_day(now_ms) + NOON_MS_OF_DAY;
    noon_today + (cfg.period_days() - 1) * MILLIS_PER_DAY
}

/// The per-character variable holding points earned this round but not yet
/// exchanged for marks (Java `Olympiad.UNCLAIMED_OLYMPIAD_POINTS_VAR`).
pub(crate) const UNCLAIMED_POINTS_VAR: &str = "UNCLAIMED_OLYMPIAD_POINTS";

/// Day of week for an epoch-millis instant, 0 = Sunday … 6 = Saturday (epoch
/// day 0, 1970-01-01, was a Thursday → offset 4).
fn day_of_week(now_ms: i64) -> i64 {
    (now_ms.div_euclid(MILLIS_PER_DAY) + 4).rem_euclid(7)
}

fn ms_of_day(now_ms: i64) -> i64 {
    now_ms.rem_euclid(MILLIS_PER_DAY)
}

/// Whether `now_ms` falls inside a competition window (a competition day,
/// between 18:00 and 18:00 + 6 h).
pub(crate) fn in_comp_window(cfg: &OlympiadConfig, now_ms: i64) -> bool {
    let start = cfg.comp_start_ms_of_day();
    cfg.competition_days.contains(&day_of_week(now_ms))
        && (start..start + cfg.comp_period_ms).contains(&ms_of_day(now_ms))
}

/// The epoch-millis instant the window covering `now_ms` closes.
fn window_end(cfg: &OlympiadConfig, now_ms: i64) -> i64 {
    now_ms - ms_of_day(now_ms) + cfg.comp_start_ms_of_day() + cfg.comp_period_ms
}

/// Milliseconds from `now_ms` to the next competition-day 18:00 strictly in the
/// future (Java `getMillisToCompBegin` / `setNewCompBegin`).
pub(crate) fn next_comp_start_delay_ms(cfg: &OlympiadConfig, now_ms: i64) -> i64 {
    let today_start = now_ms - ms_of_day(now_ms) + cfg.comp_start_ms_of_day();
    for d in 0..8 {
        let candidate = today_start + d * MILLIS_PER_DAY;
        if candidate > now_ms && cfg.competition_days.contains(&day_of_week(candidate)) {
            return candidate - now_ms;
        }
    }
    MILLIS_PER_DAY // unreachable (a competition day always falls within a week)
}

/// Convert a wall-clock delay to a scheduler fire tick (>= next tick).
fn fire_at(world: &World, delay_ms: i64) -> u64 {
    // `.max(1)` on the *ticks*: the Olympiad poll must always land on a later
    // tick, or a zero-delay reschedule would spin inside one tick.
    world.tick + ms_to_ticks(delay_ms).max(1)
}

/// Arm the competition-window and weekly-refresh schedules at boot (Java
/// `Olympiad.init` + `scheduleWeeklyChange`). Called once the persisted state
/// has been applied.
pub(crate) fn schedule_at_boot(world: &mut World) {
    let now = commons::util::now_millis();

    // Weekly refresh: at the persisted instant (or soon, if it has passed).
    let wk_delay = world.olympiad.next_weekly_change - now;
    world.scheduler.schedule(
        fire_at(world, wk_delay),
        ScheduledTask::OlympiadWeeklyChange,
    );

    if world.olympiad.period == 0 {
        // Competition period: arm the window + the month-end round transition.
        arm_comp_schedule(world, now);
        if world.olympiad.olympiad_end <= now {
            // Fresh install, or the end elapsed while the server was down: set a
            // new period boundary rather than ending instantly.
            world.olympiad.olympiad_end = next_olympiad_end(&world.cfg.olympiad, now);
        }
        world.scheduler.schedule(
            fire_at(world, world.olympiad.olympiad_end - now),
            ScheduledTask::OlympiadEnd,
        );
    } else {
        // Validation period: only the transition back to a new cycle is armed.
        world.scheduler.schedule(
            fire_at(world, world.olympiad.validation_end - now),
            ScheduledTask::OlympiadValidationEnd,
        );
    }
}

/// Arm the daily competition window (Java `Olympiad.init`): open it now if we're
/// inside one, otherwise schedule the next start.
fn arm_comp_schedule(world: &mut World, now: i64) {
    if in_comp_window(&world.cfg.olympiad, now) {
        open_comp_window(world, now);
    } else {
        world.scheduler.schedule(
            fire_at(world, next_comp_start_delay_ms(&world.cfg.olympiad, now)),
            ScheduledTask::OlympiadCompStart,
        );
    }
}

/// Open the window: registration/matches are allowed until it closes.
fn open_comp_window(world: &mut World, now: i64) {
    world.olympiad.in_comp_period = true;
    world.olympiad.comp_end_tick = fire_at(world, window_end(&world.cfg.olympiad, now) - now);
    world
        .scheduler
        .schedule(world.olympiad.comp_end_tick, ScheduledTask::OlympiadCompEnd);
    tracing::info!("Olympiad: competition window open.");
    // `OlympiadGameManager` starts sweeping for matches (Java scheduleAtFixedRate).
    world.scheduler.schedule(
        fire_at(world, GAME_MANAGER_PERIOD_MS),
        ScheduledTask::OlympiadGameManager,
    );
}

/// `OlympiadCompStart`: begin the day's competition window.
pub(crate) fn handle_comp_start(world: &mut World) {
    if world.olympiad.period != 0 {
        return;
    }
    open_comp_window(world, commons::util::now_millis());
}

/// `OlympiadCompEnd`: close the window and schedule the next one.
pub(crate) fn handle_comp_end(world: &mut World) {
    world.olympiad.in_comp_period = false;
    // Java also clears the waiting lists (and any lingering games) at comp end.
    world.olympiad.non_class_registers.clear();
    world.olympiad.class_registers.clear();
    world.olympiad.matches.clear();
    world.olympiad.in_competition.clear();
    tracing::info!("Olympiad: competition window closed.");
    let now = commons::util::now_millis();
    world.scheduler.schedule(
        fire_at(world, next_comp_start_delay_ms(&world.cfg.olympiad, now)),
        ScheduledTask::OlympiadCompStart,
    );
}

/// `OlympiadWeeklyChange`: add the weekly points, reset the weekly match
/// counters (both skipped during the validation period), and reschedule.
pub(crate) fn handle_weekly_change(world: &mut World) {
    let (weekly_points, weekly_period) = (
        world.cfg.olympiad.weekly_points,
        world.cfg.olympiad.weekly_period_ms,
    );
    if world.olympiad.period == 0 {
        for noble in world.olympiad.nobles.values_mut() {
            noble.points += weekly_points;
            noble.comp_done_week = 0;
        }
    }
    let now = commons::util::now_millis();
    world.olympiad.next_weekly_change = now + weekly_period;
    world.scheduler.schedule(
        fire_at(world, weekly_period),
        ScheduledTask::OlympiadWeeklyChange,
    );
}

mod heroes;
mod matches;
mod observer;
mod registration;
mod season;

pub(crate) use heroes::*;
pub(crate) use matches::*;
pub(crate) use observer::*;
pub(crate) use registration::*;
pub(crate) use season::*;

/// Apply the boot-loaded `olympiad_data` + `olympiad_nobles` (Java
/// `Olympiad.load` / `loadNoblesRank`) into the live state.
pub(crate) fn apply_loaded(
    world: &mut World,
    current_cycle: i32,
    period: i32,
    olympiad_end: i64,
    validation_end: i64,
    next_weekly_change: i64,
    nobles: Vec<OlympiadNobleRow>,
    eom: Vec<OlympiadEomRow>,
) {
    let oly = &mut world.olympiad;
    oly.eom_nobles = eom;
    oly.current_cycle = current_cycle;
    oly.period = period;
    oly.olympiad_end = olympiad_end;
    oly.validation_end = validation_end;
    oly.next_weekly_change = next_weekly_change;
    oly.nobles = nobles
        .into_iter()
        .map(|n| {
            (
                n.char_id,
                NobleStats {
                    class_id: n.class_id,
                    // The saved name isn't in `olympiad_nobles`; it is filled in
                    // when the noble next registers (Java reads it via a join).
                    name: String::new(),
                    points: n.points,
                    comp_done: n.comp_done,
                    comp_won: n.comp_won,
                    comp_lost: n.comp_lost,
                    comp_drawn: n.comp_drawn,
                    comp_done_week: n.comp_done_week,
                },
            )
        })
        .collect();
    tracing::info!(
        "GameLoop: loaded Olympiad (cycle {current_cycle}, period {period}, {} nobles).",
        world.olympiad.nobles.len()
    );
}

/// `Olympiad.saveOlympiadStatus` + `saveNobleData` — persist the period row and
/// every noble record. Called on shutdown (and can be called on demand).
pub(crate) fn save_all(world: &World) {
    let oly = &world.olympiad;
    let nobles = oly
        .nobles
        .iter()
        .map(|(&char_id, n)| OlympiadNobleRow {
            char_id,
            class_id: n.class_id,
            points: n.points,
            comp_done: n.comp_done,
            comp_won: n.comp_won,
            comp_lost: n.comp_lost,
            comp_drawn: n.comp_drawn,
            comp_done_week: n.comp_done_week,
        })
        .collect();
    let _ = world.db.send(DbCommand::SaveOlympiad {
        current_cycle: oly.current_cycle,
        period: oly.period,
        olympiad_end: oly.olympiad_end,
        validation_end: oly.validation_end,
        next_weekly_change: oly.next_weekly_change,
        nobles,
    });
}
