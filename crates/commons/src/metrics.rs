//! Counters and gauges — the third answer, next to diagnostics and audit.
//!
//! Logs answer *what happened to this player*. Audit answers *what did this
//! account do, months ago*. Neither answers *how is the server doing right
//! now*, and reaching for log lines to answer it is what creates the volume
//! problem [`crate::logging`] then has to shed. A counter costs one relaxed
//! atomic add and stays one number no matter how often it fires.
//!
//! Deliberately tiny: an atomic per metric and a name-keyed registry. Nothing
//! here writes anywhere — [`crate::monitor`] samples [`readings`] into its
//! ring, and the dashboard's monitoring page charts them
//! (`docs/MONITORING.md`).
//!
//! ```ignore
//! metrics::counter("packets_handled").incr();
//! metrics::gauge("players_online").set(world.clients.len() as u64);
//! ```

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// A monotonically increasing count.
#[derive(Clone)]
pub struct Counter(Arc<AtomicU64>);

impl Counter {
    pub fn incr(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// A value that can move in both directions.
#[derive(Clone)]
pub struct Gauge(Arc<AtomicU64>);

impl Gauge {
    pub fn set(&self, v: u64) {
        self.0.store(v, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Atomic `+1`, for gauges tracked as a running count (e.g. open
    /// connections) rather than set wholesale on each observation. A
    /// load-then-`set` from concurrent callers would lose updates; this does
    /// not.
    pub fn incr(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Atomic `-1`, saturating at zero so a mismatched close (or one racing
    /// ahead of its matching open) cannot wrap the counter negative.
    pub fn decr(&self) {
        let _ = self
            .0
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_sub(1))
            });
    }

    /// `+1` now, `-1` when the returned guard drops. For a count of live
    /// things owned by a task: the decrement rides the guard, so a task that
    /// panics or returns early still gives its slot back, which a `decr()`
    /// at the end of the task body would not.
    #[must_use = "the gauge is decremented when the guard drops"]
    pub fn hold(&self) -> GaugeHold {
        self.incr();
        GaugeHold(self.clone())
    }
}

/// Returned by [`Gauge::hold`]; decrements the gauge on drop.
pub struct GaugeHold(Gauge);

impl Drop for GaugeHold {
    fn drop(&mut self) {
        self.0.decr();
    }
}

/// What a series is, recorded at first registration. Consumers that work in
/// intervals rather than instants ([`crate::monitor`]) need it: a counter's
/// interval value is its delta, a gauge's is its current reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Counter,
    Gauge,
}

type Registry = Mutex<BTreeMap<String, (Kind, Arc<AtomicU64>)>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, BTreeMap<String, (Kind, Arc<AtomicU64>)>> {
    match registry().lock() {
        Ok(m) => m,
        // A poisoned registry must not take the server down over bookkeeping.
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The first registration of a name fixes its kind; a later `gauge()` for a
/// name already registered as a counter (or the reverse) shares the slot and
/// leaves the kind alone.
fn slot(name: &str, kind: Kind) -> Arc<AtomicU64> {
    lock_registry()
        .entry(name.to_string())
        .or_insert_with(|| (kind, Arc::new(AtomicU64::new(0))))
        .1
        .clone()
}

/// Get-or-create. Hold the returned handle in a hot path rather than calling
/// this per event — the lookup takes a lock, the handle does not.
pub fn counter(name: &str) -> Counter {
    Counter(slot(name, Kind::Counter))
}

/// Get-or-create, as [`counter`].
pub fn gauge(name: &str) -> Gauge {
    Gauge(slot(name, Kind::Gauge))
}

/// One series' current value, with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub name: String,
    pub kind: Kind,
    pub value: u64,
}

/// Every metric, its kind and its current value, sorted by name.
pub fn readings() -> Vec<Reading> {
    lock_registry()
        .iter()
        .map(|(name, (kind, v))| Reading {
            name: name.clone(),
            kind: *kind,
            value: v.load(Ordering::Relaxed),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate_and_share_one_slot() {
        let a = counter("test_shared_counter");
        let b = counter("test_shared_counter");
        a.incr();
        b.add(4);
        // Both handles must address the same underlying cell, or a caller that
        // re-fetches by name would silently start a second count.
        assert_eq!(a.get(), 5);
        assert_eq!(b.get(), 5);
    }

    #[test]
    fn gauges_move_in_both_directions() {
        let g = gauge("test_gauge");
        g.set(10);
        assert_eq!(g.get(), 10);
        g.set(3);
        assert_eq!(g.get(), 3);
    }

    #[test]
    fn gauge_incr_decr_track_a_running_count() {
        let g = gauge("test_gauge_running_count");
        g.set(0);
        g.incr();
        g.incr();
        assert_eq!(g.get(), 2);
        g.decr();
        assert_eq!(g.get(), 1);
    }

    #[test]
    fn gauge_hold_gives_its_slot_back_even_on_panic() {
        let g = gauge("test_gauge_hold");
        g.set(0);
        let held = g.hold();
        assert_eq!(g.get(), 1);
        drop(held);
        assert_eq!(g.get(), 0);

        let g2 = g.clone();
        let _ = std::panic::catch_unwind(move || {
            let _held = g2.hold();
            panic!("connection task blew up");
        });
        assert_eq!(g.get(), 0, "unwinding must still run the decrement");
    }

    #[test]
    fn readings_carry_the_kind_of_the_first_registration() {
        counter("test_kind_counter").incr();
        gauge("test_kind_gauge").set(7);
        // Re-fetching under the other kind shares the slot, not the kind.
        gauge("test_kind_counter");
        let all = readings();
        let find = |n: &str| all.iter().find(|r| r.name == n).cloned().unwrap();
        assert_eq!(find("test_kind_counter").kind, Kind::Counter);
        assert_eq!(find("test_kind_gauge").kind, Kind::Gauge);
        assert_eq!(find("test_kind_gauge").value, 7);
    }

    #[test]
    fn gauge_decr_saturates_at_zero_rather_than_wrapping() {
        let g = gauge("test_gauge_saturating");
        g.set(0);
        g.decr();
        assert_eq!(g.get(), 0, "a decr below zero must not wrap to u64::MAX");
    }

    #[test]
    fn readings_include_registered_names() {
        counter("test_readings_metric").incr();
        assert!(readings().iter().any(|r| r.name == "test_readings_metric"));
    }
}
