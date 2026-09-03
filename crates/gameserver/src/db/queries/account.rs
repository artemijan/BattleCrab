//! Per-account and per-player settings: account variables, premium, buffer
//! schemes and community-board favorites.

use models::entity;

use models::sea_orm::DatabaseConnection;
use models::sea_orm::{EntityTrait, QueryOrder};

/// Best-effort read of one `account_gsdata` variable (Java
/// `AccountVariables.restoreMe`). Returns `None` on a missing row or any error
/// (e.g. the table absent in a minimal test schema), mirroring Java's
/// catch-and-default-empty behaviour.
pub(super) async fn load_account_var(
    db: &DatabaseConnection,
    account: &str,
    var: &str,
) -> Option<String> {
    entity::account_gsdata::Entity::find_by_id((account.to_string(), var.to_string()))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|row| row.value)
}
/// Best-effort boot load of the whole `account_premium` table (Java
/// `PremiumManager` has no table-wide load; this port caches all rows so the
/// admin `//premium_*` commands work for offline accounts). Missing table → empty.
pub(crate) async fn load_premium(db: &DatabaseConnection) -> Vec<(String, i64)> {
    entity::account_premium::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.account_name.to_lowercase(), row.enddate))
        .collect()
}
/// Boot load of the whole `buffer_schemes` table (Java `SchemeBufferTable.load`).
/// `skills` is stored comma-joined; parse it here, drop empties. Availability
/// filtering (skills still in the buffer table) happens on the game thread,
/// where the datapack lives. Missing table → empty.
pub(crate) async fn load_buffer_schemes(db: &DatabaseConnection) -> Vec<(i32, String, Vec<i32>)> {
    entity::buffer_schemes::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let skills = row
                .skills
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            (row.object_id, row.scheme_name, skills)
        })
        .collect()
}
/// Boot load of the whole `bbs_favorites` table (Java `FavoriteBoard` loads it
/// per-player on `_bbsgetfav`; this port caches all rows at boot like the
/// buffer schemes). `ORDER BY favAddDate DESC` matches Java's list order.
/// Missing table → empty.
pub(crate) async fn load_favorites(
    db: &DatabaseConnection,
) -> Vec<(i32, i32, String, String, String)> {
    entity::bbs_favorites::Entity::find()
        .order_by_desc(entity::bbs_favorites::Column::FavAddDate)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            (
                row.player_id,
                row.fav_id,
                row.fav_title,
                row.fav_bypass,
                row.fav_add_date,
            )
        })
        .collect()
}
