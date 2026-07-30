//! Baseline: the four login-server tables.
//!
//! Transcribed from `dist/db_installer/sql/sqlite/**` by
//! `tools/gen_migrations.py` — do not hand-edit. Column types are passed
//! through verbatim (`MEDIUMINT`, `TINYINT`, …) so the schema matches the one
//! the Java installer produces; `crates/migration/tests/dist_parity.rs` proves
//! it column by column.
//!
//! Every statement is `IF NOT EXISTS`, which is what lets `l2r-migrate up`
//! adopt the live production database: it records the migration as applied
//! without touching a single existing table.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Dropped in reverse order by `down`.
const TABLES: &[&str] = &["accounts", "account_data", "accounts_ipauth", "gameservers"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_accounts(manager).await?;
        create_account_data(manager).await?;
        create_accounts_ipauth(manager).await?;
        create_gameservers(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in TABLES.iter().rev() {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(*table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

/// `accounts`
async fn create_accounts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("accounts"))
                .if_not_exists()
                .col(
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
    manager
        .get_connection()
        .execute_unprepared("CREATE UNIQUE INDEX IF NOT EXISTS `accounts_master_email` ON `accounts` (`email` COLLATE NOCASE) WHERE `login` IS NULL")
        .await?;
    Ok(())
}

/// `account_data`
async fn create_account_data(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("account_data"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("account_name"))
                        .custom(Alias::new("VARCHAR(45)"))
                        .not_null()
                        .default(Expr::cust("''")),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(Alias::new("VARCHAR(20)"))
                        .not_null()
                        .default(Expr::cust("''")),
                )
                .col(
                    ColumnDef::new(Alias::new("value"))
                        .custom(Alias::new("VARCHAR(255)"))
                        .null(),
                )
                .primary_key(
                    Index::create()
                        .col(Alias::new("account_name"))
                        .col(Alias::new("var")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `accounts_ipauth`
async fn create_accounts_ipauth(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("accounts_ipauth"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("login"))
                        .custom(Alias::new("varchar(45)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("ip"))
                        .custom(Alias::new("char(15)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(Alias::new("varchar(10)"))
                        .null()
                        .default(Expr::cust("'allow'")),
                )
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// `gameservers`
async fn create_gameservers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new("gameservers"))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("server_id"))
                        .custom(Alias::new("INT"))
                        .not_null()
                        .default(Expr::cust("'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("hexid"))
                        .custom(Alias::new("varchar(50)"))
                        .not_null()
                        .default(Expr::cust("''")),
                )
                .col(
                    ColumnDef::new(Alias::new("host"))
                        .custom(Alias::new("varchar(50)"))
                        .not_null()
                        .default(Expr::cust("''")),
                )
                .primary_key(Index::create().col(Alias::new("server_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}
