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

use crate::db::{DbCommand, HeroRow, OlympiadEomRow, OlympiadNobleRow};
use crate::model::Player;
use crate::model::olympiad::{
    CompetitionType, NobleStats, OlympiadMatch, OlympiadState, REG_CLOSE_BEFORE_END_MS,
};
use crate::network::server_packets::{self as sp, SmParam, sm_ids};
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
/// Java's `LIMIT 10` on every class-leaderboard query. The page has fifteen
/// rows; the rest are blanked.
const LEADER_BOARD_LIMIT: usize = 10;
/// The hero animation played on claiming (Java `new SocialAction(id, 20016)`).
const HERO_SOCIAL_ACTION: i32 = 20016;
/// Java `Hero.ACTION_HERO_GAINED` — the diary entry written by `setHeroGained`.
const HERO_ACTION_GAINED_HERO: i32 = 2;
/// `AltOlyVPeriod` — the validation period after a round ends (24 h).
const VALIDATION_PERIOD_MS: i64 = 86_400_000;
/// `AltOlyPeriod = DAY` × `AltOlyPeriodMultiplier = 14` — the round runs for
/// this many days (the last is the validation day), ending at noon.
const OLYMPIAD_PERIOD_DAYS: i64 = 14;
/// Noon, as milliseconds past midnight (Java `setNewOlympiadEnd` anchors the
/// end at `HOUR_OF_DAY 12`).
const NOON_MS_OF_DAY: i64 = 12 * 3600 * 1000;

/// `Olympiad.setNewOlympiadEnd`'s `DAY` branch: noon today plus
/// `(multiplier - 1)` days (the final day is reserved for validation).
pub(crate) fn next_olympiad_end(now_ms: i64) -> i64 {
    let noon_today = now_ms - ms_of_day(now_ms) + NOON_MS_OF_DAY;
    noon_today + (OLYMPIAD_PERIOD_DAYS - 1) * MS_PER_DAY
}

/// The per-character variable holding points earned this round but not yet
/// exchanged for marks (Java `Olympiad.UNCLAIMED_OLYMPIAD_POINTS_VAR`).
pub(crate) const UNCLAIMED_POINTS_VAR: &str = "UNCLAIMED_OLYMPIAD_POINTS";
/// `AltOlyCompRewItem = 45584` — "Mark of Battle", the exchange reward.
pub(crate) const MARK_ITEM: i32 = 45584;
/// `AltOlyMarkPerPoint = 20` — marks granted per unclaimed point.
pub(crate) const MARK_PER_POINT: i64 = 20;
/// `AltOlyHeroPoints = 300` — trade-point bonus for being a hero.
const HERO_TRADE_POINTS: i32 = 300;
/// `AltOlyRank{1..5}Points` — trade-point bonus by end-of-round percentile rank.
const RANK_TRADE_POINTS: [i32; 5] = [200, 80, 50, 30, 15];

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
            // new period boundary rather than ending instantly.
            world.olympiad.olympiad_end = next_olympiad_end(now);
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

/// Apply the boot-loaded `heroes` rows (Java `Hero.init`) into the live crown.
pub(crate) fn apply_heroes_loaded(
    world: &mut World,
    heroes: Vec<HeroRow>,
    diary: Vec<(i32, i64, i8, i32)>,
) {
    world.olympiad.heroes = heroes.iter().map(|h| (h.char_id, h.class_id)).collect();
    world.olympiad.hero_counts = heroes.iter().map(|h| (h.char_id, h.count)).collect();
    world.olympiad.claimed_heroes = heroes
        .iter()
        .filter(|h| h.claimed)
        .map(|h| h.char_id)
        .collect();
    world.olympiad.hero_info = heroes
        .iter()
        .map(|h| {
            (
                h.char_id,
                crate::model::olympiad::HeroInfo {
                    name: h.name.clone(),
                    clan_id: h.clan_id,
                    message: h.message.clone(),
                },
            )
        })
        .collect();
    // Group the diary entries by hero (already oldest-first from the query).
    let mut hero_diary: std::collections::HashMap<i32, Vec<crate::model::olympiad::DiaryEntry>> =
        std::collections::HashMap::new();
    for (char_id, time, action, param) in diary {
        hero_diary
            .entry(char_id)
            .or_default()
            .push(crate::model::olympiad::DiaryEntry {
                time,
                action,
                param,
            });
    }
    world.olympiad.hero_diary = hero_diary;
    tracing::info!("GameLoop: loaded {} Olympiad heroes.", heroes.len());
}

