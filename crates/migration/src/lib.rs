//! Database migrations, as SeaORM Rust rather than the per-dialect `.sql` trees
//! in `dist/db_installer`.
//!
//! Running them: `l2r-migrate up` (this crate's binary) — see docs/DATABASE.md.
//!
//! # Two properties worth keeping
//!
//! 1. **Idempotent.** Every baseline statement is `IF NOT EXISTS` and the
//!    master-account rebuild checks for its own column first, so `up` against
//!    the live production database records the migrations as applied and
//!    changes nothing. That is how an existing deployment adopts this.
//! 2. **Faithful.** Column types come across from the dist DDL verbatim, and
//!    `tests/dist_parity.rs` compares the migrated schema against that DDL
//!    column by column. The dist tree is authoritative; if the two disagree,
//!    the migration is wrong.

pub use sea_orm_migration::prelude::*;

mod m20260801_000001_baseline_login;
mod m20260801_000002_baseline_game;
mod m20260801_000003_master_accounts;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260801_000001_baseline_login::Migration),
            Box::new(m20260801_000002_baseline_game::Migration),
            Box::new(m20260801_000003_master_accounts::Migration),
        ]
    }
}
