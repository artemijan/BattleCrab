//! Port of `org.l2jmobius.gameserver` — the Interlude Classic game server.
//!
//! Architecture: one game thread owns [`world::World`]; network, DB, and other
//! services run on their own threads and talk to it through channels. See
//! `docs/CONCURRENCY_MODEL.md` and `docs/PLAN_GAME_SERVER.md`.

pub mod config;
pub mod game_loop;
pub mod loginlink;
pub mod network;
pub mod scheduler;
pub mod session;
pub mod world;
