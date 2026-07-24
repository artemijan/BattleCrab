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
//! the validation day a fresh cycle begins with a clean noble table. The
//! stadium instancing (needs G27), the countdown ceremony, and persisting the
//! hero crown to the `heroes` table (for relogs / offline heroes) are the
//! remaining follow-ups.

use crate::db::{DbCommand, OlympiadNobleRow};
use crate::model::olympiad::{
    CompetitionType, NobleStats, OlympiadMatch, OlympiadState, REG_CLOSE_BEFORE_END_MS,
};
use crate::model::Player;
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::world::World;

// --- the competition-period state machine (dist `config/Olympiad.ini`) ---

const MS_PER_DAY: i64 = 86_400_000;
/// `AltOlyStartTime = 18` (18:00), as milliseconds past midnight.
const COMP_START_MS_OF_DAY: i64 = 18 * 3600 * 1000;
/// `AltOlyCPeriod` — the competition window length (6 h).
const COMP_PERIOD_MS: i64 = 21_600_000;
/// `AltOlyWPeriod` — the weekly refresh interval (1 week).
const WEEKLY_PERIOD_MS: i64 = 604_800_000;
/// `AltOlyWeeklyPoints` — points added to every noble each week.
const WEEKLY_POINTS: i32 = 10;
/// `AltOlyCompetitionDays = 1,7` (Java `Calendar` Sun=1…Sat=7) → 0-indexed
/// days-of-week Sunday (0) and Saturday (6): the Olympiad runs weekends only.
const COMP_DAYS: &[i64] = &[0, 6];

/// How often the match-making sweep runs while the window is open (Java
/// `OlympiadGameManager` fixed rate).
const GAME_MANAGER_PERIOD_MS: i64 = 30_000;
/// Stadiums available for concurrent matches (Java one `OlympiadGameTask` per
/// `OlympiadStadiumZone`; `zones/olympiad_stadium.xml` defines four).
const NUM_ARENAS: usize = 4;
/// `AltOlyNonClassedParticipants = 20` — the non-class queue must hold at least
/// this many before any 1v1 matches are generated (Java
/// `hasEnoughRegisteredNonClassed`).
const NONCLASSED_MIN: usize = 20;

/// `AltOlyMinMatchesForPoints = 10` — matches needed to be hero-eligible.
const HERO_MIN_MATCHES: i32 = 10;
/// `AltOlyVPeriod` — the validation period after a round ends (24 h).
const VALIDATION_PERIOD_MS: i64 = 86_400_000;
/// The competition month length. TODO(G25): Java's `setNewOlympiadEnd` uses the
/// calendar month boundary; this is a 30-day approximation.
const OLYMPIAD_PERIOD_MS: i64 = 30 * MS_PER_DAY;

/// Day of week for an epoch-millis instant, 0 = Sunday … 6 = Saturday (epoch
/// day 0, 1970-01-01, was a Thursday → offset 4).
fn day_of_week(now_ms: i64) -> i64 {
    (now_ms.div_euclid(MS_PER_DAY) + 4).rem_euclid(7)
}

fn ms_of_day(now_ms: i64) -> i64 {
    now_ms.rem_euclid(MS_PER_DAY)
}

/// Whether `now_ms` falls inside a competition window (a competition day,
/// between 18:00 and 18:00 + 6 h).
pub(crate) fn in_comp_window(now_ms: i64) -> bool {
    COMP_DAYS.contains(&day_of_week(now_ms))
        && (COMP_START_MS_OF_DAY..COMP_START_MS_OF_DAY + COMP_PERIOD_MS)
            .contains(&ms_of_day(now_ms))
}

/// The epoch-millis instant the window covering `now_ms` closes.
fn window_end(now_ms: i64) -> i64 {
    now_ms - ms_of_day(now_ms) + COMP_START_MS_OF_DAY + COMP_PERIOD_MS
}

