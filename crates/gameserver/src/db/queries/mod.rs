//! The `load_*` readers and the handful of writers they sit beside — the
//! only place that speaks sea-orm. Split by the same domains as `commands`.

pub(super) mod account;
pub(super) mod character_load;
pub(super) mod character_store;
pub(super) mod clans;
pub(super) mod commerce;
pub(super) mod minigames;
pub(super) mod olympiad;
pub(super) mod residences;
pub(super) mod social;
pub(super) mod world;

use super::ItemRow;
use commons::util::now_millis;
use models::entity;

use models::sea_orm::ActiveModelTrait;
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::DbErr;
use tracing::warn;

/// `characters.createDate` is a `date` column SQLite fills with `date('now')`;
/// the entity carries it as text, so the value is formatted here.
pub(super) fn today() -> String {
    commons::util::format_date(now_millis())
}
/// Runs an insert that the caller treats as best-effort, logging a failure the
/// way the old `exec` helper did.
pub(super) async fn insert_or_warn<A: ActiveModelTrait>(
    db: &DatabaseConnection,
    insert: models::sea_orm::Insert<A>,
) {
    if let Err(e) = insert.exec(db).await {
        warn!("DB thread: insert failed: {e}");
    }
}
/// Logs a failed fire-and-forget write, the way the old `exec` helper did.
///
/// The DB thread must not stop for one bad statement: the game thread has
/// already applied the change in memory and is not waiting for a reply.
pub(crate) fn warn_err<T>(res: Result<T, DbErr>) {
    if let Err(e) = res {
        warn!("DB thread: query failed: {e}");
    }
}
/// One `items` row as every container flush writes it (player inventory, clan
/// warehouse, offline warehouse, mail attachments). `loc_override` replaces the
/// row's own `(loc, loc_data)` for containers that store a fixed location.
pub(crate) fn item_row_model(
    owner_id: i32,
    it: &ItemRow,
    loc_override: Option<(&str, i32)>,
) -> entity::items::ActiveModel {
    let (loc, loc_data) = match loc_override {
        Some((loc, loc_data)) => (loc.to_string(), loc_data),
        None => (it.loc.clone(), it.loc_data),
    };
    entity::items::ActiveModel {
        owner_id: Set(Some(owner_id)),
        object_id: Set(it.object_id),
        item_id: Set(Some(it.item_id)),
        count: Set(it.count),
        enchant_level: Set(Some(it.enchant_level)),
        loc: Set(Some(loc)),
        loc_data: Set(Some(loc_data)),
        custom_type1: Set(Some(it.custom_type1)),
        custom_type2: Set(Some(it.custom_type2)),
        mana_left: Set(it.mana_left),
        time: Set(it.time.into()),
        ..Default::default()
    }
}
