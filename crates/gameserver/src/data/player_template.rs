//! Port of `data/xml/PlayerTemplateData` — per-class base data from
//! `data/stats/chars/baseStats/*.xml`. G3 needs the creation-screen stats
//! (base STR/DEX/…), the level-1 HP/MP, and the creation spawn points.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

use crate::enums::Race;

pub const BASE_STATS_DIR: &str = "data/stats/chars/baseStats";

/// The creatable base classes and their race / mage flag (from `ClassId`).
/// Only these can be created; `NewCharacter` offers exactly this set.
pub const CREATABLE_CLASSES: &[(i32, Race, bool)] = &[
    (0, Race::Human, false),   // Human Fighter
    (10, Race::Human, true),   // Human Mystic
    (18, Race::Elf, false),    // Elven Fighter
    (25, Race::Elf, true),     // Elven Mystic
    (31, Race::DarkElf, false), // Dark Fighter
    (38, Race::DarkElf, true),  // Dark Mystic
    (44, Race::Orc, false),    // Orc Fighter
    (49, Race::Orc, true),     // Orc Mystic
    (53, Race::Dwarf, false),  // Dwarven Fighter
    (123, Race::Kamael, false), // Male Soldier
    (124, Race::Kamael, false), // Female Soldier
    (182, Race::Ertheia, false), // Ertheia Fighter
    (183, Race::Ertheia, true), // Ertheia Wizard
];

pub fn creatable_race(class_id: i32) -> Option<Race> {
    CREATABLE_CLASSES.iter().find(|(id, _, _)| *id == class_id).map(|(_, r, _)| *r)
}

#[derive(Debug, Clone)]
pub struct PlayerTemplate {
    pub class_id: i32,
    pub base_str: i32,
    pub base_dex: i32,
    pub base_con: i32,
    pub base_int: i32,
    pub base_wit: i32,
    pub base_men: i32,
    /// Level-1 HP/MP (max at creation).
    pub base_hp: f64,
    pub base_mp: f64,
    /// Random spawn points offered at creation.
    pub creation_points: Vec<(i32, i32, i32)>,
}

impl PlayerTemplate {
    pub fn race(&self) -> Option<Race> {
        creatable_race(self.class_id)
    }
}

pub struct PlayerTemplateData {
    templates: HashMap<i32, PlayerTemplate>,
}

impl PlayerTemplateData {
    pub fn load() -> Self {
        let mut templates = HashMap::new();
        let dir = std::fs::read_dir(BASE_STATS_DIR)
            .unwrap_or_else(|e| panic!("PlayerTemplateData: cannot read {BASE_STATS_DIR}: {e}"));
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                continue;
            }
            if let Some(t) = parse_template(&path) {
                templates.insert(t.class_id, t);
            }
        }
        info!("PlayerTemplateData: Loaded {} character templates.", templates.len());
        Self { templates }
    }

    pub fn get(&self, class_id: i32) -> Option<&PlayerTemplate> {
        self.templates.get(&class_id)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { templates: HashMap::new() }
    }

    #[doc(hidden)]
    pub fn from_vec(templates: Vec<PlayerTemplate>) -> Self {
        Self { templates: templates.into_iter().map(|t| (t.class_id, t)).collect() }
    }
}

fn parse_template(path: &std::path::Path) -> Option<PlayerTemplate> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut reader = Reader::from_str(&content);

    let mut t = PlayerTemplate {
        class_id: -1,
        base_str: 0,
        base_dex: 0,
        base_con: 0,
        base_int: 0,
        base_wit: 0,
        base_men: 0,
        base_hp: 0.0,
        base_mp: 0.0,
        creation_points: Vec::new(),
    };

    let mut cur_tag: Vec<u8> = Vec::new();
    let mut in_creation_points = false;
    let mut in_level_1 = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"creationPoints" => in_creation_points = true,
                    b"level" => {
                        let val = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"val")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse::<i32>().ok())
                            .unwrap_or(0);
                        in_level_1 = val == 1;
                    }
                    _ => {}
                }
                cur_tag = name;
            }
            Ok(Event::Empty(e)) => {
                if in_creation_points && e.name().as_ref() == b"node" {
                    let (mut x, mut y, mut z) = (0, 0, 0);
                    for a in e.attributes().flatten() {
                        let v = String::from_utf8_lossy(&a.value).parse::<i32>().unwrap_or(0);
                        match a.key.as_ref() {
                            b"x" => x = v,
                            b"y" => y = v,
                            b"z" => z = v,
                            _ => {}
                        }
                    }
                    t.creation_points.push((x, y, z));
                }
            }
            Ok(Event::Text(txt)) => {
                let text = txt.unescape().unwrap_or_default();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let int = || text.parse::<i32>().unwrap_or(0);
                let flt = || text.parse::<f64>().unwrap_or(0.0);
                match cur_tag.as_slice() {
                    b"classId" => t.class_id = int(),
                    b"baseSTR" => t.base_str = int(),
                    b"baseDEX" => t.base_dex = int(),
                    b"baseCON" => t.base_con = int(),
                    b"baseINT" => t.base_int = int(),
                    b"baseWIT" => t.base_wit = int(),
                    b"baseMEN" => t.base_men = int(),
                    b"hp" if in_level_1 => t.base_hp = flt(),
                    b"mp" if in_level_1 => t.base_mp = flt(),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"creationPoints" => in_creation_points = false,
                b"level" => in_level_1 = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    if t.class_id < 0 {
        return None;
    }
    Some(t)
}
