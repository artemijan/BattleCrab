//! Community board (BBS) config — the `EnableCommunityBoard`/`BBSDefault`
//! keys of `General.ini` plus the whole `config/Custom/CommunityBoard.ini`
//! block that `Config.java` loads (`CUSTOM_CB_ENABLED`, prices, enable flags,
//! `COMMUNITY_AVAILABLE_BUFFS`, `COMMUNITY_AVAILABLE_TELEPORTS`).
//!
//! This dist runs `CustomCommunityBoard = True`, so the live board is the
//! custom one (navigation → buffer / heal / teleport / multisell / premium).
//! The retail forum boards (`_bbsloc`/`_bbsclan`/…) are unreachable under the
//! custom navigation and stay deferred — see `game_loop::community_board`'s
//! module header.
//!
//! `available_teleports` is the `CommunityTeleportList` key: destinations by
//! name, the name being what the `_bbsteleport;<name>` bypass carries.

use std::collections::{HashMap, HashSet};

use commons::config::PropertiesParser;

use super::general::GENERAL_CONFIG_FILE;

pub const CUSTOM_COMMUNITY_BOARD_CONFIG_FILE: &str = "config/Custom/CommunityBoard.ini";

/// Everything the community board handlers read at runtime. Travels to the
/// game thread on `World.cfg` (see [`crate::config::CombatConfig`]).
#[derive(Debug, Clone)]
pub struct CommunityBoardConfig {
    /// `General.ini` `EnableCommunityBoard`: the master switch. When off the
    /// board answers `THE_COMMUNITY_SERVER_IS_CURRENTLY_OFFLINE`.
    pub enabled: bool,
    /// `General.ini` `BBSDefault`: the command the plain board button
    /// (`RequestShowBoard`) opens. Dist ships `_bbshome`.
    pub bbs_default: String,
    /// `CustomCommunityBoard`: use the custom navigation board vs. the retail
    /// forum board. Dist ships `True` → the html path prefix becomes `Custom/`.
    pub custom_enabled: bool,

    /// `CommunityCurrencyId`: item id charged for board actions (dist 57 adena).
    pub currency_id: i32,
    pub enable_multisells: bool,
    pub enable_teleports: bool,
    pub enable_buffs: bool,
    pub enable_heal: bool,
    pub enable_delevel: bool,

    pub teleport_price: i64,
    pub buff_price: i64,
    pub heal_price: i64,
    pub delevel_price: i64,

    /// `CommunityKarmaDisabled`: refuse the board to players with negative
    /// reputation (karma).
    pub karma_disabled: bool,
    /// `CommunityCastAnimations`: play the buff cast animation to the caster.
    pub cast_animations: bool,

    /// `CommunityPremiumSystem`: gates the `_bbspremium` buy action. Java also
    /// requires the global `Config.PREMIUM_SYSTEM_ENABLED` (see
    /// [`crate::game_loop::admin::premium::premium_system_enabled`]).
    pub community_premium_system: bool,
    /// `CommunityPremiumBuyCoinId`: item id charged when buying premium (dist 57).
    pub premium_coin_id: i32,
    /// `CommunityPremiumPricePerDay`: coin cost per day of premium (dist 1000000).
    pub premium_price_per_day: i64,

    /// `CommunityAvailableBuffs`: the skill-id whitelist the `_bbsbuff`/scheme
    /// handlers honor (anti-exploit — a bypass naming any other skill is
    /// silently skipped).
    pub available_buffs: HashSet<i32>,
    /// `CommunityTeleportList`: `_bbsteleport;<name>` → destination.
    pub available_teleports: HashMap<String, (i32, i32, i32)>,
}

impl Default for CommunityBoardConfig {
    fn default() -> Self {
        // Java `Config`/`PropertiesParser` fallbacks. `enabled`/`custom` off so
        // tests that don't opt in behave like a vanilla (non-custom) server.
        Self {
            enabled: false,
            bbs_default: "_bbshome".to_string(),
            custom_enabled: false,
            currency_id: 57,
            enable_multisells: true,
            enable_teleports: true,
            enable_buffs: true,
            enable_heal: true,
            enable_delevel: false,
            teleport_price: 0,
            buff_price: 0,
            heal_price: 0,
            delevel_price: 0,
            karma_disabled: true,
            cast_animations: false,
            community_premium_system: true,
            premium_coin_id: 57,
            premium_price_per_day: 1_000_000,
            available_buffs: HashSet::new(),
            available_teleports: HashMap::new(),
        }
    }
}

