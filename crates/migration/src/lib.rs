//! Database migrations — the schema's only definition.
//!
//! Running them: `l2r-migrate up` (this crate's binary) — see docs/DATABASE.md.
//!
//! The baselines were transcribed from the Java installer's per-dialect `.sql`
//! trees (`dist/db_installer`, since removed); column types came across
//! verbatim, so the schema matches the one the Java server used.
//!
//! # A property worth keeping
//!
//! **Idempotent.** Every baseline statement is `IF NOT EXISTS` and the
//! later rebuilds check for their own result first, so `up` against the live
//! production database records the migrations as applied and changes nothing.
//! That is how an existing deployment adopted this; `tests/idempotent.rs`
//! keeps a second `up` a no-op.

pub use sea_orm_migration::prelude::*;

mod m20260801_000001_baseline_login;
mod m20260801_000002_baseline_game;
mod m20260801_000003_master_accounts;
mod m20260908_000001_grandboss_real_hp;
mod m20261007_000001_ip_bans;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260801_000001_baseline_login::Migration),
            Box::new(m20260801_000002_baseline_game::Migration),
            Box::new(m20260801_000003_master_accounts::Migration),
            Box::new(m20260908_000001_grandboss_real_hp::Migration),
            Box::new(m20261007_000001_ip_bans::Migration),
        ]
    }
}
