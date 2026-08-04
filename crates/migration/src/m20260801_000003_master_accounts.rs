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
use sea_orm_migration::sea_orm::ConnectionTrait;

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

/// One master account per address. A partial, collated unique index —
/// neither `WHERE` nor `COLLATE` is expressible through sea-query's index
/// builder, so this one statement is raw SQL.
const MASTER_EMAIL_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS `accounts_master_email` \
     ON `accounts` (`email` COLLATE NOCASE) WHERE `login` IS NULL";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_column("accounts", "is_verified").await? {
            // Already the master-account shape (the live DB, or a re-run).
            // Still make sure the index is there: the hand-applied SQL and this
            // migration must converge on the same schema.
            manager
                .get_connection()
                .execute_unprepared(MASTER_EMAIL_INDEX)
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
                            .custom(Alias::new("VARCHAR(45)"))
                            .null()
                            .default(Expr::cust("NULL"))
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password"))
                            .custom(Alias::new("VARCHAR(45)"))
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .custom(Alias::new("varchar(255)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        // NULL on a game account, 0/1 on a master account —
                        // the three-state column the dashboard reads.
                        ColumnDef::new(Alias::new("is_verified"))
                            .custom(Alias::new("TINYINT"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_time"))
                            .custom(Alias::new("timestamp"))
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastactive"))
                            .custom(Alias::new("bigint"))
                            .not_null()
                            .default(Expr::cust("'0'")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("accessLevel"))
                            .custom(Alias::new("TINYINT"))
                            .not_null()
                            .default(Expr::cust("0")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastIP"))
                            .custom(Alias::new("CHAR(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastServer"))
                            .custom(Alias::new("TINYINT"))
                            .null()
                            .default(Expr::cust("1")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("pcIp"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop1"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop2"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop3"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop4"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
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
            .execute_unprepared(MASTER_EMAIL_INDEX)
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
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("accounts_old"))
                    .col(
                        ColumnDef::new(Alias::new("login"))
                            .custom(Alias::new("VARCHAR(45)"))
                            .not_null()
                            .default(Expr::cust("''")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password"))
                            .custom(Alias::new("VARCHAR(45)"))
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .custom(Alias::new("varchar(255)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_time"))
                            .custom(Alias::new("timestamp"))
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastactive"))
                            .custom(Alias::new("bigint"))
                            .not_null()
                            .default(Expr::cust("'0'")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("accessLevel"))
                            .custom(Alias::new("TINYINT"))
                            .not_null()
                            .default(Expr::cust("0")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastIP"))
                            .custom(Alias::new("CHAR(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("lastServer"))
                            .custom(Alias::new("TINYINT"))
                            .null()
                            .default(Expr::cust("1")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("pcIp"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop1"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop2"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop3"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("hop4"))
                            .custom(Alias::new("char(15)"))
                            .null()
                            .default(Expr::cust("NULL")),
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
