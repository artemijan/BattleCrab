//! `Olympiad.ini` — the season clock, the point economy and the match rules.
//!
//! The port already carried every one of these values, each as a `const` with
//! the Java key in its doc comment. That is how they were verified against the
//! dist in the first place, and it is also why they were invisible to an
//! operator: editing `Olympiad.ini` changed nothing. This module turns the
//! comments into reads.
//!
//! Four keys are parsed and **inert on this dist**, for reasons that are
//! Java's rather than the port's:
//!
//! - `AltOlyWeaponEnchantLimit` / `AltOlyArmorEnchantLimit` are **-1**, which
//!   Java reads as "no limit" — the check is skipped entirely rather than
//!   compared against -1.
//! - `AltOlyRestrictedItems` is **empty**, so the ban list bans nothing.
//! - `AltOlyWinReward` / `AltOlyLoserReward` are **`None`**, the literal
//!   string, which Java parses to an empty reward list.
//!
//! They are carried so the values are visible and so honouring them later is a
//! change at the use site, not a hunt through the ini.

use crate::config::common::parse_tuples_separated_by_semicolon;
use commons::config::PropertiesParser;

pub const OLYMPIAD_CONFIG_FILE: &str = "config/Olympiad.ini";

/// Milliseconds in a day, for the period arithmetic below.
const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OlympiadConfig {
    /// `OlympiadEnabled` — whether the season clock runs at all.
    pub enabled: bool,
    /// `AltOlyStartTime` (hour) and `AltOlyMin` (minute): when the daily
    /// competition window opens.
    pub start_hour: i64,
    pub start_minute: i64,
    /// `AltOlyCPeriod` — how long the window stays open (6 h).
    pub comp_period_ms: i64,
    /// `AltOlyBattle` — a single match's time limit (5 min).
    pub battle_ms: i64,
    /// `AltOlyWPeriod` — the weekly point-refresh interval.
    pub weekly_period_ms: i64,
    /// `AltOlyVPeriod` — the validation window after a round closes (24 h).
    pub validation_period_ms: i64,
    /// `AltOlyStartPoints` — what a newly-registered noble starts with.
    pub start_points: i32,
    /// `AltOlyWeeklyPoints` — added to every noble each week.
    pub weekly_points: i32,
    /// `AltOlyMaxPoints` — the most a single match can move.
    pub max_points: i32,
    /// `AltOlyClassedParticipants` / `AltOlyNonClassedParticipants` — how many
    /// must be queued before matches are generated.
    pub classed_participants: usize,
    pub nonclassed_participants: usize,
    /// `AltOlyCompRewItem` — the item points are exchanged for.
    pub comp_reward_item: i32,
    /// `AltOlyMarkPerPoint` — how many of it per unclaimed point.
    pub mark_per_point: i64,
    /// `AltOlyMinMatchesForPoints` — matches needed to be hero-eligible.
    pub min_matches_for_points: i32,
    /// `AltOlyHeroPoints` — the end-of-round bonus for being a hero.
    pub hero_points: i32,
    /// `AltOlyRank1Points`…`AltOlyRank5Points`, by percentile rank.
    pub rank_points: [i32; 5],
    /// `AltOlyDividerClassed` / `AltOlyDividerNonClassed` — the loser's points
    /// are divided by this to compute the transfer.
    pub divider_classed: i32,
    pub divider_nonclassed: i32,
    /// `AltOlyMaxWeeklyMatches` — the per-noble weekly cap.
    pub max_weekly_matches: i32,
    /// `AltOlyCompetitionDays`, converted from Java's `Calendar` numbering
    /// (Sunday = 1 … Saturday = 7) to **0-indexed** days, which is what the
    /// season clock compares against.
    pub competition_days: Vec<i64>,
    /// `AltOlyPeriod` × `AltOlyPeriodMultiplier` — how long a round runs. Only
    /// `DAY` occurs on this dist; `WEEK`/`MONTH` are carried by the unit below.
    pub period_unit_days: i64,
    pub period_multiplier: i64,
    /// `AltOlyWaitTime` (seconds) — the pause between a match being made and
    /// the fighters being teleported in.
    pub wait_time_secs: i64,
    /// `AltOlyAnnounceGames` — whether match starts are announced server-wide.
    pub announce_games: bool,
    /// `AltOlyShowMonthlyWinners` — whether the Monument lists last round's
    /// heroes.
    pub show_monthly_winners: bool,
    /// `AltOlyLogFights` — Java writes an `olympiad.log` line per match.
    pub log_fights: bool,
    /// `AltOlyWeaponEnchantLimit` / `AltOlyArmorEnchantLimit`. **-1 means no
    /// limit** and is the shipped value; see the module note.
    pub weapon_enchant_limit: i32,
    pub armor_enchant_limit: i32,
    /// `AltOlyRestrictedItems` — item ids barred from the arena. Empty here.
    pub restricted_items: Vec<i32>,
    /// `AltOlyWinReward` / `AltOlyLoserReward` as `(item_id, count)` pairs.
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
            start_hour: 18,
            start_minute: 0,
            comp_period_ms: 21_600_000,
            battle_ms: 300_000,
            weekly_period_ms: 604_800_000,
            validation_period_ms: 86_400_000,
            start_points: 10,
            weekly_points: 10,
            max_points: 10,
            classed_participants: 20,
            nonclassed_participants: 20,
            comp_reward_item: 45584,
            mark_per_point: 20,
            min_matches_for_points: 10,
            hero_points: 300,
            rank_points: [200, 80, 50, 30, 15],
            divider_classed: 5,
            divider_nonclassed: 5,
            max_weekly_matches: 30,
            competition_days: vec![0, 6],
            period_unit_days: 1,
            period_multiplier: 14,
            wait_time_secs: 120,
            announce_games: true,
            show_monthly_winners: true,
            log_fights: false,
            weapon_enchant_limit: -1,
            armor_enchant_limit: -1,
            restricted_items: Vec::new(),
            win_reward: Vec::new(),
            loser_reward: Vec::new(),
        }
    }
}

