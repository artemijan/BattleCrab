//! `Custom/AutoPotions.ini` — the `.apon` / `.apoff` self-healing loop: once a
//! second, top the player up from their own potions when HP/CP/MP drops below a
//! threshold.
//!
//! **Enabled on this dist** (`AutoPotionsEnabled = True`), which is what pulls
//! it inside the ROADMAP scope gate. See `PLAN_G33_CUSTOM_INI_AUDIT.md`.

use commons::config::PropertiesParser;

pub const AUTO_POTIONS_CONFIG_FILE: &str = "config/Custom/AutoPotions.ini";

/// One of the three pools the loop watches.
#[derive(Debug, Clone, Default)]
pub struct AutoPotionPool {
    pub enabled: bool,
    /// Below this percentage of the pool's maximum, drink.
    pub percentage: i32,
    /// Candidate item ids **in order**: Java takes the first one the player
    /// actually carries, so the list is a preference ranking.
    pub item_ids: Vec<i32>,
}

impl AutoPotionPool {
    fn from_parser(p: &PropertiesParser, enable_key: &str, pct_key: &str, ids_key: &str) -> Self {
        Self {
            enabled: p.get_bool(enable_key, false),
            percentage: p.get_int(pct_key, 0),
            item_ids: p
                .get_string(ids_key, "")
                .split(',')
                .filter_map(|id| id.trim().parse().ok())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AutoPotionsConfig {
    /// `AutoPotionsEnabled` — the master gate; Java registers the voiced
    /// command only when it is on.
    pub enabled: bool,
    /// `AutoPotionsInOlympiad` (**false** here) — otherwise a player in a match
    /// is dropped from the loop rather than merely skipped.
    pub in_olympiad: bool,
    /// `AutoPotionMinimumLevel` (1) — checked when the command is *typed*, not
    /// on the tick, so a lower-level character who somehow joined keeps going.
    pub minimum_level: i32,
    pub cp: AutoPotionPool,
    pub hp: AutoPotionPool,
    pub mp: AutoPotionPool,
}

impl AutoPotionsConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, AUTO_POTIONS_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        Self {
            enabled: p.get_bool("AutoPotionsEnabled", false),
            in_olympiad: p.get_bool("AutoPotionsInOlympiad", false),
            minimum_level: p.get_int("AutoPotionMinimumLevel", 1),
            cp: AutoPotionPool::from_parser(
                p,
                "AutoCpEnabled",
                "AutoCpPercentage",
                "AutoCpItemIds",
            ),
            hp: AutoPotionPool::from_parser(
                p,
                "AutoHpEnabled",
                "AutoHpPercentage",
                "AutoHpItemIds",
            ),
            mp: AutoPotionPool::from_parser(
                p,
                "AutoMpEnabled",
                "AutoMpPercentage",
                "AutoMpItemIds",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_values_load() {
        let cfg =
            AutoPotionsConfig::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(cfg.enabled, "AutoPotionsEnabled = True");
        assert!(!cfg.in_olympiad, "and off during a match");
        assert_eq!(cfg.minimum_level, 1);
        assert!(cfg.hp.enabled && cfg.cp.enabled && cfg.mp.enabled);
        assert_eq!(cfg.hp.percentage, 70);
        assert_eq!(cfg.mp.percentage, 30);
        // The order is a preference ranking, so it has to survive the parse.
        assert_eq!(cfg.hp.item_ids, vec![1540, 1539, 1061, 1060]);
        assert_eq!(cfg.cp.item_ids, vec![5592, 5591]);
        assert_eq!(cfg.mp.item_ids, vec![728]);
    }
}
