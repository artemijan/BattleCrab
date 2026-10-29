//! Port of `data/xml/PlayerTemplateData` — per-class base data from
//! `data/stats/players/templates/**/*.xml` (`StartingClass`, `1stClass` …
//! `4rthClass`, `Ertheia`, `BalthusKnights`). G3 needs the creation-screen
//! stats (base STR/DEX/…), the level-1 HP/MP, and the creation spawn points.
//!
//! The per-level tables are sized by `ExperienceData.getMaxLevel()` (Java's
//! `MAX_LEVEL`, one above the cap), and a `<level val>` past `MAX_LEVEL - 1`
//! ends that file's `lvlUpgainData`. A file that stops short of the cap has its
//! remaining levels extrapolated, exactly as Java's "Generate missing stats
//! automatically" loop does.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::enums::Race;

pub const TEMPLATES_DIR: &str = "data/stats/players/templates";

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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PlayerTemplate {
    pub class_id: i32,
    pub base_str: i32,
    pub base_dex: i32,
    pub base_con: i32,
    pub base_int: i32,
    pub base_wit: i32,
    pub base_men: i32,
    pub base_luc: i32,
    pub base_cha: i32,
    /// Per-level max HP/MP/CP (`lvlUpgainData`), indexed by level.
    pub hp_table: Vec<f64>,
    pub mp_table: Vec<f64>,
    pub cp_table: Vec<f64>,
    /// Per-level passive HP/MP/CP regen (`lvlUpgainData`'s `hpRegen`/`mpRegen`/
    /// `cpRegen`), indexed by level.
    pub hp_regen_table: Vec<f64>,
    pub mp_regen_table: Vec<f64>,
    pub cp_regen_table: Vec<f64>,
    // Base combat stats — the naked-class bases the stat finalizers build on
    // (modifiers/items fold in downstream, `model::calc_*` / `ItemStats`).
    pub base_p_atk: i32,
    pub base_p_atk_spd: i32,
    pub base_m_atk: i32,
    pub base_m_atk_spd: i32,
    pub base_crit_rate: i32,
    pub base_m_crit_rate: i32,
    /// `basePSkillCritRate`; no template sets it, so every class has Java's
    /// default of 5.
    pub base_p_skill_crit_rate: i32,
    pub base_atk_range: i32,
    /// Sum of the base armor-slot defenses / jewel defenses.
    pub base_p_def: i32,
    pub base_m_def: i32,
    /// Per-slot naked defense (Java `PlayerTemplate._baseSlotDef` /
    /// `getBaseDefBySlot`), keyed by `PaperdollSlot as usize`. The pDef/mDef
    /// finalizers subtract the occupied slots' entries from the summed base so
    /// worn gear *replaces* the naked slot defense rather than stacking on it.
    pub base_def_by_slot: HashMap<usize, i32>,
    // Movement (before the run-speed multiplier).
    pub base_run_spd: i32,
    pub base_walk_spd: i32,
    pub base_swim_run_spd: i32,
    pub base_swim_walk_spd: i32,
    /// Collision box, **per gender** — the dist gives every class both a
    /// `<collisionMale>` and a `<collisionFemale>`, and they differ (a female
    /// Human Fighter is 7.5/22.8 against the male 9/23.5). Collision feeds cast
    /// range, `position`'s height checks and `CharInfo`, so reading the male
    /// box for both sexes is visible, not cosmetic.
    ///
    /// Use [`PlayerTemplate::collision`] rather than these directly.
    pub collision_radius: f64,
    pub collision_height: f64,
    pub collision_radius_female: f64,
    pub collision_height_female: f64,
    /// Random spawn points offered at creation.
    pub creation_points: Vec<(i32, i32, i32)>,
}

impl PlayerTemplate {
    pub fn race(&self) -> Option<Race> {
        creatable_race(self.class_id)
    }

