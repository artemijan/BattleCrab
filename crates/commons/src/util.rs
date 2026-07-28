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

    /// `Rnd.nextInt()` — full-range random i32.
    pub fn next_int() -> i32 {
        random()
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
