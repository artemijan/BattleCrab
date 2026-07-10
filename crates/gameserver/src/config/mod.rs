//! Port of the game-server sections of `org.l2jmobius.Config`.
//!
//! Reads the **existing** `dist/game/config/*.ini` files verbatim through the
//! shared [`PropertiesParser`](commons::config::PropertiesParser). Ported
//! incrementally: each milestone adds the ini files / keys its subsystem needs.

pub mod character;
pub mod hexid;
pub mod server;

pub use character::CharacterConfig;
pub use hexid::HexId;
pub use server::ServerConfig;

/// Aggregate of every loaded config section, owned for the process lifetime
/// (mirrors the giant static `Config` class, but as an owned value we pass
/// around rather than global mutable state — decision #4).
pub struct Config {
    pub server: ServerConfig,
    pub character: CharacterConfig,

    /// Game-server identity on the login server (`hexid.txt` / generated).
    pub hex_id: Vec<u8>,
    pub server_id: i32,

    /// Whether the login server should reserve the GS host on login
    /// (`RESERVE_HOST_ON_LOGIN`) and whether the server is GM-only
    /// (`SERVER_GMONLY`). Stock defaults; wired to config in a later milestone.
    pub reserve_host_on_login: bool,
    pub server_gmonly: bool,
}

impl Config {
    /// Java: `Config.load(ServerMode.GAME)`.
    pub fn load() -> Self {
        let server = ServerConfig::load();
        let character = CharacterConfig::load();
        let hex = HexId::load(server.request_id);
        Self {
            server,
            character,
            hex_id: hex.hex_id,
            server_id: hex.server_id,
            reserve_host_on_login: false,
            server_gmonly: false,
        }
    }
}
