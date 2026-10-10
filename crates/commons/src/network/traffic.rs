//! Wire-level traffic counters — `docs/MONITORING.md` §2 (P1). The raw in/out
//! of a server's client sockets, independent of what the server then makes of
//! it. Shared by the login and game servers so the two graph the same series
//! side by side (each process has its own registry, so the names don't clash).

use crate::metrics::{Counter, Gauge};

/// Every frame that arrives off the socket, including ones a rate limiter then
/// rejects; the gameserver's `packets_handled` counts what actually reached a
/// handler, so the gap between the two is itself a signal.
pub fn packets_in() -> &'static Counter {
    static C: std::sync::OnceLock<Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| crate::metrics::counter("packets_in"))
}

pub fn bytes_in() -> &'static Counter {
    static C: std::sync::OnceLock<Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| crate::metrics::counter("bytes_in"))
}

pub fn packets_out() -> &'static Counter {
    static C: std::sync::OnceLock<Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| crate::metrics::counter("packets_out"))
}

pub fn bytes_out() -> &'static Counter {
    static C: std::sync::OnceLock<Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| crate::metrics::counter("bytes_out"))
}

/// Accept-time series. `connections_accepted` is the lifetime total;
/// `connections_open` is the live count, held per connection task through
/// [`Gauge::hold`].
pub fn connections_accepted() -> &'static Counter {
    static C: std::sync::OnceLock<Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| crate::metrics::counter("connections_accepted"))
}

pub fn connections_open() -> &'static Gauge {
    static G: std::sync::OnceLock<Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| crate::metrics::gauge("connections_open"))
}