/// On enter-world, apply hero status to a crowned character (Java
/// `Player.setHero(Hero.isHero(objectId))` — crowned **and** claimed, so a
/// hero who has not visited the monument yet logs in without the status).
pub(crate) fn on_enter_world(world: &mut World, object_id: i32) {
    if world.olympiad.is_hero(object_id) {
        crate::game_loop::admin::hero::set_hero(world, object_id, true);
    }
}

/// Java `Hero.claimHero`: the crowned character collects the status — at the
/// Monument of Heroes, or through a GM's `//givehero`. Marks the crown claimed
/// (in memory and in `heroes.claimed`), pays the clan its reputation, grants
/// hero status/skills, plays the hero animation, and logs the deed in the diary.
///
/// The caller is responsible for the eligibility gate
/// ([`OlympiadState::is_unclaimed_hero`]); Java's `claimHero` itself would
/// happily crown a non-hero, and both of its call sites check first.
pub(crate) fn claim_hero(world: &mut World, object_id: i32) {
    world.olympiad.claimed_heroes.insert(object_id);
    let _ = world.db.send(DbCommand::ClaimHero { char_id: object_id });

    // "Clan member $c1 was named a hero. $s2 points have been added to your Clan
    // Reputation." — clan level 3+ only, and the reputation is the clan's, not
    // the hero's.
    let (clan_id, name) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| (p.clan_id, p.name.clone()))
        .unwrap_or((0, String::new()));
    let points = world.cfg.feature.hero_points;
    if clan_id != 0 && world.clans.get(&clan_id).is_some_and(|c| c.level >= 3) {
        super::clans::add_clan_reputation(world, clan_id, points);
        let sm = sp::system_message_with(
            sm_ids::CLAN_MEMBER_C1_WAS_NAMED_A_HERO_S2_POINTS_HAVE_BEEN_ADDED_TO_YOUR_CLAN_REPUTATION,
            &[SmParam::Text(name), SmParam::Int(points)],
        );
        for member in super::clans::online_members(world, clan_id) {
            if let Some(cid) = super::helpers::client_for_player(world, member)
                && let Some(cs) = world.clients.get(&cid)
            {
                cs.send(sm.clone());
            }
        }
    }

    crate::game_loop::admin::hero::set_hero(world, object_id, true);
    // `broadcastPacket(new SocialAction(objectId, 20016))` — the hero animation.
    super::helpers::broadcast_including_self(
        world,
        object_id,
        &sp::social_action(object_id, HERO_SOCIAL_ACTION),
    );
    super::party::broadcast_user_info(world, object_id);
    // `setHeroGained` — the diary's "gained hero" entry.
    let _ = world.db.send(DbCommand::SaveHeroDiary {
        char_id: object_id,
        time: commons::util::now_millis(),
        action: HERO_ACTION_GAINED_HERO,
        param: 0,
    });
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

/// `Olympiad.loadNoblesRank`: rank the classified nobles (≥ 10 matches) by points
/// into percentile tiers — 1 (top 1 %), 2 (10 %), 3 (25 %), 4 (50 %), 5 (rest).
fn compute_noble_ranks(world: &World) -> std::collections::HashMap<i32, u8> {
    let mut classified: Vec<(i32, i32)> = world
        .olympiad
        .nobles
        .iter()
        .filter(|(_, n)| n.comp_done >= HERO_MIN_MATCHES)
        .map(|(&id, n)| (id, n.points))
        .collect();
    // Highest points first (Java orders the query by points DESC).
    classified.sort_by(|a, b| b.1.cmp(&a.1));

    let total = classified.len() as f64;
    let mut r1 = (total * 0.01).round() as usize;
    let mut r2 = (total * 0.10).round() as usize;
    let mut r3 = (total * 0.25).round() as usize;
    let mut r4 = (total * 0.50).round() as usize;
    if r1 == 0 {
        r1 = 1;
        r2 += 1;
        r3 += 1;
        r4 += 1;
    }

    let mut ranks = std::collections::HashMap::new();
    for (i, (id, _)) in classified.iter().enumerate() {
        let place = i + 1; // 1-based, like Java's `place++`
        let rank = if place <= r1 {
            1
        } else if place <= r2 {
            2
        } else if place <= r3 {
            3
        } else if place <= r4 {
            4
        } else {
            5
        };
        ranks.insert(*id, rank);
    }
    ranks
}