/// Milliseconds from `now_ms` to the next competition-day 18:00 strictly in the
/// future (Java `getMillisToCompBegin` / `setNewCompBegin`).
pub(crate) fn next_comp_start_delay_ms(now_ms: i64) -> i64 {
    let today_start = now_ms - ms_of_day(now_ms) + COMP_START_MS_OF_DAY;
    for d in 0..8 {
        let candidate = today_start + d * MS_PER_DAY;
        if candidate > now_ms && COMP_DAYS.contains(&day_of_week(candidate)) {
            return candidate - now_ms;
        }
    }
    MS_PER_DAY // unreachable (a competition day always falls within a week)
}

/// Convert a wall-clock delay to a scheduler fire tick (>= next tick).
fn fire_at(world: &World, delay_ms: i64) -> u64 {
    world.tick + (delay_ms.max(100) / 100) as u64
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
            // new month boundary rather than ending instantly.
            world.olympiad.olympiad_end = now + OLYMPIAD_PERIOD_MS;
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
    if in_comp_window(now) {
        open_comp_window(world, now);
    } else {
        world.scheduler.schedule(
            fire_at(world, next_comp_start_delay_ms(now)),
            ScheduledTask::OlympiadCompStart,
        );
    }
}

/// Open the window: registration/matches are allowed until it closes.
fn open_comp_window(world: &mut World, now: i64) {
    world.olympiad.in_comp_period = true;
    world.olympiad.comp_end_tick = fire_at(world, window_end(now) - now);
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
        fire_at(world, next_comp_start_delay_ms(now)),
        ScheduledTask::OlympiadCompStart,
    );
}

/// `OlympiadWeeklyChange`: add the weekly points, reset the weekly match
/// counters (both skipped during the validation period), and reschedule.
pub(crate) fn handle_weekly_change(world: &mut World) {
    if world.olympiad.period == 0 {
        for noble in world.olympiad.nobles.values_mut() {
            noble.points += WEEKLY_POINTS;
            noble.comp_done_week = 0;
        }
    }
    let now = commons::util::now_millis();
    world.olympiad.next_weekly_change = now + WEEKLY_PERIOD_MS;
    world.scheduler.schedule(
        fire_at(world, WEEKLY_PERIOD_MS),
        ScheduledTask::OlympiadWeeklyChange,
    );
}

/// `Olympiad.sortHerosToBe`: for each hero-title class (the `FOURTH_CLASS_GROUP`
/// category), the eligible noble with the most points becomes its hero. Eligible
/// = competitor on that class **or its parent 3rd class**, ≥ 10 matches, ≥ 1 win.
pub(crate) fn compute_heroes(world: &World) -> Vec<(i32, i32)> {
    let mut heroes = Vec::new();
    for hero_class in world.data.categories.ids("FOURTH_CLASS_GROUP") {
        let parent = world.data.skill_trees.parent_class(hero_class);
        let best = world
            .olympiad
            .nobles
            .iter()
            .filter(|(_, n)| {
                (n.class_id == hero_class || Some(n.class_id) == parent)
                    && n.comp_done >= HERO_MIN_MATCHES
                    && n.comp_won > 0
            })
            .max_by(|(_, a), (_, b)| {
                a.points
                    .cmp(&b.points)
                    .then(a.comp_done.cmp(&b.comp_done))
                    .then(a.comp_won.cmp(&b.comp_won))
            });
        if let Some((&char_id, _)) = best {
            heroes.push((char_id, hero_class));
        }
    }
    heroes
}

