//! `/admin/logs` — log search (`docs/MONITORING.md` §6). Admin-only, like the
//! rest of `/admin`: every handler starts with `require_admin`.
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

use crate::error::{ApiError, ApiResult};
use crate::logsearch::{self, Cursor, Level, LogSearch, Outcome, StreamInfo};
use crate::routes::require_admin;
use crate::state::AppState;

const DEFAULT_RANGE_MS: i64 = 86_400_000;
const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;
/// Longer than any sensible search; a cap so a pattern cannot be a payload.
const MAX_QUERY_LEN: usize = 512;
/// Compiled-program and lazy-DFA caps: `regex` is linear-time, and these
/// keep a pathological pattern from costing memory instead.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

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
}

async fn streams(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<StreamsResponse>> {
    let (_, ls) = log_search(&app, &headers).await?;
    let streams =
        tokio::task::spawn_blocking(move || ls.sources.iter().flat_map(|s| s.streams()).collect())
            .await
            .map_err(|_| {
                ApiError::Internal(crate::error::anyhow_lite::Error(
                    "stream listing failed".into(),
                ))
            })?;
    Ok(Json(StreamsResponse { streams }))
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    service: String,
    stream: String,
    from: i64,
    to: i64,
    #[serde(flatten)]
    outcome: Outcome,
}

/// A literal `q` matches case-insensitively, as a search box is expected to;
/// `regex=true` takes the pattern as written (`(?i)` opts back in).
fn compile(q: &str, regex: bool) -> ApiResult<Option<regex::bytes::Regex>> {
    if q.is_empty() {
        return Ok(None);
    }
    if q.len() > MAX_QUERY_LEN {
        return Err(ApiError::BadRequest(format!(
            "`q` is limited to {MAX_QUERY_LEN} bytes"
        )));
    }
    let pattern = if regex {
        q.to_string()
    } else {
        regex::escape(q)
    };
    regex::bytes::RegexBuilder::new(&pattern)
        .case_insensitive(!regex)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map(Some)
        .map_err(|e| ApiError::BadRequest(format!("invalid regex: {e}")))
}

/// Everything about a request that can be refused before a file is touched.
fn validate(q: &SearchQuery, now: i64) -> ApiResult<logsearch::Query> {
    let stream_name = q
        .stream
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("`stream` is required".into()))?;
    let stream = logsearch::Stream::parse(stream_name).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "unknown stream {stream_name:?}; valid: diagnostic, error, audit:<category>"
        ))
    })?;
    let to = q.to.unwrap_or(now);
    let from = q.from.unwrap_or(to - DEFAULT_RANGE_MS);
    if from >= to {
        return Err(ApiError::BadRequest("`from` must be before `to`".into()));
    }
    let level = match q.level.as_deref().filter(|l| !l.is_empty()) {
        None => None,
        Some(_) if !stream.has_level() => {
            return Err(ApiError::BadRequest(
                "audit records have no level; drop `level`".into(),
            ));
        }
        Some(l) => Some(Level::parse(l).ok_or_else(|| {
            ApiError::BadRequest("`level` must be trace, debug, info, warn or error".into())
        })?),
    };
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::BadRequest(format!(
            "`limit` must be between 1 and {MAX_LIMIT}"
        )));
    }
    let cursor = match q.cursor.as_deref().filter(|c| !c.is_empty()) {
        None => None,
        Some(c) => {
            Some(Cursor::decode(c).ok_or_else(|| ApiError::BadRequest("invalid `cursor`".into()))?)
        }
    };
    Ok(logsearch::Query {
        stream,
        from,
        to,
        pattern: compile(&q.q, q.regex)?,
        level,
        limit,
        cursor,
    })
}

async fn search(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let (actor, ls) = log_search(&app, &headers).await?;
    let service = q
        .service
        .clone()
        .ok_or_else(|| ApiError::BadRequest("`service` is required".into()))?;
    if ls.source(&service).is_none() {
        return Err(ApiError::NotFound);
    }
    let query = validate(&q, crate::monitor::epoch_ms())?;

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
            "stream": query.stream.name(),
            "q": q.q,
            "regex": q.regex,
            "from": query.from,
            "to": query.to,
        }),
    );

    let (from, to, stream) = (query.from, query.to, query.stream.name());
    let ls2 = ls.clone();
    let svc = service.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let source = ls2.source(&svc).expect("checked above");
        logsearch::search(source, &query, &ls2.bounds)
    })
    .await
    .map_err(|_| {
        ApiError::Internal(crate::error::anyhow_lite::Error("log search failed".into()))
    })?;

    Ok(Json(SearchResponse {
        service,
        stream,
        from,
        to,
        outcome,
    }))
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

    #[test]
    fn defaults_are_the_last_day_and_a_hundred_lines() {
        let v = validate(&q("diagnostic"), 1_000_000_000).unwrap();
        assert_eq!(
            (v.from, v.to),
            (1_000_000_000 - DEFAULT_RANGE_MS, 1_000_000_000)
        );
        assert_eq!(v.limit, DEFAULT_LIMIT);
        assert!(v.pattern.is_none());
    }

    #[test]
    fn literal_text_is_escaped_and_case_insensitive() {
        let p = compile("a.b (c)", false).unwrap().unwrap();
        assert!(p.is_match(b"xx A.B (C) yy"));
        assert!(!p.is_match(b"aXb (c)"), "the dot is literal");
        let r = compile("a.b", true).unwrap().unwrap();
        assert!(r.is_match(b"aXb"));
        assert!(!r.is_match(b"AXB"), "a regex is taken as written");
    }

    #[test]
    fn hostile_or_malformed_requests_are_refused_up_front() {
        assert!(compile("(", true).is_err());
        assert!(compile(&"a".repeat(MAX_QUERY_LEN + 1), false).is_err());
        // Compiles to far more than the cap: refused, not built.
        assert!(compile(r"\w{1000}\w{1000}\w{1000}", true).is_err());

        assert!(validate(&q("audit:../../etc"), 0).is_err());
        let mut audit_level = q("audit:chat");
        audit_level.level = Some("warn".into());
        assert!(validate(&audit_level, 0).is_err());
        let mut bad_level = q("diagnostic");
        bad_level.level = Some("loud".into());
        assert!(validate(&bad_level, 0).is_err());
        let mut big = q("diagnostic");
        big.limit = Some(MAX_LIMIT + 1);
        assert!(validate(&big, 0).is_err());
        let mut inverted = q("diagnostic");
        (inverted.from, inverted.to) = (Some(10), Some(5));
        assert!(validate(&inverted, 0).is_err());
        let mut cursor = q("diagnostic");
        cursor.cursor = Some("not base64!".into());
        assert!(validate(&cursor, 0).is_err());
    }
}
