//! `/admin/monitor` — per-server metrics (`docs/MONITORING.md` §5). Admin-only,
//! like the rest of `/admin`: every handler starts with `require_admin`.
//!
//! All three answer 503 when monitoring is off (`MonitorTargets` empty, or
//! `metrics.db` would not open), so the SPA can tell "disabled" from "broken".

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::monitor::store::{self, Buckets, Column};
use crate::monitor::{self, Monitor, TargetStatus};
use crate::routes::require_admin;
use crate::state::AppState;

/// Default window when `from` is omitted.
const DEFAULT_RANGE_MS: i64 = 3_600_000;
/// Default and hard cap on returned points. 7 days at 5 s is 120 960 raw
/// samples; serving those to a browser is what server-side bucketing avoids.
const DEFAULT_MAX_POINTS: i64 = 500;
const MAX_MAX_POINTS: i64 = 2000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/services", axum::routing::get(services))
        .route("/series", axum::routing::get(series))
        .route("/host", axum::routing::get(host))
}

async fn monitor(app: &AppState, headers: &HeaderMap) -> ApiResult<Arc<Monitor>> {
    require_admin(app, headers).await?;
    app.monitor
        .clone()
        .ok_or(ApiError::Unavailable("server monitoring is disabled"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServicesResponse {
    services: Vec<ServiceView>,
    poll_seconds: u64,
    retention_days: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceView {
    #[serde(flatten)]
    status: TargetStatus,
    /// From the newest sample's `started`; absent until one arrives.
    uptime_seconds: Option<i64>,
}

async fn services(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ServicesResponse>> {
    let m = monitor(&app, &headers).await?;
    let now = monitor::epoch_ms();
    let services = m
        .statuses()
        .into_iter()
        .map(|status| ServiceView {
            // Only while up: a stopped server has no uptime, whatever its last
            // sample said.
            uptime_seconds: status
                .started_ms
                .filter(|_| status.up)
                .map(|s| (now - s).max(0) / 1000),
            status,
        })
        .collect();
    Ok(Json(ServicesResponse {
        services,
        poll_seconds: m.poll_seconds,
        retention_days: m.retention_days,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeQuery {
    pub service: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// Comma-separated metric names; omitted means every column.
    pub metrics: Option<String>,
    pub max_points: Option<i64>,
}

/// The validated window and bucket width of a query.
#[derive(Debug, PartialEq)]
struct Window {
    from: i64,
    to: i64,
    bucket_ms: i64,
}

fn window(q: &RangeQuery, now: i64, retention_days: u64) -> ApiResult<Window> {
    let to = q.to.unwrap_or(now);
    let from = q.from.unwrap_or(to - DEFAULT_RANGE_MS);
    if from >= to {
        return Err(ApiError::BadRequest("`from` must be before `to`".into()));
    }
    // A day past retention, so "the whole retained week" is always a valid
    // request whatever the clock skew; anything beyond is a client bug.
    let max_range = (retention_days as i64 + 1) * 86_400_000;
    if to - from > max_range {
        return Err(ApiError::BadRequest(format!(
            "range is limited to {} days",
            retention_days + 1
        )));
    }
    let max_points = q.max_points.unwrap_or(DEFAULT_MAX_POINTS);
    if !(2..=MAX_MAX_POINTS).contains(&max_points) {
        return Err(ApiError::BadRequest(format!(
            "`maxPoints` must be between 2 and {MAX_MAX_POINTS}"
        )));
    }
    Ok(Window {
        from,
        to,
        bucket_ms: monitor::bucket_ms(to - from, max_points),
    })
}

/// Resolve `metrics=` against the column whitelist. An unknown name is a 400
/// that lists the valid ones, rather than a silently missing series.
fn metrics(raw: Option<&str>) -> ApiResult<Vec<&'static Column>> {
    let Some(raw) = raw.filter(|r| !r.trim().is_empty()) else {
        return Ok(store::COLUMNS.iter().collect());
    };
    raw.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|name| {
            store::column(name).ok_or_else(|| {
                let valid: Vec<&str> = store::COLUMNS.iter().map(|c| c.name).collect();
                ApiError::BadRequest(format!(
                    "unknown metric {name:?}; valid: {}",
                    valid.join(", ")
                ))
            })
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SeriesResponse {
    service: Option<String>,
    from: i64,
    to: i64,
    bucket_ms: i64,
    /// How each series folds into a bucket: `sum` (deltas — divide by
    /// `intervalMs` for a rate) or `max` (gauges — the bucket's peak).
    aggregation: std::collections::BTreeMap<&'static str, &'static str>,
    #[serde(flatten)]
    buckets: Buckets,
}

async fn series(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RangeQuery>,
) -> ApiResult<Json<SeriesResponse>> {
    let m = monitor(&app, &headers).await?;
    let service = q
        .service
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("`service` is required".into()))?;
    if !m.is_target(service) {
        return Err(ApiError::NotFound);
    }
    let w = window(&q, monitor::epoch_ms(), m.retention_days)?;
    let cols = metrics(q.metrics.as_deref())?;
    let buckets =
        m.db.series(service, w.from, w.to, w.bucket_ms, &cols)
            .await?;
    Ok(Json(SeriesResponse {
        service: Some(service.to_string()),
        from: w.from,
        to: w.to,
        bucket_ms: w.bucket_ms,
        aggregation: cols
            .iter()
            .map(|c| {
                (
                    c.name,
                    match c.agg {
                        store::Agg::Sum => "sum",
                        store::Agg::Max => "max",
                    },
                )
            })
            .collect(),
        buckets,
    }))
}

async fn host(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RangeQuery>,
) -> ApiResult<Json<SeriesResponse>> {
    let m = monitor(&app, &headers).await?;
    let w = window(&q, monitor::epoch_ms(), m.retention_days)?;
    let buckets = m.db.host(w.from, w.to, w.bucket_ms).await?;
    Ok(Json(SeriesResponse {
        service: None,
        from: w.from,
        to: w.to,
        bucket_ms: w.bucket_ms,
        aggregation: store::HOST_COLUMNS.iter().copied().collect(),
        buckets,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(from: Option<i64>, to: Option<i64>, max_points: Option<i64>) -> RangeQuery {
        RangeQuery {
            service: None,
            from,
            to,
            metrics: None,
            max_points,
        }
    }

    #[test]
    fn window_defaults_to_the_last_hour() {
        let w = window(&q(None, None, None), 10_000_000, 7).unwrap();
        assert_eq!((w.from, w.to), (10_000_000 - 3_600_000, 10_000_000));
        assert_eq!(w.bucket_ms % monitor::SAMPLE_STEP_MS, 0);
    }

    #[test]
    fn window_rejects_inverted_oversized_and_silly_requests() {
        assert!(window(&q(Some(5), Some(5), None), 0, 7).is_err());
        assert!(window(&q(Some(0), Some(9 * 86_400_000), None), 0, 7).is_err());
        assert!(window(&q(Some(0), Some(8 * 86_400_000), None), 0, 7).is_ok());
        assert!(window(&q(Some(0), Some(1000), Some(1)), 0, 7).is_err());
        assert!(window(&q(Some(0), Some(1000), Some(MAX_MAX_POINTS + 1)), 0, 7).is_err());
    }

    #[test]
    fn metrics_are_whitelisted() {
        assert_eq!(metrics(None).unwrap().len(), store::COLUMNS.len());
        let picked = metrics(Some("packets_in, rss_bytes")).unwrap();
        assert_eq!(
            picked.iter().map(|c| c.name).collect::<Vec<_>>(),
            vec!["packets_in", "rss_bytes"]
        );
        // The whitelist is what dynamic SQL is built from: anything else,
        // including an injection attempt, never reaches it.
        assert!(metrics(Some("packets_in) FROM accounts --")).is_err());
        assert!(metrics(Some("extra")).is_err());
    }
}