impl CommunityBoardConfig {
    /// Boot load: reads `General.ini` + `config/Custom/CommunityBoard.ini`
    /// relative to the game cwd — the same relative paths Java uses.
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let general = PropertiesParser::load_rel(root, GENERAL_CONFIG_FILE);
        let cb = PropertiesParser::load_rel(root, CUSTOM_COMMUNITY_BOARD_CONFIG_FILE);
        Self::from_parsers(&general, &cb)
    }

    /// Split out so tests can point at the real dist ini files without a cwd
    /// dependency.
    pub fn from_parsers(general: &PropertiesParser, cb: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enabled: general.get_bool("EnableCommunityBoard", d.enabled),
            bbs_default: general.get_string("BBSDefault", &d.bbs_default),
            custom_enabled: cb.get_bool("CustomCommunityBoard", d.custom_enabled),
            currency_id: cb.get_int("CommunityCurrencyId", d.currency_id),
            enable_multisells: cb.get_bool("CommunityEnableMultisells", d.enable_multisells),
            enable_teleports: cb.get_bool("CommunityEnableTeleports", d.enable_teleports),
            enable_buffs: cb.get_bool("CommunityEnableBuffs", d.enable_buffs),
            enable_heal: cb.get_bool("CommunityEnableHeal", d.enable_heal),
            enable_delevel: cb.get_bool("CommunityEnableDelevel", d.enable_delevel),
            teleport_price: cb.get_long("CommunityTeleportPrice", d.teleport_price),
            buff_price: cb.get_long("CommunityBuffPrice", d.buff_price),
            heal_price: cb.get_long("CommunityHealPrice", d.heal_price),
            delevel_price: cb.get_long("CommunityDelevelPrice", d.delevel_price),
            karma_disabled: cb.get_bool("CommunityKarmaDisabled", d.karma_disabled),
            cast_animations: cb.get_bool("CommunityCastAnimations", d.cast_animations),
            community_premium_system: cb
                .get_bool("CommunityPremiumSystem", d.community_premium_system),
            premium_coin_id: cb.get_int("CommunityPremiumBuyCoinId", d.premium_coin_id),
            premium_price_per_day: cb
                .get_long("CommunityPremiumPricePerDay", d.premium_price_per_day),
            available_buffs: cb
                .get_string("CommunityAvailableBuffs", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            available_teleports: parse_teleport_list(&cb.get_string("CommunityTeleportList", "")),
        }
    }
}

/// `CommunityTeleportList` — `Name,x,y,z;Name,x,y,z…`, keyed by the name the
/// `_bbsteleport;<name>` bypass carries back (Java `CommunityBoardConfig`).
/// Java's `parseInt` would abort the whole config on a malformed entry; this
/// skips that one entry instead.
fn parse_teleport_list(raw: &str) -> HashMap<String, (i32, i32, i32)> {
    raw.split(';')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.split(',').collect();
            let [name, x, y, z] = parts[..] else {
                return None;
            };
            Some((
                name.to_string(),
                (
                    x.trim().parse().ok()?,
                    y.trim().parse().ok()?,
                    z.trim().parse().ok()?,
                ),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(path: &str) -> String {
        format!("{}/../../dist/game/{path}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn loads_dist_community_board_values() {
        let general = PropertiesParser::load(dist("config/General.ini"));
        let cb = PropertiesParser::load(dist("config/Custom/CommunityBoard.ini"));
        let c = CommunityBoardConfig::from_parsers(&general, &cb);
        assert!(c.enabled, "EnableCommunityBoard=True");
        assert_eq!(c.bbs_default, "_bbshome", "BBSDefault=_bbshome");
        assert!(c.custom_enabled, "CustomCommunityBoard=True");
        assert_eq!(c.currency_id, 57, "CommunityCurrencyId=57");
        assert!(c.enable_heal, "CommunityEnableHeal=True");
        assert!(!c.enable_delevel, "CommunityEnableDelevel=False");
        assert!(c.karma_disabled, "CommunityKarmaDisabled=True");
        // A couple of the whitelisted buff ids from the dist list.
        assert_eq!(
            c.available_buffs.len(),
            10,
            "the ten 15642-15653 community buffs"
        );
        assert!(c.available_buffs.contains(&15642));
    }

    #[test]
    fn reads_the_teleport_list_by_name() {
        let general = PropertiesParser::load(dist("config/General.ini"));
        let cb = PropertiesParser::load(dist("config/Custom/CommunityBoard.ini"));
        let tps = CommunityBoardConfig::from_parsers(&general, &cb).available_teleports;
        // The first entry of the `\`-continued list.
        assert_eq!(tps.get("Giran"), Some(&(82698, 148638, -3473)));
        assert_eq!(tps.get("Aden"), Some(&(147450, 27064, -2208)));
    }
}
