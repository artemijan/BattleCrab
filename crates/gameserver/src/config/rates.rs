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
    /// `RateRaidbossPointsReward` — multiplies a raid boss's raid-point
    /// award. 1 on this dist.
    pub rate_raidboss_points: f64,
    /// `PetFoodRate` — multiplies what one helping of pet food restores
    /// (Java `Feed`: `normal * Config.PET_FOOD_RATE`). 1 on this dist.
    pub pet_food_rate: i32,
    /// `RateDropManor` — multiplies the manor seed/crop setup limits
    /// (`Seed.getSeedLimit`/`getCropLimit` = `_limit * RATE_DROP_MANOR`). 1 on
    /// this dist.
    pub rate_drop_manor: i32,
    /// `RatePartyXp` / `RatePartySp` — extra multiplier folded into the
    /// party-size bonus for parties of 2+ (`Party.getExpBonus`). **70** on
    /// this dist!
    pub rate_party_xp: f64,
    pub rate_party_sp: f64,

    /// `DeathDropChanceMultiplier` / `DeathDropAmountMultiplier` — the generic
    /// (non-per-item) drop multipliers for normal monsters.
    pub death_drop_chance_multiplier: f64,
    pub death_drop_amount_multiplier: f64,
    /// `SpoilDropChanceMultiplier` / `SpoilDropAmountMultiplier` — the spoil
    /// (Sweeper loot) multipliers. Java's `calculateUngroupedDrop` `SPOIL`
    /// branch seeds `rateChance`/`rateAmount` from these instead of the death
    /// multipliers; the per-item `RATE_DROP_*_BY_ID` overrides do NOT apply to
    /// spoil (only the death branch reads them).
    pub spoil_drop_chance_multiplier: f64,
    pub spoil_drop_amount_multiplier: f64,
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

    /// `RateVitalityExpMultiplier` — the exp/sp multiplier a player with any
    /// vitality points left earns (**2** on this dist).
    pub rate_vitality_exp_multiplier: f64,
    /// `RateVitalityGain` / `RateVitalityLost` — scale applied to a positive /
    /// negative vitality delta in `updateVitalityPoints` (both **1**).
    pub rate_vitality_gain: f64,
    pub rate_vitality_lost: f64,
    /// Death drops (`Player.onDieDropItem`). Two rate sets: the **player** one
    /// applies when a *monster* did the killing, the **karma** one when a
    /// playable killed a PK who is past `MinimumPKRequiredToDrop`. Each is a
    /// gate roll (`rate_drop`), per-item percentages split by inventory /
    /// equipped / equipped-weapon, and a cap on how many items may fall.
    pub player_drop_limit: i32,
    pub player_rate_drop: i32,
    pub player_rate_drop_item: i32,
    pub player_rate_drop_equip: i32,
    pub player_rate_drop_equip_weapon: i32,
    pub karma_drop_limit: i32,
    pub karma_rate_drop: i32,
    pub karma_rate_drop_item: i32,
    pub karma_rate_drop_equip: i32,
    pub karma_rate_drop_equip_weapon: i32,
    /// `PVP.ini` `MinimumPKRequiredToDrop` (Java default 4) — a PK below this
    /// many kills drops nothing to a player killer.
    pub karma_pk_limit: i32,
    /// `VitalityMaxItemsAllowed` — weekly cap on vitality-restoring item uses,
    /// reported by `ExVitalityEffectInfo` (**999**).
    pub vitality_max_items_allowed: i32,
}

impl Default for RatesConfig {
    /// Java `Config` defaults (what tests run with — notably rates ×1).
    fn default() -> Self {
        Self {
            rate_xp: 1.0,
            rate_sp: 1.0,
            rate_raidboss_points: 1.0,
            pet_food_rate: 1,
            rate_drop_manor: 1,
            rate_party_xp: 1.0,
            rate_party_sp: 1.0,
            death_drop_chance_multiplier: 1.0,
            death_drop_amount_multiplier: 1.0,
            spoil_drop_chance_multiplier: 1.0,
            spoil_drop_amount_multiplier: 1.0,
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
            rate_vitality_exp_multiplier: 2.0,
            rate_vitality_gain: 1.0,
            rate_vitality_lost: 1.0,
            vitality_max_items_allowed: 999,
            player_drop_limit: 0,
            player_rate_drop: 0,
            player_rate_drop_item: 0,
            player_rate_drop_equip: 0,
            player_rate_drop_equip_weapon: 0,
            karma_drop_limit: 0,
            karma_rate_drop: 0,
            karma_rate_drop_item: 0,
            karma_rate_drop_equip: 0,
            karma_rate_drop_equip_weapon: 0,
            karma_pk_limit: 4,
        }
    }
}

