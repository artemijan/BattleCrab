//! Port of the game-server sections of `org.l2jmobius.Config`.
//!
//! Reads the **existing** `dist/game/config/*.ini` files verbatim through the
//! shared [`PropertiesParser`](commons::config::PropertiesParser). Ported
//! incrementally: each milestone adds the ini files / keys its subsystem needs.

pub mod champion;
pub mod character;
pub mod community_board;
pub mod feature;
pub mod general;
pub mod geoengine;
pub mod hexid;
pub mod ipconfig;
pub mod npc;
pub mod offline_trade;
pub mod premium;
pub mod rates;
pub mod server;

pub use champion::ChampionConfig;
pub use character::CharacterConfig;
pub use community_board::CommunityBoardConfig;
pub use feature::FeatureConfig;
pub use general::GeneralConfig;
pub use geoengine::GeoEngineConfig;
pub use hexid::HexId;
pub use ipconfig::IpConfig;
pub use npc::NpcConfig;
pub use offline_trade::OfflineTradeConfig;
pub use premium::PremiumConfig;
pub mod grand_boss;
pub use grand_boss::GrandBossConfig;
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
    /// `GrandBoss.ini` respawn windows, read by the grand-boss lifecycle.
    pub grand_boss: GrandBossConfig,
    /// General.ini runtime keys the game thread reads (ground-item auto-destroy,
    /// …). The GM-startup keys of `GeneralConfig` travel separately via
    /// `data.gm`; this carries the rest.
    pub general: GeneralConfig,
    /// Community board (BBS) settings + the buff/teleport whitelists.
    pub community_board: CommunityBoardConfig,
    /// `Custom/PremiumSystem.ini` — the account-premium reward multipliers.
    pub premium: PremiumConfig,
    /// `Feature.ini` — the wyvern-riding gates (WyvernManager reads them).
    pub feature: FeatureConfig,
    /// `Custom/OfflineTrade.ini` — the offline-shop lifecycle.
    pub offline_trade: OfflineTradeConfig,
    /// `Custom/ChampionMonsters.ini` — the champion-monster lottery and its
    /// stat / reward multipliers.
    pub champion: ChampionConfig,
}

/// Aggregate of every loaded config section, owned for the process lifetime
/// (mirrors the giant static `Config` class, but as an owned value we pass
/// around rather than global mutable state — decision #4).
pub struct Config {
    pub server: ServerConfig,
    pub character: CharacterConfig,
    pub feature: FeatureConfig,
    pub general: GeneralConfig,
    pub geoengine: GeoEngineConfig,
    pub npc: NpcConfig,
    pub rates: RatesConfig,
    pub grand_boss: GrandBossConfig,
    pub community_board: CommunityBoardConfig,
    pub premium: PremiumConfig,
    pub offline_trade: OfflineTradeConfig,
    pub champion: ChampionConfig,

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

/// Joins a datapack-relative path from an ini file onto the datapack root.
///
/// Leading `./` is stripped first, because the shipped inis write paths as
/// `./data/geodata/` and `{root}./data/...` would only work by accident.
/// Absolute paths are returned untouched: an operator who wrote one meant it,
/// and silently reinterpreting it under the datapack would be worse than
/// ignoring the root.
pub(crate) fn datapack_path(root: &str, value: &str) -> String {
    if std::path::Path::new(value).is_absolute() {
        return value.to_string();
    }
    format!("{root}{}", value.trim_start_matches("./"))
}

impl Config {
    /// Java: `Config.load(ServerMode.GAME)`. Reads from the process working
    /// directory; `load_from` is the form that takes an explicit datapack root.
    pub fn load() -> Self {
        Self::load_from("")
    }

    /// Loads every ini under `root`, which is a **prefix** joined directly onto
    /// each file's relative path — so it must end in `/` (or be empty, meaning
    /// the working directory). Same convention as `GameData::load_from`.
    ///
    /// This is what lets the server run without chdir'ing into `dist/game`:
    /// the datapack is addressed explicitly, so paths that are *not* part of
    /// the datapack — above all the SQLite `URL`, which is shared with the
    /// login server — keep resolving against the directory the process was
    /// actually started in.
    pub fn load_from(root: &str) -> Self {
        let server = ServerConfig::load_from(root);
        let character = CharacterConfig::load_from(root);
        let feature = FeatureConfig::load_from(root);
        let general = GeneralConfig::load_from(root);
        let geoengine = GeoEngineConfig::load_from(root);
        let npc = NpcConfig::load_from(root);
        let rates = RatesConfig::load_from(root);
        let grand_boss = GrandBossConfig::load_from(root);
        let community_board = CommunityBoardConfig::load_from(root);
        let premium = PremiumConfig::load_from(root);
        let offline_trade = OfflineTradeConfig::load_from(root);
        let champion = ChampionConfig::load_from(root);
        let ip_config = IpConfig::load_from(root);
        let hex = HexId::load_from(root, server.request_id);
        Self {
            server,
            character,
            feature,
            general,
            geoengine,
            npc,
            rates,
            grand_boss,
            community_board,
            premium,
            offline_trade,
            champion,
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
            grand_boss: self.grand_boss.clone(),
            general: self.general.clone(),
            community_board: self.community_board.clone(),
            premium: self.premium.clone(),
            feature: self.feature.clone(),
            offline_trade: self.offline_trade.clone(),
            champion: self.champion.clone(),
        }
    }
}

#[cfg(test)]
mod datapack_path_tests {
    use super::datapack_path;

    #[test]
    fn joins_relative_paths_onto_the_root() {
        assert_eq!(
            datapack_path("dist/game/", "data/html"),
            "dist/game/data/html"
        );
    }

    #[test]
    fn strips_the_leading_dot_slash_the_inis_actually_use() {
        // The shipped values are written this way; `{root}./data/...` would
        // only work by accident of path normalisation.
        assert_eq!(
            datapack_path("dist/game/", "./data/geodata/"),
            "dist/game/data/geodata/"
        );
    }

    #[test]
    fn keeps_parent_traversal_so_backup_path_still_escapes_the_datapack() {
        // BackupPath is `../backup/`, i.e. dist/backup — trimming this the way
        // `./` is trimmed would silently relocate backups inside the datapack.
        assert_eq!(
            datapack_path("dist/game/", "../backup/"),
            "dist/game/../backup/"
        );
    }

    #[test]
    fn leaves_absolute_paths_alone() {
        assert_eq!(
            datapack_path("dist/game/", "/srv/l2/geodata"),
            "/srv/l2/geodata"
        );
    }

    #[test]
    fn an_empty_root_is_the_working_directory() {
        assert_eq!(datapack_path("", "./data/geodata/"), "data/geodata/");
    }
}
