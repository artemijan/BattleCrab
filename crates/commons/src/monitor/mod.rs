//! Per-server sampler — `docs/MONITORING.md` §4 (P2).
//!
//! Every `SampleSeconds` a plain thread reads the [`crate::metrics`] registry
//! and the process's CPU and memory, turns them into one interval [`Sample`],
//! and pushes it into a bounded in-memory [`Ring`]. A loopback channel
//! ([`channel`]) hands the ring to whoever asks; the dashboard is the intended
//! asker and the only writer of long-term storage (§3).
//!
//! Sampling in-process is what makes the numbers honest: the interval is
//! measured where the counters live, CPU deltas come from consecutive readings
//! of the same process, and a restart needs no handling anywhere — the
//! sampler's previous reading dies with the process, so the first sample after
//! a restart is simply counts since start.

mod channel;
pub mod process;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::config::PropertiesParser;
use crate::metrics::{Kind, Reading};

/// Datapack-relative location of the monitor config.
pub const MONITOR_CONFIG_FILE: &str = "config/Monitor.ini";

/// Everything `Monitor.ini` controls.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// `InternalMonitorBindAddress` — loopback by default, and that is the
    /// security control (see [`channel`]).
    pub bind_address: String,
    /// `InternalMonitorPort` — `0` disables the channel *and* the sampler:
    /// with nothing able to read the ring there is no point filling it.
    pub port: u16,
    /// `SampleSeconds` — the granularity every stored series has.
    pub sample_seconds: u64,
    /// `RingSamples` — how much history survives a poller being away.
    pub ring_samples: usize,
}

impl MonitorConfig {
    /// Reads `{root}config/Monitor.ini`. `default_port` differs per server
    /// (both may run on one host), so the caller supplies it.
    pub fn load(root: &str, default_port: u16) -> Self {
        Self::from_parser(
            &PropertiesParser::load_rel(root, MONITOR_CONFIG_FILE),
            default_port,
        )
    }

    /// Parses an in-memory ini body — for tests.
    pub fn from_content(content: &str, default_port: u16) -> Self {
        Self::from_parser(
            &PropertiesParser::from_content(MONITOR_CONFIG_FILE, content),
            default_port,
        )
    }

    fn from_parser(p: &PropertiesParser, default_port: u16) -> Self {
        Self {
            bind_address: p.get_string("InternalMonitorBindAddress", "127.0.0.1"),
            port: p
                .get_int("InternalMonitorPort", default_port as i32)
                .clamp(0, 65535) as u16,
            sample_seconds: p.get_int("SampleSeconds", 5).max(1) as u64,
            // 720 × 5 s = one hour: enough to cover a dashboard deploy.
            ring_samples: p.get_int("RingSamples", 720).max(1) as usize,
        }
    }
}

/// Reads allocator-level heap usage, for a server whose allocator can say
/// (the game server's mimalloc). `None` where it can't.
pub type HeapProbe = fn() -> Option<u64>;

/// One sampling interval.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Wall-clock epoch ms, floor-aligned to the sample period, so both
    /// servers' samples for the same interval share a timestamp.
    pub ts_ms: u64,
    /// When this process's sampler started, epoch ms. A poller reads uptime
    /// from it, and a change between two samples means the server restarted.
    pub started_ms: u64,
    /// Real elapsed time since the previous sample. Normally the period; more
    /// after a stall, which is what keeps a rate computed from it honest.
    pub interval_ms: u64,
    /// CPU time consumed during the interval, all threads.
    pub cpu_micros: u64,
    /// Resident set size at sample time. `None` off Linux.
    pub rss_bytes: Option<u64>,
    /// Allocator-reported heap, where a [`HeapProbe`] was given.
    pub heap_bytes: Option<u64>,
    /// Every registry series: counters as their delta over the interval,
    /// gauges as their reading at sample time.
    pub metrics: BTreeMap<String, u64>,
}

impl Sample {
    /// One NDJSON line, newline-terminated. Registry series sit under
    /// `metrics` rather than at the top level, so a counter can never collide
    /// with a fixed field's name.
    pub fn to_json_line(&self, service: &str) -> String {
        let body = serde_json::json!({
            "service": service,
            "ts": self.ts_ms,
            "started": self.started_ms,
            "interval_ms": self.interval_ms,
            "cpu_micros": self.cpu_micros,
            "rss_bytes": self.rss_bytes,
            "heap_bytes": self.heap_bytes,
            "metrics": self.metrics,
        });
        format!("{body}\n")
    }
}

/// The bounded sample history. Oldest samples fall off the front.
pub struct Ring {
    capacity: usize,
    samples: Mutex<VecDeque<Sample>>,
}

