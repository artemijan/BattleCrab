//! Port of `commons/util/Rnd.java` and small helpers.

use num_bigint_dig::BigInt;
use rand::Rng;

/// Parse a (possibly signed) base-16 hexid string into Java `BigInteger(s,16)
/// .toByteArray()` bytes (two's-complement, big-endian).
pub fn hexid_from_string(s: &str) -> Option<Vec<u8>> {
    let big = BigInt::parse_bytes(s.trim().as_bytes(), 16)?;
    Some(big.to_signed_bytes_be())
}

/// `hexToString`: two's-complement bytes → signed `BigInteger` hex string.
pub fn hexid_to_string(bytes: &[u8]) -> String {
    BigInt::from_signed_bytes_be(bytes).to_str_radix(16)
}

pub mod rnd {
    use super::*;
    use rand::random;
    use rand::seq::SliceRandom;

    /// `Rnd.nextInt()` — full-range random i32.
    pub fn next_int() -> i32 {
        random()
    }

    /// `Rnd.get(n)` — random value in `[0, n)`.
    pub fn get(n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        rand::thread_rng().gen_range(0..n)
    }

    /// `Rnd.get(min, max)` — random value in `[min, max]` (inclusive, like Java).
    pub fn get_range(min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        rand::thread_rng().gen_range(min..=max)
    }

    /// `Rnd.nextBytes(array)`.
    pub fn fill_bytes(buf: &mut [u8]) {
        rand::thread_rng().fill(buf);
    }

    /// Returns true if random(0..100) < chance.
    pub fn chance(chance: f64) -> bool {
        rand::thread_rng().gen_range(0.0..100.0) < chance
    }

    /// Returns a random element from a slice.
    pub fn get_random_element<T>(slice: &[T]) -> Option<&T> {
        slice.choose(&mut rand::thread_rng())
    }

    /// Shuffles a slice in place.
    pub fn shuffle<T>(slice: &mut [T]) {
        slice.shuffle(&mut rand::thread_rng());
    }

    /// Internal wrapper to use our RNG where `CryptoRng + RngCore` is required.
    pub struct InternalRng;
    impl rand::RngCore for InternalRng {
        fn next_u32(&mut self) -> u32 {
            rand::thread_rng().next_u32()
        }
        fn next_u64(&mut self) -> u64 {
            rand::thread_rng().next_u64()
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            rand::thread_rng().fill_bytes(dest)
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            rand::thread_rng().try_fill_bytes(dest)
        }
    }
    impl rand::CryptoRng for InternalRng {}

    /// Returns a random i32 in `[0, bound)`.
    pub fn get_bound(bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        rand::thread_rng().gen_range(0..bound)
    }

    /// Returns a random f64 in `[0.0, 1.0)`.
    pub fn get_f64() -> f64 {
        rand::thread_rng().gen_range(0.0..1.0)
    }

    /// Returns a standard normal draw (mean 0, sd 1).
    pub fn next_gaussian() -> f64 {
        use rand_distr::{Distribution, StandardNormal};
        let mut rng = rand::thread_rng();
        StandardNormal.sample(&mut rng)
    }
}

/// Hex-encode bytes (lowercase, no separator).
pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Port of `CommonUtil.generateHex(size)`: `size` random bytes, **none zero**
/// (the GS-link relies on this for its RSA leading-zero strip and hexid).
pub fn generate_hex(size: usize) -> Vec<u8> {
    let mut array = vec![0u8; size];
    rnd::fill_bytes(&mut array);
    for b in array.iter_mut() {
        while *b == 0 {
            *b = rnd::get(i8::MAX as i32) as u8;
        }
    }
    array
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Howard Hinnant's `civil_from_days`: days-since-epoch → `(year, month, day)`.
/// The port pulls in no date crate, so this is the one place the calendar math
/// lives; every date/time formatter below is expressed on top of it.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Format epoch milliseconds as `YYYY-MM-DD` (Java `SimpleDateFormat
/// ("yyyy-MM-dd")`).
pub fn format_date(millis: i64) -> String {
    let (y, m, d) = civil_from_days(millis.div_euclid(86_400_000));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The `(day, month, year)` of an epoch-millis stamp (UTC) — for html
/// `<fstring>` date slots that want the parts separately.
pub fn date_parts(millis: i64) -> (i64, i64, i64) {
    let (y, m, d) = civil_from_days(millis.div_euclid(86_400_000));
    (d, m, y)
}

/// The whole UTC calendar split of an epoch-millis stamp:
/// `(year, month, day, hour, minute, second)`.
///
/// [`civil_from_days`] only reaches the date, so every formatter that also
/// wanted a clock used to re-derive the time-of-day — and, in several cases,
/// the calendar math with it. This is that split, said once; the layout is all
/// a caller has left to choose.
pub fn civil_from_millis(millis: i64) -> (i64, i64, i64, i64, i64, i64) {
    let secs = millis.div_euclid(1000);
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let tod = secs.rem_euclid(86_400);
    (year, month, day, tod / 3600, (tod % 3600) / 60, tod % 60)
}

/// Format epoch milliseconds as `dd/MM HH:mm:ss` — Java
/// `CastleManorManager.getNextModeChange`'s `SimpleDateFormat("dd/MM HH:mm:ss")`.
/// No year, by Java's choice: the manor's next mode change is always within a
/// day. UTC here, like the port's other formatters.
pub fn format_day_month_time(millis: i64) -> String {
    let (_, month, day, h, mi, s) = civil_from_millis(millis);
    format!("{day:02}/{month:02} {h:02}:{mi:02}:{s:02}")
}
