//! `items` is **strictly read-only** to this crate, for the same reason as
//! `characters` (DASHBOARD.md §3.2): live inventories are memory-first in the
//! game server, and a write from here would be clobbered or would resurrect
//! stale rows. There are deliberately no write helpers in this module.

use models::entity::items::{Column, Entity, Model};
use models::sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::error::ApiResult;

/// The only storage locations the dashboard shows. Everything else the column
/// can hold (pet inventories, freight, mail attachments) stays out until a
/// view actually needs it.
pub const LOC_EQUIPPED: &str = "PAPERDOLL";
pub const LOC_INVENTORY: &str = "INVENTORY";
pub const LOC_WAREHOUSE: &str = "WAREHOUSE";

pub struct OwnedItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant: i32,
    pub loc: String,
    /// Slot index within `loc` — the paperdoll slot or bag position.
    pub loc_data: i32,
}

/// Explicit projection, like `characters`': the full row carries columns
/// (variation ids, time-of-use) no response needs.
fn to_owned_item(row: Model) -> OwnedItem {
    OwnedItem {
        object_id: row.object_id,
        item_id: row.item_id.unwrap_or(0),
        count: row.count,
        enchant: row.enchant_level.unwrap_or(0),
        loc: row.loc.clone().unwrap_or_default(),
        loc_data: row.loc_data.unwrap_or(0),
    }
}

/// Everything a character carries, wears, or has warehoused.
pub async fn for_character(db: &DatabaseConnection, char_id: i32) -> ApiResult<Vec<OwnedItem>> {
    let rows = Entity::find()
        .filter(Column::OwnerId.eq(char_id))
        .filter(Column::Loc.is_in([LOC_EQUIPPED, LOC_INVENTORY, LOC_WAREHOUSE]))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(to_owned_item).collect())
}