impl Ring {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            samples: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Sample>> {
        match self.samples.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn push(&self, sample: Sample) {
        let mut samples = self.lock();
        if samples.len() == self.capacity {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    /// Samples stamped strictly after `since_ms`, oldest first. Strictly, so
    /// a poller passing back the last `ts` it stored never sees it twice.
    pub fn since(&self, since_ms: u64) -> Vec<Sample> {
        self.lock()
            .iter()
            .filter(|s| s.ts_ms > since_ms)
            .cloned()
            .collect()
    }

    fn last_ts(&self) -> Option<u64> {
        self.lock().back().map(|s| s.ts_ms)
    }
}

/// Turns raw readings into interval values: a counter becomes its delta
/// against `prev` (and `prev` advances), a gauge passes through as-is.
///
/// A counter missing from `prev` was registered after the last sample and
/// counts from zero. One *below* `prev` cannot happen within a process (the
/// counters only grow); it saturates to `0` rather than wrapping.
fn interval_values(
    prev: &mut BTreeMap<String, u64>,
    readings: Vec<Reading>,
) -> BTreeMap<String, u64> {
    readings
        .into_iter()
        .map(|r| {
            let value = match r.kind {
                Kind::Gauge => r.value,
                Kind::Counter => {
                    let before = prev.insert(r.name.clone(), r.value).unwrap_or(0);
                    r.value.saturating_sub(before)
                }
            };
            (r.name, value)
        })
        .collect()
}

fn align_down(ms: u64, period_ms: u64) -> u64 {
    ms - ms % period_ms
}

/// How long to sleep from `now_ms` to the next period boundary. Sleeping to a
/// boundary rather than for a fixed period is what keeps both servers in the
/// same buckets and stops the phase drifting as each sample's work adds up.
fn until_next_boundary(now_ms: u64, period_ms: u64) -> u64 {
    period_ms - now_ms % period_ms
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The sampler's state between ticks.
struct Sampler {
    period_ms: u64,
    started_ms: u64,
    heap: Option<HeapProbe>,
    prev_counters: BTreeMap<String, u64>,
    prev_cpu: u64,
    prev_at: Instant,
}

impl Sampler {
    /// The baseline is taken here, so the first sample covers sampler start
    /// to first boundary and reports a true rate rather than a boot-time lump.
    fn new(period_ms: u64, heap: Option<HeapProbe>) -> Self {
        let mut prev_counters = BTreeMap::new();
        interval_values(&mut prev_counters, crate::metrics::readings());
        Self {
            period_ms,
            started_ms: epoch_ms(),
            heap,
            prev_counters,
            prev_cpu: process::cpu_micros(),
            prev_at: Instant::now(),
        }
    }

    fn take(&mut self, ts_ms: u64) -> Sample {
        let now = Instant::now();
        let cpu = process::cpu_micros();
        let sample = Sample {
            ts_ms,
            started_ms: self.started_ms,
            interval_ms: now.duration_since(self.prev_at).as_millis() as u64,
            cpu_micros: cpu.saturating_sub(self.prev_cpu),
            rss_bytes: process::rss_bytes(),
            heap_bytes: self.heap.and_then(|probe| probe()),
            metrics: interval_values(&mut self.prev_counters, crate::metrics::readings()),
        };
        self.prev_cpu = cpu;
        self.prev_at = now;
        sample
    }

    /// Runs forever on its own thread — a plain `std::thread`, like
    /// [`crate::metrics::spawn_reporter`], so a busy tokio runtime cannot
    /// delay a sample.
    fn run(mut self, ring: Arc<Ring>) {
        loop {
            std::thread::sleep(Duration::from_millis(until_next_boundary(
                epoch_ms(),
                self.period_ms,
            )));
            let ts = align_down(epoch_ms(), self.period_ms);
            // A wall clock stepped backwards (NTP) would repeat a bucket the
            // ring already holds, and P3 keys storage on (service, ts). Skip
            // it without advancing the baseline: the next sample then covers
            // both intervals, and its `interval_ms` says so.
            if ring.last_ts().is_some_and(|last| ts <= last) {
                continue;
            }
            ring.push(self.take(ts));
        }
    }
}

/// Start the sampler and its loopback channel, if `InternalMonitorPort` is
/// non-zero. `service` is the same name logging uses (`game_server`,
/// `login_server`) and is stamped on every line served.
///
/// A bind failure is logged and swallowed, as for the login server's status
/// channel: a server must not fail to boot over a monitoring port.
pub async fn spawn(service: &'static str, cfg: &MonitorConfig, heap: Option<HeapProbe>) {
    if cfg.port == 0 {
        info!("Monitor channel: disabled (InternalMonitorPort = 0).");
        return;
    }
    let bind = format!("{}:{}", cfg.bind_address, cfg.port);
    let listener = match TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            warn!("Monitor channel: could not bind {bind}: {e} — monitoring unavailable.");
            return;
        }
    };
    let ring = Arc::new(Ring::new(cfg.ring_samples));
    let sampler = Sampler::new(cfg.sample_seconds * 1000, heap);
    let sampler_ring = ring.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("monitor-sampler".to_string())
        .spawn(move || sampler.run(sampler_ring))
    {
        warn!("Monitor channel: could not start the sampler thread: {e} — monitoring unavailable.");
        return;
    }
    info!(
        "Monitor channel: listening on {bind} ({}s samples, {} kept).",
        cfg.sample_seconds, cfg.ring_samples
    );
    tokio::spawn(channel::accept_loop(listener, service, ring));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, kind: Kind, value: u64) -> Reading {
        Reading {
            name: name.to_string(),
            kind,
            value,
        }
    }

