//! SeaORM entities for the shared login/game database, plus the thin
//! repositories over them.
//!
//! # What lives here
//!
//! * [`entity`] — one module per table in `dist/db_installer/sql/**`, generated
//!   from that DDL (see docs/DATABASE.md) and re-typed by
//!   `tools/normalize_entities.py`. Column names are kept verbatim
//!   (`charId`, `accessLevel`, …): the schema is shared with the Java server
//!   and is not ours to modernise.
//! * [`repo`] — table-level queries with more than one consumer. Everything
//!   there is generic over `C: ConnectionTrait` so it composes inside a
//!   transaction.
//!
//! # What does not live here
//!
//! Domain aggregates. `store_player`, the character-load bundle and their kin
//! mix game structs with a Java-parity contract, so they stay in the crate that
//! owns those structs and are written against these entities. This crate must
//! never learn what a `Player` is.

pub mod entity;
pub mod repo;
pub mod value;

/// Re-exported so consumers get one SeaORM version without depending on it
/// directly (`use models::sea_orm::EntityTrait;`).
pub use sea_orm;

pub mod prelude {
    //! The imports a typical call site needs.
    pub use crate::entity::prelude::*;
    pub use crate::value::LooseF64;
    pub use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
        ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
    };
}
