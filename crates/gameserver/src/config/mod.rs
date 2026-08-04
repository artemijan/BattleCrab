//! Port of the game-server sections of `org.l2jmobius.Config`.
//!
//! Reads the **existing** `dist/game/config/*.ini` files verbatim through the
//! shared [`PropertiesParser`](commons::config::PropertiesParser). Ported
//! incrementally: each milestone adds the ini files / keys its subsystem needs.

pub mod auto_play;
pub mod auto_potions;
pub mod bot_report;
pub mod champion;
pub mod character;
pub mod chat_filter;
pub mod community_board;
pub mod custom_misc;
pub mod custom_pvp;
pub mod dualbox;
pub mod feature;
pub mod flood_protector;
pub mod general;
pub mod geoengine;
pub mod hexid;
pub mod ipconfig;
pub mod npc;
pub mod offline_trade;
pub mod premium;
pub mod rates;
pub mod security;
pub mod sell_buffs;
pub mod server;

pub use auto_play::AutoPlayConfig;
pub use auto_potions::AutoPotionsConfig;
pub use bot_report::BotReportConfig;
pub use champion::ChampionConfig;
pub use character::CharacterConfig;
pub use chat_filter::ChatFilterConfig;
pub use community_board::CommunityBoardConfig;
pub use custom_misc::{
    AllowedRacesConfig, BankingConfig, BossAnnouncementsConfig, CustomMailConfig, CustomMiscConfig,
};
pub use custom_pvp::{CustomNpcConfig, PvpRewardConfig, PvpTitleColorConfig, RandomSpawnsConfig};
pub use dualbox::DualboxConfig;
pub use feature::FeatureConfig;
pub use flood_protector::FloodProtectorsConfig;
pub use general::GeneralConfig;
pub use geoengine::GeoEngineConfig;
pub use hexid::HexId;
pub use ipconfig::IpConfig;
pub use npc::NpcConfig;
pub use offline_trade::OfflineTradeConfig;
pub use premium::PremiumConfig;
pub use sell_buffs::SellBuffsConfig;
pub mod grand_boss;
pub use grand_boss::GrandBossConfig;
pub use rates::RatesConfig;
pub use security::SecurityConfig;
pub use server::ServerConfig;

