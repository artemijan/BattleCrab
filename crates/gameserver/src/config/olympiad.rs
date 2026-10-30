//! `Olympiad.ini` — the season clock, the point economy and the match rules.
//!
//! The port already carried every one of these values, each as a `const` with
//! the Java key in its doc comment. That is how they were verified against the
//! dist in the first place, and it is also why they were invisible to an
//! operator: editing `Olympiad.ini` changed nothing. This module turns the
//! comments into reads.
//!
//! Three keys are parsed and **inert on this dist**, for reasons that are
//! Java's rather than the port's:
//!
//! - `OlympiadWeaponEnchantLimit` / `OlympiadArmorEnchantLimit` are **-1**, which
//!   Java reads as "no limit" — the check is skipped entirely rather than
//!   compared against -1.
//! - `OlympiadRestrictedItems` is **empty**, so the ban list bans nothing.
//!
//! They are carried so the values are visible and so honouring them later is a
//! change at the use site, not a hunt through the ini.

use crate::config::common::parse_tuples_separated_by_semicolon;
use commons::config::PropertiesParser;

pub const OLYMPIAD_CONFIG_FILE: &str = "config/Olympiad.ini";

/// `OlympiadPeriod` — the calendar unit `Olympiad.setNewOlympiadEnd` adds
/// `OlympiadPeriodMultiplier` of (`DAY_OF_MONTH`, `WEEK_OF_MONTH`, `MONTH`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlympiadPeriod {
    Day,
    Week,
    Month,
    /// Any other string: Java's `switch` matches no case and the round ends
    /// at noon today.
    Unknown,
}

/// Milliseconds in a day, for the period arithmetic below.
const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadConfig {
    /// `OlympiadEnabled` — whether the season clock runs at all.
    pub enabled: bool,
    /// `OlympiadStartTime` (hour) and `OlympiadMin` (minute): when the daily
    /// competition window opens.
    pub start_hour: i64,
    pub start_minute: i64,
    /// `OlympiadCPeriod` — how long the window stays open (6 h).
    pub comp_period_ms: i64,
    /// `OlympiadBattle` — a single match's time limit (5 min).
    pub battle_ms: i64,
    /// `OlympiadWPeriod` — the weekly point-refresh interval.
    pub weekly_period_ms: i64,
    /// `OlympiadVPeriod` — the validation window after a round closes (24 h).
    pub validation_period_ms: i64,
    /// `OlympiadStartPoints` — what a newly-registered noble starts with.
    pub start_points: i32,
    /// `OlympiadWeeklyPoints` — added to every noble each week.
    pub weekly_points: i32,
    /// `OlympiadMaxPoints` — the most a single match can move.
    pub max_points: i32,
    /// `OlympiadClassedParticipants` / `OlympiadNonClassedParticipants` — how many
    /// must be queued before matches are generated.
    pub classed_participants: usize,
    pub nonclassed_participants: usize,
    /// `OlympiadCompRewItem` — the item points are exchanged for.
    pub comp_reward_item: i32,
    /// `OlympiadMarkPerPoint` — how many of it per unclaimed point.
    pub mark_per_point: i64,
    /// `OlympiadMinMatchesForPoints` — matches needed to be hero-eligible.
    pub min_matches_for_points: i32,
    /// `OlympiadHeroPoints` — the end-of-round bonus for being a hero.
    pub hero_points: i32,
    /// `OlympiadRank1Points`…`OlympiadRank5Points`, by percentile rank.
    pub rank_points: [i32; 5],
    /// `OlympiadDividerClassed` / `OlympiadDividerNonClassed` — the loser's points
    /// are divided by this to compute the transfer.
    pub divider_classed: i32,
    pub divider_nonclassed: i32,
    /// `OlympiadMaxWeeklyMatches` — the per-noble weekly cap.
    pub max_weekly_matches: i32,
    /// `OlympiadCompetitionDays`, converted from Java's `Calendar` numbering
    /// (Sunday = 1 … Saturday = 7) to **0-indexed** days, which is what the
    /// season clock compares against.
    pub competition_days: Vec<i64>,
    /// `OlympiadPeriod` × `OlympiadPeriodMultiplier` — how long a round runs
    /// (`MONTH` × 1 on this dist).
    pub period: OlympiadPeriod,
    pub period_multiplier: i64,
    /// `OlympiadWaitTime` (seconds) — the pause between a match being made and
    /// the fighters being teleported in.
    pub wait_time_secs: i64,
    /// `OlympiadAnnounceGames` — whether match starts are announced server-wide.
    pub announce_games: bool,
    /// `OlympiadShowMonthlyWinners` — whether the Monument lists last round's
    /// heroes.
    pub show_monthly_winners: bool,
    /// `OlympiadLogFights` — Java writes an `olympiad.log` line per match.
    pub log_fights: bool,
    /// `OlympiadWeaponEnchantLimit` / `OlympiadArmorEnchantLimit`. **-1 means no
    /// limit** and is the shipped value; see the module note.
    pub weapon_enchant_limit: i32,
    pub armor_enchant_limit: i32,
    /// `OlympiadRestrictedItems` — item ids barred from the arena. Empty here.
    pub restricted_items: Vec<i32>,
    /// `OlympiadWinReward` / `OlympiadLoserReward` as `(item_id, count)` pairs.
    /// Both are the literal `None` here, which Java's `parseItemsList` turns
    /// into no list at all — so a match pays nothing beyond the points.
    pub win_reward: Vec<(i32, i64)>,
    pub loser_reward: Vec<(i32, i64)>,
}

