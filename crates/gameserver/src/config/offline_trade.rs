//! `Custom/OfflineTrade.ini` — port of the `OFFLINE_TRADE_CONFIG_FILE` block of
//! `Config.java`, read by the offline-shop lifecycle
//! (`OfflineTradeUtil` + `OfflineTraderTable`).
//!
//! An offline trader is a character whose *client* left but whose `Player`
//! stays in the world with its private store open, so the shop keeps trading.
//! Everything about that lifecycle — who may become one, whether they take
//! damage, how long the shops survive a restart — is tuned here.
//!
//! `OfflineAbnormalEffect` is parsed but ignored: it is an
//! `AbnormalVisualEffect` list and the port has no way to hold a *config-driven*
//! visual on a player with no buff behind it (`TODO(G33)`); this dist leaves it
//! empty anyway.

use commons::config::PropertiesParser;

pub const OFFLINE_TRADE_CONFIG_FILE: &str = "config/Custom/OfflineTrade.ini";

#[derive(Debug, Clone)]
pub struct OfflineTradeConfig {
    /// `OfflineTradeEnable` — sell / package-sell / buy stores may go offline.
    pub trade_enable: bool,
    /// `OfflineCraftEnable` — manufacture stores (and Java's `isCrafting()`
    /// dwarven recipe window) may go offline.
    pub craft_enable: bool,
    /// `OfflineModeInPeaceZone` — refuse offline mode outside a peace zone.
    pub mode_in_peace_zone: bool,
    /// `OfflineModeNoDamage` — an offline shop cannot be damaged.
    pub mode_no_damage: bool,
    /// `OfflineSetNameColor` / `OfflineNameColor` — recolour the name so the
    /// shop reads as unattended. Java decodes the value as hex (`0x` prefixed).
    pub set_name_color: bool,
    pub name_color: i32,
    /// `OfflineFame` — whether a detached character still earns fame.
    ///
    /// Read nowhere yet (`TODO(G33)`), because the thing it gates is unported:
    /// Java's fame source is `SiegeZone`'s fame task, and this only decides
    /// whether an *offline* shop inside such a zone keeps earning. Note the
    /// earlier claim here — "a later-chronicle stat with no ported source on
    /// Interlude" — was wrong: castle sieges are ported, so the source is
    /// reachable. It is inert only because this dist sets
    /// `CastleZoneFameAquirePoints = 0`. See `game_loop::siege`'s
    /// `update_player_siege_state_flags`.
    pub fame: bool,
    /// `RestoreOffliners` — restore the stored shops at boot.
    pub restore_offliners: bool,
    /// `OfflineMaxDays` — drop a stored shop older than this at restore.
    /// `0` disables the check (this dist).
    pub max_days: i32,
    /// `OfflineDisconnectFinished` — an offline shop that sells out (its store
    /// type drops to NONE) leaves the world instead of standing there empty.
    pub disconnect_finished: bool,
    /// `OfflineDisconnectSameAccount` — logging in on the same account kicks
    /// that account's offline shops out of the world.
    pub disconnect_same_account: bool,
    /// `StoreOfflineTradeInRealtime` — write the shop's rows after every
    /// transaction instead of only at shutdown.
    pub store_in_realtime: bool,
    /// `EnableOfflineCommand` — the `.offline` voiced command.
    pub enable_offline_command: bool,
}

impl Default for OfflineTradeConfig {
    /// Java `Config` defaults (the file absent): the feature off, but the
    /// housekeeping flags at the values `Config.java` hard-codes.
    fn default() -> Self {
        Self {
            trade_enable: false,
            craft_enable: false,
            mode_in_peace_zone: false,
            mode_no_damage: false,
            set_name_color: false,
            name_color: 0x0080_8080,
            fame: true,
            restore_offliners: false,
            max_days: 10,
            disconnect_finished: true,
            disconnect_same_account: false,
            store_in_realtime: true,
            enable_offline_command: true,
        }
    }
}

impl OfflineTradeConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, OFFLINE_TRADE_CONFIG_FILE))
    }

    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            trade_enable: p.get_bool("OfflineTradeEnable", d.trade_enable),
            craft_enable: p.get_bool("OfflineCraftEnable", d.craft_enable),
            mode_in_peace_zone: p.get_bool("OfflineModeInPeaceZone", d.mode_in_peace_zone),
            mode_no_damage: p.get_bool("OfflineModeNoDamage", d.mode_no_damage),
            set_name_color: p.get_bool("OfflineSetNameColor", d.set_name_color),
            // Java: `Integer.decode("0x" + value)` — the ini carries bare hex.
            name_color: i32::from_str_radix(p.get_string("OfflineNameColor", "808080").trim(), 16)
                .unwrap_or(d.name_color),
            fame: p.get_bool("OfflineFame", d.fame),
            restore_offliners: p.get_bool("RestoreOffliners", d.restore_offliners),
            max_days: p.get_int("OfflineMaxDays", d.max_days),
            disconnect_finished: p.get_bool("OfflineDisconnectFinished", d.disconnect_finished),
            disconnect_same_account: p
                .get_bool("OfflineDisconnectSameAccount", d.disconnect_same_account),
            store_in_realtime: p.get_bool("StoreOfflineTradeInRealtime", d.store_in_realtime),
            enable_offline_command: p.get_bool("EnableOfflineCommand", d.enable_offline_command),
        }
    }

    /// Java's shared gate: every offline-shop entry point is `(trade || craft)`.
    pub fn any_enabled(&self) -> bool {
        self.trade_enable || self.craft_enable
    }
}