impl OlympiadConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let p = PropertiesParser::load_rel(root, OLYMPIAD_CONFIG_FILE);
        // `AltOlyCompetitionDays = 1,7` in Java's `Calendar` numbering, where
        // Sunday is 1. The season clock works 0-indexed, so shift.
        let competition_days = {
            let raw = p.get_string("AltOlyCompetitionDays", "");
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
            .get_string("AltOlyRestrictedItems", "")
            .split(',')
            .filter_map(|s| s.trim().parse::<i32>().ok())
            .collect();
        Self {
            enabled: p.get_bool("OlympiadEnabled", d.enabled),
            start_hour: i64::from(p.get_int("AltOlyStartTime", d.start_hour as i32)),
            start_minute: i64::from(p.get_int("AltOlyMin", d.start_minute as i32)),
            comp_period_ms: p.get_long("AltOlyCPeriod", d.comp_period_ms),
            battle_ms: p.get_long("AltOlyBattle", d.battle_ms),
            weekly_period_ms: p.get_long("AltOlyWPeriod", d.weekly_period_ms),
            validation_period_ms: p.get_long("AltOlyVPeriod", d.validation_period_ms),
            start_points: p.get_int("AltOlyStartPoints", d.start_points),
            weekly_points: p.get_int("AltOlyWeeklyPoints", d.weekly_points),
            max_points: p.get_int("AltOlyMaxPoints", d.max_points),
            classed_participants: p
                .get_int("AltOlyClassedParticipants", d.classed_participants as i32)
                .max(0) as usize,
            nonclassed_participants: p
                .get_int(
                    "AltOlyNonClassedParticipants",
                    d.nonclassed_participants as i32,
                )
                .max(0) as usize,
            comp_reward_item: p.get_int("AltOlyCompRewItem", d.comp_reward_item),
            mark_per_point: i64::from(p.get_int("AltOlyMarkPerPoint", d.mark_per_point as i32)),
            min_matches_for_points: p
                .get_int("AltOlyMinMatchesForPoints", d.min_matches_for_points),
            hero_points: p.get_int("AltOlyHeroPoints", d.hero_points),
            rank_points: [
                p.get_int("AltOlyRank1Points", d.rank_points[0]),
                p.get_int("AltOlyRank2Points", d.rank_points[1]),
                p.get_int("AltOlyRank3Points", d.rank_points[2]),
                p.get_int("AltOlyRank4Points", d.rank_points[3]),
                p.get_int("AltOlyRank5Points", d.rank_points[4]),
            ],
            divider_classed: p.get_int("AltOlyDividerClassed", d.divider_classed),
            divider_nonclassed: p.get_int("AltOlyDividerNonClassed", d.divider_nonclassed),
            max_weekly_matches: p.get_int("AltOlyMaxWeeklyMatches", d.max_weekly_matches),
            competition_days,
            // `AltOlyPeriod` is `DAY`, `WEEK` or `MONTH`; only the first
            // occurs here. A month is taken as 30 days, matching Java's
            // `Calendar.MONTH` roll closely enough for the end-of-round anchor.
            period_unit_days: match p.get_string("AltOlyPeriod", "DAY").trim() {
                "WEEK" => 7,
                "MONTH" => 30,
                _ => 1,
            },
            period_multiplier: i64::from(
                p.get_int("AltOlyPeriodMultiplier", d.period_multiplier as i32),
            ),
            wait_time_secs: i64::from(p.get_int("AltOlyWaitTime", d.wait_time_secs as i32)),
            announce_games: p.get_bool("AltOlyAnnounceGames", d.announce_games),
            show_monthly_winners: p.get_bool("AltOlyShowMonthlyWinners", d.show_monthly_winners),
            log_fights: p.get_bool("AltOlyLogFights", d.log_fights),
            weapon_enchant_limit: p.get_int("AltOlyWeaponEnchantLimit", d.weapon_enchant_limit),
            armor_enchant_limit: p.get_int("AltOlyArmorEnchantLimit", d.armor_enchant_limit),
            restricted_items,
            win_reward: items_list(p.get_string("AltOlyWinReward", "none")),
            loser_reward: items_list(p.get_string("AltOlyLoserReward", "none")),
        }
    }

    /// The competition window's opening time, as milliseconds past midnight.
    pub fn comp_start_ms_of_day(&self) -> i64 {
        (self.start_hour * 60 + self.start_minute) * 60 * 1000
    }

    /// `Olympiad.setNewOlympiadEnd`'s span: how many days a round runs.
    pub fn period_days(&self) -> i64 {
        (self.period_unit_days * self.period_multiplier).max(1)
    }

    /// `AltOlyVPeriod` in whole days, for the validation-window arithmetic.
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

    /// `AltOlyCompetitionDays = 1,7` is Java `Calendar` numbering (Sunday = 1),
    /// and the season clock is 0-indexed — so the shipped file means Sunday
    /// and Saturday, i.e. weekends. Off-by-one here would move the Olympiad to
    /// Monday and Sunday.
    #[test]
    fn competition_days_shift_out_of_calendar_numbering() {
        let loaded = OlympiadConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(loaded.competition_days, vec![0, 6], "Sunday and Saturday");
    }

    /// The four keys the module note calls inert, pinned so the note stays
    /// true: a datapack that starts restricting items or capping enchants
    /// should fail this and be looked at.
    #[test]
    fn the_inert_keys_are_still_inert() {
        let l = OlympiadConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(l.weapon_enchant_limit, -1, "-1 = no limit");
        assert_eq!(l.armor_enchant_limit, -1, "-1 = no limit");
        assert!(l.restricted_items.is_empty(), "no banned items");
        assert!(l.win_reward.is_empty(), "`None` parses to no reward");
        assert!(l.loser_reward.is_empty(), "`None` parses to no reward");
    }
}
