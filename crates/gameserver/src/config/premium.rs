//! `Custom/PremiumSystem.ini` — port of the `PREMIUM_SYSTEM_CONFIG_FILE` block
//! of `Config.java`. Premium is an account-scoped flag (`account_premium`,
//! cached in `World.premium`); this carries what that flag *does*: the reward
//! multipliers `Attackable.doItemDrop`/the XP path apply to a premium killer.
//!
//! Only the keys the ported subsystems read are loaded. Not here (no backing
//! subsystem yet): the PC-café acquisition tuning (`AcquisitionPoints*`,
//! `RewardLowExpKills*` — G16 PC_CAFE_RETAIL_LIKE), `PremiumOnlyFishing`
//! (G32), and the per-item-id drop tables (`PremiumRateDropChanceByItemId` /
//! `…AmountByItemId`) — TODO(G16) below.

use commons::config::PropertiesParser;

pub const PREMIUM_SYSTEM_CONFIG_FILE: &str = "config/Custom/PremiumSystem.ini";

#[derive(Debug, Clone)]
pub struct PremiumConfig {
    /// `EnablePremiumSystem` — master switch (True on this dist). When false
    /// `Player.hasPremiumStatus()` is always false and every rate below is
    /// inert.
    pub enabled: bool,
    /// `PremiumRateXp` / `PremiumRateSp` — multipliers applied to a premium
    /// killer's exp/sp reward, *before* the vitality/skill bonus multiplier
    /// (`Attackable.onKill`). Both **2** on this dist.
    pub rate_xp: f64,
    pub rate_sp: f64,
    /// `PremiumRateDropChance` / `PremiumRateDropAmount` — drop-roll
    /// multipliers for a premium killer (1 / 2 on this dist).
    pub rate_drop_chance: f64,
    pub rate_drop_amount: f64,
    /// `PremiumRateSpoilChance` / `PremiumRateSpoilAmount` (1 / 2).
    pub rate_spoil_chance: f64,
    pub rate_spoil_amount: f64,
}

impl Default for PremiumConfig {
    /// Java `Config` defaults — the system off, but the ×2 rates it would use.
    fn default() -> Self {
        Self {
            enabled: false,
            rate_xp: 2.0,
            rate_sp: 2.0,
            rate_drop_chance: 2.0,
            rate_drop_amount: 1.0,
            rate_spoil_chance: 2.0,
            rate_spoil_amount: 1.0,
        }
    }
}

impl PremiumConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load(format!(
            "{root}{PREMIUM_SYSTEM_CONFIG_FILE}"
        )))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enabled: p.get_bool("EnablePremiumSystem", d.enabled),
            rate_xp: p.get_float("PremiumRateXp", d.rate_xp as f32) as f64,
            rate_sp: p.get_float("PremiumRateSp", d.rate_sp as f32) as f64,
            rate_drop_chance: p.get_float("PremiumRateDropChance", d.rate_drop_chance as f32) as f64,
            rate_drop_amount: p.get_float("PremiumRateDropAmount", d.rate_drop_amount as f32) as f64,
            rate_spoil_chance: p.get_float("PremiumRateSpoilChance", d.rate_spoil_chance as f32) as f64,
            rate_spoil_amount: p.get_float("PremiumRateSpoilAmount", d.rate_spoil_amount as f32) as f64,
            // TODO(G16): Java also reads PremiumRateDropChanceByItemId /
            // PremiumRateDropAmountByItemId into per-item override maps
            // (`Config.PREMIUM_RATE_DROP_CHANCE_BY_ID`), consulted ahead of the
            // flat rates in `Attackable.calculateDrops`. The flat rates are
            // ported here; the per-item overrides are not.
        }
    }
}
