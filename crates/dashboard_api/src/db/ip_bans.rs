//! `ip_bans` — the address bans the Audit page manages and the login server
//! enforces on every new connection (`models::repo::ip_bans`).
//!
//! The servers read it per connection, so nothing here has to tell a running
//! server about a change. Besides `/admin/ip-bans`, the game server writes it
//! when an address keeps connecting without authenticating.

use models::entity::ip_bans::{ActiveModel, Column, Entity, Model};
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, QueryOrder, TransactionTrait,
};

use crate::error::{ApiError, ApiResult};

/// One ban as the Audit page shows it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpBan {
    pub ip: String,
    /// Epoch ms; `None` for a permanent ban.
    pub expires_at: Option<i64>,
    pub reason: String,
    /// The admin's master address.
    pub banned_by: String,
    pub created_at: i64,
}

impl From<Model> for IpBan {
    fn from(m: Model) -> Self {
        Self {
            ip: m.ip,
            expires_at: (m.expires_at > 0).then_some(m.expires_at),
            reason: m.reason,
            banned_by: m.banned_by,
            created_at: m.created_at,
        }
    }
}

/// What an admin sets on a ban.
#[derive(Debug, Clone)]
pub struct BanFields {
    pub ip: String,
    pub expires_at: Option<i64>,
    pub reason: String,
}

/// Every ban, expired ones included, newest first.
pub async fn list(db: &DatabaseConnection) -> ApiResult<Vec<IpBan>> {
    Ok(Entity::find()
        .order_by_desc(Column::CreatedAt)
        .order_by_asc(Column::Ip)
        .all(db)
        .await?
        .into_iter()
        .map(IpBan::from)
        .collect())
}

fn row(fields: &BanFields, banned_by: &str, now: i64) -> ActiveModel {
    ActiveModel {
        ip: Set(fields.ip.clone()),
        expires_at: Set(fields.expires_at.unwrap_or(0)),
        reason: Set(fields.reason.clone()),
        banned_by: Set(banned_by.to_string()),
        created_at: Set(now),
    }
}

/// Bans `fields.ip`, replacing any ban already on it
/// (`models::repo::ip_bans::ban`).
pub async fn upsert(
    db: &DatabaseConnection,
    fields: &BanFields,
    banned_by: &str,
    now: i64,
) -> ApiResult<IpBan> {
    Ok(models::repo::ip_bans::ban(
        db,
        &fields.ip,
        fields.expires_at.unwrap_or(0),
        &fields.reason,
        banned_by,
        now,
    )
    .await?
    .into())
}

/// Rewrites the ban on `ip` — its address too, when `fields.ip` differs. The
/// edit is attributed to the admin making it. 404 when there is no such ban;
/// 400 when the new address already has a ban of its own.
pub async fn update(
    db: &DatabaseConnection,
    ip: &str,
    fields: &BanFields,
    banned_by: &str,
    now: i64,
) -> ApiResult<IpBan> {
    let txn = db.begin().await?;
    if Entity::find_by_id(ip.to_string())
        .one(&txn)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound);
    }
    if fields.ip != ip
        && Entity::find_by_id(fields.ip.clone())
            .one(&txn)
            .await?
            .is_some()
    {
        return Err(ApiError::BadRequest(format!(
            "{} is already banned; edit that ban instead",
            fields.ip
        )));
    }
    Entity::delete_by_id(ip.to_string()).exec(&txn).await?;
    let model = row(fields, banned_by, now).insert(&txn).await?;
    txn.commit().await?;
    Ok(model.into())
}

/// Lifts the ban on `ip`. 404 when there is none.
pub async fn delete(db: &DatabaseConnection, ip: &str) -> ApiResult<()> {
    let result = Entity::delete_by_id(ip.to_string()).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}
