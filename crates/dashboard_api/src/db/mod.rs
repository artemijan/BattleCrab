//! Data access against the live game SQLite DB — one pool, two tables.
//!
//! `accounts` is writable in exactly two columns; `characters` is read-only.
//! See PLAN_DASHBOARD.md §5.5.

pub mod accounts;
pub mod characters;
