//! Port of `gameserver/data/BotReportTable.java` + the
//! `handlers/playeractions/BotReport.java` entry point.
//!
//! A player targets a suspected bot and uses the client's report button
//! (`/AutoHuntingReport`, `ActionData.xml` id 65). Each reporter has 7 points a
//! day and a cooldown between reports; each report is remembered per (bot,
//! reporter) pair so the same person cannot report the same target twice. When
//! a bot's report count crosses a threshold in `BotReportPunishments.xml` it is
//! cast a punishment skill — chat block, party block, trade block, a speed
//! debuff, or the PvP flag.
//!
//! **The entry point is in the datapack, not `java/`.** `reportBot` has no
//! caller anywhere under `java/`; the only one is a *player action handler*
//! shipped in `dist/game/data/scripts/handlers/playeractions/`. Grepping only
//! the Java source tree makes this feature look dead.
//!
//! Not modelled: **fake players** as report targets (Java accepts an
//! `isFakePlayer()` NPC whose template is `fakePlayerTalkable`, and merely
//! counts the report without punishing since the punish path needs a `Player`).

use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::skill_by_id;
use std::collections::HashMap;

use crate::config::bot_report::BotReportConfig;
use crate::model::Player;
use crate::model::components::Position;
use crate::network::server_packets::{SmParam, sm_ids};
use crate::world::World;

/// Java `BotReportTable`'s action-block ids. Negative by design: they share a
/// namespace with real `ActionData.xml` ids, which are non-negative.
/// No skill on this dist blocks attacking or "all actions", so these two have
/// no reader; they are kept because the five ids are one namespace and a future
/// punishment skill referencing `-1`/`-4` must not silently mean nothing.
#[allow(dead_code)]
pub const ATTACK_ACTION_BLOCK_ID: i32 = -1;
pub const TRADE_ACTION_BLOCK_ID: i32 = -2;
pub const PARTY_ACTION_BLOCK_ID: i32 = -3;
#[allow(dead_code)]
pub const ACTION_BLOCK_ID: i32 = -4;
pub const CHAT_BLOCK_ID: i32 = -5;

/// `ActionData.xml` id 65 — the `/AutoHuntingReport` button.
pub const BOT_REPORT_ACTION_ID: i32 = 65;

/// Java `ReporterCharData`: a reporter's daily budget.
#[derive(Debug, Clone, Copy)]
pub struct ReporterCharData {
    /// Java `_reportPoints`, starting at 7 and reset daily.
    pub points: i8,
    /// Java `_lastReport` — millis, 0 = never.
    pub last_report: i64,
}

impl Default for ReporterCharData {
    fn default() -> Self {
        Self {
            points: DAILY_POINTS,
            last_report: 0,
        }
    }
}

/// Java's hard-coded `setPoints(7)` / `_reportPoints = 7`.
pub const DAILY_POINTS: i8 = 7;

/// Java `ReportedCharData`: who reported this character, and when.
#[derive(Debug, Clone, Default)]
pub struct ReportedCharData {
    /// reporter object id → report time (millis).
    pub reporters: HashMap<i32, i64>,
}

impl ReportedCharData {
    pub fn report_count(&self) -> usize {
        self.reporters.len()
    }
}

/// The in-memory registries (Java's three maps on the singleton).
#[derive(Debug, Default)]
pub struct BotReportTable {
    /// bot object id → its reports.
    pub reports: HashMap<i32, ReportedCharData>,
    /// reporter object id → daily budget.
    pub reporters: HashMap<i32, ReporterCharData>,
    /// Java `_ipRegistry`: last report time per reporter address, so a second
    /// character on the same connection cannot double up. Java packs the four
    /// octets into an int; keying by the address string is equivalent and also
    /// works for IPv6, which Java's `hashIp` would throw on.
    pub ip_registry: HashMap<String, i64>,
}

/// Why a report was refused, so the caller can send the right message. Java
/// sends each `SystemMessage` inline and returns `false`.
enum Refusal {
    /// No message — Java returns `false` silently.
    Silent,
    Sm(i16),
    /// `YOU_CAN_MAKE_ANOTHER_REPORT_IN_S1_MINUTE_S_YOU_HAVE_S2_POINT_S_REMAINING`
    Cooldown {
        minutes: i32,
        points: i32,
    },
}

/// Java `handlers/playeractions/BotReport.useAction` — the button itself.
pub(crate) fn handle_bot_report_action(world: &mut World, client_id: u32, player_oid: i32) {
    if !world.cfg.bot_report.enabled {
        crate::game_loop::helpers::send_sm_to_player(
            world,
            player_oid,
            sm_ids::S1_TEXT,
            &[SmParam::Text("This feature is disabled.".into())],
        );
        return;
    }
    let _ = report_bot(world, client_id, player_oid);
}