/// `OlympiadEndTask`: the monthly round ends — enter the validation period,
/// crown the new heroes, and schedule the return to a fresh cycle.
pub(crate) fn handle_olympiad_end(world: &mut World) {
    if world.olympiad.period != 0 {
        return;
    }
    world.olympiad.period = 1;

    // Uncrown the previous cycle's (online) heroes, then crown the new ones.
    let old: Vec<i32> = world.olympiad.heroes.iter().map(|(id, _)| *id).collect();
    for id in old {
        if is_online(world, id) {
            crate::game_loop::admin::hero::set_hero(world, id, false);
        }
    }
    let heroes = compute_heroes(world);
    for &(id, _) in &heroes {
        if is_online(world, id) {
            crate::game_loop::admin::hero::set_hero(world, id, true);
        }
    }
    world.olympiad.heroes = heroes;
    tracing::info!(
        "Olympiad: round {} ended; {} heroes crowned.",
        world.olympiad.current_cycle,
        world.olympiad.heroes.len()
    );
    // TODO(G25): broadcast ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED to all
    // online players, and persist the crown to the `heroes` table so it survives
    // relogs / applies to offline heroes on login.

    let now = commons::util::now_millis();
    world.olympiad.validation_end = now + VALIDATION_PERIOD_MS;
    save_all(world);
    world.scheduler.schedule(
        fire_at(world, VALIDATION_PERIOD_MS),
        ScheduledTask::OlympiadValidationEnd,
    );
}

/// `ValidationEndTask`: the validation period ends — start a new cycle's
/// competition period with a clean noble table.
pub(crate) fn handle_validation_end(world: &mut World) {
    world.olympiad.period = 0;
    world.olympiad.current_cycle += 1;
    world.olympiad.nobles.clear(); // `deleteNobles` (TRUNCATE olympiad_nobles)
    let now = commons::util::now_millis();
    world.olympiad.olympiad_end = now + OLYMPIAD_PERIOD_MS;
    save_all(world);
    tracing::info!(
        "Olympiad: validation ended; cycle {} begins.",
        world.olympiad.current_cycle
    );
    // Re-arm the competition window + the next month-end.
    arm_comp_schedule(world, now);
    world.scheduler.schedule(
        fire_at(world, OLYMPIAD_PERIOD_MS),
        ScheduledTask::OlympiadEnd,
    );
}

/// `OlympiadGameManager.run`: while the window is open, fill the free stadiums
/// with 1v1 matches drawn from the non-class queue, then reschedule. Stops when
/// the window closes.
pub(crate) fn handle_game_manager(world: &mut World) {
    if !world.olympiad.in_comp_period {
        return; // window closed — the sweep stops until it reopens
    }
    make_matches(world);
    world.scheduler.schedule(
        fire_at(world, GAME_MANAGER_PERIOD_MS),
        ScheduledTask::OlympiadGameManager,
    );
}

/// Pair waiting nobles into the free stadiums. Only runs once the non-class
/// queue is large enough (Java `hasEnoughRegisteredNonClassed`); then each free
/// arena takes a 2-player game until the queue runs dry.
fn make_matches(world: &mut World) {
    if world.olympiad.non_class_registers.len() < NONCLASSED_MIN {
        return;
    }
    // The stadium slots already busy with a running match.
    let mut busy: Vec<bool> = vec![false; NUM_ARENAS];
    for m in &world.olympiad.matches {
        if let Some(slot) = busy.get_mut(m.arena) {
            *slot = true;
        }
    }
    for (arena, &is_busy) in busy.iter().enumerate() {
        if is_busy {
            continue;
        }
        let Some((player_a, player_b)) = draw_pair(world) else {
            break; // not enough online players left in the queue
        };
        world.olympiad.in_competition.insert(player_a);
        world.olympiad.in_competition.insert(player_b);
        start_match(world, arena, player_a, player_b);
    }
}

/// The single grassy-arena spawn points (`zones/olympiad_stadium.xml`), for
/// player one and player two. TODO(G25): the four stadiums are separate
/// instances (needs G27); until then matches share these coordinates.
const ARENA_SPAWN_A: (i32, i32, i32) = (-89597, -252841, -3320);
const ARENA_SPAWN_B: (i32, i32, i32) = (-86544, -252846, -3320);
/// `AltOlyBattle` — the battle length (5 min); an undecided fight is a draw.
const BATTLE_MS: i64 = 300_000;
/// How often a running match is polled for a result.
const MATCH_POLL_MS: i64 = 1000;
/// `AltOlyDividerNonClassed` / `AltOlyMaxPoints` — the point-transfer formula.
const POINT_DIVIDER: i32 = 5;
const MAX_TRANSFER_POINTS: i32 = 10;

