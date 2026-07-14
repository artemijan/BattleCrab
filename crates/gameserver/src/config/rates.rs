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
    /// `RatePartyXp` / `RatePartySp` — extra multiplier folded into the
    /// party-size bonus for parties of 2+ (`Party.getExpBonus`). **70** on
    /// this dist!
    pub rate_party_xp: f64,
    pub rate_party_sp: f64,

    /// `DeathDropChanceMultiplier` / `DeathDropAmountMultiplier` — the generic
    /// (non-per-item) drop multipliers for normal monsters.
    pub death_drop_chance_multiplier: f64,
    pub death_drop_amount_multiplier: f64,
    /// `DropChanceMultiplierByItemId` / `DropAmountMultiplierByItemId` —
    /// per-item overrides (the dist boosts adena ×50 chance / ×30 amount).
    pub drop_chance_by_id: HashMap<i32, f64>,
    pub drop_amount_by_id: HashMap<i32, f64>,
    /// `RaidDropChanceMultiplier` / `RaidDropAmountMultiplier` — the drop
    /// multipliers for raid/grand bosses (`Npc.isRaid()`). Currently consumed
    /// only by the `NpcViewMod` drop-list preview; the death-drop roll itself
    /// still applies the death multipliers to raids (raid loot is deferred).
    pub raid_drop_chance_multiplier: f64,
    pub raid_drop_amount_multiplier: f64,

    /// `DropMaxOccurrencesNormal` — how many sub-100%-chance drop rolls one
    /// kill can award (raid variant deferred with raids).
    pub drop_max_occurrences_normal: i32,

    /// `RateQuestDrop` — multiplies quest-kill drop chance *and* amount
    /// (`AbstractScript.giveItemRandomly`). **10** on this dist.
    pub rate_quest_drop: f64,
    /// `RateQuestReward` / `RateQuestRewardAdena` — turn-in reward
    /// multipliers (`rewardItems`; the per-EtcItem-type multipliers behind
    /// `RateQuestRewardUseMultipliers = False` are not ported). Both **10**.
    pub rate_quest_reward: f64,
    pub rate_quest_reward_adena: f64,
    /// `RateQuestRewardXP` / `RateQuestRewardSP` — quest `addExpAndSp`
    /// multipliers. Both **10**.
    pub rate_quest_reward_xp: f64,
    pub rate_quest_reward_sp: f64,

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
            rate_party_xp: 1.0,
            rate_party_sp: 1.0,
            death_drop_chance_multiplier: 1.0,
            death_drop_amount_multiplier: 1.0,
            drop_chance_by_id: HashMap::new(),
            drop_amount_by_id: HashMap::new(),
            raid_drop_chance_multiplier: 1.0,
            raid_drop_amount_multiplier: 1.0,
            drop_max_occurrences_normal: 2,
            rate_quest_drop: 1.0,
            rate_quest_reward: 1.0,
            rate_quest_reward_adena: 1.0,
            rate_quest_reward_xp: 1.0,
            rate_quest_reward_sp: 1.0,
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
            rate_party_xp: p.get_float("RatePartyXp", 1.0) as f64,
            rate_party_sp: p.get_float("RatePartySp", 1.0) as f64,
            death_drop_chance_multiplier: p.get_float("DeathDropChanceMultiplier", 1.0) as f64,
            death_drop_amount_multiplier: p.get_float("DeathDropAmountMultiplier", 1.0) as f64,
            raid_drop_chance_multiplier: p.get_float("RaidDropChanceMultiplier", 1.0) as f64,
            raid_drop_amount_multiplier: p.get_float("RaidDropAmountMultiplier", 1.0) as f64,
            drop_chance_by_id: parse_id_multiplier_list(&p.get_string("DropChanceMultiplierByItemId", "")),
            drop_amount_by_id: parse_id_multiplier_list(&p.get_string("DropAmountMultiplierByItemId", "")),
            drop_max_occurrences_normal: p.get_int("DropMaxOccurrencesNormal", d.drop_max_occurrences_normal),
            rate_quest_drop: p.get_float("RateQuestDrop", 1.0) as f64,
            rate_quest_reward: p.get_float("RateQuestReward", 1.0) as f64,
            rate_quest_reward_adena: p.get_float("RateQuestRewardAdena", 1.0) as f64,
            rate_quest_reward_xp: p.get_float("RateQuestRewardXP", 1.0) as f64,
            rate_quest_reward_sp: p.get_float("RateQuestRewardSP", 1.0) as f64,
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