    /// `(radius, height)` for the given sex — Java
    /// `PlayerTemplate.getCollisionRadius()`/`getCollisionHeight()`, which
    /// branch on `appearance.isFemale()`.
    ///
    /// Falls back to the male box when a template carries no
    /// `<collisionFemale>`, so a datapack that omits it degrades to the old
    /// behaviour rather than to a zero-size cylinder.
    pub fn collision(&self, is_female: bool) -> (f64, f64) {
        if is_female && self.collision_radius_female > 0.0 {
            (self.collision_radius_female, self.collision_height_female)
        } else {
            (self.collision_radius, self.collision_height)
        }
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

    /// Java `getBaseDefBySlot(slotId)` — the naked defense of an *empty* slot,
    /// 0 when the slot has no base contribution (`_baseSlotDef`'s `getOrDefault`).
    pub fn base_def_by_slot(&self, slot: usize) -> i32 {
        self.base_def_by_slot.get(&slot).copied().unwrap_or(0)
    }
}

/// `<basePDef>`/`<baseMDef>` child tag → the `PaperdollSlot as usize` its base
/// defense belongs to (Java's per-slot `_baseSlotDef` keys). Tags with no
/// paperdoll slot (none here) are ignored.
fn def_slot_index(tag: &str) -> Option<usize> {
    use crate::model::inventory::PaperdollSlot;
    Some(match tag {
        "chest" => PaperdollSlot::Chest,
        "legs" => PaperdollSlot::Legs,
        "head" => PaperdollSlot::Head,
        "feet" => PaperdollSlot::Feet,
        "gloves" => PaperdollSlot::Gloves,
        "underwear" => PaperdollSlot::Under,
        "cloak" => PaperdollSlot::Cloak,
        "hair" => PaperdollSlot::Hair,
        "rear" => PaperdollSlot::REar,
        "lear" => PaperdollSlot::LEar,
        "rfinger" => PaperdollSlot::RFinger,
        "lfinger" => PaperdollSlot::LFinger,
        "neck" => PaperdollSlot::Neck,
        _ => return None,
    } as usize)
}

fn table_get(table: &[f64], level: i32) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    let idx = (level.max(1) as usize).min(table.len() - 1);
    table[idx]
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerTemplateData {
    templates: HashMap<i32, PlayerTemplate>,
}

impl PlayerTemplateData {
    /// Every loaded class id.
    pub fn class_ids(&self) -> Vec<i32> {
        self.templates.keys().copied().collect()
    }

    pub fn load() -> Self {
        Self::load_from("")
    }
    /// The templates under this dist's shipped level cap.
    pub fn load_from(file_path: &str) -> Self {
        Self::load_from_with(
            file_path,
            crate::data::experience::DIST_PLAYER_MAXIMUM_LEVEL as u8,
        )
    }

