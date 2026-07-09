//! Port of `commons/util/Rnd.java` and small helpers.

use rand::Rng;

pub mod rnd {
    use super::*;

    /// `Rnd.nextInt()` — full-range random i32.
    pub fn next_int() -> i32 {
        rand::thread_rng().gen()
    }

    /// `Rnd.get(n)` — random value in `[0, n)`.
    pub fn get(n: i32) -> i32 {
        rand::thread_rng().gen_range(0..n)
    }

    /// `Rnd.get(min, max)` — random value in `[min, max]` (inclusive, like Java).
    pub fn get_range(min: i32, max: i32) -> i32 {
        rand::thread_rng().gen_range(min..=max)
    }

    /// `Rnd.nextBytes(array)`.
    pub fn fill_bytes(buf: &mut [u8]) {
        rand::thread_rng().fill(buf);
    }
}

/// Hex-encode bytes (lowercase, no separator).
pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}