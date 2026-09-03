//! `Custom/OfflineTrade.ini` — port of the `OFFLINE_TRADE_CONFIG_FILE` block of
//! `Config.java`, read by the offline-shop lifecycle
//! (`OfflineTradeUtil` + `OfflineTraderTable`).
//!
//! An offline trader is a character whose *client* left but whose `Player`
//! stays in the world with its private store open, so the shop keeps trading.
//! Everything about that lifecycle — who may become one, whether they take
//! damage, how long the shops survive a restart — is tuned here.
//!
//! `OfflineAbnormalEffect` marks the shop with a visual effect. Java picks
//! **one at random** from the list per trader, not all of them, so a populated
//! list gives a street of shops a mix of markers. This dist leaves it empty.

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
    /// `OfflineAbnormalEffect` — abnormal visual effect names to mark an
    /// unattended shop with, resolved to client ids at load. Java picks one at
    /// random per trader (`Rnd.get(size())`), so this is a palette, not a set
    /// to apply together. Unknown names are dropped at parse rather than
    /// failing the load — a typo in a cosmetic list must not stop the server.
    pub abnormal_effects: Vec<i16>,
    /// `OfflineFame` — whether a detached character still earns fame.
    ///
    /// Read by `siege::handle_siege_fame`, the port of Java's `FameTask`: a
    /// detached shop standing in an active siege zone keeps earning unless
    /// this is off. Inert on this dist, which sets
    /// `CastleZoneFameAquirePoints = 0` — the task runs and pays nothing.
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
            abnormal_effects: Vec::new(),
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
            abnormal_effects: p
                .get_string("OfflineAbnormalEffect", "")
                .split(&[',', ';'][..])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .filter_map(crate::model::skill::abnormal::abnormal_visual_client_id)
                .collect(),
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
