//! Port of `org.l2jmobius.gameserver` — the Interlude Classic game server.
//!
//! Architecture: one game thread owns [`world::World`]; network, DB, and other
//! services run on their own threads and talk to it through channels. See
//! `docs/THREADING_MODEL.md` for the model and its trade-offs, and
//! `docs/PROJECT_LAYOUT.md` for where things live.

pub mod character;
pub mod config;
pub mod data;
pub mod db;
pub mod enums;
pub mod game_loop;
pub mod geo;
pub mod loginlink;
pub mod model;
pub mod network;
pub mod scheduler;
pub mod scripts;
pub mod session;
pub mod store;
pub mod world;
