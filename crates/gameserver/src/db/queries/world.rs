//! World-state reads: NPC respawns, grand bosses, ground items, global
//! variables, and the object-id watermark.

use super::super::FIRST_OID;
use super::super::GroundItemRow;
use super::warn_err;
use models::entity;

use models::sea_orm::ActiveValue::Set;
use models::sea_orm::DatabaseConnection;
use models::sea_orm::{ColumnTrait, EntityTrait, QueryOrder, QuerySelect, TransactionTrait};
use tracing::warn;

/// One `npc_respawns` row — a raid boss's persisted state.
#[derive(Debug, Clone, Copy)]
pub struct NpcRespawnRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Absolute unix millis the boss is due back, or 0 when it's alive (Java
    /// stores 0 for a living boss and the due time for a dead one).
    pub respawn_time: i64,
    pub cur_hp: f64,
    pub cur_mp: f64,
}
/// Boot load of the whole `npc_respawns` table (Java `DBSpawnManager.load`).
/// Missing table → empty, like the other boot loads.
pub(crate) async fn load_npc_respawns(db: &DatabaseConnection) -> Vec<NpcRespawnRow> {
    entity::npc_respawns::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| NpcRespawnRow {
            npc_id: row.id,
            x: row.x,
            y: row.y,
            z: row.z,
            heading: row.heading,
            respawn_time: row.respawn_time,
            cur_hp: row.current_hp,
            cur_mp: row.current_mp,
        })
        .collect()
}
/// Java's `IdManager` hands out ids from a single pool shared by every
/// world-object type, so the next free id must clear the high-water mark of
/// every table that stores one — not just `characters` (a fresh id here that
/// collides with an existing `items.object_id` fails its INSERT silently).
pub(crate) async fn load_next_id(db: &DatabaseConnection) -> i64 {
    let max_char = entity::characters::Entity::find()
        .select_only()
        .column_as(entity::characters::Column::CharId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    let max_item = entity::items::Entity::find()
        .select_only()
        .column_as(entity::items::Column::ObjectId.max(), "m")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or(0);
    (max_char.max(max_item) + 1).max(FIRST_OID)
}
pub(crate) async fn load_grandboss_data(
    db: &DatabaseConnection,
) -> Vec<crate::model::grand_boss::GrandBoss> {
    entity::grandboss_data::Entity::find()
        .order_by_asc(entity::grandboss_data::Column::BossId)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| crate::model::grand_boss::GrandBoss {
            boss_id: r.boss_id,
            loc_x: r.loc_x,
            loc_y: r.loc_y,
            loc_z: r.loc_z,
            heading: r.heading,
            respawn_time: r.respawn_time,
            current_hp: r.current_hp,
            current_mp: r.current_mp,
            status: r.status,
        })
        .collect()
}
/// `CastleManager.load`: every `castle` row (id/name/side).
/// `GlobalVariablesManager.restoreMe` — the whole `global_variables` table.
pub(crate) async fn load_global_variables(db: &DatabaseConnection) -> Vec<(String, String)> {
    entity::global_variables::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.var, r.value.unwrap_or_default()))
        .collect()
}
/// `ItemsOnGroundManager.load()`'s `SELECT` — every persisted ground item.
pub(crate) async fn load_ground_items(db: &DatabaseConnection) -> Vec<GroundItemRow> {
    match entity::itemsonground::Entity::find().all(db).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| GroundItemRow {
                object_id: r.object_id,
                item_id: r.item_id.unwrap_or(0),
                count: r.count,
                enchant_level: r.enchant_level.unwrap_or(0),
                x: r.x.unwrap_or(0),
                y: r.y.unwrap_or(0),
                z: r.z.unwrap_or(0),
                drop_time_ms: r.drop_time,
                equipable: r.equipable.unwrap_or(0) != 0,
            })
            .collect(),
        Err(e) => {
            warn!("load_ground_items: {e}");
            Vec::new()
        }
    }
}
/// `ItemsOnGroundManager.emptyTable()`.
pub(crate) async fn clear_ground_items(db: &DatabaseConnection) {
    warn_err(entity::itemsonground::Entity::delete_many().exec(db).await);
}
/// `ItemsOnGroundManager.run()` — empty, then reinsert the live set. Java does
/// the same thing for the same reason: a ground item has no stable identity to
/// diff against once it has been picked up or decayed.
pub(crate) async fn store_ground_items(db: &DatabaseConnection, items: &[GroundItemRow]) {
    clear_ground_items(db).await;
    if items.is_empty() {
        return;
    }
    let Ok(tx) = db.begin().await else {
        warn!("store_ground_items: could not open a transaction");
        return;
    };
    for it in items {
        warn_err(
            entity::itemsonground::Entity::insert(entity::itemsonground::ActiveModel {
                object_id: Set(it.object_id),
                item_id: Set(Some(it.item_id)),
                count: Set(it.count),
                enchant_level: Set(Some(it.enchant_level)),
                x: Set(Some(it.x)),
                y: Set(Some(it.y)),
                z: Set(Some(it.z)),
                drop_time: Set(it.drop_time_ms),
                equipable: Set(Some(i32::from(it.equipable))),
            })
            .exec(&tx)
            .await,
        );
    }
    warn_err(tx.commit().await);
}
