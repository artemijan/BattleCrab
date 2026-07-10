//! Port of the game-server sections of `org.l2jmobius.Config`.
//!
//! Reads the **existing** `dist/game/config/*.ini` files verbatim through the
//! shared [`PropertiesParser`](commons::config::PropertiesParser). Ported
//! incrementally: each milestone adds the ini files / keys its subsystem needs.
//! G0 covers `Server.ini` (boot, database, network binding, restart schedule).

pub mod server;

pub use server::ServerConfig;

/// Aggregate of every loaded config section, owned for the process lifetime
/// (mirrors the giant static `Config` class, but as an owned value we pass
/// around rather than global mutable state — decision #4).
pub struct Config {
    pub server: ServerConfig,
}

impl Config {
    /// Java: `Config.load(ServerMode.GAME)`.
    pub fn load() -> Self {
        Self { server: ServerConfig::load() }
    }
}
