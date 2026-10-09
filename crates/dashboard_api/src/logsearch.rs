//! The dashboard's side of log search (`docs/MONITORING.md` §6).
//!
//! The scanner is `commons::logsearch`, and it runs where the files are: the
//! game and login servers search their own logs when asked over their monitor
//! channel (`crate::monitor::Monitor::log_search`), so they can sit on other
//! machines. The dashboard searches only its own logs here, in-process.

use std::sync::Arc;
use std::time::Duration;

use commons::logsearch::{Bounds, Source};

use crate::config::DashboardConfig;

/// Where the dashboard writes its own audit records. Shared with `main`,
/// which sets it, so search finds them where they are.
pub const DASHBOARD_AUDIT_DIR: &str = "log/audit-dashboard";

/// The service name the dashboard's own logs are listed under.
pub const DASHBOARD_SERVICE: &str = "dashboard_api";

/// Everything a search request needs that outlives it.
pub struct LogSearch {
    /// The dashboard's own files.
    pub local: Source,
    /// Every search's budget, local or sent to a server.
    pub bounds: Bounds,
    /// One permit per running search, local or remote. A remote search holds
    /// one of the server's blocking threads and reads its disk; this keeps
    /// the dashboard from asking for more than `LogSearchConcurrency` at once.
    pub permits: Arc<tokio::sync::Semaphore>,
}

impl LogSearch {
    pub fn new(local: Source, bounds: Bounds, concurrency: usize) -> Self {
        Self {
            local,
            bounds,
            permits: Arc::new(tokio::sync::Semaphore::new(concurrency.max(1))),
        }
    }

    /// `None` when `LogSearchEnabled` is off; `/admin/logs` then answers 503.
    /// The dashboard's own files are at the root `main` hands
    /// `commons::logging::init`.
    pub fn from_config(config: &DashboardConfig) -> Option<Arc<Self>> {
        if !config.log_search_enabled {
            tracing::info!("log search: disabled (LogSearchEnabled = False)");
            return None;
        }
        Some(Arc::new(Self::new(
            Source::resolve(DASHBOARD_SERVICE, "dist/game/", DASHBOARD_AUDIT_DIR),
            Bounds {
                max_bytes: config.log_search_max_bytes,
                deadline: Duration::from_millis(config.log_search_timeout_ms),
            },
            config.log_search_concurrency,
        )))
    }
}
