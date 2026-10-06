//! The two timestamp shapes the log files carry, as epoch milliseconds: the
//! RFC 3339 stamp on every line (`timestamp` in diagnostic JSON, `ts` in audit
//! NDJSON, the leading token of an `_error.log` line), and the date
//! `tracing-appender` puts in a rotated filename. Both are UTC — the appender
//! rotates on UTC boundaries and every writer stamps in UTC.
//!
//! Hand-rolled rather than a date crate: two fixed formats, and this runs on
//! every line a search touches.

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn num(b: &[u8]) -> Option<i64> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(b).ok()?.parse().ok()
}

fn date(b: &[u8]) -> Option<i64> {
    // YYYY-MM-DD
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, m, d) = (num(&b[0..4])?, num(&b[5..7])?, num(&b[8..10])?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d) * DAY_MS)
}

/// `2026-08-14T00:18:26.402259Z` (or with a `±HH:MM` offset) → epoch ms.
pub fn parse_rfc3339(s: &[u8]) -> Option<i64> {
    if s.len() < 20 || s[10] != b'T' || s[13] != b':' || s[16] != b':' {
        return None;
    }
    let day = date(&s[0..10])?;
    let (h, mi, sec) = (num(&s[11..13])?, num(&s[14..16])?, num(&s[17..19])?);
    if h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut ms = 0;
    if let Some(frac) = rest.strip_prefix(b".") {
        let digits = frac.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        // Milliseconds: the first three digits, right-padded with zeros.
        let mut digits_ms = frac[..digits.min(3)].iter().map(|c| (c - b'0') as i64);
        for _ in 0..3 {
            ms = ms * 10 + digits_ms.next().unwrap_or(0);
        }
        rest = &frac[digits..];
    }
    let offset_ms = match rest {
        b"Z" | b"z" => 0,
        [sign @ (b'+' | b'-'), oh1, oh2, b':', om1, om2] => {
            let oh = num(&[*oh1, *oh2])?;
            let om = num(&[*om1, *om2])?;
            let v = (oh * 60 + om) * 60_000;
            if *sign == b'+' { v } else { -v }
        }
        _ => return None,
    };
    Some(day + h * HOUR_MS + mi * 60_000 + sec * 1000 + ms - offset_ms)
}

/// The span a rotated filename's date covers: `2026-08-14` is that UTC day,
/// `2026-08-14-07` that UTC hour. `[start, end)` in epoch ms.
pub fn file_date_span(key: &str) -> Option<(i64, i64)> {
    let b = key.as_bytes();
    match b.len() {
        10 => date(b).map(|d| (d, d + DAY_MS)),
        13 if b[10] == b'-' => {
            let h = num(&b[11..13]).filter(|h| *h < 24)?;
            let start = date(&b[0..10])? + h * HOUR_MS;
            Some((start, start + HOUR_MS))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_anchors() {
        assert_eq!(parse_rfc3339(b"1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339(b"2000-03-01T00:00:00Z"),
            Some(951_868_800_000)
        );
        // Leap day, and a value checked against `date -u -d … +%s%3N`.
        assert_eq!(
            parse_rfc3339(b"2024-02-29T12:00:00Z"),
            Some(1_709_208_000_000)
        );
        assert_eq!(
            parse_rfc3339(b"2026-08-14T00:18:26.402259Z"),
            Some(1_786_666_706_402)
        );
    }

    #[test]
    fn fractions_and_offsets() {
        let base = parse_rfc3339(b"2026-08-14T00:00:00Z").unwrap();
        assert_eq!(parse_rfc3339(b"2026-08-14T00:00:00.5Z"), Some(base + 500));
        assert_eq!(
            parse_rfc3339(b"2026-08-14T00:00:00.123999Z"),
            Some(base + 123)
        );
        assert_eq!(parse_rfc3339(b"2026-08-14T02:00:00+02:00"), Some(base));
        assert_eq!(parse_rfc3339(b"2026-08-13T22:30:00-01:30"), Some(base));
    }

    #[test]
    fn garbage_is_none_not_a_panic() {
        for s in [
            &b""[..],
            b"not a timestamp",
            b"2026-08-14 00:00:00Z",
            b"2026-13-01T00:00:00Z",
            b"2026-08-14T25:00:00Z",
            b"2026-08-14T00:00:00",
            b"2026-08-14T00:00:00.Z",
            b"2026-08-14T00:00:00+0200",
        ] {
            assert_eq!(parse_rfc3339(s), None, "{:?}", String::from_utf8_lossy(s));
        }
    }

    #[test]
    fn filename_dates_cover_a_day_or_an_hour() {
        let d = parse_rfc3339(b"2026-08-14T00:00:00Z").unwrap();
        assert_eq!(file_date_span("2026-08-14"), Some((d, d + DAY_MS)));
        assert_eq!(
            file_date_span("2026-08-14-07"),
            Some((d + 7 * HOUR_MS, d + 8 * HOUR_MS))
        );
        assert_eq!(file_date_span("2026-08-14-24"), None);
        assert_eq!(file_date_span("latest"), None);
    }
}
