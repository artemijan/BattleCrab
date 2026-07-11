//! `NPC.ini` — port of the `NPC_CONFIG_FILE` block of `Config.java`, scoped
//! to the keys the G9 combat/AI slice consumes.

use commons::config::PropertiesParser;

pub const NPC_CONFIG_FILE: &str = "config/NPC.ini";

#[derive(Debug, Clone)]
pub struct NpcConfig {
    /// `DefaultCorpseTime` (seconds) — decay delay for NPCs whose template
    /// carries no `<corpseTime>`.
    pub default_corpse_time: i32,
    /// `MaxDriftRange` — how far a monster may wander/chase from its spawn
    /// before AI walks it back home.
    pub max_drift_range: i32,
}

impl Default for NpcConfig {
    fn default() -> Self {
        Self { default_corpse_time: 7, max_drift_range: 300 }
    }
}

impl NpcConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(NPC_CONFIG_FILE);
        let d = Self::default();
        Self {
            default_corpse_time: p.get_int("DefaultCorpseTime", d.default_corpse_time),
            max_drift_range: p.get_int("MaxDriftRange", d.max_drift_range),
        }
    }
}
