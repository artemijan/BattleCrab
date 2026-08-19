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
    /// `RateKarmaLost` — the divisor on experience *earned* when working karma
    /// off (`Formulas.calculateKarmaLost`). **`-1` means "use `RateXp`"**, which
    /// is what this dist ships, and Java resolves that at load time rather than
    /// at each use — so this field never holds -1.
    pub rate_karma_lost: f64,
    /// `RateRaidbossPointsReward` — multiplies a raid boss's raid-point
    /// award. 1 on this dist.
    pub rate_raidboss_points: f64,
    /// `RateSiegeGuardsPrice` — multiplies the price of a `CASTLE_GUARD`
    /// item (the mercenary tickets) wherever it appears on a buy list
    /// (Java `Product.getPrice`). **1 on this dist**, so the multiply is
    /// numerically inert — carried anyway because it is the only thing
    /// standing between a server that raises it and a free garrison.
    pub rate_siege_guards_price: f64,
    /// `RateKarmaExpLost` — scales the **exp a PK loses on death** (Java
    /// multiplies `percentLost` by it when `getReputation() < 0`). 1 here.
    pub rate_karma_exp_lost: f64,
    /// `PetXpRate` / `SinEaterXpRate` — multiply what a pet earns. Java picks
    /// the Sin Eater rate for that pet and `PetXpRate` for the rest. Both 1.
    pub pet_xp_rate: f64,
    pub sin_eater_xp_rate: f64,
    /// `RateInstanceXp` / `RateInstanceSp` / `RateInstancePartyXp`. **-1 is a
    /// sentinel**, not a multiplier: Java reads it as "use `RateXp`/`RateSp`",
    /// which is what this dist ships, so instances run at the ordinary rate.
    pub rate_instance_xp: f64,
    pub rate_instance_sp: f64,
    pub rate_instance_party_xp: f64,
    /// `RateExtractable` — the extractable-item yield multiplier. 1 here.
    pub rate_extractable: f64,
    /// `UseQuestRewardMultipliers` — **False** here, and it gates the four
    /// per-type rates below entirely: with it off, quest rewards use the flat
    /// `RateQuestReward` instead and the type split never runs.
    pub use_quest_reward_multipliers: bool,
    /// `RateQuestRewardPotion` / `Scroll` / `Recipe` / `Material`, all 1 and
    /// all unreachable while the flag above is off.
    pub rate_quest_reward_potion: f64,
    pub rate_quest_reward_scroll: f64,
    pub rate_quest_reward_recipe: f64,
    pub rate_quest_reward_material: f64,
    /// `HerbDropAmountMultiplier` / `HerbDropChanceMultiplier` — applied to
    /// herb drop groups at NPC-template load. Both 1.
    pub herb_drop_amount_multiplier: f64,
    pub herb_drop_chance_multiplier: f64,
    /// `DropMaxOccurrencesRaidboss` — how many times one raid-boss drop group
    /// may roll. 1 here.
    pub drop_max_occurrences_raidboss: i32,
    /// `EventItemMaxLevelDifference` — the level gap past which an event drop
    /// is withheld. **9**, the one non-neutral value in this block, and inert
    /// only because no event drop is configured.
    pub event_item_max_level_difference: i32,
    /// `BossDropEnable` — **False**; with it off the three keys below never
    /// apply. They describe an extra drop injected into every boss in a level
    /// band at `NpcData` load.
    pub boss_drop_enable: bool,
    pub boss_drop_min_level: i32,
    pub boss_drop_max_level: i32,
    /// `BossDropList` — `itemId,min,max,chance;…`.
    pub boss_drop_list: Vec<(i32, i64, i64, f64)>,
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
    /// `PVP.ini` `ListOfNonDroppableItems` — item ids a dying PK never
    /// scatters, whatever the rates say. Kept **sorted**, as Java sorts it for
    /// the `Arrays.binarySearch` the drop filter uses. Lives here with the
    /// other death-drop settings rather than in `config::pvp`, following
    /// `karma_pk_limit`: PVP.ini's keys are split by consumer.
    pub karma_nondroppable_items: Vec<i32>,
    /// `PVP.ini` `ListOfPetItems` — the same list for pet gear (collars,
    /// armour, food), checked separately by Java and merged nowhere.
    pub karma_nondroppable_pet_items: Vec<i32>,
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
            rate_karma_lost: 1.0,
            rate_raidboss_points: 1.0,
            rate_siege_guards_price: 1.0,
            rate_karma_exp_lost: 1.0,
            pet_xp_rate: 1.0,
            sin_eater_xp_rate: 1.0,
            rate_instance_xp: -1.0,
            rate_instance_sp: -1.0,
            rate_instance_party_xp: -1.0,
            rate_extractable: 1.0,
            use_quest_reward_multipliers: false,
            rate_quest_reward_potion: 1.0,
            rate_quest_reward_scroll: 1.0,
            rate_quest_reward_recipe: 1.0,
            rate_quest_reward_material: 1.0,
            herb_drop_amount_multiplier: 1.0,
            herb_drop_chance_multiplier: 1.0,
            drop_max_occurrences_raidboss: 1,
            event_item_max_level_difference: 9,
            boss_drop_enable: false,
            boss_drop_min_level: 40,
            boss_drop_max_level: 999,
            boss_drop_list: vec![(4356, 1, 2, 100.0)],
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
            karma_nondroppable_items: Vec::new(),
            karma_nondroppable_pet_items: Vec::new(),
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
            rate_karma_lost: {
                // `RATE_KARMA_LOST = getFloat("RateKarmaLost", -1); if (== -1)
                // RATE_KARMA_LOST = RATE_XP;`
                let raw = p.get_float("RateKarmaLost", -1.0) as f64;
                if raw == -1.0 {
                    p.get_float("RateXp", 1.0) as f64
                } else {
                    raw
                }
            },
            rate_raidboss_points: p.get_float("RateRaidbossPointsReward", 1.0) as f64,
            rate_siege_guards_price: p.get_float("RateSiegeGuardsPrice", 1.0) as f64,
            rate_karma_exp_lost: f64::from(p.get_float("RateKarmaExpLost", 1.0)),
            pet_xp_rate: f64::from(p.get_float("PetXpRate", 1.0)),
            sin_eater_xp_rate: f64::from(p.get_float("SinEaterXpRate", 1.0)),
            rate_instance_xp: f64::from(p.get_float("RateInstanceXp", -1.0)),
            rate_instance_sp: f64::from(p.get_float("RateInstanceSp", -1.0)),
            rate_instance_party_xp: f64::from(p.get_float("RateInstancePartyXp", -1.0)),
            rate_extractable: f64::from(p.get_float("RateExtractable", 1.0)),
            use_quest_reward_multipliers: p.get_bool("UseQuestRewardMultipliers", false),
            rate_quest_reward_potion: f64::from(p.get_float("RateQuestRewardPotion", 1.0)),
            rate_quest_reward_scroll: f64::from(p.get_float("RateQuestRewardScroll", 1.0)),
            rate_quest_reward_recipe: f64::from(p.get_float("RateQuestRewardRecipe", 1.0)),
            rate_quest_reward_material: f64::from(p.get_float("RateQuestRewardMaterial", 1.0)),
            herb_drop_amount_multiplier: f64::from(p.get_float("HerbDropAmountMultiplier", 1.0)),
            herb_drop_chance_multiplier: f64::from(p.get_float("HerbDropChanceMultiplier", 1.0)),
            drop_max_occurrences_raidboss: p.get_int("DropMaxOccurrencesRaidboss", 1),
            event_item_max_level_difference: p.get_int("EventItemMaxLevelDifference", 9),
            boss_drop_enable: p.get_bool("BossDropEnable", false),
            boss_drop_min_level: p.get_int("BossDropMinLevel", 40),
            boss_drop_max_level: p.get_int("BossDropMaxLevel", 999),
            boss_drop_list: p
                .get_string("BossDropList", "")
                .split(';')
                .filter_map(|e| {
                    let f: Vec<&str> = e.split(',').map(str::trim).collect();
                    match f[..] {
                        [id, lo, hi, ch] => Some((
                            id.parse().ok()?,
                            lo.parse().ok()?,
                            hi.parse().ok()?,
                            ch.parse().ok()?,
                        )),
                        _ => None,
                    }
                })
                .collect(),
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
            karma_nondroppable_items: sorted_ids(
                &PropertiesParser::load_rel(root, "config/PVP.ini").get_string(
                    "ListOfNonDroppableItems",
                    "57,1147,425,1146,461,10,2368,7,6,2370,2369,6842,6611,6612,6613,6614,6615,6616,6617,6618,6619,6620,6621,7694,8181,5575,7694,9388,9389,9390",
                ),
            ),
            karma_nondroppable_pet_items: sorted_ids(
                &PropertiesParser::load_rel(root, "config/PVP.ini").get_string(
                    "ListOfPetItems",
                    "2375,3500,3501,3502,4422,4423,4424,4425,6648,6649,6650,9882",
                ),
            ),
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

