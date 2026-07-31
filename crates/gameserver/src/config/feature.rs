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
    /// `AllowRideMountsDuringSiege` — the strider/wolf equivalent, read by
    /// Java `Player.mount(pet)`. TODO(G29): consumed when pet mounting lands.
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
        }
    }
}
