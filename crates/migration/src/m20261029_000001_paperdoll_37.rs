//! Salvation's 37-slot paperdoll (`Inventory.PAPERDOLL_*`). The five agathion
//! slots take indices 17–21, so every slot Interlude stored from 17 up (the
//! six talismans, cloak, belt, brooch and six brooch jewels) moves up by 5.
//! An equipped item's slot is `items.loc_data` on a `PAPERDOLL` row; no other
//! table stores a paperdoll index, and pet gear never reaches slot 17.
//!
//! The shift runs once, under the migration tracker; it is not idempotent on
//! its own, because a row at 22 cannot tell whether it was shifted already.
//!
//! `down` shifts back. Anything worn in an agathion slot has no Interlude slot
//! to return to, so it goes back to the bag (`INVENTORY`, `loc_data` 0, which
//! is how Interlude stores an unequipped item).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `PAPERDOLL_AGATHION1`, the first index that moves.
const FIRST_MOVED: i32 = 17;
/// How many agathion slots were inserted.
const SHIFT: i32 = 5;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(&format!(
                "UPDATE items SET loc_data = loc_data + {SHIFT} \
                 WHERE loc = 'PAPERDOLL' AND loc_data >= {FIRST_MOVED}"
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(&format!(
            "UPDATE items SET loc = 'INVENTORY', loc_data = 0 \
             WHERE loc = 'PAPERDOLL' AND loc_data >= {FIRST_MOVED} \
             AND loc_data < {}",
            FIRST_MOVED + SHIFT
        ))
        .await?;
        db.execute_unprepared(&format!(
            "UPDATE items SET loc_data = loc_data - {SHIFT} \
             WHERE loc = 'PAPERDOLL' AND loc_data >= {}",
            FIRST_MOVED + SHIFT
        ))
        .await?;
        Ok(())
    }
}