/// `Olympiad.getOlympiadTradePoint`: the points a noble may exchange for marks —
/// a hero bonus plus a rank bonus. Zero for the unranked or point-less.
fn olympiad_trade_point(
    world: &World,
    ranks: &std::collections::HashMap<i32, u8>,
    object_id: i32,
) -> i32 {
    let Some(&rank) = ranks.get(&object_id) else {
        return 0;
    };
    if world
        .olympiad
        .nobles
        .get(&object_id)
        .map_or(0, |n| n.points)
        == 0
    {
        return 0;
    }
    // Java `isHero(objectId) || isUnclaimedHero(objectId)` — the trade bonus
    // rides the crown, not the claim, so a hero who has not been to the monument
    // yet still exchanges at the hero rate.
    let hero = if world.olympiad.is_crowned(object_id) {
        HERO_TRADE_POINTS
    } else {
        0
    };
    hero + RANK_TRADE_POINTS[(rank as usize) - 1]
}

/// After a round ends, bank each noble's exchangeable points on their
/// `UNCLAIMED_OLYMPIAD_POINTS` variable (Java `loadNoblesRank`'s reward loop) —
/// on the live component for online nobles, straight to `character_variables`
/// for offline ones.
fn store_trade_points(world: &mut World) {
    let ranks = compute_noble_ranks(world);
    let ids: Vec<i32> = world.olympiad.nobles.keys().copied().collect();
    for oid in ids {
        let points = olympiad_trade_point(world, &ranks, oid);
        if points <= 0 {
            continue;
        }
        if let Some(v) = world
            .objects
            .get_component_mut::<crate::model::components::PlayerVariables>(&oid)
        {
            v.set_int(UNCLAIMED_POINTS_VAR, points);
        } else {
            let _ = world.db.send(DbCommand::StoreCharVar {
                char_id: oid,
                var: UNCLAIMED_POINTS_VAR.to_string(),
                value: points.to_string(),
            });
        }
    }
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
    // The new crown grants **no** status yet: Java's `computeNewHeroes` writes
    // `claimed = false` and stops, so each hero must collect the title at the
    // Monument of Heroes (`claim_hero`) before `setHero` runs.
    let heroes = compute_heroes(world);
    world.olympiad.heroes = heroes;
    // Record each new hero's display data (name from the noble, clan from the
    // online player) for the `ExHeroList` window.
    world.olympiad.hero_info.clear();
    let crowned: Vec<i32> = world.olympiad.heroes.iter().map(|(id, _)| *id).collect();
    for id in crowned {
        let name = world
            .olympiad
            .nobles
            .get(&id)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let clan_id = world
            .objects
            .get_component::<Player>(&id)
            .map(|p| p.clan_id)
            .unwrap_or(0);
        world.olympiad.hero_info.insert(
            id,
            crate::model::olympiad::HeroInfo {
                name,
                clan_id,
                // A freshly-crowned hero has not written any words yet.
                message: String::new(),
            },
        );
    }
    tracing::info!(
        "Olympiad: round {} ended; {} heroes crowned.",
        world.olympiad.current_cycle,
        world.olympiad.heroes.len()
    );
    // Persist the crown (Java `Hero.computeNewHeroes`): bump each hero's count
    // and replace the `heroes` table so it survives relogs and applies to
    // offline heroes on their next login.
    let hero_rows: Vec<HeroRow> = world
        .olympiad
        .heroes
        .iter()
        .map(|&(char_id, class_id)| {
            let count = world
                .olympiad
                .hero_counts
                .get(&char_id)
                .copied()
                .unwrap_or(0)
                + 1;
            world.olympiad.hero_counts.insert(char_id, count);
            let info = world.olympiad.hero_info.get(&char_id);
            HeroRow {
                char_id,
                class_id,
                count,
                // Not persisted (no columns), but carried for consistency.
                name: info.map(|i| i.name.clone()).unwrap_or_default(),
                clan_id: info.map(|i| i.clan_id).unwrap_or(0),
                message: info.map(|i| i.message.clone()).unwrap_or_default(),
                // Java `computeNewHeroes` writes `claimed = false` for both the
                // re-crowned and the newly crowned: a fresh cycle must be
                // collected again at the monument.
                claimed: false,
            }
        })
        .collect();
    world.olympiad.claimed_heroes.clear();
    let _ = world.db.send(DbCommand::SaveHeroes { heroes: hero_rows });

    // Bank each noble's exchangeable points for the mark exchange at the manager.
    store_trade_points(world);
    // Announce the round's end to everyone online.
    let announce = sp::system_message_with(
        sm_ids::ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED,
        &[SmParam::Int(world.olympiad.current_cycle)],
    );
    for cs in world.clients.values() {
        if matches!(cs, crate::session::ClientSession::InGame(_)) {
            cs.send(announce.clone());
        }
    }

    let now = commons::util::now_millis();
    world.olympiad.validation_end = now + VALIDATION_PERIOD_MS;
    save_all(world);
    // `updateMonthlyData`, which Java runs right after `saveOlympiadStatus`:
    // freeze this cycle's nobles as the leaderboard the Olympiad Manager shows
    // until the next round ends. The DB half rides the same channel behind
    // `save_all`'s `SaveOlympiad`, so it copies rows that are already written.
    snapshot_eom(world);
    world.scheduler.schedule(
        fire_at(world, VALIDATION_PERIOD_MS),
        ScheduledTask::OlympiadValidationEnd,
    );
}

