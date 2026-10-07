//! Baseline: the four login-server tables.
//!
//! Transcribed from the Java installer's SQLite DDL (`dist/db_installer`, since
//! removed). Column types are passed through verbatim (`MEDIUMINT`, `TINYINT`,
//! …) on SQLite, so the schema matches the one the Java installer produced; on
//! PostgreSQL `crate::dialect` maps each to the type its entity reads. Applied
//! databases depend on it: change the schema with a new migration, not here.
//!
//! Every statement is `IF NOT EXISTS`, which is what lets `l2r-migrate up`
//! adopt the live production database: it records the migration as applied
//! without touching a single existing table.

use sea_orm_migration::prelude::*;

use crate::dialect::{dflt, master_email_index, ty};
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

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
///
/// The entity's key is `rowid`. SQLite gives every table one implicitly;
/// PostgreSQL has none, so there it is a real `SERIAL` column.
async fn create_accounts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    if manager.get_database_backend() == DatabaseBackend::Postgres {
        table.col(
            ColumnDef::new(Alias::new("rowid"))
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        );
    }
    manager
        .create_table(
            table
                .table(Alias::new("accounts"))
                .if_not_exists()
                .col(
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
    manager
        .get_connection()
        .execute_unprepared(master_email_index(manager))
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
                        .custom(ty(manager, "VARCHAR(45)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("var"))
                        .custom(ty(manager, "VARCHAR(20)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("value"))
                        .custom(ty(manager, "VARCHAR(255)"))
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
                        .custom(ty(manager, "varchar(45)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("ip"))
                        .custom(ty(manager, "char(15)"))
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("type"))
                        .custom(ty(manager, "varchar(10)"))
                        .null()
                        .default(dflt(manager, "'allow'")),
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
                        .custom(ty(manager, "INT"))
                        .not_null()
                        .default(dflt(manager, "'0'")),
                )
                .col(
                    ColumnDef::new(Alias::new("hexid"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .col(
                    ColumnDef::new(Alias::new("host"))
                        .custom(ty(manager, "varchar(50)"))
                        .not_null()
                        .default(dflt(manager, "''")),
                )
                .primary_key(Index::create().col(Alias::new("server_id")))
                .to_owned(),
        )
        .await?;
    Ok(())
}
