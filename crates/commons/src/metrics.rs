//! Counters and gauges — the third answer, next to diagnostics and audit.
//!
//! Logs answer *what happened to this player*. Audit answers *what did this
//! account do, months ago*. Neither answers *how is the server doing right
//! now*, and reaching for log lines to answer it is what creates the volume
//! problem [`crate::logging`] then has to shed. A counter costs one relaxed
//! atomic add and stays one number no matter how often it fires.
//!
//! Deliberately tiny: an atomic per metric, a name-keyed registry, and a
//! reporter that emits the whole set as one structured log event on an
//! interval. That event lands in the JSON diagnostic file like anything else,
//! so a log shipper or `jq` can graph it with no extra plumbing and no metrics
//! endpoint to secure.
//!
//! ```ignore
//! metrics::counter("packets_handled").incr();
//! metrics::gauge("players_online").set(world.clients.len() as u64);
//! ```

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

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
}

type Registry = Mutex<BTreeMap<String, Arc<AtomicU64>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn slot(name: &str) -> Arc<AtomicU64> {
    let mut map = match registry().lock() {
        Ok(m) => m,
        // A poisoned registry must not take the server down over bookkeeping.
        Err(poisoned) => poisoned.into_inner(),
    };
    map.entry(name.to_string())
        .or_insert_with(|| Arc::new(AtomicU64::new(0)))
        .clone()
}

/// Get-or-create. Hold the returned handle in a hot path rather than calling
/// this per event — the lookup takes a lock, the handle does not.
pub fn counter(name: &str) -> Counter {
    Counter(slot(name))
}

/// Get-or-create, as [`counter`].
pub fn gauge(name: &str) -> Gauge {
    Gauge(slot(name))
}

/// Every metric and its current value, sorted by name.
pub fn snapshot() -> Vec<(String, u64)> {
    let map = match registry().lock() {
        Ok(m) => m,
        Err(poisoned) => poisoned.into_inner(),
    };
    map.iter()
        .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
        .collect()
}

/// Emits the whole snapshot as one structured event every `interval_seconds`.
///
/// One event rather than one per metric, so a single log line is a complete
/// picture at that instant. `0` disables the reporter.
pub fn spawn_reporter(interval_seconds: u64) {
    if interval_seconds == 0 {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("metrics-reporter".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(interval_seconds));
                let values = snapshot();
                if values.is_empty() {
                    continue;
                }
                // Audit sink health rides along: a non-zero `blocked` is the
                // visible cost of never dropping a record.
                let json: serde_json::Map<String, serde_json::Value> = values
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect();
                tracing::info!(
                    metrics = %serde_json::Value::Object(json),
                    audit_written = crate::audit::written_count(),
                    audit_blocked = crate::audit::blocked_count(),
                    "metrics"
                );
            }
        });
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
    fn snapshot_includes_registered_names() {
        counter("test_snapshot_metric").incr();
        let names: Vec<String> = snapshot().into_iter().map(|(k, _)| k).collect();
        assert!(names.iter().any(|n| n == "test_snapshot_metric"));
    }
}
