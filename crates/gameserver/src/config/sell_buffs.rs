//! `Custom/SellBuffs.ini` — the player buff shop: a character sits down and
//! sells casts of their own buffs for a configured currency.
//!
//! **Enabled on this dist** (`SellBuffEnable = True`), which is what pulls it
//! inside the ROADMAP scope gate. See `docs/PLAN_G33_CUSTOM_INI_AUDIT.md`.

use commons::config::PropertiesParser;

pub const SELL_BUFFS_CONFIG_FILE: &str = "config/Custom/SellBuffs.ini";

#[derive(Debug, Clone)]
pub struct SellBuffsConfig {
    /// `SellBuffEnable` (True here) — the master gate. Java only registers the
    /// voiced command and bypass handlers when it is on, so with it off the
    /// `.sellbuff` line is *said* rather than handled.
    pub enabled: bool,
    /// `MpCostMultipler` (1) — what a sold cast costs the **seller** in MP:
    /// the skill's `mpConsume` times this.
    pub mp_multiplier: i32,
    /// `PaymentID` (57, adena) — the currency a buyer pays in.
    pub payment_id: i32,
    /// `MinimumPrice` / `MaximumPrice` — the per-buff price bounds a seller
    /// may set (100 000 … 100 000 000 here).
    pub min_price: i64,
    pub max_price: i64,
    /// `MaxBuffs` (20) — how many entries one seller may list.
    pub max_buffs: usize,
}

impl Default for SellBuffsConfig {
    fn default() -> Self {
        // Java `Config`'s defaults for an absent file.
        Self {
            enabled: false,
            mp_multiplier: 1,
            payment_id: 57,
            min_price: 100_000,
            max_price: 100_000_000,
            max_buffs: 15,
        }
    }
}

impl SellBuffsConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, SELL_BUFFS_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enabled: p.get_bool("SellBuffEnable", d.enabled),
            mp_multiplier: p.get_int("MpCostMultipler", d.mp_multiplier),
            payment_id: p.get_int("PaymentID", d.payment_id),
            min_price: p.get_long("MinimumPrice", d.min_price),
            max_price: p.get_long("MaximumPrice", d.max_price),
            max_buffs: p.get_int("MaxBuffs", d.max_buffs as i32).max(0) as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_values_load() {
        let cfg =
            SellBuffsConfig::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(cfg.enabled, "SellBuffEnable = True");
        assert_eq!(cfg.payment_id, 57);
        assert_eq!((cfg.min_price, cfg.max_price), (100_000, 100_000_000));
        assert_eq!(cfg.max_buffs, 20, "the dist raises Java's default of 15");
    }
}
