//! `Feature.ini` — the residence-feature keys the port reads. Only the wyvern
//! riding gates so far (the siege calendar and clan-hall data come from their
//! own sources); the rest of the file lands with the subsystems that read it.

use commons::config::PropertiesParser;

pub const FEATURE_CONFIG_FILE: &str = "config/Feature.ini";

#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// `AllowRideWyvernAlways` — ride wyvern ignoring the Seven Signs Seal of
    /// Strife. **False** on this dist, which means every *castle* wyvern
    /// manager serves the Dusk-block page (`wyvernmanager-dusk.html`) and only
    /// a clan-hall manager can hand out wyverns — exactly Java's behavior
    /// with Seven Signs removed from the codebase but the flag kept.
    pub allow_ride_wyvern_always: bool,
    /// `AllowRideWyvernDuringSiege` — when false, the manager refuses while
    /// its residence's siege is active or the player is inside an active siege
    /// zone. **True** on this dist.
    pub allow_ride_wyvern_during_siege: bool,
    /// `AllowRideMountsDuringSiege` (**False** here) — the strider/wolf
    /// equivalent, read by `Player.mount` (which refuses inside a live siege
    /// zone) and by `SiegeZone.onEnter` (which dismounts / untransforms a rider
    /// who walks in).
    pub allow_ride_mounts_during_siege: bool,
    /// `BuyTaxForNeutralSide` (15 here) — the percent a castle takes off every
    /// taxed purchase made inside its tax zone while it is `NEUTRAL`.
    /// Java `Config.CASTLE_BUY_TAX_NEUTRAL`, read by `Castle.getTaxPercent`.
    pub castle_buy_tax_neutral: i32,
    /// `BuyTaxForLightSide` (0 here).
    pub castle_buy_tax_light: i32,
    /// `BuyTaxForDarkSide` (30 here).
    pub castle_buy_tax_dark: i32,
    /// `SellTaxForNeutralSide` (15 here). `TaxType.SELL` has **no consumer** in
    /// this Java build (nothing calls `getTaxPercent(SELL)`), so the sell keys
    /// are parsed for completeness and drive nothing — same as Java.
    pub castle_sell_tax_neutral: i32,
    /// `SellTaxForLightSide` (0 here).
    pub castle_sell_tax_light: i32,
    /// `SellTaxForDarkSide` (30 here).
    pub castle_sell_tax_dark: i32,
    /// `CompleteAcademyMinPoints` (190) / `CompleteAcademyMaxPoints` (650) —
    /// the clan-reputation reward for graduating an academy member, scaled by
    /// the level they *joined* at: max at ≤16, min at ≥39, and
    /// `max - (level - 16) * 20` in between (Java `Player.setClassId`).
    pub complete_academy_min_points: i32,
    pub complete_academy_max_points: i32,
    /// `HeroPoints` (1000) — the clan reputation a clan of level ≥ 3 earns when
    /// one of its members claims hero status (Java `Hero.claimHero`).
    pub hero_points: i32,
    /// The castle-function fees (`Castle<X>FunctionFeeRatio` / `…FeeLvl1/2`,
    /// Java `CS_*_FEE*`): rental cost per period, per function level. Ratios
    /// are in milliseconds (the dist ships 7 days for every castle function).
    pub cs_tele_fee_ratio: i64,
    pub cs_tele_fee: [i64; 2],
    pub cs_support_fee_ratio: i64,
    pub cs_support_fee: [i64; 2],
    pub cs_mpreg_fee_ratio: i64,
    pub cs_mpreg_fee: [i64; 2],
    pub cs_hpreg_fee_ratio: i64,
    pub cs_hpreg_fee: [i64; 2],
    pub cs_expreg_fee_ratio: i64,
    pub cs_expreg_fee: [i64; 2],
    /// Door/wall upgrade prices (`OuterDoorUpgradePriceLvlN` …): indexed
    /// `[type-1][slot]` where the slots hold the level 2 / 3 / 5 prices.
    pub door_upgrade_price: [[i64; 3]; 3],
    /// `TrapUpgradePriceLvlN` — the flame-tower (damage-zone) upgrade prices.
    pub trap_upgrade_price: [i64; 4],

    // --- the clan-reputation economy -------------------------------------
    /// `SiegeHourList` — the hours a castle owner may pick for their siege.
    pub siege_hour_list: Vec<u32>,
    /// `TakeCastlePoints` — reputation the captor gains (`Castle
    /// .updateClansReputation`). Capped by what the former owner *had* when
    /// there was one, which is Java's `min(TAKE, maxreward)`.
    pub take_castle_points: i32,
    /// `CastleDefendedPoints` — reputation for holding your own castle.
    pub castle_defended_points: i32,
    /// `LooseCastlePoints` — reputation the former owner loses.
    pub loose_castle_points: i32,
    /// `LevelUp{20And25..81Plus}ReputationScore` — clan reputation granted per
    /// level a member gains, by the level they reached. **Every band is 0 on
    /// this dist**, so the whole grant is inert here; the bands are carried so
    /// raising one works.
    pub level_up_reputation: [i32; 13],
    /// `LevelObtainedReputationScoreMultiplier` — applied to the band total,
    /// rounded **up** (Java `Math.ceil`).
    pub level_obtained_reputation_multiplier: f64,
    /// `ReputationScorePerKill` — moved between clans on a mutual-war kill.
    pub reputation_score_per_kill: i32,
    /// `CreateRoyalGuardCost` / `CreateKnightUnitCost` /
    /// `ReinforceKnightUnitCost` — sub-pledge reputation prices.
    pub create_royal_guard_cost: i32,
    pub create_knight_unit_cost: i32,
    pub reinforce_knight_unit_cost: i32,
    /// `FortressBloodOathCount` — Blood Oaths paid per fortress owned. Read by
    /// `Clan` in Java, but forts do not exist on this dist, so nothing ever
    /// asks.
    pub fortress_blood_oath_count: i32,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        // Java's config defaults (== this dist's ini values).
        Self {
            allow_ride_wyvern_always: false,
            allow_ride_wyvern_during_siege: true,
            allow_ride_mounts_during_siege: false,
            castle_buy_tax_neutral: 15,
            castle_buy_tax_light: 0,
            castle_buy_tax_dark: 30,
            castle_sell_tax_neutral: 15,
            castle_sell_tax_light: 0,
            castle_sell_tax_dark: 30,
            complete_academy_min_points: 190,
            complete_academy_max_points: 650,
            hero_points: 1000,
            cs_tele_fee_ratio: 604_800_000,
            cs_tele_fee: [1000, 10000],
            cs_support_fee_ratio: 604_800_000,
            cs_support_fee: [49000, 120_000],
            cs_mpreg_fee_ratio: 604_800_000,
            cs_mpreg_fee: [45000, 65000],
            cs_hpreg_fee_ratio: 604_800_000,
            cs_hpreg_fee: [12000, 20000],
            cs_expreg_fee_ratio: 604_800_000,
            cs_expreg_fee: [63000, 70000],
            door_upgrade_price: [
                [3_000_000, 4_000_000, 5_000_000],
                [750_000, 900_000, 1_000_000],
                [1_600_000, 1_800_000, 2_000_000],
            ],
            trap_upgrade_price: [3_000_000, 4_000_000, 5_000_000, 6_000_000],
            siege_hour_list: vec![16, 20],
            take_castle_points: 1500,
            castle_defended_points: 750,
            loose_castle_points: 3000,
            level_up_reputation: [0; 13],
            level_obtained_reputation_multiplier: 1.0,
            reputation_score_per_kill: 1,
            create_royal_guard_cost: 5000,
            create_knight_unit_cost: 10_000,
            reinforce_knight_unit_cost: 5000,
            fortress_blood_oath_count: 1,
        }
    }
}