/// Java `BotReportTable.reportBot`.
pub(crate) fn report_bot(world: &mut World, client_id: u32, reporter_oid: i32) -> bool {
    let target_oid = match world
        .objects
        .get_component::<crate::model::components::TargetRef>(&reporter_oid)
    {
        Some(t) => match t.0 {
            Some(oid) => oid,
            None => return false,
        },
        None => return false,
    };

    match check_report(world, reporter_oid, target_oid) {
        Err(Refusal::Silent) => return false,
        Err(Refusal::Sm(id)) => {
            crate::game_loop::helpers::send_sm_to_player(world, reporter_oid, id, &[]);
            return false;
        }
        Err(Refusal::Cooldown { minutes, points }) => {
            crate::game_loop::helpers::send_sm_to_player(
                world,
                reporter_oid,
                sm_ids::YOU_CAN_MAKE_ANOTHER_REPORT_IN_S1_MINUTES_S2_POINTS_LEFT,
                &[SmParam::Int(minutes), SmParam::Int(points)],
            );
            return false;
        }
        Ok(()) => {}
    }

    let now = commons::util::now_millis();
    let ip = player_ip(world, reporter_oid);
    let points_left = {
        let table = &mut world.bot_reports;
        table
            .reports
            .entry(target_oid)
            .or_default()
            .reporters
            .insert(reporter_oid, now);
        let rep = table.reporters.entry(reporter_oid).or_default();
        // Java `registerReport`: spend a point and stamp the time.
        rep.points -= 1;
        rep.last_report = now;
        table.ip_registry.insert(ip, now);
        rep.points
    };

    let bot_name = player_name_or_empty(world, target_oid);
    crate::game_loop::helpers::send_sm_to_player(
        world,
        reporter_oid,
        sm_ids::C1_WAS_REPORTED_AS_A_BOT,
        &[SmParam::Text(bot_name.clone())],
    );
    crate::game_loop::helpers::send_sm_to_player(
        world,
        reporter_oid,
        sm_ids::YOU_REPORTED_C1_S2_POINTS_LEFT,
        &[SmParam::Text(bot_name), SmParam::Int(points_left as i32)],
    );

    handle_report(world, target_oid);
    let _ = client_id;
    true
}

/// Every refusal in `reportBot`, in Java's order. Split out so the mutation
/// above is unconditional once this returns `Ok`.
fn check_report(world: &World, reporter_oid: i32, target_oid: i32) -> Result<(), Refusal> {
    // Java: the target must be a player (or a talkable fake player) and not
    // the reporter themselves.
    if target_oid == reporter_oid || world.objects.get_component::<Player>(&target_oid).is_none() {
        return Err(Refusal::Silent);
    }

    // Peace / PvP zones are off limits.
    if let Some(pos) = world.objects.get_component::<Position>(&target_oid) {
        use crate::data::zone_data::ZoneKind;
        let protected = world
            .data
            .zone_data
            .zones_at(pos.x, pos.y, pos.z)
            .any(|z| matches!(z.kind, ZoneKind::Peace | ZoneKind::Pvp));
        if protected {
            return Err(Refusal::Sm(
                sm_ids::CANNOT_REPORT_IN_PEACE_ZONE_OR_BATTLEGROUND,
            ));
        }
    }

    if world.olympiad.is_in_competition(target_oid) || world.olympiad.is_registered(target_oid) {
        return Err(Refusal::Sm(sm_ids::THIS_CHARACTER_CANNOT_MAKE_A_REPORT));
    }

    // Clan war: you cannot report someone you are already at war with.
    if at_war_with(world, reporter_oid, target_oid) {
        return Err(Refusal::Sm(sm_ids::CANNOT_REPORT_DURING_A_CLAN_WAR));
    }

    // Java: `bot.getExp() == bot.getStat().getStartingExp()` — "has not
    // acquired any XP after connecting".
    //
    // **`setStartingExp` has no caller anywhere in Java or the datapack**, so
    // `_startingXp` is always 0 and the test really means "the target has zero
    // exp" — i.e. an untouched level-1 character. Ported as the behaviour it
    // has, not the behaviour its name claims; porting the *intent* would need a
    // per-session exp snapshot Java never takes, and would refuse reports Java
    // allows.
    let gained_nothing = world
        .objects
        .get_component::<Player>(&target_oid)
        .is_some_and(|p| p.exp == 0);
    if gained_nothing {
        return Err(Refusal::Sm(
            sm_ids::CANNOT_REPORT_A_CHARACTER_WITH_NO_XP_GAINED,
        ));
    }

    let table = &world.bot_reports;
    // Java: a *reported* character may not report anyone. Note the lookup is
    // `_reports.containsKey(reporterId)` — the reporter's own object id in the
    // **reported** map.
    if table.reports.contains_key(&reporter_oid) {
        return Err(Refusal::Sm(sm_ids::REPORTED_USERS_CANNOT_REPORT_OTHERS));
    }

    let delay = world.cfg.bot_report.report_delay_millis;
    let now = commons::util::now_millis();
    let ip = player_ip(world, reporter_oid);
    if let Some(&last) = table.ip_registry.get(&ip)
        && (now - last) <= delay
    {
        return Err(Refusal::Sm(sm_ids::ALREADY_REPORTED_BY_CLAN_OR_IP));
    }

    if let Some(rcd) = table.reports.get(&target_oid) {
        if rcd.reporters.contains_key(&reporter_oid) {
            return Err(Refusal::Sm(sm_ids::CANNOT_REPORT_THIS_PERSON_AGAIN));
        }
        if !world.cfg.bot_report.allow_reports_from_same_clan_members
            && reported_by_same_clan(world, rcd, reporter_oid)
        {
            return Err(Refusal::Sm(sm_ids::ALREADY_REPORTED_BY_CLAN_OR_IP));
        }
    }

    if let Some(rep) = table.reporters.get(&reporter_oid) {
        if rep.points == 0 {
            return Err(Refusal::Sm(sm_ids::ALL_REPORT_POINTS_USED));
        }
        let reuse = now - rep.last_report;
        if reuse < delay {
            // Java prints `reuse / 60000` — the time *elapsed*, not the time
            // remaining. Kept: the message is wrong upstream, and a player
            // comparing two servers would notice a "fix" as a difference.
            return Err(Refusal::Cooldown {
                minutes: (reuse / 60_000) as i32,
                points: rep.points as i32,
            });
        }
    }

    Ok(())
}