/// Java `Olympiad.updateMonthlyData` — replace the end-of-cycle snapshot with a
/// copy of the live nobles, in memory and in `olympiad_nobles_eom`.
fn snapshot_eom(world: &mut World) {
    world.olympiad.eom_nobles = world
        .olympiad
        .nobles
        .values()
        .map(|n| OlympiadEomRow {
            class_id: n.class_id,
            name: n.name.clone(),
            points: n.points,
            comp_done: n.comp_done,
            comp_won: n.comp_won,
        })
        .collect();
    let _ = world.db.send(DbCommand::SnapshotOlympiadEom);
}

/// Java `Olympiad.getClassLeaderBoard(classId)` — the top ten of the last
/// completed cycle for one class: at least `AltOlyMinMatchesForPoints` matches,
/// ordered by points, then matches played, then wins, all descending. The
/// Soulhound branch (class 132/133) is Kamael-only and unreachable here.
pub(crate) fn class_leader_board(world: &World, class_id: i32) -> Vec<String> {
    let mut rows: Vec<&OlympiadEomRow> = world
        .olympiad
        .eom_nobles
        .iter()
        .filter(|n| n.class_id == class_id && n.comp_done >= HERO_MIN_MATCHES)
        .collect();
    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.comp_done.cmp(&a.comp_done))
            .then(b.comp_won.cmp(&a.comp_won))
    });
    rows.into_iter()
        .take(LEADER_BOARD_LIMIT)
        .map(|n| n.name.clone())
        .collect()
}