/// The outcome of a match once it resolves.
enum MatchResult {
    Win { winner: i32, loser: i32 },
    Draw,
}

/// Begin a match: port both fighters to the arena (remembering where they came
/// from), then start polling for the result. TODO(G25): the Java countdown
/// ceremony + buff strip; here the fight is live immediately.
fn start_match(world: &mut World, arena: usize, player_a: i32, player_b: i32) {
    let return_a = position_of(world, player_a);
    let return_b = position_of(world, player_b);
    let deadline_tick = world.tick + (BATTLE_MS / 100) as u64;
    world.olympiad.matches.push(OlympiadMatch {
        arena,
        player_a,
        player_b,
        deadline_tick,
        return_a,
        return_b,
    });
    crate::game_loop::death::teleport_player(
        world,
        player_a,
        ARENA_SPAWN_A.0,
        ARENA_SPAWN_A.1,
        ARENA_SPAWN_A.2,
    );
    crate::game_loop::death::teleport_player(
        world,
        player_b,
        ARENA_SPAWN_B.0,
        ARENA_SPAWN_B.1,
        ARENA_SPAWN_B.2,
    );
    tracing::info!("Olympiad: match in arena {arena}: {player_a} vs {player_b}.");
    world.scheduler.schedule(
        fire_at(world, MATCH_POLL_MS),
        ScheduledTask::OlympiadMatchTick { arena },
    );
}

/// `OlympiadGameTask` poll: a fighter who died or vanished loses; both surviving
/// past the battle deadline is a draw. Otherwise keep watching.
pub(crate) fn handle_match_tick(world: &mut World, arena: usize) {
    let Some(m) = world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena == arena)
        .cloned()
    else {
        return; // already resolved
    };
    let a_gone = !is_online(world, m.player_a);
    let b_gone = !is_online(world, m.player_b);
    let a_dead = a_gone || is_dead(world, m.player_a);
    let b_dead = b_gone || is_dead(world, m.player_b);

    let result = match (a_dead, b_dead) {
        (false, false) if world.tick < m.deadline_tick => {
            // Battle still on — keep polling.
            world.scheduler.schedule(
                fire_at(world, MATCH_POLL_MS),
                ScheduledTask::OlympiadMatchTick { arena },
            );
            return;
        }
        (true, true) => MatchResult::Draw, // both down (or timeout with both alive → below)
        (true, false) => MatchResult::Win {
            winner: m.player_b,
            loser: m.player_a,
        },
        (false, true) => MatchResult::Win {
            winner: m.player_a,
            loser: m.player_b,
        },
        _ => MatchResult::Draw, // deadline reached, both alive
    };
    resolve_match(world, &m, &result);
}

/// Apply the result (Java `validateWinner`), port both fighters back, and clear
/// the match.
fn resolve_match(world: &mut World, m: &OlympiadMatch, result: &MatchResult) {
    match result {
        MatchResult::Win { winner, loser } => {
            let diff = point_transfer(world, *winner, *loser);
            update_noble(world, *winner, |n| {
                n.points += diff;
                n.comp_won += 1;
            });
            update_noble(world, *loser, |n| {
                n.points = (n.points - diff).max(0);
                n.comp_lost += 1;
            });
            send_sm(world, *winner, sm_ids::CONGRATULATIONS_C1_YOU_WIN_THE_MATCH);
        }
        MatchResult::Draw => {
            update_noble(world, m.player_a, |n| n.comp_drawn += 1);
            update_noble(world, m.player_b, |n| n.comp_drawn += 1);
        }
    }
    // Both played a match this week (Java increments COMP_DONE / COMP_DONE_WEEK
    // for both regardless of outcome).
    for oid in [m.player_a, m.player_b] {
        update_noble(world, oid, |n| {
            n.comp_done += 1;
            n.comp_done_week += 1;
        });
    }

    // Port the fighters back and free them.
    for (oid, ret) in [(m.player_a, m.return_a), (m.player_b, m.return_b)] {
        world.olympiad.in_competition.remove(&oid);
        if is_online(world, oid) {
            crate::game_loop::death::teleport_player(world, oid, ret.0, ret.1, ret.2);
        }
    }
    world.olympiad.matches.retain(|x| x.arena != m.arena);
    save_all(world);
}

