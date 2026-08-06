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
        }
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
        }
    }
}
