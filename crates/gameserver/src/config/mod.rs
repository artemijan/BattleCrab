//! Port of the game-server sections of `org.l2jmobius.Config`.
//!
//! Reads the **existing** `dist/game/config/*.ini` files verbatim through the
//! shared [`PropertiesParser`](commons::config::PropertiesParser). Ported
//! incrementally: each milestone adds the ini files / keys its subsystem needs.

pub mod character;
pub mod general;
pub mod geoengine;
pub mod hexid;
pub mod ipconfig;
pub mod npc;
pub mod rates;
pub mod server;

pub use character::CharacterConfig;
pub use general::GeneralConfig;
pub use geoengine::GeoEngineConfig;
pub use hexid::HexId;
pub use ipconfig::IpConfig;
pub use npc::NpcConfig;
pub use rates::RatesConfig;
pub use server::ServerConfig;

/// The config keys the combat/AI/reward systems read at runtime, bundled so
/// they travel to the game thread as one value (`World.cfg`). Tests get Java
/// defaults via `Default` (notably ×1 rates).
#[derive(Debug, Clone, Default)]
pub struct CombatConfig {
    pub character: CharacterConfig,
    pub npc: NpcConfig,
    pub rates: RatesConfig,
    /// General.ini runtime keys the game thread reads (ground-item auto-destroy,
    /// …). The GM-startup keys of `GeneralConfig` travel separately via
    /// `data.gm`; this carries the rest.
    pub general: GeneralConfig,
}

/// Aggregate of every loaded config section, owned for the process lifetime
/// (mirrors the giant static `Config` class, but as an owned value we pass
/// around rather than global mutable state — decision #4).
pub struct Config {
    pub server: ServerConfig,
    pub character: CharacterConfig,
    pub general: GeneralConfig,
    pub geoengine: GeoEngineConfig,
    pub npc: NpcConfig,
    pub rates: RatesConfig,

    /// Network (subnet, host) pairs advertised to the login server.
    pub ip_config: IpConfig,

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
        let general = GeneralConfig::load();
        let geoengine = GeoEngineConfig::load();
        let npc = NpcConfig::load();
        let rates = RatesConfig::load();
        let ip_config = IpConfig::load();
        let hex = HexId::load(server.request_id);
        Self {
            server,
            character,
            general,
            geoengine,
            npc,
            rates,
            ip_config,
            hex_id: hex.hex_id,
            server_id: hex.server_id,
            reserve_host_on_login: false,
            server_gmonly: false,
        }
    }

    /// The runtime bundle the game thread keeps on `World`.
    pub fn combat(&self) -> CombatConfig {
        CombatConfig {
            character: self.character.clone(),
            npc: self.npc.clone(),
            rates: self.rates.clone(),
            general: self.general.clone(),
        }
    }
}
