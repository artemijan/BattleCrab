//! Cron-style scheduling patterns — the port of `commons/time/SchedulingPattern`
//! narrowed to what the datapack can express.
//!
//! Five space-separated fields, in the order the event config documents them:
//! `minute (0-59) hour (0-23) day-of-month (1-31) month (1-12) day-of-week
//! (0-7, where both 0 and 7 are Sunday)`. Each field accepts `*`, a value, a
//! `a-b` range, a `a,b,c` list, and a `/n` step on any of those.
//!
//! Java's matcher also supports `L`/`W`-style day-of-month tokens and multiple
//! patterns separated by `|`; nothing in this datapack uses them, so they parse
//! as an error here rather than being silently mis-read.
//!
//! **Times are evaluated in UTC**, where Java uses the server's local zone —
//! the same convention the rest of this port's wall-clock work follows.

/// One parsed field of a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    /// Allowed values, already expanded (small sets: ≤ 60 entries).
    values: Vec<u32>,
}

impl Field {
    fn parse(spec: &str, min: u32, max: u32) -> Option<Self> {
        let mut values = Vec::new();
        for part in spec.split(',') {
            let (range, step) = match part.split_once('/') {
                Some((r, s)) => (r, s.parse::<u32>().ok().filter(|&s| s > 0)?),
                None => (part, 1),
            };
            let (lo, hi) = if range == "*" {
                (min, max)
            } else if let Some((a, b)) = range.split_once('-') {
                (a.parse().ok()?, b.parse().ok()?)
            } else {
                let v: u32 = range.parse().ok()?;
                (v, v)
            };
            if lo < min || hi > max || lo > hi {
                return None;
            }
            values.extend((lo..=hi).step_by(step as usize));
        }
        if values.is_empty() {
            return None;
        }
        values.sort_unstable();
        values.dedup();
        Some(Self { values })
    }

    fn matches(&self, value: u32) -> bool {
        self.values.binary_search(&value).is_ok()
    }
}

/// A parsed scheduling pattern (Java `SchedulingPattern`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulingPattern {
    minute: Field,
    hour: Field,
    day_of_month: Field,
    month: Field,
    day_of_week: Field,
}

impl SchedulingPattern {
    /// Parse `"m h dom mon dow"`; `None` when the pattern is malformed (Java
    /// throws `InvalidPatternException`, which the loader logs and skips).
    pub fn parse(pattern: &str) -> Option<Self> {
        let parts: Vec<&str> = pattern.split_whitespace().collect();
        if parts.len() != 5 {
            return None;
        }
        Some(Self {
            minute: Field::parse(parts[0], 0, 59)?,
            hour: Field::parse(parts[1], 0, 23)?,
            day_of_month: Field::parse(parts[2], 1, 31)?,
            month: Field::parse(parts[3], 1, 12)?,
            // 0 and 7 both mean Sunday, as in Java.
            day_of_week: Field::parse(parts[4], 0, 7)?,
        })
    }

    /// Java `match(millis)` — does this minute match the pattern?
    pub fn matches(&self, millis: i64) -> bool {
        let (month, day, hour, minute) = civil_fields(millis);
        let dow = day_of_week(millis);
        self.minute.matches(minute)
            && self.hour.matches(hour)
            && self.day_of_month.matches(day)
            && self.month.matches(month)
            // Sunday matches whether the pattern wrote 0 or 7.
            && (self.day_of_week.matches(dow) || (dow == 0 && self.day_of_week.matches(7)))
    }

    /// Java `next(millis)` — the next matching moment strictly after `millis`,
    /// at minute granularity. `None` for a pattern no date satisfies (e.g.
    /// `0 0 30 2 *`), which Java would loop on forever.
    pub fn next_after(&self, millis: i64) -> Option<i64> {
        const MINUTE: i64 = 60_000;
        // Start at the next whole minute.
        let mut t = (millis / MINUTE + 1) * MINUTE;
        // Four years covers every leap-year case a five-field pattern can name.
        let limit = t + 4 * 366 * 24 * 60 * MINUTE;
        while t < limit {
            if self.matches(t) {
                return Some(t);
            }
            t += MINUTE;
        }
        None
    }

    /// Java `getDelayToNextFromNow()` — milliseconds until the next match.
    pub fn delay_from(&self, now_millis: i64) -> Option<i64> {
        self.next_after(now_millis).map(|t| t - now_millis)
    }
}

/// [`crate::util::civil_from_millis`] narrowed to the field widths `Field`
/// matches on. Every part is bounded by the calendar, so the casts can't wrap.
fn civil_fields(millis: i64) -> (u32, u32, u32, u32) {
    let (_, month, day, hour, minute, _) = crate::util::civil_from_millis(millis);
    (month as u32, day as u32, hour as u32, minute as u32)
}

/// Day of week in UTC, `0` = Sunday (epoch day 0 was a Thursday).
fn day_of_week(millis: i64) -> u32 {
    let days = millis.div_euclid(86_400_000);
    ((days + 4).rem_euclid(7)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;
    const MINUTE: i64 = 60_000;

    #[test]
    fn parses_and_rejects() {
        assert!(SchedulingPattern::parse("0 20 * * *").is_some());
        assert!(SchedulingPattern::parse("*/15 * * * *").is_some());
        assert!(SchedulingPattern::parse("0,30 8-18 1-15 1,7 1-5").is_some());
        // Wrong field count, out-of-range values and junk are refused.
        assert!(SchedulingPattern::parse("0 20 * *").is_none());
        assert!(SchedulingPattern::parse("60 20 * * *").is_none());
        assert!(SchedulingPattern::parse("0 24 * * *").is_none());
        assert!(SchedulingPattern::parse("0 20 * * L").is_none());
    }

    #[test]
    fn matches_the_named_minute() {
        // The dist's commented-out example: 20:00 every day.
        let p = SchedulingPattern::parse("0 20 * * *").unwrap();
        assert!(p.matches(20 * HOUR));
        assert!(!p.matches(20 * HOUR + MINUTE));
        assert!(!p.matches(19 * HOUR));
        assert!(p.matches(5 * DAY + 20 * HOUR), "any day");
    }

    #[test]
    fn next_after_finds_the_following_slot() {
        let p = SchedulingPattern::parse("0 20 * * *").unwrap();
        // From 19:30 on epoch day 0 → 20:00 the same day.
        assert_eq!(p.next_after(19 * HOUR + 30 * MINUTE), Some(20 * HOUR));
        // From 20:00 exactly → tomorrow (strictly after).
        assert_eq!(p.next_after(20 * HOUR), Some(DAY + 20 * HOUR));
        assert_eq!(p.delay_from(19 * HOUR), Some(HOUR));
    }

    #[test]
    fn day_of_week_and_sunday_aliasing() {
        // Epoch day 0 (1970-01-01) was a Thursday = 4; day 3 is Sunday.
        assert_eq!(day_of_week(0), 4);
        assert_eq!(day_of_week(3 * DAY), 0);
        let sunday_0 = SchedulingPattern::parse("0 0 * * 0").unwrap();
        let sunday_7 = SchedulingPattern::parse("0 0 * * 7").unwrap();
        assert!(sunday_0.matches(3 * DAY));
        assert!(sunday_7.matches(3 * DAY), "7 is Sunday too");
        assert!(!sunday_0.matches(4 * DAY));
    }

    #[test]
    fn an_impossible_pattern_terminates() {
        // 30 February never happens — Java's search would spin forever.
        let p = SchedulingPattern::parse("0 0 30 2 *").unwrap();
        assert_eq!(p.next_after(0), None);
    }
}
