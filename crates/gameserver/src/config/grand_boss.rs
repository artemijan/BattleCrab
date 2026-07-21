//! `GrandBoss.ini` — the per-boss respawn windows.
//!
//! Every grand boss respawns a configured number of **hours** after death,
//! plus or minus a random spread: Java's
//! `(INTERVAL + getRandom(-RANDOM, RANDOM)) * 3600000L`. Baium ships an
//! interval with no `Random` key, so the spread defaults to 0 rather than
//! being assumed symmetric.

use commons::config::PropertiesParser;

const GRAND_BOSS_CONFIG_FILE: &str = "config/GrandBoss.ini";

/// One boss's respawn window, in hours.
#[derive(Debug, Clone, Copy)]
pub struct RespawnWindow {
    pub interval_hours: i32,
    /// `± this many hours`. 0 for a boss with no `RandomOf…Spawn` key.
    pub random_hours: i32,
}

#[derive(Debug, Clone)]
pub struct GrandBossConfig {
    pub antharas: RespawnWindow,
    pub valakas: RespawnWindow,
    pub baium: RespawnWindow,
    pub core: RespawnWindow,
    pub orfen: RespawnWindow,
    pub queen_ant: RespawnWindow,
    pub zaken: RespawnWindow,
}

impl Default for GrandBossConfig {
    fn default() -> Self {
        // The dist's own values, so a test world matches production without
        // reading the file.
        Self {
            antharas: RespawnWindow { interval_hours: 264, random_hours: 72 },
            valakas: RespawnWindow { interval_hours: 264, random_hours: 72 },
            baium: RespawnWindow { interval_hours: 121, random_hours: 0 },
            core: RespawnWindow { interval_hours: 60, random_hours: 24 },
            orfen: RespawnWindow { interval_hours: 48, random_hours: 20 },
            queen_ant: RespawnWindow { interval_hours: 36, random_hours: 17 },
            zaken: RespawnWindow { interval_hours: 60, random_hours: 20 },
        }
    }
}

impl GrandBossConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(base: &str) -> Self {
        let d = Self::default();
        let p = PropertiesParser::load_rel(base, GRAND_BOSS_CONFIG_FILE);
        let w = |name: &str, def: RespawnWindow| RespawnWindow {
            interval_hours: p.get_int(&format!("IntervalOf{name}Spawn"), def.interval_hours),
            random_hours: p.get_int(&format!("RandomOf{name}Spawn"), def.random_hours),
        };
        Self {
            antharas: w("Antharas", d.antharas),
            valakas: w("Valakas", d.valakas),
            baium: w("Baium", d.baium),
            core: w("Core", d.core),
            orfen: w("Orfen", d.orfen),
            queen_ant: w("QueenAnt", d.queen_ant),
            zaken: w("Zaken", d.zaken),
        }
    }

    /// The window for a boss NPC id, or `None` for an id with no configured
    /// window — which is how a non-grand-boss reaches this by mistake.
    pub fn window_for(&self, boss_npc_id: i32) -> Option<RespawnWindow> {
        Some(match boss_npc_id {
            29019 => self.antharas,
            29028 => self.valakas,
            29020 => self.baium,
            29006 => self.core,
            29014 => self.orfen,
            29001 => self.queen_ant,
            29022 => self.zaken,
            _ => return None,
        })
    }
}