/// Java's `reportedBySameClan`: is any existing reporter in the would-be
/// reporter's clan?
fn reported_by_same_clan(world: &World, rcd: &ReportedCharData, reporter_oid: i32) -> bool {
    let Some(clan_id) = world
        .objects
        .get_component::<Player>(&reporter_oid)
        .map(|p| p.clan_id)
        .filter(|id| *id != 0)
    else {
        return false;
    };
    rcd.reporters.keys().any(|&other| {
        world
            .objects
            .get_component::<Player>(&other)
            .is_some_and(|p| p.clan_id == clan_id)
    })
}

fn at_war_with(world: &World, a_oid: i32, b_oid: i32) -> bool {
    let clan_of = |oid: i32| {
        world
            .objects
            .get_component::<Player>(&oid)
            .map(|p| p.clan_id)
            .filter(|id| *id != 0)
    };
    let (Some(a), Some(b)) = (clan_of(a_oid), clan_of(b_oid)) else {
        return false;
    };
    crate::game_loop::clans::wars::at_war_between(world, a, b)
}

/// The reporter's address (Java `hashIp` off the `GameClient`).
fn player_ip(world: &World, object_id: i32) -> String {
    world
        .clients
        .client_of_player(object_id)
        .and_then(|cid| world.clients.get(&cid))
        .map(|s| s.addr().ip().to_string())
        .unwrap_or_default()
}

/// Java `AbstractEffect.checkCondition`, as `TradeRequest` uses it: does any
/// live `BOT_PENALTY` buff on this player block `action_id`?
///
/// Java looks up the *first* buff of abnormal type `BOT_PENALTY` and walks its
/// effects; every `BlockAction` carrier on this dist declares that abnormal
/// type, so scanning the bearer's `BlockAction` effects is equivalent and does
/// not need the abnormal-type index.
pub(crate) fn is_action_blocked(world: &World, player_oid: i32, action_id: i32) -> bool {
    let Some(buffs) = world
        .objects
        .get_component::<crate::model::components::Buffs>(&player_oid)
    else {
        return false;
    };
    buffs.0.iter().any(|buff| {
        world
            .data
            .skill_data
            .get(buff.skill_id, 1)
            .is_some_and(|skill| {
                skill.effects.iter().any(|e| match e {
                    crate::model::skill::SkillEffect::BlockAction { blocked_actions } => {
                        blocked_actions.contains(&action_id)
                    }
                    _ => false,
                })
            })
    })
}