/// `validateWinner`'s `pointDiff`: `min(winnerPts, loserPts) / divider`,
/// clamped to `[1, ALT_OLY_MAX_POINTS]`.
fn point_transfer(world: &World, winner: i32, loser: i32) -> i32 {
    let wp = world.olympiad.nobles.get(&winner).map_or(0, |n| n.points);
    let lp = world.olympiad.nobles.get(&loser).map_or(0, |n| n.points);
    (wp.min(lp) / POINT_DIVIDER).clamp(1, MAX_TRANSFER_POINTS)
}

fn update_noble(world: &mut World, object_id: i32, f: impl FnOnce(&mut NobleStats)) {
    if let Some(n) = world.olympiad.nobles.get_mut(&object_id) {
        f(n);
    }
}

fn position_of(world: &World, object_id: i32) -> (i32, i32, i32) {
    world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
        .map(|p| (p.x, p.y, p.z))
        .unwrap_or((0, 0, 0))
}

fn is_dead(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Vitals>(&object_id)
        .is_some_and(|v| v.dead)
}

/// `OlympiadGameNormal.createListOfParticipants`: draw two distinct **online**
/// players at random from the non-class queue, removing them. Offline entries
/// are dropped. Returns `None` if fewer than two online players remain.
fn draw_pair(world: &mut World) -> Option<(i32, i32)> {
    let first = draw_online(world)?;
    match draw_online(world) {
        Some(second) => Some((first, second)),
        None => {
            // No valid opponent — put the first player back (Java re-adds it).
            world.olympiad.non_class_registers.insert(first);
            None
        }
    }
}

/// Remove and return a random online player from the non-class queue, discarding
/// offline entries along the way. `None` when the queue empties.
fn draw_online(world: &mut World) -> Option<i32> {
    loop {
        let len = world.olympiad.non_class_registers.len();
        if len == 0 {
            return None;
        }
        let idx = world.roll(len as i32) as usize;
        let oid = *world.olympiad.non_class_registers.iter().nth(idx)?;
        world.olympiad.non_class_registers.remove(&oid);
        if is_online(world, oid) {
            return Some(oid);
        }
        // Offline: dropped from the queue, keep drawing.
    }
}