/// The `LevelUp…ReputationScore` bands, as `(min_level, max_level)` in Java's
/// order. `level_up_reputation[i]` is the score for band `i`.
pub const REPUTATION_LEVEL_BANDS: [(i32, i32); 13] = [
    (20, 25),
    (26, 30),
    (31, 35),
    (36, 40),
    (41, 45),
    (46, 50),
    (51, 55),
    (56, 60),
    (61, 65),
    (66, 70),
    (71, 75),
    (76, 80),
    (81, 120),
];

impl FeatureConfig {
    /// The reputation one level *at* `level` is worth, before the multiplier.
    /// 0 outside every band — including below 20, which is most of the newbie
    /// game.
    pub fn reputation_for_level(&self, level: i32) -> i32 {
        REPUTATION_LEVEL_BANDS
            .iter()
            .position(|&(lo, hi)| (lo..=hi).contains(&level))
            .map_or(0, |i| self.level_up_reputation[i])
    }
}

impl FeatureConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, FEATURE_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            allow_ride_wyvern_always: p
                .get_bool("AllowRideWyvernAlways", d.allow_ride_wyvern_always),
            allow_ride_wyvern_during_siege: p.get_bool(
                "AllowRideWyvernDuringSiege",
                d.allow_ride_wyvern_during_siege,
            ),
            allow_ride_mounts_during_siege: p.get_bool(
                "AllowRideMountsDuringSiege",
                d.allow_ride_mounts_during_siege,
            ),
            castle_buy_tax_neutral: p.get_int("BuyTaxForNeutralSide", d.castle_buy_tax_neutral),
            castle_buy_tax_light: p.get_int("BuyTaxForLightSide", d.castle_buy_tax_light),
            castle_buy_tax_dark: p.get_int("BuyTaxForDarkSide", d.castle_buy_tax_dark),
            hero_points: p.get_int("HeroPoints", d.hero_points),
            complete_academy_min_points: p
                .get_int("CompleteAcademyMinPoints", d.complete_academy_min_points),
            complete_academy_max_points: p
                .get_int("CompleteAcademyMaxPoints", d.complete_academy_max_points),
            castle_sell_tax_neutral: p.get_int("SellTaxForNeutralSide", d.castle_sell_tax_neutral),
            castle_sell_tax_light: p.get_int("SellTaxForLightSide", d.castle_sell_tax_light),
            castle_sell_tax_dark: p.get_int("SellTaxForDarkSide", d.castle_sell_tax_dark),
            cs_tele_fee_ratio: p.get_int("CastleTeleportFunctionFeeRatio", 604_800_000) as i64,
            cs_tele_fee: [
                p.get_int("CastleTeleportFunctionFeeLvl1", 1000) as i64,
                p.get_int("CastleTeleportFunctionFeeLvl2", 10000) as i64,
            ],
            cs_support_fee_ratio: p.get_int("CastleSupportFunctionFeeRatio", 604_800_000) as i64,
            cs_support_fee: [
                p.get_int("CastleSupportFeeLvl1", 49000) as i64,
                p.get_int("CastleSupportFeeLvl2", 120_000) as i64,
            ],
            cs_mpreg_fee_ratio: p.get_int("CastleMpRegenerationFunctionFeeRatio", 604_800_000)
                as i64,
            cs_mpreg_fee: [
                p.get_int("CastleMpRegenerationFeeLvl1", 45000) as i64,
                p.get_int("CastleMpRegenerationFeeLvl2", 65000) as i64,
            ],
            cs_hpreg_fee_ratio: p.get_int("CastleHpRegenerationFunctionFeeRatio", 604_800_000)
                as i64,
            cs_hpreg_fee: [
                p.get_int("CastleHpRegenerationFeeLvl1", 12000) as i64,
                p.get_int("CastleHpRegenerationFeeLvl2", 20000) as i64,
            ],
            cs_expreg_fee_ratio: p.get_int("CastleExpRegenerationFunctionFeeRatio", 604_800_000)
                as i64,
            cs_expreg_fee: [
                p.get_int("CastleExpRegenerationFeeLvl1", 63000) as i64,
                p.get_int("CastleExpRegenerationFeeLvl2", 70000) as i64,
            ],
            door_upgrade_price: [
                [
                    p.get_int("OuterDoorUpgradePriceLvl2", 3_000_000) as i64,
                    p.get_int("OuterDoorUpgradePriceLvl3", 4_000_000) as i64,
                    p.get_int("OuterDoorUpgradePriceLvl5", 5_000_000) as i64,
                ],
                [
                    p.get_int("InnerDoorUpgradePriceLvl2", 750_000) as i64,
                    p.get_int("InnerDoorUpgradePriceLvl3", 900_000) as i64,
                    p.get_int("InnerDoorUpgradePriceLvl5", 1_000_000) as i64,
                ],
                [
                    p.get_int("WallUpgradePriceLvl2", 1_600_000) as i64,
                    p.get_int("WallUpgradePriceLvl3", 1_800_000) as i64,
                    p.get_int("WallUpgradePriceLvl5", 2_000_000) as i64,
                ],
            ],
            trap_upgrade_price: [
                p.get_int("TrapUpgradePriceLvl1", 3_000_000) as i64,
                p.get_int("TrapUpgradePriceLvl2", 4_000_000) as i64,
                p.get_int("TrapUpgradePriceLvl3", 5_000_000) as i64,
                p.get_int("TrapUpgradePriceLvl4", 6_000_000) as i64,
            ],
            siege_hour_list: {
                let raw = p.get_string("SiegeHourList", "");
                let hrs: Vec<u32> = raw
                    .split(',')
                    .filter_map(|h| h.trim().parse().ok())
                    .collect();
                if hrs.is_empty() {
                    d.siege_hour_list.clone()
                } else {
                    hrs
                }
            },
            take_castle_points: p.get_int("TakeCastlePoints", d.take_castle_points),
            castle_defended_points: p.get_int("CastleDefendedPoints", d.castle_defended_points),
            loose_castle_points: p.get_int("LooseCastlePoints", d.loose_castle_points),
            level_up_reputation: {
                // The keys are named by band, not by index, so build the array
                // from `REPUTATION_LEVEL_BANDS`' order.
                let names = [
                    "LevelUp20And25ReputationScore",
                    "LevelUp26And30ReputationScore",
                    "LevelUp31And35ReputationScore",
                    "LevelUp36And40ReputationScore",
                    "LevelUp41And45ReputationScore",
                    "LevelUp46And50ReputationScore",
                    "LevelUp51And55ReputationScore",
                    "LevelUp56And60ReputationScore",
                    "LevelUp61And65ReputationScore",
                    "LevelUp66And70ReputationScore",
                    "LevelUp71And75ReputationScore",
                    "LevelUp76And80ReputationScore",
                    "LevelUp81PlusReputationScore",
                ];
                let mut out = [0i32; 13];
                for (slot, key) in out.iter_mut().zip(names) {
                    *slot = p.get_int(key, 0);
                }
                out
            },
            level_obtained_reputation_multiplier: f64::from(p.get_float(
                "LevelObtainedReputationScoreMultiplier",
                d.level_obtained_reputation_multiplier as f32,
            )),
            reputation_score_per_kill: p
                .get_int("ReputationScorePerKill", d.reputation_score_per_kill),
            create_royal_guard_cost: p.get_int("CreateRoyalGuardCost", d.create_royal_guard_cost),
            create_knight_unit_cost: p.get_int("CreateKnightUnitCost", d.create_knight_unit_cost),
            reinforce_knight_unit_cost: p
                .get_int("ReinforceKnightUnitCost", d.reinforce_knight_unit_cost),
            fortress_blood_oath_count: p
                .get_int("FortressBloodOathCount", d.fortress_blood_oath_count),
        }
    }
}