/// `ValidationEndTask`: the validation period ends — start a new cycle's
/// competition period with a clean noble table.
pub(crate) fn handle_validation_end(world: &mut World) {
    world.olympiad.period = 0;
    world.olympiad.current_cycle += 1;
    world.olympiad.nobles.clear(); // `deleteNobles` (TRUNCATE olympiad_nobles)
    let now = commons::util::now_millis();
    world.olympiad.olympiad_end = next_olympiad_end(now);
    save_all(world);
    tracing::info!(
        "Olympiad: validation ended; cycle {} begins.",
        world.olympiad.current_cycle
    );
    // Re-arm the competition window + the next period end.
    arm_comp_schedule(world, now);
    world.scheduler.schedule(
        fire_at(world, world.olympiad.olympiad_end - now),
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

/// One step of the pre-fight ceremony (Java `OlympiadGameTask`'s countdowns).
enum CountdownStep {
    /// Announce "moved to the stadium / match starts in `secs` seconds" (the
    /// `sm` id) to both fighters.
    Say { sm: i16, secs: i32 },
    /// The teleport countdown reached zero: port both into the arena + strip.
    Enter,
    /// The battle countdown reached zero: the fight begins.
    Fight,
}
use CountdownStep::{Enter, Fight, Say};

/// `(delay_ms_since_previous_step, step)`. First the 120 s teleport countdown
/// (`YOU_WILL_BE_MOVED…` 1492 at `AltOlyWaitTime`'s checkpoints), then the
/// teleport, then the 60 s battle countdown (`THE_MATCH_WILL_START…` 1495).
const COUNTDOWN: &[(u64, CountdownStep)] = &[
    (
        0,
        Say {
            sm: 1492,
            secs: 120,
        },
    ),
    (60_000, Say { sm: 1492, secs: 60 }),
    (30_000, Say { sm: 1492, secs: 30 }),
    (15_000, Say { sm: 1492, secs: 15 }),
    (5_000, Say { sm: 1492, secs: 10 }),
    (5_000, Say { sm: 1492, secs: 5 }),
    (1_000, Say { sm: 1492, secs: 4 }),
    (1_000, Say { sm: 1492, secs: 3 }),
    (1_000, Say { sm: 1492, secs: 2 }),
    (1_000, Say { sm: 1492, secs: 1 }),
    (1_000, Enter),
    (0, Say { sm: 1495, secs: 60 }),
    (5_000, Say { sm: 1495, secs: 55 }),
    (5_000, Say { sm: 1495, secs: 50 }),
    (10_000, Say { sm: 1495, secs: 40 }),
    (10_000, Say { sm: 1495, secs: 30 }),
    (10_000, Say { sm: 1495, secs: 20 }),
    (10_000, Say { sm: 1495, secs: 10 }),
    (5_000, Say { sm: 1495, secs: 5 }),
    (1_000, Say { sm: 1495, secs: 4 }),
    (1_000, Say { sm: 1495, secs: 3 }),
    (1_000, Say { sm: 1495, secs: 2 }),
    (1_000, Say { sm: 1495, secs: 1 }),
    (1_000, Fight),
];

/// Register a match and start its pre-fight ceremony (Java `OlympiadGameTask`
/// from `BEGIN`): the fighters are announced, teleported in + buff-stripped
/// after the wait countdown, and the fight begins after the battle countdown.
pub(crate) fn start_match(world: &mut World, arena: usize, player_a: i32, player_b: i32) {
    // A private instance so concurrent bouts sharing arena coords stay isolated.
    let instance_id = world.instances.create(0);
    world.olympiad.matches.push(OlympiadMatch {
        arena,
        player_a,
        player_b,
        instance_id,
        deadline_tick: 0, // set when the battle actually begins
        return_a: position_of(world, player_a),
        return_b: position_of(world, player_b),
    });
    tracing::info!("Olympiad: match in arena {arena}: {player_a} vs {player_b}.");
    world.scheduler.schedule(
        world.tick,
        ScheduledTask::OlympiadCountdown { arena, step: 0 },
    );
}

/// Run one ceremony step and schedule the next.
pub(crate) fn handle_countdown(world: &mut World, arena: usize, step: usize) {
    let Some(m) = world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena == arena)
        .cloned()
    else {
        return; // the match was resolved/aborted
    };
    let Some((_, action)) = COUNTDOWN.get(step) else {
        return;
    };
    match action {
        Say { sm, secs } => {
            for oid in [m.player_a, m.player_b] {
                send_sm_int(world, oid, *sm, *secs);
            }
        }
        Enter => {
            for (oid, spawn) in [(m.player_a, ARENA_SPAWN_A), (m.player_b, ARENA_SPAWN_B)] {
                world
                    .objects
                    .add_components(&oid, crate::model::components::InstanceId(m.instance_id));
                crate::game_loop::death::teleport_player(world, oid, spawn.0, spawn.1, spawn.2);
                strip_buffs(world, oid);
            }
        }
        Fight => {
            if let Some(mm) = world.olympiad.matches.iter_mut().find(|x| x.arena == arena) {
                mm.deadline_tick = world.tick + (BATTLE_MS / 100) as u64;
            }
            world.scheduler.schedule(
                fire_at(world, MATCH_POLL_MS),
                ScheduledTask::OlympiadMatchTick { arena },
            );
            return; // the fight is on — the match tick takes over
        }
    }
    if let Some((next_delay, _)) = COUNTDOWN.get(step + 1) {
        world.scheduler.schedule(
            fire_at(world, *next_delay as i64),
            ScheduledTask::OlympiadCountdown {
                arena,
                step: step + 1,
            },
        );
    }
}

/// `AbstractOlympiadGame.removeBuffs`: drop every active (non-passive) buff so
/// nobody enters the arena pre-buffed.
pub(crate) fn strip_buffs(world: &mut World, object_id: i32) {
    let skills: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    for skill_id in skills {
        crate::game_loop::skills::effects::handle_buff_expire(world, object_id, skill_id);
    }
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
    // Java's `olympiad` logger (`AbstractOlympiadGame`/`Olympiad`). Ungated,
    // like accounting: match outcomes decide hero status, so they are kept
    // regardless of the diagnostic settings.
    {
        let name_of = |oid: i32| {
            world
                .objects
                .get_component::<crate::model::Player>(&oid)
                .map(|p| p.name.clone())
        };
        let (outcome, winner, loser) = match result {
            MatchResult::Win { winner, loser } => ("win", Some(*winner), Some(*loser)),
            MatchResult::Draw => ("draw", None, None),
        };
        commons::audit::record(
            commons::audit::Category::Olympiad,
            serde_json::json!({
                "outcome": outcome,
                "player_a": name_of(m.player_a),
                "player_a_oid": m.player_a,
                "player_b": name_of(m.player_b),
                "player_b_oid": m.player_b,
                "winner": winner.and_then(name_of),
                "loser": loser.and_then(name_of),
            }),
        );
    }

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

    // Port the fighters back to the overworld and free them.
    for (oid, ret) in [(m.player_a, m.return_a), (m.player_b, m.return_b)] {
        world.olympiad.in_competition.remove(&oid);
        world
            .objects
            .remove_component::<crate::model::components::InstanceId>(&oid);
        if is_online(world, oid) {
            crate::game_loop::death::teleport_player(world, oid, ret.0, ret.1, ret.2);
        }
    }
    world.olympiad.matches.retain(|x| x.arena != m.arena);
    world.instances.destroy(m.instance_id);
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
    eom: Vec<crate::db::OlympiadEomRow>,
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

    // Java `AbstractOlympiadGame.checkPlayer`: the owner of a cursed weapon is
    // refused — "$c1 does not meet the participation requirements. The owner of
    // $s2 cannot participate in the Olympiad."
    let cursed_id = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map_or(0, |p| p.cursed_weapon_equipped_id);
    if cursed_id != 0 {
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(sp::system_message_with(
                sm_ids::C1_DOES_NOT_MEET_THE_PARTICIPATION_REQUIREMENTS_THE_OWNER_OF_S2_CANNOT_PARTICIPATE_IN_THE_OLYMPIAD,
                &[
                    SmParam::PlayerName(info.name.clone()),
                    SmParam::ItemName(cursed_id),
                ],
            ));
        }
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
    crate::game_loop::helpers::send_sm_to_player(world, object_id, sm_id, &[]);
}

/// Send a system message with a single integer argument (the countdown seconds).
fn send_sm_int(world: &World, object_id: i32, sm_id: i16, value: i32) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(sp::system_message_with(sm_id, &[SmParam::Int(value)]));
    }
}

