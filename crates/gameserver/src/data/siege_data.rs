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

// ---------------------------------------------------------------------------
// The weekly siege calendar (`config/SiegeSchedule.xml`, Java
// `SiegeScheduleData`). G24 slice 1, PLAN_G24_SIEGE_SCHEDULE.md.
// ---------------------------------------------------------------------------

const SIEGE_SCHEDULE_FILE: &str = "config/SiegeSchedule.xml";

/// One castle's siege slot: the weekday (`Mon=0..Sun=6`), the hour of day, and
/// whether sieges are enabled for it.
#[derive(Debug, Clone, Copy)]
pub struct SiegeScheduleEntry {
    pub weekday: u32,
    pub hour: u32,
    pub enabled: bool,
}

/// `data/xsd`-validated `<schedule castleId day hour siegeEnabled …/>` rows.
pub fn load_siege_schedule(file_path: &str) -> HashMap<i32, SiegeScheduleEntry> {
    let mut out = HashMap::new();
    let path = format!("{file_path}{SIEGE_SCHEDULE_FILE}");
    let Ok(content) = std::fs::read_to_string(&path) else { return out };
    let mut reader = quick_xml::Reader::from_str(&content);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                if e.name().as_ref() == b"schedule" =>
            {
                let attr = |k: &[u8]| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == k)
                        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
                };
                let (Some(castle_id), Some(day), Some(hour)) = (
                    attr(b"castleId").and_then(|v| v.parse::<i32>().ok()),
                    attr(b"day").and_then(|v| weekday_from_name(&v)),
                    attr(b"hour").and_then(|v| v.parse::<u32>().ok()),
                ) else {
                    continue;
                };
                let enabled = attr(b"siegeEnabled").is_none_or(|v| v.eq_ignore_ascii_case("true"));
                out.insert(castle_id, SiegeScheduleEntry { weekday: day, hour, enabled });
            }
            _ => {}
        }
    }
    out
}

/// Day name → `Mon=0..Sun=6` (the convention `next_siege_millis` uses).
fn weekday_from_name(name: &str) -> Option<u32> {
    Some(match name.to_ascii_uppercase().as_str() {
        "MONDAY" => 0,
        "TUESDAY" => 1,
        "WEDNESDAY" => 2,
        "THURSDAY" => 3,
        "FRIDAY" => 4,
        "SATURDAY" => 5,
        "SUNDAY" => 6,
        _ => return None,
    })
}