impl Default for OlympiadConfig {
    /// The dist's own values, so a test world matches production without
    /// reading the file.
    fn default() -> Self {
        Self {
            enabled: true,
            start_hour: 20,
            start_minute: 0,
            comp_period_ms: 14_400_000,
            battle_ms: 300_000,
            weekly_period_ms: 604_800_000,
            validation_period_ms: 86_400_000,
            start_points: 10,
            weekly_points: 10,
            max_points: 10,
            classed_participants: 20,
            nonclassed_participants: 10,
            comp_reward_item: 45584,
            mark_per_point: 20,
            min_matches_for_points: 10,
            hero_points: 30,
            rank_points: [60, 50, 45, 40, 30],
            divider_classed: 5,
            divider_nonclassed: 5,
            max_weekly_matches: 30,
            competition_days: vec![5, 6],
            period: OlympiadPeriod::Month,
            period_multiplier: 1,
            wait_time_secs: 120,
            announce_games: true,
            show_monthly_winners: true,
            log_fights: false,
            weapon_enchant_limit: -1,
            armor_enchant_limit: -1,
            restricted_items: Vec::new(),
            win_reward: vec![(45584, 12)],
            loser_reward: vec![(45584, 7)],
        }
    }
}

impl OlympiadConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let p = PropertiesParser::load_rel(root, OLYMPIAD_CONFIG_FILE);
        // `OlympiadCompetitionDays = 1,7` in Java's `Calendar` numbering, where
        // Sunday is 1. The season clock works 0-indexed, so shift.
        let competition_days = {
            let raw = p.get_string("OlympiadCompetitionDays", "");
            let days: Vec<i64> = raw
                .split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .map(|d| (d - 1).rem_euclid(7))
                .collect();
            if days.is_empty() {
                d.competition_days.clone()
            } else {
                days
            }
        };
        // `Config.parseItemsList`: `itemId,count;itemId,count`, and the
        // literal `none` (any case) means an empty list.
        let items_list = |raw: String| -> Vec<(i32, i64)> {
            if raw.trim().eq_ignore_ascii_case("none") {
                return Vec::new();
            }
            parse_tuples_separated_by_semicolon(&raw)
        };
        let restricted_items = p
            .get_string("OlympiadRestrictedItems", "")
            .split(',')
            .filter_map(|s| s.trim().parse::<i32>().ok())
            .collect();
        Self {
            enabled: p.get_bool("OlympiadEnabled", d.enabled),
            start_hour: i64::from(p.get_int("OlympiadStartTime", d.start_hour as i32)),
            start_minute: i64::from(p.get_int("OlympiadMin", d.start_minute as i32)),
            comp_period_ms: p.get_long("OlympiadCPeriod", d.comp_period_ms),
            battle_ms: p.get_long("OlympiadBattle", d.battle_ms),
            weekly_period_ms: p.get_long("OlympiadWPeriod", d.weekly_period_ms),
            validation_period_ms: p.get_long("OlympiadVPeriod", d.validation_period_ms),
            start_points: p.get_int("OlympiadStartPoints", d.start_points),
            weekly_points: p.get_int("OlympiadWeeklyPoints", d.weekly_points),
            max_points: p.get_int("OlympiadMaxPoints", d.max_points),
            classed_participants: p
                .get_int("OlympiadClassedParticipants", d.classed_participants as i32)
                .max(0) as usize,
            nonclassed_participants: p
                .get_int(
                    "OlympiadNonClassedParticipants",
                    d.nonclassed_participants as i32,
                )
                .max(0) as usize,
            comp_reward_item: p.get_int("OlympiadCompRewItem", d.comp_reward_item),
            mark_per_point: i64::from(p.get_int("OlympiadMarkPerPoint", d.mark_per_point as i32)),
            min_matches_for_points: p
                .get_int("OlympiadMinMatchesForPoints", d.min_matches_for_points),
            hero_points: p.get_int("OlympiadHeroPoints", d.hero_points),
            rank_points: [
                p.get_int("OlympiadRank1Points", d.rank_points[0]),
                p.get_int("OlympiadRank2Points", d.rank_points[1]),
                p.get_int("OlympiadRank3Points", d.rank_points[2]),
                p.get_int("OlympiadRank4Points", d.rank_points[3]),
                p.get_int("OlympiadRank5Points", d.rank_points[4]),
            ],
            divider_classed: p.get_int("OlympiadDividerClassed", d.divider_classed),
            divider_nonclassed: p.get_int("OlympiadDividerNonClassed", d.divider_nonclassed),
            max_weekly_matches: p.get_int("OlympiadMaxWeeklyMatches", d.max_weekly_matches),
            competition_days,
            period: match p.get_string("OlympiadPeriod", "MONTH").trim() {
                "DAY" => OlympiadPeriod::Day,
                "WEEK" => OlympiadPeriod::Week,
                "MONTH" => OlympiadPeriod::Month,
                _ => OlympiadPeriod::Unknown,
            },
            period_multiplier: i64::from(
                p.get_int("OlympiadPeriodMultiplier", d.period_multiplier as i32),
            ),
            wait_time_secs: i64::from(p.get_int("OlympiadWaitTime", d.wait_time_secs as i32)),
            announce_games: p.get_bool("OlympiadAnnounceGames", d.announce_games),
            show_monthly_winners: p.get_bool("OlympiadShowMonthlyWinners", d.show_monthly_winners),
            log_fights: p.get_bool("OlympiadLogFights", d.log_fights),
            weapon_enchant_limit: p.get_int("OlympiadWeaponEnchantLimit", d.weapon_enchant_limit),
            armor_enchant_limit: p.get_int("OlympiadArmorEnchantLimit", d.armor_enchant_limit),
            restricted_items,
            win_reward: items_list(p.get_string("OlympiadWinReward", "none")),
            loser_reward: items_list(p.get_string("OlympiadLoserReward", "none")),
        }
    }

    /// The competition window's opening time, as milliseconds past midnight.
    pub fn comp_start_ms_of_day(&self) -> i64 {
        (self.start_hour * 60 + self.start_minute) * 60 * 1000
    }

    /// `OlympiadVPeriod` in whole days, for the validation-window arithmetic.
    pub fn validation_days(&self) -> i64 {
        (self.validation_period_ms / MILLIS_PER_DAY).max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults claim to be the shipped values; hold them to it, the way
    /// `grand_boss` and `pvp` do.
    #[test]
    fn default_config_matches_the_shipped_ini() {
        let loaded = OlympiadConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(loaded, OlympiadConfig::default());
    }

    /// `OlympiadCompetitionDays = 1,7` is Java `Calendar` numbering (Sunday = 1),
    /// and the season clock is 0-indexed — so the shipped file means Sunday
    /// and Saturday, i.e. weekends. Off-by-one here would move the Olympiad to
    /// Monday and Sunday.
    #[test]
    fn competition_days_shift_out_of_calendar_numbering() {
        let loaded = OlympiadConfig::load_from(crate::data::DIST_GAME);
        // `6,7` in `Calendar` numbering.
        assert_eq!(loaded.competition_days, vec![5, 6], "Friday and Saturday");
    }

    /// The three keys the module note calls inert, pinned so the note stays
    /// true: a datapack that starts restricting items or capping enchants
    /// should fail this and be looked at.
    #[test]
    fn the_inert_keys_are_still_inert() {
        let l = OlympiadConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(l.weapon_enchant_limit, -1, "-1 = no limit");
        assert_eq!(l.armor_enchant_limit, -1, "-1 = no limit");
        assert!(l.restricted_items.is_empty(), "no banned items");
    }
}
