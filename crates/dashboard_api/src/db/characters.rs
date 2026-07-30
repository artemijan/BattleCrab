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

use models::entity::characters::{Column, Entity, Model};
use models::sea_orm::sea_query::Query;
use models::sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use crate::error::ApiResult;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSummary {
    /// The game account this character sits on. A master account can own
    /// several, so the UI needs it to group the list.
    pub account_name: String,
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

/// The allowlist, as a projection over the entity: the full `Model` carries
/// coordinates, access level and inventory-adjacent columns that must not reach
/// the API (§5.6), so nothing here reads a field outside this function.
fn to_summary(row: Model) -> CharacterSummary {
    CharacterSummary {
        account_name: row.account_name.unwrap_or_default(),
        name: row.char_name,
        level: row.level.unwrap_or(1),
        class_id: row.classid.unwrap_or(0),
        race: row.race.unwrap_or(0),
        sex: row.sex.unwrap_or(0),
        online_time: row.onlinetime.map(i64::from).unwrap_or(0),
        last_access: row.last_access,
        online: row.online.unwrap_or(0) != 0,
    }
}

pub async fn list_for_account(
    db: &DatabaseConnection,
    login: &str,
) -> ApiResult<Vec<CharacterSummary>> {
    let rows = Entity::find()
        .filter(Column::AccountName.eq(super::accounts::normalize_login(login)))
        .filter(Column::Deletetime.eq(0))
        .order_by_desc(Column::LastAccess)
        .all(db)
        .await?;

    Ok(rows.into_iter().map(to_summary).collect())
}

/// Every character on every game account under a master address.
///
/// A master account has no `login` of its own, so there is nothing to match
/// `characters.account_name` against directly — the join goes through the game
/// accounts that share the address. `login IS NOT NULL` keeps the master's own
/// row out of the subquery.
pub async fn list_for_master(
    db: &DatabaseConnection,
    email: &str,
) -> ApiResult<Vec<CharacterSummary>> {
    use models::entity::accounts;

    let logins = Query::select()
        .column(accounts::Column::Login)
        .from(accounts::Entity)
        .and_where(accounts::Column::Login.is_not_null())
        .and_where(super::accounts::email_eq(
            &super::accounts::normalize_email(email),
        ))
        .to_owned();

    let rows = Entity::find()
        .filter(Column::Deletetime.eq(0))
        .filter(Column::AccountName.in_subquery(logins))
        .order_by_desc(Column::LastAccess)
        .all(db)
        .await?;

    Ok(rows.into_iter().map(to_summary).collect())
}

/// Player count for the public status endpoint.
///
/// Caveat worth remembering: this reads persisted `online` flags, so a hard
/// crash can leave them set — it says nothing about whether the process is up
/// (PLAN_DASHBOARD.md §12 open question 3).
pub async fn online_count(db: &DatabaseConnection) -> ApiResult<i64> {
    let count = Entity::find()
        .filter(Column::Online.ne(0))
        .count(db)
        .await?;
    Ok(count as i64)
}