fn send_sm_c1(world: &World, object_id: i32, sm_id: i16, name: &str) {
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, object_id)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(sp::system_message_with(
            sm_id,
            &[SmParam::PlayerName(name.to_string())],
        ));
    }
}

// ---------------------------------------------------------------------------
// Observer mode (spectating matches) — Java `Player.enter/leaveOlympiad
// ObserverMode`, `OlyManager`'s `watchmatch`/`arenachange` bypasses, and the
// `RequestOlympiadObserverEnd`/`RequestOlympiadMatchList` packets.
// ---------------------------------------------------------------------------

use crate::model::components::{OlympiadObserver, Position};

/// The spectator stand — midway between the two arena spawns. (Java draws a
/// random point from the zone's `spectatorSpawns`; the port has one arena, so a
/// fixed vantage point suffices — matches are instance-scoped anyway.)
const OBSERVE_SPAWN: (i32, i32, i32) = (-88070, -252843, -3320);

/// The ongoing match at arena `arena` (its index), if any.
fn arena_match(world: &World, arena: i32) -> Option<&OlympiadMatch> {
    world
        .olympiad
        .matches
        .iter()
        .find(|m| m.arena as i32 == arena)
}

fn player_name(world: &World, oid: i32) -> String {
    world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

/// Java `OlyManager` `watchmatch` / `RequestOlympiadMatchList`: send the list of
/// ongoing matches a spectator can jump between.
pub(crate) fn send_match_list(world: &World, client_id: u32) {
    let rows: Vec<sp::OlympiadMatchRow> = world
        .olympiad
        .matches
        .iter()
        .map(|m| sp::OlympiadMatchRow {
            arena: m.arena as i32,
            // A match in the live list is under way (post-countdown).
            running: true,
            player_a: player_name(world, m.player_a),
            player_b: player_name(world, m.player_b),
        })
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::ex_olympiad_match_list(&rows));
    }
}

