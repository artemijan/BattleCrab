//! Server monitoring — the dashboard half (`docs/MONITORING.md` §3, §5).
//!
//! Each game/login server samples itself into an in-memory ring
//! (`commons::monitor`). This module polls those rings over loopback, is the
//! sole writer of `metrics.db`, prunes past the retention window, and answers
//! the `/admin/monitor` queries. It also asks
//! the servers for their live client lists on demand (§10), which are never
//! stored, passes on the Audit page's disconnects, and carries log searches to
//! the server whose files they read (§6).

pub mod store;
pub mod wire;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use commons::logsearch::{SearchRequest, StreamInfo};
use commons::monitor::clients::ClientRecord;

use crate::config::DashboardConfig;
use store::MetricsDb;

/// The servers' sample period (`Monitor.ini`'s `SampleSeconds` default). Query
/// buckets are whole multiples of it, so a bucket never splits a sample.
pub const SAMPLE_STEP_MS: i64 = 5000;

/// How long one poll may take, connect to last byte. Loopback and at most an
/// hour of samples, so anything near this is a wedged server.
const POLL_TIMEOUT: Duration = Duration::from_secs(3);

/// A full ring is ~720 lines of well under 1 KB; this is generous headroom and
/// still a hard stop for anything misbehaving on the port.
const MAX_POLL_BYTES: u64 = 8 * 1024 * 1024;

/// A search answer carries up to `logsearch::MAX_HIT_BYTES` of matching lines,
/// which JSON escaping can grow; this leaves room for that and stops a
/// runaway answer all the same.
const MAX_LOG_BYTES: u64 = 64 * 1024 * 1024;

/// A search may take its whole deadline; this covers listing the files,
/// serializing the hits, and the trip, on top.
const LOG_TIMEOUT_SLACK: Duration = Duration::from_secs(5);

/// Polls run this long after each period boundary, so the servers — which
/// sample *on* the boundary — have already pushed the sample being fetched.
const POLL_OFFSET_MS: u64 = 1500;

const PRUNE_EVERY: Duration = Duration::from_secs(3600);

/// One `service=host:port` entry of `MonitorTargets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub service: String,
    pub address: String,
}

/// Parses `MonitorTargets`. Malformed entries are an error, not skipped: a
/// typo should fail loudly at boot rather than silently monitor one server.
pub fn parse_targets(raw: &str) -> Result<Vec<Target>, String> {
    let mut out: Vec<Target> = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (service, address) = entry
            .split_once('=')
            .map(|(s, a)| (s.trim(), a.trim()))
            .filter(|(s, a)| !s.is_empty() && a.contains(':'))
            .ok_or_else(|| format!("MonitorTargets entry {entry:?} is not service=host:port"))?;
        if out.iter().any(|t| t.service == service) {
            return Err(format!("MonitorTargets names {service:?} twice"));
        }
        out.push(Target {
            service: service.to_string(),
            address: address.to_string(),
        });
    }
    Ok(out)
}

/// What `/admin/monitor/services` reports for one target.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetStatus {
    pub service: String,
    pub address: String,
    /// The last poll succeeded. A refused connection is the normal shape of a
    /// stopped server.
    pub up: bool,
    pub last_poll_ms: Option<i64>,
    /// Newest sample stored for this service.
    pub last_sample_ts: Option<i64>,
    /// When the server's sampler started, from its newest sample.
    pub started_ms: Option<i64>,
    pub last_error: Option<String>,
}

/// How one target answered a request asked of every target at once: the
/// client list, or the log stream listing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetAnswer {
    pub service: String,
    pub up: bool,
    pub error: Option<String>,
}

/// What [`Monitor::kick_covered`] did.
#[derive(Debug, Default)]
pub struct KickSummary {
    /// The connections it closed, as they were listed just before.
    pub kicked: Vec<ClientRecord>,
    /// Targets that could not be asked, or could not kick.
    pub errors: Vec<String>,
}