impl RatesConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, RATES_CONFIG_FILE);
        let d = Self::default();
        Self {
            rate_xp: p.get_float("RateXp", 1.0) as f64,
            rate_sp: p.get_float("RateSp", 1.0) as f64,
            rate_raidboss_points: p.get_float("RateRaidbossPointsReward", 1.0) as f64,
            pet_food_rate: p.get_int("PetFoodRate", 1),
            rate_drop_manor: p.get_int("RateDropManor", 1),
            rate_party_xp: p.get_float("RatePartyXp", 1.0) as f64,
            rate_party_sp: p.get_float("RatePartySp", 1.0) as f64,
            death_drop_chance_multiplier: p.get_float("DeathDropChanceMultiplier", 1.0) as f64,
            death_drop_amount_multiplier: p.get_float("DeathDropAmountMultiplier", 1.0) as f64,
            spoil_drop_chance_multiplier: p.get_float("SpoilDropChanceMultiplier", 1.0) as f64,
            spoil_drop_amount_multiplier: p.get_float("SpoilDropAmountMultiplier", 1.0) as f64,
            raid_drop_chance_multiplier: p.get_float("RaidDropChanceMultiplier", 1.0) as f64,
            raid_drop_amount_multiplier: p.get_float("RaidDropAmountMultiplier", 1.0) as f64,
            drop_chance_by_id: parse_id_multiplier_list(
                &p.get_string("DropChanceMultiplierByItemId", ""),
            ),
            drop_amount_by_id: parse_id_multiplier_list(
                &p.get_string("DropAmountMultiplierByItemId", ""),
            ),
            drop_max_occurrences_normal: p
                .get_int("DropMaxOccurrencesNormal", d.drop_max_occurrences_normal),
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
            rate_vitality_exp_multiplier: p.get_float(
                "RateVitalityExpMultiplier",
                d.rate_vitality_exp_multiplier as f32,
            ) as f64,
            rate_vitality_gain: p.get_float("RateVitalityGain", d.rate_vitality_gain as f32) as f64,
            rate_vitality_lost: p.get_float("RateVitalityLost", d.rate_vitality_lost as f32) as f64,
            vitality_max_items_allowed: p
                .get_int("VitalityMaxItemsAllowed", d.vitality_max_items_allowed),
            player_drop_limit: p.get_int("PlayerDropLimit", d.player_drop_limit),
            player_rate_drop: p.get_int("PlayerRateDrop", d.player_rate_drop),
            player_rate_drop_item: p.get_int("PlayerRateDropItem", d.player_rate_drop_item),
            player_rate_drop_equip: p.get_int("PlayerRateDropEquip", d.player_rate_drop_equip),
            player_rate_drop_equip_weapon: p
                .get_int("PlayerRateDropEquipWeapon", d.player_rate_drop_equip_weapon),
            karma_drop_limit: p.get_int("KarmaDropLimit", d.karma_drop_limit),
            karma_rate_drop: p.get_int("KarmaRateDrop", d.karma_rate_drop),
            karma_rate_drop_item: p.get_int("KarmaRateDropItem", d.karma_rate_drop_item),
            karma_rate_drop_equip: p.get_int("KarmaRateDropEquip", d.karma_rate_drop_equip),
            karma_rate_drop_equip_weapon: p
                .get_int("KarmaRateDropEquipWeapon", d.karma_rate_drop_equip_weapon),
            // Lives in PVP.ini, not Rates.ini.
            karma_pk_limit: PropertiesParser::load_rel(root, "config/PVP.ini")
                .get_int("MinimumPKRequiredToDrop", d.karma_pk_limit),
        }
    }
}

/// Java `Config`'s `id,mult;id,mult;…` list shape (used by both per-item drop
/// multiplier keys). Malformed entries are skipped like Java's try/catch.
pub(crate) fn parse_id_multiplier_list(raw: &str) -> HashMap<i32, f64> {
    let mut out = HashMap::new();
    for entry in raw.split(';') {
        let mut it = entry.split(',');
        if let (Some(id), Some(mult)) = (it.next(), it.next())
            && let (Ok(id), Ok(mult)) = (id.trim().parse::<i32>(), mult.trim().parse::<f64>())
        {
            out.insert(id, mult);
        }
    }
    out
}