/// Java `OlyManager.arenachange` → `Player.enterOlympiadObserverMode`: teleport
/// the viewer into the chosen arena's instance as a hidden spectator. Refused
/// outside the competition period, or while registered / competing.
pub(crate) fn enter_observer(world: &mut World, client_id: u32, player_oid: i32, arena: i32) {
    if !world.olympiad.in_comp_period
        || world.olympiad.is_registered(player_oid)
        || world.olympiad.is_in_competition(player_oid)
    {
        return;
    }
    let Some(instance_id) = arena_match(world, arena).map(|m| m.instance_id) else {
        return; // no match at that arena
    };

    // On first entry, remember where to return to (Java `setLastLocation`).
    let already = world.objects.has_component::<OlympiadObserver>(&player_oid);
    if !already {
        let return_pos = world
            .objects
            .get_component::<Position>(&player_oid)
            .map(|p| (p.x, p.y, p.z))
            .unwrap_or(OBSERVE_SPAWN);
        world
            .objects
            .add_components(&player_oid, OlympiadObserver { return_pos, arena });
    } else if let Some(o) = world
        .objects
        .get_component_mut::<OlympiadObserver>(&player_oid)
    {
        o.arena = arena; // switching arenas
    }
    // Scope the viewer to the match's instance so they see only that fight.
    world.objects.add_components(
        &player_oid,
        crate::model::components::InstanceId(instance_id),
    );
    crate::game_loop::death::teleport_player(
        world,
        player_oid,
        OBSERVE_SPAWN.0,
        OBSERVE_SPAWN.1,
        OBSERVE_SPAWN.2,
    );
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::ex_olympiad_mode(3));
    }
    // Java `enterOlympiadObserverMode` also makes the spectator invulnerable +
    // invisible so a stray AoE can't touch them and they don't clutter the
    // arena. Set the two flags (adding the component if absent), leaving any
    // other admin flag untouched.
    set_observer_flags(world, player_oid, true);
}

/// Toggle the spectator's invulnerable + invisible flags (Java the observer
/// mode's `setInvul`/`setInvisible`), adding the `AdminFlags` component on first
/// use and preserving any other flags already set (e.g. a GM's).
fn set_observer_flags(world: &mut World, player_oid: i32, on: bool) {
    use crate::model::components::AdminFlags;
    if world
        .objects
        .get_component::<AdminFlags>(&player_oid)
        .is_none()
    {
        if !on {
            return; // absent already means every flag false
        }
        world
            .objects
            .add_components(&player_oid, AdminFlags::default());
    }
    if let Some(f) = world.objects.get_component_mut::<AdminFlags>(&player_oid) {
        f.invul = on;
        f.hidden = on;
    }
}

/// Java `RequestOlympiadObserverEnd` → `Player.leaveOlympiadObserverMode`:
/// teleport the spectator back and drop the observer state.
pub(crate) fn leave_observer(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(observer) = world
        .objects
        .get_component::<OlympiadObserver>(&player_oid)
        .copied()
    else {
        return;
    };
    world
        .objects
        .remove_component::<OlympiadObserver>(&player_oid);
    world
        .objects
        .remove_component::<crate::model::components::InstanceId>(&player_oid);
    // Clear the spectator's invul + invisible (Java restores the normal state).
    set_observer_flags(world, player_oid, false);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::ex_olympiad_mode(0));
    }
    let (x, y, z) = observer.return_pos;
    crate::game_loop::death::teleport_player(world, player_oid, x, y, z);
}

/// Java `Hero.showHeroDiary` (`_diary?class=<classId>&page=<n>`): render the
/// paginated notable-deeds log of the hero holding `classId`, in the clicked
/// NPC's window.
pub(crate) fn show_hero_diary(world: &mut World, client_id: u32, npc_oid: i32, args: &str) {
    const PER_PAGE: usize = 10;
    let class_id = query_param(args, "class").unwrap_or(0);
    let page = query_param(args, "page").unwrap_or(1).max(1) as usize;

    // Resolve the hero of that class.
    let Some(&(char_id, _)) = world
        .olympiad
        .heroes
        .iter()
        .find(|(_, cls)| *cls == class_id)
    else {
        return;
    };
    let Some(info) = world.olympiad.hero_info.get(&char_id).cloned() else {
        return;
    };
    let Some(template) = crate::data::htm_cache::read_htm(format!(
        "{}data/html/olympiad/herodiary.htm",
        world.data.root
    )) else {
        return;
    };

    // Entries newest-first; slice the requested page.
    let empty = Vec::new();
    let entries = world.olympiad.hero_diary.get(&char_id).unwrap_or(&empty);
    let total = entries.len();
    let mut list = String::new();
    let mut color = true;
    let start = (page - 1) * PER_PAGE;
    let mut last = start;
    for (i, entry) in entries.iter().rev().enumerate().skip(start).take(PER_PAGE) {
        last = i;
        let date = diary_date(entry.time);
        let action = diary_action_text(world, entry.action, entry.param);
        let bg = if color {
            "<table width=270 bgcolor=\"131210\">"
        } else {
            "<table width=270>"
        };
        list.push_str(&format!(
            "<tr><td>{bg}<tr><td width=270><font color=\"LEVEL\">{date}:xx</font></td></tr>\
             <tr><td width=270>{action}</td></tr><tr><td>&nbsp;</td></tr></table></td></tr>"
        ));
        color = !color;
    }

    // Pagination buttons (Java's prev = older page, next = newer page).
    let prev = if total > 0 && last < total - 1 {
        format!(
            "<button value=\"Prev\" action=\"bypass _diary?class={class_id}&page={}\" \
             width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
            page + 1
        )
    } else {
        String::new()
    };
    let next = if page > 1 {
        format!(
            "<button value=\"Next\" action=\"bypass _diary?class={class_id}&page={}\" \
             width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
            page - 1
        )
    } else {
        String::new()
    };

    let html = template
        .replace("%heroname%", &info.name)
        .replace("%message%", &info.message)
        .replace("%list%", &list)
        .replace("%buttprev%", &prev)
        .replace("%buttnext%", &next);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::npc_html_message(npc_oid, &html));
    }
}