    /// `max_level` is `ExperienceData.getMaxLevel()` (Java `MAX_LEVEL`).
    pub fn load_from_with(file_path: &str, max_level: u8) -> Self {
        let mut templates = HashMap::new();
        // Deliberately **not** `xml::xml_files_in`: that helper answers an
        // unreadable directory with an empty list, and an empty player-template
        // set is not a degraded server, it is one where no character can be
        // created or loaded. The panic names the CWD because the usual cause is
        // a datapack path resolved from the wrong directory.
        let dir = format!("{file_path}{TEMPLATES_DIR}");
        if let Err(e) = std::fs::read_dir(&dir) {
            let cwd = std::env::current_dir().unwrap();
            panic!("PlayerTemplateData: cannot read {TEMPLATES_DIR}: {e}, CWD: {cwd:?}")
        }
        for path in crate::data::xml::xml_files_under(&dir) {
            if let Some(t) = parse_template(&path, max_level as usize) {
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

    /// The template every *stat* path wants: the active class's, falling back
    /// to the base class's. A subclass id that never got its own template file
    /// would otherwise recompute a player's stats off `PlayerTemplate::default`
    /// — zeroed base stats and collision box — the moment they switched to it.
    pub fn get_or_base(&self, class_id: i32, base_class_id: i32) -> Option<&PlayerTemplate> {
        self.get(class_id).or_else(|| self.get(base_class_id))
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

fn parse_template(path: &std::path::Path, max_level: usize) -> Option<PlayerTemplate> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut reader = Reader::from_str(&content);

    let mut t = PlayerTemplate {
        class_id: -1,
        base_p_skill_crit_rate: 5,
        ..Default::default()
    };
    // Java `new double[ExperienceData.getMaxLevel()]`; index by level.
    t.hp_table = vec![0.0; max_level];
    t.mp_table = vec![0.0; max_level];
    t.cp_table = vec![0.0; max_level];
    t.hp_regen_table = vec![0.0; max_level];
    t.mp_regen_table = vec![0.0; max_level];
    t.cp_regen_table = vec![0.0; max_level];
    // The last `<level val>` read, for the missing-level extrapolation; and
    // whether a level past the cap ended the gain data (Java `return`s).
    let mut last_level: usize = 0;
    let mut gain_ended = false;

    let mut cur_tag = String::new();
    let mut in_creation_points = false;
    let mut cur_level: i32 = 0; // 0 = not inside a <level>
    // Nested section we're inside, if any (for summed / grouped values).
    let mut section: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_owned();
                match name.as_str() {
                    "creationPoints" => in_creation_points = true,
                    "basePDef" | "baseMDef" | "baseMoveSpd" | "collisionMale"
                    | "collisionFemale" => section = Some(name.clone()),
                    "level" if !gain_ended => {
                        cur_level = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == "val")
                            .and_then(|a| a.value.parse::<i32>().ok())
                            .unwrap_or(0);
                        if cur_level as usize > max_level.saturating_sub(1) {
                            // `if (level > (maxLevel - 1)) return;` — the rest
                            // of the file, extrapolation included, is skipped.
                            gain_ended = true;
                            cur_level = 0;
                        } else {
                            last_level = cur_level.max(0) as usize;
                        }
                    }
                    _ => {}
                }
                cur_tag = name;
            }
            Ok(Event::Empty(e)) => {
                if in_creation_points && e.name().as_ref() == "node" {
                    let (mut x, mut y, mut z) = (0, 0, 0);
                    for a in e.attributes().flatten() {
                        let v = a.value.parse::<i32>().unwrap_or(0);
                        match a.key.as_ref() {
                            "x" => x = v,
                            "y" => y = v,
                            "z" => z = v,
                            _ => {}
                        }
                    }
                    t.creation_points.push((x, y, z));
                }
            }
            Ok(Event::Text(txt)) => {
                let text = txt.into_inner();
                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let int = || text.parse::<i32>().unwrap_or(0);
                let flt = || text.parse::<f64>().unwrap_or(0.0);
                // Per-level HP/MP/CP tables.
                if cur_level > 0 && (cur_level as usize) < t.hp_table.len() {
                    match cur_tag.as_str() {
                        "hp" => t.hp_table[cur_level as usize] = flt(),
                        "mp" => t.mp_table[cur_level as usize] = flt(),
                        "cp" => t.cp_table[cur_level as usize] = flt(),
                        "hpRegen" => t.hp_regen_table[cur_level as usize] = flt(),
                        "mpRegen" => t.mp_regen_table[cur_level as usize] = flt(),
                        "cpRegen" => t.cp_regen_table[cur_level as usize] = flt(),
                        _ => {}
                    }
                    continue;
                }
                // Summed / grouped sections.
                match section.as_deref() {
                    Some("basePDef") => {
                        t.base_p_def += int();
                        if let Some(slot) = def_slot_index(&cur_tag) {
                            t.base_def_by_slot.insert(slot, int());
                        }
                        continue;
                    }
                    Some("baseMDef") => {
                        t.base_m_def += int();
                        if let Some(slot) = def_slot_index(&cur_tag) {
                            t.base_def_by_slot.insert(slot, int());
                        }
                        continue;
                    }
                    Some("baseMoveSpd") => {
                        match cur_tag.as_str() {
                            "walk" => t.base_walk_spd = int(),
                            "run" => t.base_run_spd = int(),
                            "slowSwim" => t.base_swim_walk_spd = int(),
                            "fastSwim" => t.base_swim_run_spd = int(),
                            _ => {}
                        }
                        continue;
                    }
                    Some("collisionMale") => {
                        match cur_tag.as_str() {
                            "radius" => t.collision_radius = flt(),
                            "height" => t.collision_height = flt(),
                            _ => {}
                        }
                        continue;
                    }
                    Some("collisionFemale") => {
                        match cur_tag.as_str() {
                            "radius" => t.collision_radius_female = flt(),
                            "height" => t.collision_height_female = flt(),
                            _ => {}
                        }
                        continue;
                    }
                    _ => {}
                }
                match cur_tag.as_str() {
                    "classId" => t.class_id = int(),
                    "baseSTR" => t.base_str = int(),
                    "baseDEX" => t.base_dex = int(),
                    "baseCON" => t.base_con = int(),
                    "baseINT" => t.base_int = int(),
                    "baseWIT" => t.base_wit = int(),
                    "baseMEN" => t.base_men = int(),
                    "baseLUC" => t.base_luc = int(),
                    "baseCHA" => t.base_cha = int(),
                    "basePAtk" => t.base_p_atk = int(),
                    "basePAtkSpd" => t.base_p_atk_spd = int(),
                    "baseMAtk" => t.base_m_atk = int(),
                    "baseMAtkSpd" => t.base_m_atk_spd = int(),
                    "baseCritRate" => t.base_crit_rate = int(),
                    "baseMCritRate" => t.base_m_crit_rate = int(),
                    "basePSkillCritRate" => t.base_p_skill_crit_rate = int(),
                    "baseAtkRange" => t.base_atk_range = int(),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "creationPoints" => in_creation_points = false,
                "level" => cur_level = 0,
                "basePDef" | "baseMDef" | "baseMoveSpd" | "collisionMale" | "collisionFemale" => {
                    section = None
                }
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
    if !gain_ended {
        generate_missing_levels(&mut t, last_level, max_level);
    }
    Some(t)
}

/// Java's "Generate missing stats automatically" loop, run when a template's
/// `lvlUpgainData` stops short of `maxLevel - 1`.
fn generate_missing_levels(t: &mut PlayerTemplate, mut level: usize, max_level: usize) {
    while level >= 2 && level + 1 < max_level {
        level += 1;
        let l = level as f64;
        let grow = |m1: f64| (((m1 * l) / (l - 1.0)) + ((m1 * (l + 1.0)) / (l - 1.0))) / 2.0;
        t.hp_table[level] = grow(t.hp_table[level - 1]);
        t.mp_table[level] = grow(t.mp_table[level - 1]);
        t.cp_table[level] = grow(t.cp_table[level - 1]);
        t.hp_regen_table[level] = t.hp_regen_table[level - 1] * 2.0 - t.hp_regen_table[level - 2];
        t.mp_regen_table[level] = t.mp_regen_table[level - 1] * 2.0 - t.mp_regen_table[level - 2];
        t.cp_regen_table[level] = t.cp_regen_table[level - 1] * 2.0 - t.cp_regen_table[level - 2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;

    /// All 155 Salvation templates load from the tiered directories, and the
    /// tables reach the level-110 cap: Human Fighter (0, `StartingClass/`) at
    /// level 110 is `hp 2134.65 / mp 978.3 / cp 853.6`, and Tyrr Duelist (152,
    /// `4rthClass/`) and the Balthus Sigel Knight (139, `BalthusKnights/`) are
    /// present.
    #[test]
    fn loads_every_salvation_template_up_to_the_cap() {
        let data = dist::player_templates();
        assert_eq!(data.class_ids().len(), 155);
        let hf = data.get(0).expect("Human Fighter");
        assert_eq!(hf.base_hp_max(110), 2134.65);
        assert_eq!(hf.base_mp_max(110), 978.3);
        assert_eq!(hf.base_cp_max(110), 853.6);
        assert!(data.get(152).is_some(), "Tyrr Duelist");
        assert!(data.get(139).is_some(), "Balthus Sigel Knight");
        // The two Balthus-only classes, from `BalthusKnights/`.
        for id in [190, 191] {
            assert!(data.get(id).is_some(), "Balthus class {id}");
        }
        // LUC and CHA come from every template; `basePSkillCritRate` from none.
        let evisc = data.get(190).unwrap();
        assert_eq!((evisc.base_luc, evisc.base_cha), (34, 41));
        assert!(data.class_ids().iter().all(|&id| {
            let t = data.get(id).unwrap();
            t.base_luc > 0 && t.base_cha > 0 && t.base_p_skill_crit_rate == 5
        }));
    }

    /// Every class ships **both** collision boxes and they differ, so reading
    /// the male one for a female character is a visible error — collision feeds
    /// cast range, the position height checks and `CharInfo`.
    ///
    /// Asserted across the whole dist rather than on one class: the failure
    /// this guards against is a parser that silently drops `<collisionFemale>`,
    /// which one hand-picked class could miss.
    #[test]
    fn every_class_has_a_distinct_female_collision_box() {
        let data = dist::player_templates();
        let ids = data.class_ids();
        assert!(ids.len() > 30, "sanity: templates loaded ({})", ids.len());
        let all: Vec<&PlayerTemplate> = ids.iter().filter_map(|id| data.get(*id)).collect();

        let mut differing = 0;
        for t in &all {
            assert!(
                t.collision_radius_female > 0.0 && t.collision_height_female > 0.0,
                "class {} parsed no <collisionFemale>",
                t.class_id
            );
            assert_eq!(
                t.collision(false),
                (t.collision_radius, t.collision_height),
                "class {}: male lookup must give the male box",
                t.class_id
            );
            assert_eq!(
                t.collision(true),
                (t.collision_radius_female, t.collision_height_female),
                "class {}: female lookup must give the female box",
                t.class_id
            );
            if t.collision(true) != t.collision(false) {
                differing += 1;
            }
        }
        // Male Soul Hound (132) is the one Salvation template whose two boxes
        // are the same (8.0 / 25.2); every other class differs, so a test that
        // passed while returning the male box for both would still fail here.
        assert_eq!(
            differing,
            all.len() - 1,
            "the two boxes differ for every class but Male Soul Hound"
        );
        let soul_hound = data.get(132).expect("Male Soul Hound");
        assert_eq!(soul_hound.collision(true), (8.0, 25.2));
    }
}
