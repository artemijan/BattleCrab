//! `Rates.ini` — port of the `RATES_CONFIG_FILE` block of `Config.java`,
//! scoped to the keys the G9 combat/reward slice consumes.

use std::collections::HashMap;

use commons::config::PropertiesParser;

pub const RATES_CONFIG_FILE: &str = "config/Rates.ini";

#[derive(Debug, Clone)]
pub struct RatesConfig {
    /// `RateXp` / `RateSp` — multiply every NPC's template exp/sp reward.
    pub rate_xp: f64,
    pub rate_sp: f64,

    /// `DeathDropChanceMultiplier` / `DeathDropAmountMultiplier` — the generic
    /// (non-per-item) drop multipliers for normal monsters.
    pub death_drop_chance_multiplier: f64,
    pub death_drop_amount_multiplier: f64,
    /// `DropChanceMultiplierByItemId` / `DropAmountMultiplierByItemId` —
    /// per-item overrides (the dist boosts adena ×50 chance / ×30 amount).
    pub drop_chance_by_id: HashMap<i32, f64>,
    pub drop_amount_by_id: HashMap<i32, f64>,

    /// `DropMaxOccurrencesNormal` — how many sub-100%-chance drop rolls one
    /// kill can award (raid variant deferred with raids).
    pub drop_max_occurrences_normal: i32,

    /// The level-gap gates: full drop chance while `mobLevel - playerLevel >=
    /// -minDiff`, scaling linearly down to `minGapChance`% at `-maxDiff`.
    pub drop_adena_min_level_difference: i32,
    pub drop_adena_max_level_difference: i32,
    pub drop_adena_min_level_gap_chance: f64,
    pub drop_item_min_level_difference: i32,
    pub drop_item_max_level_difference: i32,
    pub drop_item_min_level_gap_chance: f64,
}

impl Default for RatesConfig {
    /// Java `Config` defaults (what tests run with — notably rates ×1).
    fn default() -> Self {
        Self {
            rate_xp: 1.0,
            rate_sp: 1.0,
            death_drop_chance_multiplier: 1.0,
            death_drop_amount_multiplier: 1.0,
            drop_chance_by_id: HashMap::new(),
            drop_amount_by_id: HashMap::new(),
            drop_max_occurrences_normal: 2,
            drop_adena_min_level_difference: 8,
            drop_adena_max_level_difference: 15,
            drop_adena_min_level_gap_chance: 10.0,
            drop_item_min_level_difference: 5,
            drop_item_max_level_difference: 10,
            drop_item_min_level_gap_chance: 10.0,
        }
    }
}

impl RatesConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(RATES_CONFIG_FILE);
        let d = Self::default();
        Self {
            rate_xp: p.get_float("RateXp", 1.0) as f64,
            rate_sp: p.get_float("RateSp", 1.0) as f64,
            death_drop_chance_multiplier: p.get_float("DeathDropChanceMultiplier", 1.0) as f64,
            death_drop_amount_multiplier: p.get_float("DeathDropAmountMultiplier", 1.0) as f64,
            drop_chance_by_id: parse_id_multiplier_list(&p.get_string("DropChanceMultiplierByItemId", "")),
            drop_amount_by_id: parse_id_multiplier_list(&p.get_string("DropAmountMultiplierByItemId", "")),
            drop_max_occurrences_normal: p.get_int("DropMaxOccurrencesNormal", d.drop_max_occurrences_normal),
            drop_adena_min_level_difference: p.get_int("DropAdenaMinLevelDifference", 8),
            drop_adena_max_level_difference: p.get_int("DropAdenaMaxLevelDifference", 15),
            drop_adena_min_level_gap_chance: p.get_float("DropAdenaMinLevelGapChance", 10.0) as f64,
            drop_item_min_level_difference: p.get_int("DropItemMinLevelDifference", 5),
            drop_item_max_level_difference: p.get_int("DropItemMaxLevelDifference", 10),
            drop_item_min_level_gap_chance: p.get_float("DropItemMinLevelGapChance", 10.0) as f64,
        }
    }
}

/// Java `Config`'s `id,mult;id,mult;…` list shape (used by both per-item drop
/// multiplier keys). Malformed entries are skipped like Java's try/catch.
fn parse_id_multiplier_list(raw: &str) -> HashMap<i32, f64> {
    let mut out = HashMap::new();
    for entry in raw.split(';') {
        let mut it = entry.split(',');
        if let (Some(id), Some(mult)) = (it.next(), it.next()) {
            if let (Ok(id), Ok(mult)) = (id.trim().parse::<i32>(), mult.trim().parse::<f64>()) {
                out.insert(id, mult);
            }
        }
    }
    out
}