/// The config keys the combat/AI/reward systems read at runtime, bundled so
/// they travel to the game thread as one value (`World.cfg`). Tests get Java
/// defaults via `Default` (notably ×1 rates).
#[derive(Debug, Clone, Default)]
pub struct CombatConfig {
    /// `Server.ini`. Carried here so the game loop can reach the scheduled
    /// restart settings; the network half is read once at boot in `main`.
    pub server: ServerConfig,
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
    /// `Custom/DualboxCheck.ini` — the per-IP participation caps.
    pub dualbox: DualboxConfig,
    /// `Custom/ChampionMonsters.ini` — the champion-monster lottery and its
    /// stat / reward multipliers.
    pub champion: ChampionConfig,
    /// `Custom/Banking.ini` — the adena ↔ goldbar voiced commands.
    pub banking: BankingConfig,
    /// `Custom/BossAnnouncements.ini` — the server-wide boss spawn lines.
    pub boss_announcements: BossAnnouncementsConfig,
    /// `Custom/OnlineInfo.ini` + `PrivateStoreRange.ini` + `WalkerBotProtection.ini`.
    pub custom_misc: CustomMiscConfig,
    /// `Custom/AllowedPlayerRaces.ini` — read by character creation, which runs
    /// on the game thread like everything else.
    pub allowed_races: AllowedRacesConfig,
    /// `Custom/PvpRewardItem.ini` — the per-kill item payout.
    pub pvp_reward: PvpRewardConfig,
    /// `Custom/PvpTitleColor.ini` — the PvP-count title/colour ladder.
    pub pvp_title_color: PvpTitleColorConfig,
    /// `Custom/RandomSpawns.ini` — the monster spawn-point jitter.
    pub random_spawns: RandomSpawnsConfig,
    /// `Custom/ChatModeration.ini` + `Custom/NoblessMaster.ini`.
    pub custom_npc: CustomNpcConfig,
    /// `Custom/SellBuffs.ini` — the player buff shop.
    pub sell_buffs: SellBuffsConfig,
    /// `Custom/AutoPotions.ini` — the `.apon` self-healing loop.
    pub auto_potions: AutoPotionsConfig,
    /// `Custom/CustomMailManager.ini` — the inbound mail table poll.
    pub custom_mail: CustomMailConfig,
    /// `Custom/AutoPlay.ini` — the `.play` auto-hunt panel and loops.
    pub auto_play: AutoPlayConfig,
    /// `FloodProtector.ini` — the per-client packet rate limits, read by the
    /// dispatch gate.
    pub flood_protector: FloodProtectorsConfig,
    /// The `Say2` filters — `chatfilter.txt` + `BanChatChannels`.
    pub chat_filter: ChatFilterConfig,
    /// `General.ini`'s bot-report block + `BotReportPunishments.xml`.
    pub bot_report: BotReportConfig,
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
    pub dualbox: DualboxConfig,
    pub champion: ChampionConfig,
    pub banking: BankingConfig,
    pub boss_announcements: BossAnnouncementsConfig,
    pub custom_misc: CustomMiscConfig,
    pub allowed_races: AllowedRacesConfig,
    pub pvp_reward: PvpRewardConfig,
    pub pvp_title_color: PvpTitleColorConfig,
    pub random_spawns: RandomSpawnsConfig,
    pub custom_npc: CustomNpcConfig,
    pub sell_buffs: SellBuffsConfig,
    pub auto_potions: AutoPotionsConfig,
    pub custom_mail: CustomMailConfig,
    pub auto_play: AutoPlayConfig,
    pub flood_protector: FloodProtectorsConfig,
    /// `Security.ini` — transport-level flood limits (no Java counterpart).
    pub security: SecurityConfig,
    pub chat_filter: ChatFilterConfig,
    pub bot_report: BotReportConfig,

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
        let dualbox = DualboxConfig::load_from(root);
        let champion = ChampionConfig::load_from(root);
        let banking = BankingConfig::load_from(root);
        let boss_announcements = BossAnnouncementsConfig::load_from(root);
        let custom_misc = CustomMiscConfig::load_from(root);
        let allowed_races = AllowedRacesConfig::load_from(root);
        let pvp_reward = PvpRewardConfig::load_from(root);
        let pvp_title_color = PvpTitleColorConfig::load_from(root);
        let random_spawns = RandomSpawnsConfig::load_from(root);
        let custom_npc = CustomNpcConfig::load_from(root);
        let sell_buffs = SellBuffsConfig::load_from(root);
        let auto_potions = AutoPotionsConfig::load_from(root);
        let custom_mail = CustomMailConfig::load_from(root);
        let auto_play = AutoPlayConfig::load_from(root);
        let flood_protector = FloodProtectorsConfig::load_from(root);
        let security = SecurityConfig::load_from(root);
        let chat_filter = ChatFilterConfig::load_from(root);
        let bot_report = BotReportConfig::load_from(root);
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
            dualbox,
            champion,
            banking,
            boss_announcements,
            custom_misc,
            allowed_races,
            pvp_reward,
            pvp_title_color,
            random_spawns,
            custom_npc,
            sell_buffs,
            auto_potions,
            custom_mail,
            auto_play,
            flood_protector,
            security,
            chat_filter,
            bot_report,
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
            server: ServerConfig::default(),
            character: self.character.clone(),
            npc: self.npc.clone(),
            rates: self.rates.clone(),
            grand_boss: self.grand_boss.clone(),
            general: self.general.clone(),
            community_board: self.community_board.clone(),
            premium: self.premium.clone(),
            feature: self.feature.clone(),
            offline_trade: self.offline_trade.clone(),
            dualbox: self.dualbox.clone(),
            champion: self.champion.clone(),
            banking: self.banking.clone(),
            boss_announcements: self.boss_announcements.clone(),
            custom_misc: self.custom_misc.clone(),
            allowed_races: self.allowed_races.clone(),
            pvp_reward: self.pvp_reward.clone(),
            pvp_title_color: self.pvp_title_color.clone(),
            random_spawns: self.random_spawns.clone(),
            custom_npc: self.custom_npc.clone(),
            sell_buffs: self.sell_buffs.clone(),
            auto_potions: self.auto_potions.clone(),
            custom_mail: self.custom_mail.clone(),
            auto_play: self.auto_play.clone(),
            flood_protector: self.flood_protector.clone(),
            chat_filter: self.chat_filter.clone(),
            bot_report: self.bot_report.clone(),
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
