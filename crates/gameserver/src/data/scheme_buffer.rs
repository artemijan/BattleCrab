//! Port of `SchemeBufferTable`'s XML load (`data/SchemeBufferSkills.xml`) — the
//! `_availableBuffs` table. Maps a buff skill id → the level the buffer applies
//! it at. The community board's scheme-execute (`_bbs_buff_scheme_execute`)
//! looks up the level here (Java `getAvailableBuff(skillId).getLevel()`), since
//! a saved scheme stores only skill ids.
//!
//! The retail `SchemeBuffer` NPC also uses this table's price/category/desc
//! columns; the custom community board does not, so only `id → level` is kept.

use crate::data::xml::attr_i32_trimmed as attr;
use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

const SCHEME_BUFFER_SKILLS_XML: &str = "data/SchemeBufferSkills.xml";

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemeBufferData {
    /// `_availableBuffs`: skill id → the level to cast at.
    levels: HashMap<i32, i32>,
}

impl SchemeBufferData {
    pub fn load_from(file_path: &str) -> Self {
        let content = std::fs::read_to_string(format!("{file_path}{SCHEME_BUFFER_SKILLS_XML}"))
            .unwrap_or_default();
        let levels = parse(&content);
        info!("SchemeBufferData: Loaded {} available buffs.", levels.len());
        Self { levels }
    }

    /// Java `getAvailableBuff(skillId).getLevel()`, or `None` when the skill is
    /// not a registered buffer skill.
    pub fn level_of(&self, skill_id: i32) -> Option<i32> {
        self.levels.get(&skill_id).copied()
    }

    /// Whether the skill is a registered buffer skill (`_availableBuffs
    /// .containsKey`), used to filter saved schemes on load.
    pub fn contains(&self, skill_id: i32) -> bool {
        self.levels.contains_key(&skill_id)
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, skill_id: i32, level: i32) {
        self.levels.insert(skill_id, level);
    }
}

fn parse(content: &str) -> HashMap<i32, i32> {
    let mut reader = Reader::from_str(content);
    let mut out = HashMap::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"buff" => {
                if let (Some(id), Some(level)) = (attr(&e, b"id"), attr(&e, b"level")) {
                    out.insert(id, level);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist() -> String {
        format!("{}/../../dist/game/", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn loads_available_buff_levels_from_dist() {
        let d = SchemeBufferData::load_from(&dist());
        // Concentration (1078) is registered at level 6 in the dist xml.
        assert_eq!(d.level_of(1078), Some(6), "Concentration level 6");
        // Wind Walk (1204) at level 2.
        assert_eq!(d.level_of(1204), Some(2), "Wind Walk level 2");
        assert!(d.contains(1059), "Empower is a registered buffer skill");
        assert_eq!(d.level_of(999_999), None, "unknown skill has no level");
    }
}
