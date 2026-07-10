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
    (0, Race::Human, false),     // Human Fighter
    (10, Race::Human, true),     // Human Mystic
    (18, Race::Elf, false),      // Elven Fighter
    (25, Race::Elf, true),       // Elven Mystic
    (31, Race::DarkElf, false),  // Dark Fighter
    (38, Race::DarkElf, true),   // Dark Mystic
    (44, Race::Orc, false),      // Orc Fighter
    (49, Race::Orc, true),       // Orc Mystic
    (53, Race::Dwarf, false),    // Dwarven Fighter
    (123, Race::Kamael, false),  // Male Soldier
    (124, Race::Kamael, false),  // Female Soldier
    (182, Race::Ertheia, false), // Ertheia Fighter
    (183, Race::Ertheia, true),  // Ertheia Wizard
];

pub fn creatable_race(class_id: i32) -> Option<Race> {
    CREATABLE_CLASSES
        .iter()
        .find(|(id, _, _)| *id == class_id)
        .map(|(_, r, _)| *r)
}

#[derive(Debug, Clone, Default)]
pub struct PlayerTemplate {
    pub class_id: i32,
    pub base_str: i32,
    pub base_dex: i32,
    pub base_con: i32,
    pub base_int: i32,
    pub base_wit: i32,
    pub base_men: i32,
    /// Per-level max HP/MP/CP (`lvlUpgainData`), indexed by level.
    pub hp_table: Vec<f64>,
    pub mp_table: Vec<f64>,
    pub cp_table: Vec<f64>,
    /// Per-level passive HP/MP/CP regen (`lvlUpgainData`'s `hpRegen`/`mpRegen`/
    /// `cpRegen`), indexed by level.
    pub hp_regen_table: Vec<f64>,
    pub mp_regen_table: Vec<f64>,
    pub cp_regen_table: Vec<f64>,
    // Base combat stats (TODO(G7): full stat calc with modifiers/items).
    pub base_p_atk: i32,
    pub base_p_atk_spd: i32,
    pub base_m_atk: i32,
    pub base_m_atk_spd: i32,
    pub base_crit_rate: i32,
    pub base_m_crit_rate: i32,
    pub base_atk_range: i32,
    /// Sum of the base armor-slot defenses / jewel defenses.
    pub base_p_def: i32,
    pub base_m_def: i32,
    // Movement (before the run-speed multiplier).
    pub base_run_spd: i32,
    pub base_walk_spd: i32,
    pub base_swim_run_spd: i32,
    pub base_swim_walk_spd: i32,
    /// Collision (male; TODO: female variant).
    pub collision_radius: f64,
    pub collision_height: f64,
    /// Random spawn points offered at creation.
    pub creation_points: Vec<(i32, i32, i32)>,
}

impl PlayerTemplate {
    pub fn race(&self) -> Option<Race> {
        creatable_race(self.class_id)
    }

    /// Level-1 max HP (used at creation).
    pub fn base_hp(&self) -> f64 {
        self.base_hp_max(1)
    }
    pub fn base_mp(&self) -> f64 {
        self.base_mp_max(1)
    }

    /// `getBaseHpMax(level)` — clamps out-of-range levels to the table ends.
    pub fn base_hp_max(&self, level: i32) -> f64 {
        table_get(&self.hp_table, level)
    }
    pub fn base_mp_max(&self, level: i32) -> f64 {
        table_get(&self.mp_table, level)
    }
    pub fn base_cp_max(&self, level: i32) -> f64 {
        table_get(&self.cp_table, level)
    }
    pub fn base_hp_regen(&self, level: i32) -> f64 {
        table_get(&self.hp_regen_table, level)
    }
    pub fn base_mp_regen(&self, level: i32) -> f64 {
        table_get(&self.mp_regen_table, level)
    }
    pub fn base_cp_regen(&self, level: i32) -> f64 {
        table_get(&self.cp_regen_table, level)
    }
}

fn table_get(table: &[f64], level: i32) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    let idx = (level.max(1) as usize).min(table.len() - 1);
    table[idx]
}

pub struct PlayerTemplateData {
    templates: HashMap<i32, PlayerTemplate>,
}

impl PlayerTemplateData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut templates = HashMap::new();
        let dir = std::fs::read_dir(format!("{file_path}{BASE_STATS_DIR}")).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("PlayerTemplateData: cannot read {BASE_STATS_DIR}: {e}, CWD: {cwd:?}")
        });
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                continue;
            }
            if let Some(t) = parse_template(&path) {
                templates.insert(t.class_id, t);
            }
        }
        info!(
            "PlayerTemplateData: Loaded {} character templates.",
            templates.len()
        );
        Self { templates }
    }

    pub fn get(&self, class_id: i32) -> Option<&PlayerTemplate> {
        self.templates.get(&class_id)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    #[doc(hidden)]
    pub fn from_vec(templates: Vec<PlayerTemplate>) -> Self {
        Self {
            templates: templates.into_iter().map(|t| (t.class_id, t)).collect(),
        }
    }
}

