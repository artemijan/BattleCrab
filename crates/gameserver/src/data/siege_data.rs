//! Siege control/flame tower spawns, parsed from `config/Siege.ini` (Java
//! `SiegeManager.load`'s `<Castle>ControlTowerN` / `<Castle>FlameTowerN =
//! x,y,z,npcId[,hp,zoneIds]` keys), keyed by castle id. Spawned onto the
//! battlefield at siege start alongside the guards.

use std::collections::HashMap;

use commons::config::PropertiesParser;

use crate::model::siege::SiegeSpawn;

const SIEGE_CONFIG_FILE: &str = "config/Siege.ini";

/// The nine Interlude castles, in `castle` table id order (the config keys are
/// prefixed with the castle name).
const CASTLE_NAMES: [(&str, i32); 9] = [
    ("Gludio", 1),
    ("Dion", 2),
    ("Giran", 3),
    ("Oren", 4),
    ("Aden", 5),
    ("Innadril", 6),
    ("Goddard", 7),
    ("Rune", 8),
    ("Schuttgart", 9),
];

/// Load the control + flame tower spawns per castle. Each `<Castle><Kind>Tower<N>`
/// value is `x,y,z,npcId[,…]`; we take the first four fields.
pub fn load_siege_towers(file_path: &str) -> HashMap<i32, Vec<SiegeSpawn>> {
    let p = PropertiesParser::load(format!("{file_path}{SIEGE_CONFIG_FILE}"));
    let mut out: HashMap<i32, Vec<SiegeSpawn>> = HashMap::new();
    for (name, castle_id) in CASTLE_NAMES {
        for kind in ["ControlTower", "FlameTower"] {
            for n in 1..20 {
                let key = format!("{name}{kind}{n}");
                if !p.contains_key(&key) {
                    break; // towers are numbered contiguously from 1
                }
                let nums: Vec<i32> = p.get_string(&key, "").split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if let [x, y, z, npc_id, ..] = nums[..] {
                    out.entry(castle_id).or_default().push(SiegeSpawn { npc_id, x, y, z, heading: 0 });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_siege_towers_from_dist() {
        let towers = load_siege_towers(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Giran (castle 3) has both control (13002) and flame (13004) towers.
        let giran = towers.get(&3).expect("Giran towers");
        assert!(giran.iter().any(|s| s.npc_id == 13002), "a control tower");
        assert!(giran.iter().any(|s| s.npc_id == 13004), "a flame tower");
        // GludioControlTower1=-18325,112811,-2377,13002.
        let gludio = towers.get(&1).expect("Gludio towers");
        assert!(
            gludio.iter().any(|s| s.npc_id == 13002 && s.x == -18325 && s.y == 112811 && s.z == -2377),
            "GludioControlTower1 parsed at its coords"
        );
    }
}
