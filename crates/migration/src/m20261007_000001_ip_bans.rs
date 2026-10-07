//! `ip_bans`: addresses the servers refuse, managed from the dashboard's Audit
//! page (`docs/MONITORING.md` §10). It replaces the boot-time `banned_ip.cfg`.
//! The login server reads it on every new connection, so a change needs no
//! reload. `IF NOT EXISTS`, like the baselines, so a re-run is a no-op.

use sea_orm_migration::prelude::*;

use crate::dialect::{dflt, ty};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ip_bans"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("ip"))
                            .custom(ty(manager, "VARCHAR(45)"))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("expires_at"))
                            .custom(ty(manager, "bigint"))
                            .not_null()
                            .default(dflt(manager, "'0'")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("reason"))
                            .custom(ty(manager, "VARCHAR(255)"))
                            .not_null()
                            .default(dflt(manager, "''")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("banned_by"))
                            .custom(ty(manager, "VARCHAR(255)"))
                            .not_null()
                            .default(dflt(manager, "''")),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .custom(ty(manager, "bigint"))
                            .not_null()
                            .default(dflt(manager, "'0'")),
                    )
                    .primary_key(Index::create().col(Alias::new("ip")))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("ip_bans"))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