fn is_online(world: &World, object_id: i32) -> bool {
    world.objects.get_component::<Player>(&object_id).is_some()
}

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
) {
    let oly = &mut world.olympiad;
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

/// The player fields the registration gates and the noble record need.
struct NobleInfo {
    name: String,
    /// The active class (for the eligibility category + level check).
    class_id: i32,
    /// The main class the noble competes on (Java `getBaseClass`).
    base_class_id: i32,
    level: i32,
}

fn noble_info(world: &World, object_id: i32) -> Option<NobleInfo> {
    let p = world.objects.get_component::<Player>(&object_id)?;
    Some(NobleInfo {
        name: p.name.clone(),
        class_id: p.class_id,
        base_class_id: p.base_class_id,
        level: p.level,
    })
}

/// Java `OlympiadManager`'s "Classic noble equivalent" gate: the character must
/// be in the 3rd- or 4th-class group **and** at least level 55.
fn is_eligible(world: &World, info: &NobleInfo) -> bool {
    let cats = &world.data.categories;
    let class_ok = cats.contains("THIRD_CLASS_GROUP", info.class_id)
        || cats.contains("FOURTH_CLASS_GROUP", info.class_id);
    class_ok && info.level >= 55
}

/// `OlympiadManager.registerNoble` — join a match waiting list. Returns whether
/// the character is now registered, sending the appropriate system message
/// either way (Java's behaviour).
pub(crate) fn register(world: &mut World, object_id: i32, kind: CompetitionType) -> bool {
    let Some(info) = noble_info(world, object_id) else {
        return false;
    };

    // Only during the competition period.
    if !world.olympiad.in_comp_period {
        send_sm(
            world,
            object_id,
            sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS,
        );
        return false;
    }

    // Eligibility (3rd/4th class + level 55).
    if !is_eligible(world, &info) {
        send_sm_c1(
            world,
            object_id,
            sm_ids::CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS,
            &info.name,
        );
        return false;
    }

    // Registration closes 20 minutes before the window ends.
    let ms_to_end = world.olympiad.comp_end_tick.saturating_sub(world.tick) * 100;
    if ms_to_end < REG_CLOSE_BEFORE_END_MS {
        send_sm(
            world,
            object_id,
            sm_ids::PARTICIPATION_REQUESTS_ARE_NO_LONGER_BEING_ACCEPTED,
        );
        return false;
    }

    // Weekly match cap.
    if world.olympiad.remaining_weekly_matches(object_id) < 1 {
        send_sm(
            world,
            object_id,
            sm_ids::THE_MAXIMUM_MATCHES_YOU_CAN_PARTICIPATE_IN_1_WEEK_IS_30,
        );
        return false;
    }

    // Already fighting a match, or already waiting (Java reports which list).
    if world.olympiad.is_in_competition(object_id) {
        return false;
    }
    if world.olympiad.is_registered(object_id) {
        let sm = if world.olympiad.non_class_registers.contains(&object_id) {
            sm_ids::C1_IS_ALREADY_REGISTERED_ON_THE_WAITING_LIST_FOR_THE_ALL_CLASS_BATTLE
        } else {
            sm_ids::C1_IS_ALREADY_REGISTERED_ON_THE_CLASS_MATCH_WAITING_LIST
        };
        send_sm_c1(world, object_id, sm, &info.name);
        return false;
    }

    // First-ever registration creates the noble's record with the starting points.
    world
        .olympiad
        .nobles
        .entry(object_id)
        .or_insert_with(|| NobleStats::fresh(info.base_class_id, info.name.clone()));

    match kind {
        CompetitionType::Classed => {
            world
                .olympiad
                .class_registers
                .entry(OlympiadState::class_group(info.base_class_id))
                .or_default()
                .insert(object_id);
            send_sm(
                world,
                object_id,
                sm_ids::YOU_HAVE_BEEN_REGISTERED_FOR_THE_OLYMPIAD_WAITING_LIST_FOR_A_CLASS_BATTLE,
            );
        }
        CompetitionType::NonClassed => {
            world.olympiad.non_class_registers.insert(object_id);
            send_sm(
                world,
                object_id,
                sm_ids::YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH,
            );
        }
    }
    true
}

/// `OlympiadManager.unRegisterNoble` — leave the waiting list.
pub(crate) fn unregister(world: &mut World, object_id: i32) -> bool {
    let Some(info) = noble_info(world, object_id) else {
        return false;
    };

    if !world.olympiad.in_comp_period {
        send_sm(
            world,
            object_id,
            sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS,
        );
        return false;
    }

    if !is_eligible(world, &info) {
        send_sm_c1(
            world,
            object_id,
            sm_ids::CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS,
            &info.name,
        );
        return false;
    }

    if !world.olympiad.is_registered(object_id) {
        send_sm(
            world,
            object_id,
            sm_ids::YOU_ARE_NOT_CURRENTLY_REGISTERED_FOR_THE_OLYMPIAD,
        );
        return false;
    }

    // Java refuses to unregister a noble already pulled into a running match.
    if world.olympiad.is_in_competition(object_id) {
        return false;
    }

    if world.olympiad.remove_registration(object_id).is_some() {
        send_sm(
            world,
            object_id,
            sm_ids::YOU_HAVE_BEEN_REMOVED_FROM_THE_OLYMPIAD_WAITING_LIST,
        );
        return true;
    }
    false
}

fn send_sm(world: &World, object_id: i32, sm_id: i16) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(sp::system_message_with(sm_id, &[]));
        }
    }
}

fn send_sm_c1(world: &World, object_id: i32, sm_id: i16, name: &str) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(sp::system_message_with(
                sm_id,
                &[SmParam::PlayerName(name.to_string())],
            ));
        }
    }
}