pub struct Monitor {
    pub db: MetricsDb,
    targets: Vec<Target>,
    status: Mutex<BTreeMap<String, TargetStatus>>,
    pub poll_seconds: u64,
    pub retention_days: u64,
}

impl Monitor {
    pub fn new(
        db: MetricsDb,
        targets: Vec<Target>,
        poll_seconds: u64,
        retention_days: u64,
    ) -> Self {
        let status = targets
            .iter()
            .map(|t| {
                (
                    t.service.clone(),
                    TargetStatus {
                        service: t.service.clone(),
                        address: t.address.clone(),
                        ..TargetStatus::default()
                    },
                )
            })
            .collect();
        Self {
            db,
            targets,
            status: Mutex::new(status),
            poll_seconds: poll_seconds.max(1),
            retention_days,
        }
    }

    pub fn is_target(&self, service: &str) -> bool {
        self.targets.iter().any(|t| t.service == service)
    }

    fn lock_status(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, TargetStatus>> {
        match self.status.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Every target's status, in `MonitorTargets` order.
    pub fn statuses(&self) -> Vec<TargetStatus> {
        let status = self.lock_status();
        self.targets
            .iter()
            .filter_map(|t| status.get(&t.service).cloned())
            .collect()
    }

    /// Poll one target and store what it returned. Never fails the caller:
    /// the outcome lands in the target's status instead.
    pub async fn poll(&self, target: &Target) {
        let outcome = self.poll_inner(target).await;
        let mut status = self.lock_status();
        let st = status.entry(target.service.clone()).or_default();
        let was_failing = st.last_error.is_some();
        st.last_poll_ms = Some(epoch_ms());
        match outcome {
            Ok(newest) => {
                st.up = true;
                st.last_error = None;
                if let Some((ts, started)) = newest {
                    st.last_sample_ts = Some(ts);
                    st.started_ms = Some(started);
                }
            }
            Err(e) => {
                // Logged only when a target starts failing, so a stopped
                // server costs one line, not one every five seconds.
                if !was_failing {
                    tracing::warn!(
                        "monitor: {} ({}) unavailable: {e}",
                        target.service,
                        target.address
                    );
                }
                st.up = false;
                st.last_error = Some(e);
            }
        }
    }

    /// `Ok(Some((newest ts, started)))` when samples were stored.
    async fn poll_inner(&self, target: &Target) -> Result<Option<(i64, i64)>, String> {
        let since = self
            .db
            .last_ts(&target.service)
            .await
            .map_err(|e| format!("metrics db: {e}"))?
            .unwrap_or(0);
        let body = tokio::time::timeout(
            POLL_TIMEOUT,
            request(&target.address, &format!("since {since}"), MAX_POLL_BYTES),
        )
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;
        let parsed = wire::parse_body(&body);
        if let Some(e) = parsed.error {
            return Err(format!("channel refused the request: {e}"));
        }
        if parsed.malformed > 0 {
            tracing::warn!(
                "monitor: {} sent {} unparseable line(s)",
                target.service,
                parsed.malformed
            );
        }
        // A target answering as some other service is almost always two ports
        // swapped in MonitorTargets. Storing its samples under the configured
        // name would put game traffic on the login server's charts.
        if let Some(other) = parsed.samples.iter().find(|s| s.service != target.service) {
            return Err(format!(
                "answered as service {:?}, expected {:?} — check MonitorTargets",
                other.service, target.service
            ));
        }
        self.db
            .insert_samples(&parsed.samples)
            .await
            .map_err(|e| format!("metrics db: {e}"))?;
        Ok(parsed
            .samples
            .iter()
            .max_by_key(|s| s.ts)
            .map(|s| (s.ts, s.started)))
    }

    /// Asks every target at once. A target that can't answer is reported in
    /// its [`TargetAnswer`] and contributes no rows; the others still do.
    async fn ask_all<T, F, Fut>(&self, ask: F) -> (Vec<T>, Vec<TargetAnswer>)
    where
        T: Send + 'static,
        F: Fn(Target) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<T>, String>> + Send + 'static,
    {
        let mut asks = tokio::task::JoinSet::new();
        for (i, target) in self.targets.iter().cloned().enumerate() {
            let answer = ask(target);
            asks.spawn(async move { (i, answer.await) });
        }
        let mut answers = Vec::with_capacity(self.targets.len());
        while let Some(joined) = asks.join_next().await {
            if let Ok(answer) = joined {
                answers.push(answer);
            }
        }
        // MonitorTargets order, whichever answered first.
        answers.sort_by_key(|(i, _)| *i);
        let mut rows = Vec::new();
        let mut sources = Vec::with_capacity(answers.len());
        for (i, answer) in answers {
            let service = self.targets[i].service.clone();
            match answer {
                Ok(r) => {
                    rows.extend(r);
                    sources.push(TargetAnswer {
                        service,
                        up: true,
                        error: None,
                    });
                }
                Err(e) => sources.push(TargetAnswer {
                    service,
                    up: false,
                    error: Some(e),
                }),
            }
        }
        (rows, sources)
    }

    /// Every target's live client list.
    pub async fn clients(&self) -> (Vec<ClientRecord>, Vec<TargetAnswer>) {
        self.ask_all(|target| async move { client_list(&target).await })
            .await
    }

    /// Every target's log streams (§6), listed by the server that writes them.
    pub async fn log_streams(&self) -> (Vec<StreamInfo>, Vec<TargetAnswer>) {
        self.ask_all(|target| async move { log_streams(&target).await })
            .await
    }

    /// Runs `search` on `service`, over its own files. The answer is the
    /// server's `Outcome`, passed on as it came. `None` when `service` is not
    /// a target.
    pub async fn log_search(
        &self,
        service: &str,
        search: &SearchRequest,
    ) -> Option<Result<serde_json::Map<String, serde_json::Value>, String>> {
        let target = self.targets.iter().find(|t| t.service == service)?;
        Some(log_search(target, search).await)
    }

    /// Asks `service` to close connection `id`, the one that opened at
    /// `connected_ms`. `Ok(false)` when it is not open (it already left);
    /// `None` when `service` is not a target.
    pub async fn kick(
        &self,
        service: &str,
        id: u64,
        connected_ms: u64,
    ) -> Option<Result<bool, String>> {
        let target = self.targets.iter().find(|t| t.service == service)?;
        Some(kick(target, id, connected_ms).await)
    }

    /// Closes every open connection, on every target, whose address `ban`
    /// covers (`models::repo::ip_bans::covering`). Asks for a fresh list
    /// first, so it only kicks connections that exist now.
    pub async fn kick_covered(&self, ban: &str) -> KickSummary {
        let (clients, sources) = self.clients().await;
        let mut summary = KickSummary {
            errors: sources
                .into_iter()
                .filter_map(|s| s.error.map(|e| format!("{}: {e}", s.service)))
                .collect(),
            ..KickSummary::default()
        };
        for c in clients.iter().filter(|c| {
            models::repo::ip_bans::covering(&c.ip)
                .iter()
                .any(|b| b == ban)
        }) {
            match self.kick(&c.service, c.id, c.connected_ms).await {
                Some(Ok(true)) => summary.kicked.push(c.clone()),
                Some(Ok(false)) | None => {}
                Some(Err(e)) => summary.errors.push(format!("{}: {e}", c.service)),
            }
        }
        summary
    }

    pub async fn prune(&self) {
        let cutoff = epoch_ms() - self.retention_days as i64 * 86_400_000;
        match self.db.prune(cutoff).await {
            Ok(n) if n > 0 => tracing::info!("monitor: pruned {n} sample(s) past retention"),
            Ok(_) => {}
            Err(e) => tracing::warn!("monitor: prune failed: {e}"),
        }
    }

    /// The poll loop. Runs until the process exits.
    pub async fn run(self: Arc<Self>) {
        let period_ms = self.poll_seconds * 1000;
        let mut last_prune: Option<Instant> = None;
        loop {
            let now = epoch_ms() as u64;
            let wait = (period_ms - now % period_ms + POLL_OFFSET_MS) % period_ms;
            tokio::time::sleep(Duration::from_millis(wait.max(1))).await;

            for target in &self.targets {
                self.poll(target).await;
            }
            if last_prune.is_none_or(|t| t.elapsed() >= PRUNE_EVERY) {
                self.prune().await;
                last_prune = Some(Instant::now());
            }
        }
    }
}

/// One request on the monitor channel: a line in, at most `max_bytes` of
/// body out.
async fn request(address: &str, line: &str, max_bytes: u64) -> std::io::Result<String> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream.write_all(format!("{line}\n").as_bytes()).await?;
    let mut buf = Vec::new();
    stream.take(max_bytes).read_to_end(&mut buf).await?;
    String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// One target's `clients` answer. A target answering as another service is
/// refused for the same reason [`Monitor::poll`] refuses its samples.
async fn client_list(target: &Target) -> Result<Vec<ClientRecord>, String> {
    let body = tokio::time::timeout(
        POLL_TIMEOUT,
        request(&target.address, "clients", MAX_POLL_BYTES),
    )
    .await
    .map_err(|_| "timed out".to_string())?
    .map_err(|e| e.to_string())?;
    let parsed = wire::parse_clients(&body);
    if let Some(e) = parsed.error {
        return Err(format!("channel refused the request: {e}"));
    }
    if parsed.malformed > 0 {
        tracing::warn!(
            "monitor: {} sent {} unparseable client line(s)",
            target.service,
            parsed.malformed
        );
    }
    if let Some(other) = parsed.clients.iter().find(|c| c.service != target.service) {
        return Err(format!(
            "answered as service {:?}, expected {:?} — check MonitorTargets",
            other.service, target.service
        ));
    }
    Ok(parsed.clients)
}

async fn kick(target: &Target, id: u64, connected_ms: u64) -> Result<bool, String> {
    let body = tokio::time::timeout(
        POLL_TIMEOUT,
        request(
            &target.address,
            &format!("kick {id} {connected_ms}"),
            MAX_POLL_BYTES,
        ),
    )
    .await
    .map_err(|_| "timed out".to_string())?
    .map_err(|e| e.to_string())?;
    wire::parse_kick(&body)
}

/// One target's `logs streams` answer, refused if it names another service.
async fn log_streams(target: &Target) -> Result<Vec<StreamInfo>, String> {
    let body = tokio::time::timeout(
        POLL_TIMEOUT,
        request(&target.address, "logs streams", MAX_POLL_BYTES),
    )
    .await
    .map_err(|_| "timed out".to_string())?
    .map_err(|e| e.to_string())?;
    let streams = wire::parse_streams(&body)?;
    if let Some(other) = streams.iter().find(|s| s.service != target.service) {
        return Err(format!(
            "answered as service {:?}, expected {:?} — check MonitorTargets",
            other.service, target.service
        ));
    }
    Ok(streams)
}

async fn log_search(
    target: &Target,
    search: &SearchRequest,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let json = serde_json::to_string(search).map_err(|e| e.to_string())?;
    let deadline = Duration::from_millis(search.timeout_ms) + LOG_TIMEOUT_SLACK;
    let body = tokio::time::timeout(
        deadline,
        request(
            &target.address,
            &format!("logs search {json}"),
            MAX_LOG_BYTES,
        ),
    )
    .await
    .map_err(|_| "timed out".to_string())?
    .map_err(|e| e.to_string())?;
    let outcome = wire::parse_search(&body)?;
    match outcome.get("service").and_then(|s| s.as_str()) {
        Some(s) if s == target.service => Ok(outcome),
        other => Err(format!(
            "answered as service {:?}, expected {:?} — check MonitorTargets",
            other.unwrap_or("unnamed"),
            target.service
        )),
    }
}

pub fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The bucket width for a `range_ms` query returning at most `max_points`.
///
/// Buckets are epoch-aligned, so a range generally touches one more bucket
/// than `range / width` — a width of at least `range / (max_points - 1)` is
/// what keeps the count at or under `max_points`. Rounded up to a whole
/// number of sample periods, so no bucket ever splits a sample.
pub fn bucket_ms(range_ms: i64, max_points: i64) -> i64 {
    let gaps = max_points.max(2) - 1;
    let raw = (range_ms.max(1) + gaps - 1) / gaps;
    // Round up to a whole number of periods (`i64::div_ceil` is unstable).
    ((raw + SAMPLE_STEP_MS - 1) / SAMPLE_STEP_MS).max(1) * SAMPLE_STEP_MS
}

/// Open `metrics.db` and start the poll loop, unless monitoring is disabled
/// (`MonitorTargets` empty) or cannot start. Never fatal: the dashboard's
/// account features must not go down over a monitoring problem.
pub async fn start(config: &DashboardConfig) -> Option<Arc<Monitor>> {
    let targets = match parse_targets(&config.monitor_targets) {
        Ok(t) if t.is_empty() => {
            tracing::info!("monitor: disabled (MonitorTargets is empty)");
            return None;
        }
        Ok(t) => t,
        Err(e) => {
            tracing::error!("monitor: disabled — {e}");
            return None;
        }
    };
    // Resolved like the game database's URL — against the executable's
    // directory — and logged absolute, so "where did the graphs go" has an
    // answer in the journal (§7).
    let path = PathBuf::from(&config.metrics_database);
    let path = if path.is_absolute() {
        path
    } else {
        commons::db::executable_dir().join(path)
    };
    let db = match MetricsDb::open(&path).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("monitor: disabled — cannot open {}: {e}", path.display());
            return None;
        }
    };
    tracing::info!(
        "monitor: storing samples in {} ({} day retention), polling {}",
        path.display(),
        config.metrics_retention_days,
        targets
            .iter()
            .map(|t| format!("{}@{}", t.service, t.address))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let monitor = Arc::new(Monitor::new(
        db,
        targets,
        config.metrics_poll_seconds,
        config.metrics_retention_days,
    ));
    tokio::spawn(monitor.clone().run());
    Some(monitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_parse_and_typos_fail_loudly() {
        assert_eq!(
            parse_targets(" game_server=127.0.0.1:7779 , login_server=127.0.0.1:7780 ").unwrap(),
            vec![
                Target {
                    service: "game_server".into(),
                    address: "127.0.0.1:7779".into()
                },
                Target {
                    service: "login_server".into(),
                    address: "127.0.0.1:7780".into()
                },
            ]
        );
        assert!(parse_targets("").unwrap().is_empty());
        assert!(parse_targets("game_server").is_err());
        assert!(parse_targets("=127.0.0.1:7779").is_err());
        assert!(parse_targets("game_server=7779").is_err());
        assert!(parse_targets("a=h:1,a=h:2").is_err());
    }

    #[test]
    fn a_week_at_500_points_stays_within_500_buckets_wherever_it_starts() {
        let week = 7 * 86_400_000;
        let b = bucket_ms(week, 500);
        assert_eq!(b % SAMPLE_STEP_MS, 0);
        // The off-by-one this guards: an epoch-aligned range touches
        // floor(to/b) - floor(from/b) + 1 buckets, whatever its offset.
        for from in [0, 1, 2_500, b - 1, 1_759_000_003_217] {
            let to = from + week;
            let touched = (to - 1).div_euclid(b) - from.div_euclid(b) + 1;
            assert!(touched <= 500, "from {from}: {touched} buckets of {b} ms");
        }
    }

    #[test]
    fn short_ranges_bottom_out_at_one_sample_per_bucket() {
        assert_eq!(bucket_ms(60_000, 500), SAMPLE_STEP_MS);
        assert_eq!(bucket_ms(0, 500), SAMPLE_STEP_MS);
        assert_eq!(bucket_ms(3_600_000, 2), 3_600_000, "two points: one width");
        assert_eq!(
            bucket_ms(3_600_001, 2),
            3_605_000,
            "rounded up to whole periods"
        );
    }
}