    fn sample(ts_ms: u64) -> Sample {
        Sample {
            ts_ms,
            started_ms: 1,
            interval_ms: 5000,
            cpu_micros: 0,
            rss_bytes: None,
            heap_bytes: None,
            metrics: BTreeMap::new(),
        }
    }

    #[test]
    fn counters_become_deltas_and_gauges_pass_through() {
        let mut prev = BTreeMap::new();
        let first = interval_values(
            &mut prev,
            vec![
                reading("packets_in", Kind::Counter, 100),
                reading("connections_open", Kind::Gauge, 3),
            ],
        );
        assert_eq!(first["packets_in"], 100, "no baseline: counts from zero");
        assert_eq!(first["connections_open"], 3);

        let second = interval_values(
            &mut prev,
            vec![
                reading("packets_in", Kind::Counter, 130),
                reading("connections_open", Kind::Gauge, 2),
            ],
        );
        assert_eq!(second["packets_in"], 30);
        assert_eq!(second["connections_open"], 2, "a gauge is never diffed");
    }

    #[test]
    fn a_restart_starts_counting_from_zero_without_any_special_case() {
        // The old process's `prev` died with it; the new one starts empty and
        // its counters start at zero, so the first delta is "since start".
        let mut before_restart = BTreeMap::new();
        interval_values(
            &mut before_restart,
            vec![reading("bytes_out", Kind::Counter, 9_000)],
        );

        let mut after_restart = BTreeMap::new();
        let first = interval_values(
            &mut after_restart,
            vec![reading("bytes_out", Kind::Counter, 40)],
        );
        assert_eq!(first["bytes_out"], 40);
    }

    #[test]
    fn a_counter_below_its_baseline_saturates_instead_of_wrapping() {
        let mut prev = BTreeMap::from([("ticks".to_string(), 50)]);
        let v = interval_values(&mut prev, vec![reading("ticks", Kind::Counter, 10)]);
        assert_eq!(v["ticks"], 0);
    }

    #[test]
    fn ring_evicts_the_oldest_and_since_is_strict() {
        let ring = Ring::new(3);
        for ts in [5, 10, 15, 20] {
            ring.push(sample(ts));
        }
        let all: Vec<u64> = ring.since(0).iter().map(|s| s.ts_ms).collect();
        assert_eq!(
            all,
            vec![10, 15, 20],
            "capacity 3: the first sample fell off"
        );
        let newer: Vec<u64> = ring.since(15).iter().map(|s| s.ts_ms).collect();
        assert_eq!(newer, vec![20], "the sample *at* since is not repeated");
        assert!(ring.since(20).is_empty());
    }

    #[test]
    fn boundaries_align_both_servers_to_the_same_buckets() {
        assert_eq!(align_down(1_759_000_003_217, 5000), 1_759_000_000_000);
        assert_eq!(align_down(1_759_000_005_000, 5000), 1_759_000_005_000);
        assert_eq!(until_next_boundary(1_759_000_003_217, 5000), 1783);
        // Exactly on a boundary: wait a full period, never zero (which would
        // spin and sample the same bucket twice).
        assert_eq!(until_next_boundary(1_759_000_005_000, 5000), 5000);
    }

    #[test]
    fn json_line_nests_registry_series_under_metrics() {
        let mut s = sample(1_759_000_005_000);
        s.rss_bytes = Some(64 << 20);
        s.metrics.insert("packets_in".into(), 1204);
        let line = s.to_json_line("game_server");
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["service"], "game_server");
        assert_eq!(v["ts"], 1_759_000_005_000u64);
        assert_eq!(v["started"], 1);
        assert_eq!(v["rss_bytes"], 64u64 << 20);
        assert!(v["heap_bytes"].is_null());
        assert_eq!(v["metrics"]["packets_in"], 1204);
    }

    #[test]
    fn config_defaults_and_overrides() {
        let d = MonitorConfig::from_content("", 7779);
        assert_eq!(d.port, 7779);
        assert_eq!(d.bind_address, "127.0.0.1");
        assert_eq!(d.sample_seconds, 5);
        assert_eq!(d.ring_samples, 720);

        let c = MonitorConfig::from_content(
            "InternalMonitorPort = 0\nSampleSeconds = 0\nRingSamples = 10\n",
            7779,
        );
        assert_eq!(c.port, 0);
        assert_eq!(c.sample_seconds, 1, "a zero period would spin");
        assert_eq!(c.ring_samples, 10);
    }

    #[test]
    fn sampler_reports_cpu_and_registry_deltas_for_its_interval() {
        let c = crate::metrics::counter("test_monitor_sampler_counter");
        let mut sampler = Sampler::new(5000, Some(|| Some(123)));
        c.add(7);
        let s = sampler.take(5000);
        assert_eq!(s.metrics["test_monitor_sampler_counter"], 7);
        assert_eq!(s.heap_bytes, Some(123));
        let s2 = sampler.take(10000);
        assert_eq!(s2.metrics["test_monitor_sampler_counter"], 0);
    }
}
