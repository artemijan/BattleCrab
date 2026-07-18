//! Community board (BBS) config — the `EnableCommunityBoard`/`BBSDefault`
//! keys of `General.ini` plus the whole `config/Custom/CommunityBoard.ini`
//! block that `Config.java` loads (`CUSTOM_CB_ENABLED`, prices, enable flags,
//! `COMMUNITY_AVAILABLE_BUFFS`, `COMMUNITY_AVAILABLE_TELEPORTS`).
//!
//! This dist runs `CustomCommunityBoard = True`, so the live board is the
//! custom one (navigation → buffer / heal / teleport / multisell / premium).
//! The retail boards (`_bbsloc`/`_bbsclan`/…) are unreachable under the custom
//! navigation and are not ported yet (`TODO(G30)`).
//!
//! `available_teleports` is derived exactly like Java's
//! `loadCommunityBoardAllowedTeleports`: scan the gatekeeper htmls for
//! `action="bypass _bbsteleport;<x> <y> <z>"` and use that `<x> <y> <z>`
//! string as both the bypass key and the destination. Java parses the html
//! with jsoup; a plain substring scan is enough for the fixed button markup.

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
    /// [`crate::game_loop::admin::premium::PREMIUM_SYSTEM_ENABLED`]).
    pub community_premium_system: bool,
    /// `CommunityPremiumBuyCoinId`: item id charged when buying premium (dist 57).
    pub premium_coin_id: i32,
    /// `CommunityPremiumPricePerDay`: coin cost per day of premium (dist 1000000).
    pub premium_price_per_day: i64,

    /// `CommunityAvailableBuffs`: the skill-id whitelist the `_bbsbuff`/scheme
    /// handlers honor (anti-exploit — a bypass naming any other skill is
    /// silently skipped).
    pub available_buffs: HashSet<i32>,
    /// `_bbsteleport;<coords>` → destination, scanned from the gatekeeper
    /// htmls. Key is the raw `"<x> <y> <z>"` string carried by the bypass.
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
    /// relative to the game cwd, then scans the gatekeeper htmls under
    /// `data/html/CommunityBoard/...` for the teleport whitelist — the same
    /// relative paths Java uses in `Config.load`.
    pub fn load() -> Self {
        let general = PropertiesParser::load(GENERAL_CONFIG_FILE);
        let cb = PropertiesParser::load(CUSTOM_COMMUNITY_BOARD_CONFIG_FILE);
        let mut cfg = Self::from_parsers(&general, &cb);
        cfg.available_teleports = scan_available_teleports(cfg.custom_enabled, "data/html");
        cfg
    }

    /// Split out so tests can point at the real dist ini files without a cwd
    /// dependency. Does **not** populate [`Self::available_teleports`] (that
    /// needs the html root — see [`scan_available_teleports`]).
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
            community_premium_system: cb.get_bool("CommunityPremiumSystem", d.community_premium_system),
            premium_coin_id: cb.get_int("CommunityPremiumBuyCoinId", d.premium_coin_id),
            premium_price_per_day: cb.get_long("CommunityPremiumPricePerDay", d.premium_price_per_day),
            available_buffs: cb
                .get_string("CommunityAvailableBuffs", "")
                .split(',')
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect(),
            available_teleports: HashMap::new(),
        }
    }
}

/// Port of `Config.loadCommunityBoardAllowedTeleports`: collect every
/// `bypass _bbsteleport;<x> <y> <z>` action string in the board's teleport
/// htmls. The scanned dir mirrors Java — the custom board reads the
/// `Custom/gatekeeper` tree, the retail board the flat `CommunityBoard` dir.
/// `html_root` is the `data/html` path (relative at boot, absolute in tests).
pub fn scan_available_teleports(
    custom_enabled: bool,
    html_root: &str,
) -> HashMap<String, (i32, i32, i32)> {
    let dir = if custom_enabled {
        format!("{html_root}/CommunityBoard/Custom/gatekeeper")
    } else {
        format!("{html_root}/CommunityBoard")
    };
    let mut out = HashMap::new();
    collect_teleports_in_dir(std::path::Path::new(&dir), &mut out);
    out
}

fn collect_teleports_in_dir(dir: &std::path::Path, out: &mut HashMap<String, (i32, i32, i32)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_teleports_in_dir(&path, out); // e.g. the `teleports/` subtree
        } else if path.extension().is_some_and(|e| e == "html" || e == "htm") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                extract_teleports(&text, out);
            }
        }
    }
}

/// Pull every `_bbsteleport;<x> <y> <z>` occurrence out of one html body.
/// Java keys the map on the raw `<x> <y> <z>` string (with the single spaces),
/// so we do too — that is exactly what the bypass carries back.
fn extract_teleports(html: &str, out: &mut HashMap<String, (i32, i32, i32)>) {
    for (idx, _) in html.match_indices("_bbsteleport;") {
        let rest = &html[idx + "_bbsteleport;".len()..];
        // The action value ends at the closing quote / whitespace-delimited
        // token boundary used in the button markup.
        let end = rest.find(['"', '\'']).unwrap_or(rest.len());
        let key = rest[..end].trim();
        let coords: Vec<&str> = key.split_whitespace().collect();
        if coords.len() >= 3 {
            if let (Ok(x), Ok(y), Ok(z)) = (
                coords[0].parse::<i32>(),
                coords[1].parse::<i32>(),
                coords[2].parse::<i32>(),
            ) {
                out.insert(key.to_string(), (x, y, z));
            }
        }
    }
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
        assert!(c.available_buffs.contains(&1204), "Wind Walk (1204) whitelisted");
        assert!(c.available_buffs.contains(&1085), "Acumen (1085) whitelisted");
    }

    #[test]
    fn scans_gatekeeper_teleports_from_dist() {
        let tps = scan_available_teleports(true, &dist("data/html"));
        assert!(!tps.is_empty(), "gatekeeper htmls yield teleport destinations");
        // A known Giran gatekeeper entry from the dist markup.
        assert_eq!(
            tps.get("207320 87617 -1112"),
            Some(&(207320, 87617, -1112)),
            "the `207320 87617 -1112` button parses to its coords"
        );
    }
}
