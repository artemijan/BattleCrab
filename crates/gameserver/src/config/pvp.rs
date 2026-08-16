//! `PVP.ini` — the flag timers, the reputation bounds, and the anti-feed
//! block.
//!
//! Three of this file's keys were already read elsewhere and stay where they
//! are (`VampiricAttackAffectsPvP` / `MpVampiricAttackAffectsPvP` in
//! `character`, `MinimumPKRequiredToDrop` in `rates`) — moving them would be
//! churn for its own sake. What this module adds is the block that was
//! hardcoded to the shipped values.
//!
//! **The anti-feed keys are parsed and inert, deliberately.**
//! `AntiFeedEnable = False` on this dist, and Java's `AntiFeedManager` checks
//! it first in every entry point, so the whole manager is dead weight here —
//! the dualbox, disconnected-as-dualbox and interval keys below only ever
//! matter once it is switched on. They are carried so the values are visible
//! and so turning the flag on is a code change in one place rather than a
//! hunt; the manager itself is not ported.

use commons::config::PropertiesParser;

pub const PVP_CONFIG_FILE: &str = "config/PVP.ini";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PvpConfig {
    /// `PvPVsNormalTime` (ms) — how long the flag lasts after a hostile action
    /// toward a **clean** target.
    pub pvp_normal_time_ms: i32,
    /// `PvPVsPvPTime` (ms) — the shorter flag when the target is already a PK
    /// or flagged (`checkIfPvP`).
    pub pvp_pvp_time_ms: i32,
    /// `MaxReputation` — the ceiling `Player.setReputation` clamps to. **0 on
    /// this dist**, which is why reputation can never go positive here.
    pub max_reputation: i32,
    /// `ReputationIncrease` — what killing a PK within ±10 levels pays back.
    /// **0 on this dist**, so the reward is currently nothing.
    pub reputation_increase: i32,
    /// `CanGMDropEquipment` (Java `KARMA_DROP_GM`) — whether a GM's corpse
    /// drops like anyone else's. False here, so a GM never drops.
    pub karma_drop_gm: bool,
    /// `AntiFeedEnable` — the master switch for everything below. **False**;
    /// see the module note.
    pub antifeed_enable: bool,
    pub antifeed_dualbox: bool,
    pub antifeed_disconnected_as_dualbox: bool,
    /// `AntiFeedInterval` (ms).
    pub antifeed_interval_ms: i32,
}

impl Default for PvpConfig {
    /// The dist's own values, so a test world matches production without
    /// reading the file.
    fn default() -> Self {
        Self {
            pvp_normal_time_ms: 120_000,
            pvp_pvp_time_ms: 60_000,
            max_reputation: 0,
            reputation_increase: 0,
            karma_drop_gm: false,
            antifeed_enable: false,
            antifeed_dualbox: true,
            antifeed_disconnected_as_dualbox: true,
            antifeed_interval_ms: 120,
        }
    }
}

impl PvpConfig {
    pub fn load_from(root: &str) -> Self {
        let d = Self::default();
        let p = PropertiesParser::load_rel(root, PVP_CONFIG_FILE);
        Self {
            pvp_normal_time_ms: p.get_int("PvPVsNormalTime", d.pvp_normal_time_ms),
            pvp_pvp_time_ms: p.get_int("PvPVsPvPTime", d.pvp_pvp_time_ms),
            max_reputation: p.get_int("MaxReputation", d.max_reputation),
            reputation_increase: p.get_int("ReputationIncrease", d.reputation_increase),
            karma_drop_gm: p.get_bool("CanGMDropEquipment", d.karma_drop_gm),
            antifeed_enable: p.get_bool("AntiFeedEnable", d.antifeed_enable),
            antifeed_dualbox: p.get_bool("AntiFeedDualbox", d.antifeed_dualbox),
            antifeed_disconnected_as_dualbox: p.get_bool(
                "AntiFeedDisconnectedAsDualbox",
                d.antifeed_disconnected_as_dualbox,
            ),
            antifeed_interval_ms: p.get_int("AntiFeedInterval", d.antifeed_interval_ms),
        }
    }

    /// The flag duration in 100 ms ticks, which is what the PvP flag task
    /// counts in.
    pub fn normal_ticks(&self) -> u64 {
        (self.pvp_normal_time_ms.max(0) / 100) as u64
    }
    pub fn pvp_ticks(&self) -> u64 {
        (self.pvp_pvp_time_ms.max(0) / 100) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults above claim to be the shipped values; hold them to it, the
    /// way `grand_boss` does. A drifted default is a production/test split that
    /// nothing else would catch.
    #[test]
    fn default_config_matches_the_shipped_ini() {
        let loaded = PvpConfig::load_from(crate::data::DIST_GAME);
        assert_eq!(loaded, PvpConfig::default());
        // The two that decide observable behaviour today.
        assert_eq!(loaded.max_reputation, 0, "reputation cannot go positive");
        assert!(
            !loaded.antifeed_enable,
            "the anti-feed manager is inert here"
        );
    }
}
