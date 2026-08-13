//! Shared time-unit constants for the 100 ms tick world — previously
//! re-declared file-locally by two dozen modules.

/// Game-loop ticks per second (the loop runs at [`super::TICK`], 100 ms).
pub(crate) const TICKS_PER_SECOND: u64 = 10;
pub(crate) const MILLIS_PER_MINUTE: i64 = 60_000;
pub(crate) const MILLIS_PER_HOUR: i64 = 3_600_000;
pub(crate) const MILLIS_PER_DAY: i64 = 86_400_000;