/// A comma-separated item-id list, parsed and **sorted** — Java stores these
/// two as `int[]` and sorts them so the drop filter can binary-search.
/// Unparseable entries are skipped rather than failing the load; Java would
/// throw and lose the whole config, which is a worse answer for a stray space.
fn sorted_ids(raw: &str) -> Vec<i32> {
    let mut ids: Vec<i32> = raw
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

#[cfg(test)]
mod row14_tests {
    use super::*;

    /// The block added for row 14 claims specific shipped values; hold it to
    /// them. Every one is neutral or disabled, which is *why* wiring them
    /// changes nothing today — and exactly why a drift would go unnoticed.
    #[test]
    fn the_rates_block_matches_the_shipped_ini() {
        let r = RatesConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(r.rate_karma_exp_lost, 1.0);
        assert_eq!(r.pet_xp_rate, 1.0);
        assert_eq!(r.sin_eater_xp_rate, 1.0);
        // -1 is Java's "use RateXp/RateSp" sentinel, not a multiplier.
        assert_eq!(r.rate_instance_xp, -1.0);
        assert_eq!(r.rate_instance_sp, -1.0);
        assert_eq!(r.rate_instance_party_xp, -1.0);
        assert!(
            !r.use_quest_reward_multipliers,
            "the four per-type quest rates are gated off"
        );
        assert!(!r.boss_drop_enable, "the boss-drop injection is off");
        assert_eq!(
            r.boss_drop_list,
            vec![(4356, 1, 2, 100.0)],
            "parsed even though disabled"
        );
        assert_eq!(r.event_item_max_level_difference, 9);
    }

    /// The two `PVP.ini` drop lists that live here with `karma_pk_limit`:
    /// parsed, sorted for the binary search the drop filter does, and holding
    /// the shipped ids.
    #[test]
    fn the_non_droppable_lists_are_parsed_and_sorted() {
        let r = RatesConfig::load_from(crate::data::DIST_GAME);
        assert!(
            r.karma_nondroppable_items.windows(2).all(|w| w[0] <= w[1]),
            "sorted, as `Arrays.sort` leaves Java's copy"
        );
        assert!(
            r.karma_nondroppable_pet_items
                .windows(2)
                .all(|w| w[0] <= w[1])
        );
        // Adena, the Interlude hero weapons and the hero circlet.
        for id in [57, 6611, 6621, 6842] {
            assert!(
                r.karma_nondroppable_items.binary_search(&id).is_ok(),
                "item {id} is on the non-droppable list"
            );
        }
        // The wolf collar and the baby-pet collars.
        for id in [2375, 6648, 6650] {
            assert!(
                r.karma_nondroppable_pet_items.binary_search(&id).is_ok(),
                "pet item {id} is on the pet list"
            );
        }
        assert!(
            !r.karma_nondroppable_items.contains(&1),
            "…and an ordinary Short Sword is not"
        );
    }
}
