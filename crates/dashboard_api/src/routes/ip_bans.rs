//! `/admin/ip-bans` — the IP ban list on the Audit page (`docs/MONITORING.md`
//! §10). Admin-only like the rest of `/admin`.
//!
//! A ban is enforced by the login server, which checks `ip_bans` on every new
//! connection: it stops new logins, not connections already open. That is
//! what `disconnect` on a new ban is for — it closes every connection the ban
//! covers, on both servers, through the monitor channel.
//!
//! Every change lands in the GM audit log with the acting admin.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::db::ip_bans::{self, BanFields, IpBan};
use crate::error::{ApiError, ApiResult};
use crate::monitor;
use crate::routes::require_admin;
use crate::state::AppState;

/// The column's width.
const MAX_REASON_CHARS: usize = 255;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list).post(create))
        .route("/{ip}", axum::routing::put(update).delete(remove))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResponse {
    /// The dashboard's clock, so the page tells expired from active against
    /// the clock the login server uses rather than the viewer's.
    now_ms: i64,
    bans: Vec<IpBan>,
}

async fn list(State(app): State<AppState>, headers: HeaderMap) -> ApiResult<Json<ListResponse>> {
    require_admin(&app, &headers).await?;
    Ok(Json(ListResponse {
        now_ms: monitor::epoch_ms(),
        bans: ip_bans::list(&app.db).await?,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BanRequest {
    pub ip: String,
    /// Epoch ms; absent or `null` bans for good.
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub reason: String,
}

/// Normalizes and checks what an admin typed. The address must be an IP —
/// a trailing `.0` octet widens it to a subnet — and
/// a timed ban must end in the future.
fn fields(body: &BanRequest, now: i64) -> ApiResult<BanFields> {
    let ip = body
        .ip
        .trim()
        .parse::<std::net::IpAddr>()
        .map_err(|_| ApiError::BadRequest(format!("{:?} is not an IP address", body.ip.trim())))?
        .to_string();
    let reason = body.reason.trim().to_string();
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(ApiError::BadRequest(format!(
            "the reason must be at most {MAX_REASON_CHARS} characters"
        )));
    }
    let expires_at = match body.expires_at {
        None | Some(0) => None,
        Some(at) if at <= now => {
            return Err(ApiError::BadRequest(
                "the ban would already have expired".into(),
            ));
        }
        Some(at) => Some(at),
    };
    Ok(BanFields {
        ip,
        expires_at,
        reason,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    #[serde(flatten)]
    pub ban: BanRequest,
    /// Also close every connection the ban covers, right now.
    #[serde(default)]
    pub disconnect: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateResponse {
    ban: IpBan,
    /// Connections closed because of `disconnect`.
    disconnected: usize,
    /// Servers that could not be asked to disconnect, with why. Non-empty
    /// means someone the ban covers may still be connected.
    disconnect_errors: Vec<String>,
}

fn audit(event: &str, admin: &str, details: serde_json::Value) {
    let mut record = serde_json::json!({
        "event": event,
        "source": "dashboard",
        "admin": admin,
    });
    if let (Some(record), serde_json::Value::Object(details)) = (record.as_object_mut(), details) {
        record.extend(details);
    }
    commons::audit::record(commons::audit::Category::GmAudit, record);
}

async fn create(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRequest>,
) -> ApiResult<(StatusCode, Json<CreateResponse>)> {
    let actor = require_admin(&app, &headers).await?;
    let now = monitor::epoch_ms();
    let fields = fields(&body.ban, now)?;
    let ban = ip_bans::upsert(&app.db, &fields, actor.subject(), now).await?;

    let (mut disconnected, mut disconnect_errors) = (Vec::new(), Vec::new());
    if body.disconnect {
        match &app.monitor {
            Some(m) => {
                let summary = m.kick_covered(&ban.ip).await;
                disconnected = summary.kicked;
                disconnect_errors = summary.errors;
            }
            None => disconnect_errors
                .push("server monitoring is disabled, so nobody could be disconnected".into()),
        }
    }

    tracing::info!(
        admin = %actor.subject(),
        ip = %ban.ip,
        disconnected = disconnected.len(),
        "admin: banned an IP address"
    );
    audit(
        "ip_ban",
        actor.subject(),
        serde_json::json!({
            "ip": ban.ip,
            "expires_at": ban.expires_at,
            "reason": ban.reason,
            "disconnected": disconnected
                .iter()
                .map(|c| serde_json::json!({
                    "service": c.service,
                    "client_id": c.id,
                    "ip": c.ip,
                    "account": c.account,
                }))
                .collect::<Vec<_>>(),
        }),
    );
    Ok((
        StatusCode::CREATED,
        Json(CreateResponse {
            ban,
            disconnected: disconnected.len(),
            disconnect_errors,
        }),
    ))
}

async fn update(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(ip): Path<String>,
    Json(body): Json<BanRequest>,
) -> ApiResult<Json<IpBan>> {
    let actor = require_admin(&app, &headers).await?;
    let now = monitor::epoch_ms();
    let fields = fields(&body, now)?;
    let ban = ip_bans::update(&app.db, &ip, &fields, actor.subject(), now).await?;
    tracing::info!(admin = %actor.subject(), ip = %ip, "admin: edited an IP ban");
    audit(
        "ip_ban_edit",
        actor.subject(),
        serde_json::json!({
            "ip": ip,
            "new_ip": ban.ip,
            "expires_at": ban.expires_at,
            "reason": ban.reason,
        }),
    );
    Ok(Json(ban))
}

async fn remove(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(ip): Path<String>,
) -> ApiResult<StatusCode> {
    let actor = require_admin(&app, &headers).await?;
    ip_bans::delete(&app.db, &ip).await?;
    tracing::info!(admin = %actor.subject(), ip = %ip, "admin: lifted an IP ban");
    audit("ip_unban", actor.subject(), serde_json::json!({ "ip": ip }));
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(ip: &str, expires_at: Option<i64>, reason: &str) -> BanRequest {
        BanRequest {
            ip: ip.into(),
            expires_at,
            reason: reason.into(),
        }
    }

    #[test]
    fn addresses_are_parsed_and_normalized() {
        assert_eq!(
            fields(&req(" 10.0.0.1 ", None, ""), 0).unwrap().ip,
            "10.0.0.1"
        );
        assert_eq!(
            fields(&req("10.1.0.0", None, ""), 0).unwrap().ip,
            "10.1.0.0"
        );
        assert_eq!(
            fields(&req("0:0:0:0:0:0:0:1", None, ""), 0).unwrap().ip,
            "::1"
        );
        assert!(fields(&req("10.1.*", None, ""), 0).is_err());
        assert!(fields(&req("", None, ""), 0).is_err());
    }

    #[test]
    fn a_timed_ban_must_end_in_the_future() {
        assert_eq!(
            fields(&req("1.2.3.4", Some(0), ""), 100)
                .unwrap()
                .expires_at,
            None
        );
        assert_eq!(
            fields(&req("1.2.3.4", Some(101), ""), 100)
                .unwrap()
                .expires_at,
            Some(101)
        );
        assert!(fields(&req("1.2.3.4", Some(100), ""), 100).is_err());
    }

    #[test]
    fn the_reason_fits_its_column() {
        assert!(fields(&req("1.2.3.4", None, &"é".repeat(255)), 0).is_ok());
        assert!(fields(&req("1.2.3.4", None, &"a".repeat(256)), 0).is_err());
    }
}
