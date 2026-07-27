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
}

impl Default for FeatureConfig {
    fn default() -> Self {
        // Java's config defaults (== this dist's ini values).
        Self {
            allow_ride_wyvern_always: false,
            allow_ride_wyvern_during_siege: true,
            allow_ride_mounts_during_siege: false,
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
        }
    }
}
