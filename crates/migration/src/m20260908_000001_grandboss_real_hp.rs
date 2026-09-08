//! `grandboss_data.currentHP`/`currentMP` become `double`, so SQLite stores
//! them as REAL.
//!
//! The columns shipped as `decimal(30,15)`, which gives them **NUMERIC**
//! affinity: SQLite keeps such a value as INTEGER whenever it converts
//! losslessly. Baium's stock 4068372 HP is one of those, and so is the `0.0`
//! that `on_grand_boss_killed` writes for every boss it kills. sqlx then
//! refuses INTEGER → `f64` ("mismatched types"), which fails the *whole*
//! `SELECT` — one integral row takes every other row with it. The loader
//! swallowed that error and returned no bosses at all, so nothing spawned:
//! no Queen Ant in the Ant Nest, no Orfen, no Antharas (GitHub #19).
//!
//! `double` has REAL affinity, which converts an integral value to a float on
//! the way in instead. That is what `npc_respawns.currentHp` has always used —
//! the same column, one table over, and the reason raid bosses never broke this
//! way.
//!
//! The values already stored as INTEGER stay INTEGER until they are rewritten,
//! so the rebuild `CAST`s them. SQLite cannot change a column type in place;
//! this is the same create-copy-drop-rename dance as
//! [`super::m20260801_000003_master_accounts`], and likewise skipped outright
//! when the column is already `double` (a fresh database, or a re-run).

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `grandboss_data` columns, in dist order.
const CARRIED: &[&str] = &[
    "boss_id",
    "loc_x",
    "loc_y",
    "loc_z",
    "heading",
    "respawn_time",
    "currentHP",
    "currentMP",
    "status",
];

/// The declared type of `grandboss_data.currentHP`, or `None` if the table or
/// the column is not there.
async fn hp_column_type(manager: &SchemaManager<'_>) -> Result<Option<String>, DbErr> {
    let rows = manager
        .get_connection()
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA table_info(\"grandboss_data\")",
        ))
        .await?;
    for row in rows {
        if row.try_get::<String>("", "name")? == "currentHP" {
            return Ok(Some(row.try_get::<String>("", "type")?));
        }
    }
    Ok(None)
}

/// Build `grandboss_data_new` with `currentHP`/`currentMP` declared as `ty`,
/// copy every row across casting those two, then swap it into place.
async fn rebuild(manager: &SchemaManager<'_>, ty: &str, cast: &str) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("grandboss_data_new"))
                .col(
                    ColumnDef::new(Alias::new("boss_id"))
                        .custom(Alias::new("smallint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_x"))
                        .custom(Alias::new("mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_y"))
                        .custom(Alias::new("mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("loc_z"))
                        .custom(Alias::new("mediumint"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("heading"))
                        .custom(Alias::new("mediumint"))
                        .not_null()
                        .default(Expr::cust("'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("respawn_time"))
                        .custom(Alias::new("bigint"))
                        .not_null()
                        .default(Expr::cust("'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("currentHP"))
                        .custom(Alias::new(ty))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("currentMP"))
                        .custom(Alias::new(ty))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("status"))
                        .custom(Alias::new("tinyint"))
                        .not_null()
                        .default(Expr::cust("'0'")),
                )
                .primary_key(Index::create().col(Alias::new("boss_id")))
                .to_owned(),
        )
        .await?;

    let cols = CARRIED
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let selected = CARRIED
        .iter()
        .map(|c| match *c {
            "currentHP" | "currentMP" => format!("CAST(`{c}` AS {cast})"),
            _ => format!("`{c}`"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "INSERT INTO `grandboss_data_new` ({cols}) SELECT {selected} FROM `grandboss_data`"
        ))
        .await?;
    manager
        .drop_table(Table::drop().table(Alias::new("grandboss_data")).to_owned())
        .await?;
    manager
        .rename_table(
            Table::rename()
                .table(
                    Alias::new("grandboss_data_new"),
                    Alias::new("grandboss_data"),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let Some(ty) = hp_column_type(manager).await? else {
            return Ok(()); // no table yet — the baseline builds it as `double`
        };
        // Any REAL-affinity spelling counts as done; only the shipped
        // `decimal(30,15)` needs the rebuild.
        if ty.to_uppercase().contains("DOUB") {
            return Ok(());
        }
        rebuild(manager, "double", "REAL").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let Some(ty) = hp_column_type(manager).await? else {
            return Ok(());
        };
        if !ty.to_uppercase().contains("DOUB") {
            return Ok(());
        }
        // `CAST(… AS NUMERIC)` is what a NUMERIC-affinity column would have
        // applied on insert, so rolling back restores the storage classes the
        // old schema produced — integral values included.
        rebuild(manager, "decimal(30,15)", "NUMERIC").await
    }
}