#[cfg(test)]
mod row14_tests {
    use super::*;

    /// The clan-reputation block added for row 14, held to the shipped file.
    #[test]
    fn the_reputation_block_matches_the_shipped_ini() {
        let f = FeatureConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(f.siege_hour_list, vec![16, 20]);
        assert_eq!(f.take_castle_points, 1500);
        assert_eq!(f.castle_defended_points, 750);
        assert_eq!(f.loose_castle_points, 3000);
        assert_eq!(f.reputation_score_per_kill, 1);
        assert_eq!(f.create_royal_guard_cost, 5000);
        assert_eq!(f.create_knight_unit_cost, 10_000);
        assert_eq!(f.reinforce_knight_unit_cost, 5000);
        // **Every band is 0**, which is what makes the level-up grant inert.
        assert_eq!(f.level_up_reputation, [0; 13], "no band pays anything");
        assert_eq!(f.level_obtained_reputation_multiplier, 1.0);
    }

    /// The band lookup is by the level *reached*, and everything below 20 —
    /// most of the newbie game — is worth nothing.
    #[test]
    fn reputation_bands_are_looked_up_by_level() {
        let mut f = FeatureConfig::default();
        f.level_up_reputation = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        assert_eq!(f.reputation_for_level(19), 0, "below every band");
        assert_eq!(f.reputation_for_level(20), 1, "first band starts at 20");
        assert_eq!(f.reputation_for_level(25), 1, "…and ends at 25");
        assert_eq!(f.reputation_for_level(26), 2, "next band");
        assert_eq!(f.reputation_for_level(80), 12);
        assert_eq!(f.reputation_for_level(81), 13, "the 81+ band");
        assert_eq!(f.reputation_for_level(121), 0, "past 120, nothing");
    }
}