fn parse_template(path: &std::path::Path) -> Option<PlayerTemplate> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut reader = Reader::from_str(&content);

    let mut t = PlayerTemplate {
        class_id: -1,
        ..Default::default()
    };
    // Level tables sized to the classic max (85, +buffer); index by level.
    t.hp_table = vec![0.0; 90];
    t.mp_table = vec![0.0; 90];
    t.cp_table = vec![0.0; 90];
    t.hp_regen_table = vec![0.0; 90];
    t.mp_regen_table = vec![0.0; 90];
    t.cp_regen_table = vec![0.0; 90];

    let mut cur_tag: Vec<u8> = Vec::new();
    let mut in_creation_points = false;
    let mut cur_level: i32 = 0; // 0 = not inside a <level>
                                // Nested section we're inside, if any (for summed / grouped values).
    let mut section: Option<Vec<u8>> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"creationPoints" => in_creation_points = true,
                    b"basePDef" | b"baseMDef" | b"baseMoveSpd" | b"collisionMale" => {
                        section = Some(name.clone())
                    }
                    b"level" => {
                        cur_level = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"val")
                            .and_then(|a| String::from_utf8_lossy(&a.value).parse::<i32>().ok())
                            .unwrap_or(0);
                    }
                    _ => {}
                }
                cur_tag = name;
            }
            Ok(Event::Empty(e)) => {
                if in_creation_points && e.name().as_ref() == b"node" {
                    let (mut x, mut y, mut z) = (0, 0, 0);
                    for a in e.attributes().flatten() {
                        let v = String::from_utf8_lossy(&a.value)
                            .parse::<i32>()
                            .unwrap_or(0);
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
                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let int = || text.parse::<i32>().unwrap_or(0);
                let flt = || text.parse::<f64>().unwrap_or(0.0);
                // Per-level HP/MP/CP tables.
                if cur_level > 0 && (cur_level as usize) < t.hp_table.len() {
                    match cur_tag.as_slice() {
                        b"hp" => t.hp_table[cur_level as usize] = flt(),
                        b"mp" => t.mp_table[cur_level as usize] = flt(),
                        b"cp" => t.cp_table[cur_level as usize] = flt(),
                        b"hpRegen" => t.hp_regen_table[cur_level as usize] = flt(),
                        b"mpRegen" => t.mp_regen_table[cur_level as usize] = flt(),
                        b"cpRegen" => t.cp_regen_table[cur_level as usize] = flt(),
                        _ => {}
                    }
                    continue;
                }
                // Summed / grouped sections.
                match section.as_deref() {
                    Some(b"basePDef") => {
                        t.base_p_def += int();
                        continue;
                    }
                    Some(b"baseMDef") => {
                        t.base_m_def += int();
                        continue;
                    }
                    Some(b"baseMoveSpd") => {
                        match cur_tag.as_slice() {
                            b"walk" => t.base_walk_spd = int(),
                            b"run" => t.base_run_spd = int(),
                            b"slowSwim" => t.base_swim_walk_spd = int(),
                            b"fastSwim" => t.base_swim_run_spd = int(),
                            _ => {}
                        }
                        continue;
                    }
                    Some(b"collisionMale") => {
                        match cur_tag.as_slice() {
                            b"radius" => t.collision_radius = flt(),
                            b"height" => t.collision_height = flt(),
                            _ => {}
                        }
                        continue;
                    }
                    _ => {}
                }
                match cur_tag.as_slice() {
                    b"classId" => t.class_id = int(),
                    b"baseSTR" => t.base_str = int(),
                    b"baseDEX" => t.base_dex = int(),
                    b"baseCON" => t.base_con = int(),
                    b"baseINT" => t.base_int = int(),
                    b"baseWIT" => t.base_wit = int(),
                    b"baseMEN" => t.base_men = int(),
                    b"basePAtk" => t.base_p_atk = int(),
                    b"basePAtkSpd" => t.base_p_atk_spd = int(),
                    b"baseMAtk" => t.base_m_atk = int(),
                    b"baseMAtkSpd" => t.base_m_atk_spd = int(),
                    b"baseCritRate" => t.base_crit_rate = int(),
                    b"baseMCritRate" => t.base_m_crit_rate = int(),
                    b"baseAtkRange" => t.base_atk_range = int(),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"creationPoints" => in_creation_points = false,
                b"level" => cur_level = 0,
                b"basePDef" | b"baseMDef" | b"baseMoveSpd" | b"collisionMale" => section = None,
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