/// Java `handleReport`: the exact-count punishment, then every "range"
/// punishment (a negative `neededReportCount` means "at least |n| reports").
fn handle_report(world: &mut World, bot_oid: i32) {
    let count = world
        .bot_reports
        .reports
        .get(&bot_oid)
        .map(ReportedCharData::report_count)
        .unwrap_or(0) as i32;

    let to_apply: Vec<(i32, i32, i32)> = world
        .cfg
        .bot_report
        .punishments
        .iter()
        .filter(|p| {
            p.needed_report_count == count
                || (p.needed_report_count < 0 && -p.needed_report_count <= count)
        })
        .map(|p| (p.skill_id, p.skill_level, p.sys_message_id))
        .collect();

    for (skill_id, skill_level, sys_message_id) in to_apply {
        let Some(skill) = skill_by_id(world, skill_id, skill_level) else {
            tracing::warn!(
                "BotReport: could not punish with skill {skill_id}-{skill_level}: no such skill."
            );
            continue;
        };
        crate::game_loop::skills::effects::apply_skill_effects(world, bot_oid, bot_oid, &skill);
        if sys_message_id > 0
            && let Ok(id) = i16::try_from(sys_message_id)
        {
            crate::game_loop::helpers::send_sm_to_player(world, bot_oid, id, &[]);
        }
    }
}

/// Java `scheduleResetPointTask`: the first reset at the next
/// `BotReportPointsResetHour`. Called once at game-loop start.
pub(crate) fn schedule_initial_points_reset(world: &mut World) {
    let now = commons::util::now_millis();
    let (hour, minute) = world.cfg.bot_report.reset_hour;
    let day = 86_400_000i64;
    let target_of_day = (hour as i64) * 3_600_000 + (minute as i64) * 60_000;
    let mut delay_ms = target_of_day - now.rem_euclid(day);
    if delay_ms < 0 {
        delay_ms += day;
    }
    world.scheduler.schedule(
        world.tick + (delay_ms / 100) as u64,
        crate::scheduler::ScheduledTask::BotReportPointsReset,
    );
}

/// Java `ResetPointTask.run` → `resetPointsAndSchedule()`: reset, then re-arm.
pub(crate) fn handle_points_reset(world: &mut World) {
    reset_report_points(world);
    schedule_initial_points_reset(world);
}

/// Java `resetPointsAndSchedule`, run by the daily task: everyone's budget
/// back to 7.
pub(crate) fn reset_report_points(world: &mut World) {
    for rep in world.bot_reports.reporters.values_mut() {
        rep.points = DAILY_POINTS;
    }
    tracing::info!(
        "BotReport: daily report points reset for {} reporters.",
        world.bot_reports.reporters.len()
    );
}

/// Java `loadReportedCharData`, from the rows the DB worker read.
///
/// The second half is the subtle part: a report made **after** the most recent
/// daily reset has already cost its reporter a point, so the reporter's budget
/// is rebuilt from those rows rather than starting fresh at 7.
pub(crate) fn on_loaded(world: &mut World, rows: Vec<(i32, i32, i64)>, last_reset: i64) {
    let table = &mut world.bot_reports;
    for (bot_id, reporter_id, report_time) in rows {
        table
            .reports
            .entry(bot_id)
            .or_default()
            .reporters
            .insert(reporter_id, report_time);
        if report_time > last_reset {
            match table.reporters.get_mut(&reporter_id) {
                Some(rep) => rep.points -= 1,
                None => {
                    table.reporters.insert(
                        reporter_id,
                        ReporterCharData {
                            points: DAILY_POINTS - 1,
                            last_report: 0,
                        },
                    );
                }
            }
        }
    }
    tracing::info!("BotReport: loaded {} bot reports.", table.reports.len());
}

/// Java `saveReportedCharData` — called at shutdown, clears and rewrites the
/// whole table.
pub(crate) fn save_reports(world: &World) {
    let rows: Vec<(i32, i32, i64)> = world
        .bot_reports
        .reports
        .iter()
        .flat_map(|(bot_id, rcd)| {
            rcd.reporters
                .iter()
                .map(move |(reporter_id, time)| (*bot_id, *reporter_id, *time))
        })
        .collect();
    let _ = world
        .db
        .send(crate::db::DbCommand::StoreBotReports { rows });
}

/// The most recent occurrence of `BotReportPointsResetHour` (Java builds this
/// with a `Calendar`, stepping back a day when today's slot is still ahead).
pub(crate) fn last_reset_millis(cfg: &BotReportConfig, now: i64) -> i64 {
    let (hour, minute) = cfg.reset_hour;
    let day = 86_400_000i64;
    let midnight = now - now.rem_euclid(day);
    let today = midnight + (hour as i64) * 3_600_000 + (minute as i64) * 60_000;
    if now < today { today - day } else { today }
}