/// Format one diary entry's action (Java `showHeroDiary`'s three `ACTION_*`
/// cases): 1 raid-killed (NPC name), 2 hero-gained, 3 castle-taken (castle name).
fn diary_action_text(world: &World, action: i8, param: i32) -> String {
    match action {
        1 => world
            .data
            .npc_data
            .get(param)
            .map(|t| format!("{} was defeated", t.name))
            .unwrap_or_default(),
        2 => "Gained Hero status".to_string(),
        3 => world
            .castles
            .iter()
            .find(|c| c.id == param)
            .map(|c| format!("{} Castle was successfuly taken", c.name))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Read an integer `key` from a `?a=1&b=2` query string.
fn query_param(args: &str, key: &str) -> Option<i32> {
    args.trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
}

/// Java `SimpleDateFormat("yyyy-MM-dd HH")` on the diary timestamp (UTC, like the
/// rest of the port). Hinnant's civil-from-days.
fn diary_date(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let hour = secs.rem_euclid(86_400) / 3600;
    // days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {hour:02}")
}

/// Whether the player is currently spectating a match.
pub(crate) fn is_observing(world: &World, player_oid: i32) -> bool {
    world.objects.has_component::<OlympiadObserver>(&player_oid)
}

/// Java `ExHeroList` (`Hero.getHeroes`): send the current heroes' roll — name,
/// class, and (resolved from the live clan registry) clan/ally names + crests +
/// the times-been-a-hero count.
pub(crate) fn send_hero_list(world: &World, client_id: u32) {
    let rows: Vec<sp::HeroListRow> = world
        .olympiad
        .heroes
        .iter()
        .map(|&(char_id, class_id)| {
            let info = world.olympiad.hero_info.get(&char_id);
            let name = info.map(|i| i.name.clone()).unwrap_or_default();
            let clan = info.and_then(|i| world.clans.get(&i.clan_id));
            let (clan_name, clan_crest) = clan
                .map(|c| (c.name.clone(), c.crest_id))
                .unwrap_or_default();
            let (ally_name, ally_crest) = clan
                .filter(|c| c.ally_id != 0)
                .map(|c| (c.ally_name.clone(), c.ally_crest_id))
                .unwrap_or_default();
            sp::HeroListRow {
                name,
                class_id,
                clan_name,
                clan_crest,
                ally_name,
                ally_crest,
                count: world
                    .olympiad
                    .hero_counts
                    .get(&char_id)
                    .copied()
                    .unwrap_or(0),
            }
        })
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sp::ex_hero_list(&rows));
    }
}

#[cfg(test)]
mod diary_tests {
    use super::{diary_date, query_param};

    #[test]
    fn query_param_reads_class_and_page() {
        assert_eq!(query_param("?class=88&page=2", "class"), Some(88));
        assert_eq!(query_param("?class=88&page=2", "page"), Some(2));
        assert_eq!(query_param("?class=88", "page"), None);
    }

    #[test]
    fn diary_date_formats_utc_year_month_day_hour() {
        // 2024-01-01 00:00:00 UTC = epoch day 19723.
        let ms = 19723i64 * 86_400_000;
        assert_eq!(diary_date(ms), "2024-01-01 00");
        // + 13h30m → hour 13, same day.
        assert_eq!(
            diary_date(ms + 13 * 3_600_000 + 30 * 60_000),
            "2024-01-01 13"
        );
    }
}
