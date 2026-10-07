//! Master accounts: `accounts.login` becomes nullable, `is_verified` appears,
//! and one address can own at most one master account.
//!
//! The Rust port of `docs/migrations/2026-07-21-master-accounts.sql`, which was
//! applied to the live database by hand and existed nowhere in code until now.
//! See DASHBOARD.md §15 for what a master account is: a dashboard identity
//! keyed by email, marked by a NULL `login`, which is why `login` can no longer
//! be the primary key.
//!
//! SQLite cannot relax a primary key in place, so this is the standard
//! create-copy-drop-rename dance. It is skipped wholesale when `is_verified` is
//! already present — that is what makes `l2r-migrate up` safe to run against
//! the production database, where this change is already live.

use sea_orm_migration::prelude::*;

use crate::dialect::{dflt, master_email_index, ty};
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `accounts` columns carried across the rebuild, in dist order.
const CARRIED: &[&str] = &[
    "login",
    "password",
    "email",
    "created_time",
    "lastactive",
    "accessLevel",
    "lastIP",
    "lastServer",
    "pcIp",
    "hop1",
    "hop2",
    "hop3",
    "hop4",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_column("accounts", "is_verified").await? {
            // Already the master-account shape (the live DB, or a re-run).
            // Still make sure the index is there: the hand-applied SQL and this
            // migration must converge on the same schema.
            manager
                .get_connection()
                .execute_unprepared(master_email_index(manager))
                .await?;
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("accounts_new"))
                    .col(
                        // Nullable, and no longer the primary key: a NULL login
                        // is what marks a master account.
                        ColumnDef::new(Alias::new("login"))
                            .custom(ty(manager, "VARCHAR(45)"))
                            .null()
                            .default(dflt(manager, "NULL"))
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password"))
                            .custom(ty(manager, "VARCHAR(45)"))
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .custom(ty(manager, "varchar(255)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        // NULL on a game account, 0/1 on a master account —
                        // the three-state column the dashboard reads.
                        ColumnDef::new(Alias::new("is_verified"))
                            .custom(ty(manager, "TINYINT"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_time"))
                            .custom(ty(manager, "timestamp"))
                            .not_null()
                            .default(dflt(manager, "CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastactive"))
                            .custom(ty(manager, "bigint"))
                            .not_null()
                            .default(dflt(manager, "'0'")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("accessLevel"))
                            .custom(ty(manager, "TINYINT"))
                            .not_null()
                            .default(dflt(manager, "0")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastIP"))
                            .custom(ty(manager, "CHAR(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastServer"))
                            .custom(ty(manager, "TINYINT"))
                            .null()
                            .default(dflt(manager, "1")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("pcIp"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop1"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop2"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop3"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop4"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .to_owned(),
            )
            .await?;

        // Every existing row keeps its login, so they all become *game*
        // accounts (`is_verified` NULL). Nobody has a master account
        // afterwards — intended: an existing address links to the master its
        // owner registers later, by the shared address.
        copy_rows(manager, "accounts", "accounts_new").await?;

        manager
            .drop_table(Table::drop().table(Alias::new("accounts")).to_owned())
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(Alias::new("accounts_new"), Alias::new("accounts"))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(master_email_index(manager))
            .await?;
        Ok(())
    }

    /// Rebuilds the dist shape: `login` back to a NOT NULL primary key.
    ///
    /// **Master accounts do not survive this** — they have no login, and there
    /// is nowhere to put them. Game accounts are untouched.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("accounts", "is_verified").await? {
            return Ok(());
        }
        // PostgreSQL databases never had the dist shape: the baseline builds
        // the master-account one, and `up` only added the index.
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX IF EXISTS accounts_master_email")
                .await?;
            return Ok(());
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("accounts_old"))
                    .col(
                        ColumnDef::new(Alias::new("login"))
                            .custom(ty(manager, "VARCHAR(45)"))
                            .not_null()
                            .default(dflt(manager, "''")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password"))
                            .custom(ty(manager, "VARCHAR(45)"))
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .custom(ty(manager, "varchar(255)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_time"))
                            .custom(ty(manager, "timestamp"))
                            .not_null()
                            .default(dflt(manager, "CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastactive"))
                            .custom(ty(manager, "bigint"))
                            .not_null()
                            .default(dflt(manager, "'0'")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("accessLevel"))
                            .custom(ty(manager, "TINYINT"))
                            .not_null()
                            .default(dflt(manager, "0")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastIP"))
                            .custom(ty(manager, "CHAR(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastServer"))
                            .custom(ty(manager, "TINYINT"))
                            .null()
                            .default(dflt(manager, "1")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("pcIp"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop1"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop2"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop3"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop4"))
                            .custom(ty(manager, "char(15)"))
                            .null()
                            .default(dflt(manager, "NULL")),
                    )
                    .primary_key(Index::create().col(Alias::new("login")))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(&format!(
                "INSERT INTO `accounts_old` ({cols}) SELECT {cols} FROM `accounts` \
                 WHERE `login` IS NOT NULL",
                cols = CARRIED
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("accounts")).to_owned())
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(Alias::new("accounts_old"), Alias::new("accounts"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

/// `INSERT INTO <to> (cols) SELECT cols FROM <from>` over [`CARRIED`].
async fn copy_rows(manager: &SchemaManager<'_>, from: &str, to: &str) -> Result<(), DbErr> {
    let cols = CARRIED
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "INSERT INTO `{to}` ({cols}) SELECT {cols} FROM `{from}`"
        ))
        .await?;
    Ok(())
}
