//! `/admin/logs` — log search (`docs/MONITORING.md` §6). Admin-only, like the
//! rest of `/admin`: every handler starts with `require_admin`.
//!
//! Each service's files are searched where they are: the dashboard's own
//! in-process, the game and login servers' by the server itself, asked over
//! its monitor channel. So a request validates here, for a clear 400, and
//! again on the server, which does not take the dashboard's word for it.
//!
//! **Audit logs hold player chat and IP addresses**, so every search writes a
//! `gmaudit` record naming the admin and what they searched for — reading
//! someone's chat is exactly the kind of action that needs attribution
//! (DASHBOARD.md §16.5).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use commons::logsearch::{self, SearchRequest, StreamInfo};

use crate::error::{ApiError, ApiResult};
use crate::logsearch::{DASHBOARD_SERVICE, LogSearch};
use crate::monitor::TargetAnswer;
use crate::routes::require_admin;
use crate::state::AppState;

const DEFAULT_RANGE_MS: i64 = 86_400_000;
const DEFAULT_LIMIT: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/streams", axum::routing::get(streams))
        .route("/search", axum::routing::get(search))
}

async fn log_search(
    app: &AppState,
    headers: &HeaderMap,
) -> ApiResult<(crate::db::accounts::Account, Arc<LogSearch>)> {
    let actor = require_admin(app, headers).await?;
    let ls = app
        .log_search
        .clone()
        .ok_or(ApiError::Unavailable("log search is disabled"))?;
    Ok((actor, ls))
}

#[derive(Serialize)]
struct StreamsResponse {
    streams: Vec<StreamInfo>,
    /// How each server answered. One that is down lists no streams; this
    /// says why, rather than leaving it to look like a server with no logs.
    sources: Vec<TargetAnswer>,
}

async fn streams(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<StreamsResponse>> {
    let (_, ls) = log_search(&app, &headers).await?;
    let (mut streams, sources) = match &app.monitor {
        Some(m) => m.log_streams().await,
        None => (Vec::new(), Vec::new()),
    };
    let local = tokio::task::spawn_blocking(move || ls.local.streams())
        .await
        .map_err(|_| {
            ApiError::Internal(crate::error::anyhow_lite::Error(
                "stream listing failed".into(),
            ))
        })?;
    streams.extend(local);
    Ok(Json(StreamsResponse { streams, sources }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub service: Option<String>,
    pub stream: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    #[serde(default)]
    pub q: String,
    /// `q` is a regular expression rather than literal text.
    #[serde(default)]
    pub regex: bool,
    /// Minimum level (`warn` = WARN and ERROR). Not for audit streams.
    pub level: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// The query with its defaults filled in and this dashboard's budget
/// attached — what a server is sent — and, validated, what a local search
/// runs.
fn resolve(
    q: &SearchQuery,
    now: i64,
    bounds: &logsearch::Bounds,
) -> ApiResult<(SearchRequest, logsearch::Query, logsearch::Bounds)> {
    let stream = q
        .stream
        .clone()
        .ok_or_else(|| ApiError::BadRequest("`stream` is required".into()))?;
    let to = q.to.unwrap_or(now);
    let request = SearchRequest {
        stream,
        from: q.from.unwrap_or(to - DEFAULT_RANGE_MS),
        to,
        q: q.q.clone(),
        regex: q.regex,
        level: q.level.clone(),
        limit: q.limit.unwrap_or(DEFAULT_LIMIT),
        cursor: q.cursor.clone(),
        max_bytes: bounds.max_bytes,
        timeout_ms: bounds.deadline.as_millis() as u64,
    };
    let (query, bounds) = request.validate().map_err(ApiError::BadRequest)?;
    Ok((request, query, bounds))
}

async fn search(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Map<String, serde_json::Value>>> {
    let (actor, ls) = log_search(&app, &headers).await?;
    let service = q
        .service
        .clone()
        .ok_or_else(|| ApiError::BadRequest("`service` is required".into()))?;
    // `None` is the dashboard's own logs; anything else is a server's.
    let remote = match &app.monitor {
        _ if service == DASHBOARD_SERVICE => None,
        Some(m) if m.is_target(&service) => Some(m.clone()),
        _ => return Err(ApiError::NotFound),
    };
    let (request, query, bounds) = resolve(&q, crate::monitor::epoch_ms(), &ls.bounds)?;

    // Refuse rather than queue: a queued search would still be holding the
    // admin's request open past any useful wait.
    let permit = ls
        .permits
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::RateLimited)?;

    // Before the scan, so a search that is cut short is still on record.
    commons::audit::record(
        commons::audit::Category::GmAudit,
        serde_json::json!({
            "event": "log_search",
            "source": "dashboard",
            "admin": actor.subject(),
            "service": service,
            "stream": request.stream,
            "q": request.q,
            "regex": request.regex,
            "from": request.from,
            "to": request.to,
        }),
    );

    let mut outcome = match remote {
        Some(m) => {
            let answer = m.log_search(&service, &request).await;
            drop(permit);
            answer
                .expect("checked is_target above")
                .map_err(|e| ApiError::Upstream(format!("{service} did not answer: {e}")))?
        }
        None => {
            let ls2 = ls.clone();
            let found = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                logsearch::search(&ls2.local, &query, &bounds)
            })
            .await
            .map_err(|_| {
                ApiError::Internal(crate::error::anyhow_lite::Error("log search failed".into()))
            })?;
            match serde_json::to_value(found) {
                Ok(serde_json::Value::Object(o)) => o,
                _ => {
                    return Err(ApiError::Internal(crate::error::anyhow_lite::Error(
                        "log search failed".into(),
                    )));
                }
            }
        }
    };
    outcome.insert("service".into(), service.into());
    outcome.insert("stream".into(), request.stream.into());
    outcome.insert("from".into(), request.from.into());
    outcome.insert("to".into(), request.to.into());
    Ok(Json(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(stream: &str) -> SearchQuery {
        SearchQuery {
            service: Some("game_server".into()),
            stream: Some(stream.into()),
            from: None,
            to: None,
            q: String::new(),
            regex: false,
            level: None,
            limit: None,
            cursor: None,
        }
    }

    fn bounds() -> logsearch::Bounds {
        logsearch::Bounds {
            max_bytes: 1 << 20,
            deadline: std::time::Duration::from_millis(3000),
        }
    }

    #[test]
    fn defaults_are_the_last_day_and_a_hundred_lines_on_this_dashboards_budget() {
        let (r, _, _) = resolve(&q("diagnostic"), 1_000_000_000, &bounds()).unwrap();
        assert_eq!(
            (r.from, r.to),
            (1_000_000_000 - DEFAULT_RANGE_MS, 1_000_000_000)
        );
        assert_eq!(r.limit, DEFAULT_LIMIT);
        assert_eq!((r.max_bytes, r.timeout_ms), (1 << 20, 3000));
    }

    #[test]
    fn a_request_the_server_would_refuse_is_a_400_here() {
        let mut no_stream = q("diagnostic");
        no_stream.stream = None;
        assert!(matches!(
            resolve(&no_stream, 0, &bounds()),
            Err(ApiError::BadRequest(_))
        ));
        let mut bad = q("diagnostic");
        bad.q = "(".into();
        bad.regex = true;
        assert!(matches!(
            resolve(&bad, 0, &bounds()),
            Err(ApiError::BadRequest(m)) if m.contains("invalid regex")
        ));
    }
}
