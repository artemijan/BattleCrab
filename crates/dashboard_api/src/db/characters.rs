//! `characters` is **strictly read-only** to this crate (PLAN_DASHBOARD.md §3.2).
//!
//! Live character state is memory-first in the game server and flushed only by
//! autosave/logout/shutdown — any write here would be silently clobbered, or
//! would resurrect stale values over newer ones. There are deliberately no
//! write helpers in this module.
//!
//! Columns are an explicit allowlist, never `SELECT *`: the table carries
//! coordinates, access level and inventory-adjacent fields that must not reach
//! the API (§5.6).

use sqlx::SqlitePool;

use crate::error::ApiResult;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSummary {
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub race: i32,
    pub sex: i32,
    /// Seconds played, as the game server accounts it.
    pub online_time: i64,
    pub last_access: i64,
    pub online: bool,
}

pub async fn list_for_account(pool: &SqlitePool, login: &str) -> ApiResult<Vec<CharacterSummary>> {
    let rows: Vec<(String, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i64>, i64, Option<i32>)> =
        sqlx::query_as(
            "SELECT char_name, level, classid, race, sex, onlinetime, lastAccess, online \
             FROM characters WHERE account_name = ? AND deletetime = 0 \
             ORDER BY lastAccess DESC",
        )
        .bind(super::accounts::normalize_login(login))
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(
            |(name, level, class_id, race, sex, online_time, last_access, online)| CharacterSummary {
                name,
                level: level.unwrap_or(1),
                class_id: class_id.unwrap_or(0),
                race: race.unwrap_or(0),
                sex: sex.unwrap_or(0),
                online_time: online_time.unwrap_or(0),
                last_access,
                online: online.unwrap_or(0) != 0,
            },
        )
        .collect())
}

/// Player count for the public status endpoint.
///
/// Caveat worth remembering: this reads persisted `online` flags, so a hard
/// crash can leave them set — it says nothing about whether the process is up
/// (PLAN_DASHBOARD.md §12 open question 3).
pub async fn online_count(pool: &SqlitePool) -> ApiResult<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM characters WHERE online <> 0")
        .fetch_one(pool)
        .await?;
    Ok(count)
}
