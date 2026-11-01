//! Drop the three Interlude tables Salvation does not ship
//! (`L2J_Mobius_05.0_Salvation/dist/sql/game`): `lottery` (the Lucky Lottery),
//! `global_tasks` (Interlude's `GlobalTaskManager` stamps) and
//! `custom_teleport` (never read by the port).
//!
//! `down` recreates them empty from the baseline definitions. The rows are
//! gone; the code that read them stays on the `interlude` branch.

use sea_orm_migration::prelude::*;

use crate::m20260801_000002_baseline_game::{
    create_custom_teleport, create_global_tasks, create_lottery,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const DROPPED: &[&str] = &["lottery", "global_tasks", "custom_teleport"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in DROPPED {
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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_lottery(manager).await?;
        create_global_tasks(manager).await?;
        create_custom_teleport(manager).await?;
        Ok(())
    }
}
