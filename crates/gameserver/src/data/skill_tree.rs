//! Port of `data/xml/SkillTreeData` — scoped to base-class skill trees
//! (`data/skillTrees/StartingClass/*.xml`): the level-1 auto-get skills a new
//! character starts with (`initNewChar`) plus the full class-skill
//! progression driving `RequestAcquireSkill`'s `CLASS` learn path (G6).
//!
//! Confirmed no `preReqSkill`/`item` children exist anywhere in
//! `StartingClass/*.xml`, so those Java code paths (prerequisite skills,
//! required consumables) stay out of scope — every entry here is gated by
//! `getLevel`/`levelUpSp` alone. Parent-class trees (`ClassChange/`, 2nd/3rd
//! class transfer) are out of scope too — G3/G4 only build base classes.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const STARTING_CLASS_DIR: &str = "data/skillTrees/StartingClass";

/// A skill a character knows: `(skill_id, skill_level)`.
pub type Skill = (i32, i32);

/// One `<skill>` entry from a class tree (Java `SkillLearn`, trimmed to the
/// fields the `CLASS` learn path needs).
#[derive(Debug, Clone)]
pub struct SkillLearn {
    pub skill_id: i32,
    pub skill_level: i32,
    pub name: String,
    /// Minimum character level to learn this entry (`getLevel` attribute).
    pub get_level: i32,
    /// SP cost (0 for autoGet skills, which are granted free at creation).
    pub level_up_sp: i64,
    pub auto_get: bool,
}

pub struct SkillTreeData {
    /// Every `<skill>` entry per base class id, in document order.
    trees: HashMap<i32, Vec<SkillLearn>>,
}

impl SkillTreeData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut trees: HashMap<i32, Vec<SkillLearn>> = HashMap::new();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{STARTING_CLASS_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_tree(&path, &mut trees);
            }
        }
        let total: usize = trees.values().map(|v| v.len()).sum();
        info!(
            "SkillTreeData: Loaded skill trees for {} classes ({total} skill levels).",
            trees.len()
        );
        Self { trees }
    }

    /// The skills a freshly created character of `class_id` starts with
    /// (Java `getAvailableSkills(..., includeAutoGet=true)` at level 1).
    pub fn initial_skills(&self, class_id: i32) -> Vec<Skill> {
        self.trees
            .get(&class_id)
            .map(|entries| entries.iter().filter(|s| s.auto_get).map(|s| (s.skill_id, s.skill_level)).collect())
            .unwrap_or_default()
    }

    /// Java `SkillTreeData.getAvailableSkills` narrowed to `AcquireSkillType.CLASS`:
    /// every entry whose `getLevel` the player has reached and whose
    /// `skillLevel` is exactly one past what they currently know (0 for an
    /// unknown skill, so `skillLevel == 1` is "new"). autoGet entries are
    /// already granted at creation, so they never reappear here once known.
    pub fn available_skills(&self, class_id: i32, level: i32, known: &HashMap<i32, i32>) -> Vec<&SkillLearn> {
        let Some(entries) = self.trees.get(&class_id) else { return Vec::new() };
        entries
            .iter()
            .filter(|s| !s.auto_get && s.get_level <= level)
            .filter(|s| s.skill_level == known.get(&s.skill_id).copied().unwrap_or(0) + 1)
            .collect()
    }

    /// Java `Player.rewardSkills` → `getAvailableAutoGetSkills`: every autoGet
    /// entry reachable at `level`. The caller keeps only levels above what the
    /// player already knows.
    pub fn auto_get_skills(&self, class_id: i32, level: i32) -> Vec<&SkillLearn> {
        let Some(entries) = self.trees.get(&class_id) else { return Vec::new() };
        entries.iter().filter(|s| s.auto_get && s.get_level <= level).collect()
    }

    /// Java `SkillTreeData.getAllAvailableSkills` (the `AutoLearnSkills` path,
    /// `includeAutoGet=true`) narrowed to base-class trees, which carry no
    /// forgotten-scroll / removeSkills / required-item entries. For every skill
    /// in the class tree it returns the highest `skill_level` whose `get_level`
    /// the player has reached — but only when that beats what they already
    /// know. Java grants levels one at a time through a holder until it stops
    /// changing; since `get_level` rises with `skill_level`, taking the max
    /// reachable level per skill lands on the same final state in one pass.
    pub fn all_available_skills(&self, class_id: i32, level: i32, known: &HashMap<i32, i32>) -> Vec<Skill> {
        let Some(entries) = self.trees.get(&class_id) else { return Vec::new() };
        let mut best: HashMap<i32, i32> = HashMap::new();
        for s in entries.iter().filter(|s| s.get_level <= level) {
            let slot = best.entry(s.skill_id).or_insert(0);
            *slot = (*slot).max(s.skill_level);
        }
        best.into_iter().filter(|&(id, lvl)| lvl > known.get(&id).copied().unwrap_or(0)).collect()
    }

    /// The `SkillLearn` for a specific `(class_id, skill_id, skill_level)`,
    /// used by `RequestAcquireSkill` to re-validate the client's request.
    pub fn skill_learn(&self, class_id: i32, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
        self.trees
            .get(&class_id)?
            .iter()
            .find(|s| s.skill_id == skill_id && s.skill_level == skill_level)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { trees: HashMap::new() }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, class_id: i32, learn: SkillLearn) {
        self.trees.entry(class_id).or_default().push(learn);
    }
}

fn parse_tree(path: &std::path::Path, out: &mut HashMap<i32, Vec<SkillLearn>>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);
    let mut current_class: Option<i32> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"skillTree" => {
                    current_class = attr_i32(&e, b"classId");
                }
                b"skill" => {
                    let Some(class_id) = current_class else {
                        continue;
                    };
                    let skill_id = attr_i32(&e, b"skillId").unwrap_or(-1);
                    let skill_level = attr_i32(&e, b"skillLevel").unwrap_or(0);
                    let get_level = attr_i32(&e, b"getLevel").unwrap_or(99);
                    let level_up_sp = attr_i64(&e, b"levelUpSp").unwrap_or(0);
                    let name = attr_str(&e, b"skillName").unwrap_or_default();
                    let auto_get = attr_str(&e, b"autoGet").as_deref() == Some("true") || (get_level <= 1 && skill_level == 1);
                    if skill_id > 0 {
                        out.entry(class_id).or_default().push(SkillLearn {
                            skill_id,
                            skill_level,
                            name,
                            get_level,
                            level_up_sp,
                            auto_get,
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

fn attr_i64(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i64> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learn(skill_id: i32, skill_level: i32, get_level: i32, level_up_sp: i64) -> SkillLearn {
        SkillLearn { skill_id, skill_level, name: String::new(), get_level, level_up_sp, auto_get: false }
    }

    #[test]
    fn available_skills_gates_by_level_and_next_skill_level_only() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(3, 1, 5, 50));

        // Below the level gate: nothing learnable.
        assert!(data.available_skills(0, 4, &HashMap::new()).is_empty());

        // At the gate: both level-1 entries, not the level-2 (still gated by
        // its own getLevel *and* by not knowing level 1 yet).
        let known = HashMap::new();
        let mut ids: Vec<i32> = data.available_skills(0, 5, &known).iter().map(|s| s.skill_id).collect();
        ids.sort();
        assert_eq!(ids, vec![3, 91]);

        // Already knowing skill 91 at level 1: level 5 no longer offers 91
        // level 1 again (would need level 10 for the next level); skill 3
        // (still unknown) stays available regardless.
        let mut known = HashMap::new();
        known.insert(91, 1);
        let ids_at_5: Vec<i32> = data.available_skills(0, 5, &known).iter().map(|s| s.skill_id).collect();
        assert_eq!(ids_at_5, vec![3]);
        let mut ids_at_10: Vec<i32> = data.available_skills(0, 10, &known).iter().map(|s| s.skill_id).collect();
        ids_at_10.sort();
        assert_eq!(ids_at_10, vec![3, 91], "91's next level (getLevel=10) now shows up alongside still-unlearned 3");
    }

    #[test]
    fn all_available_skills_returns_highest_reachable_level_per_skill() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(91, 3, 20, 300));
        data.insert_for_test(0, learn(3, 1, 5, 50));
        let auto = SkillLearn { auto_get: true, ..learn(1000, 1, 1, 0) };
        data.insert_for_test(0, auto);

        // Level below any gate: nothing but the level-1 autoGet skill.
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 1, &HashMap::new());
        got.sort();
        assert_eq!(got, vec![(1000, 1)]);

        // At level 10, skill 91 jumps straight to its level-2 max (not 1), plus
        // skill 3 and the autoGet — all in one pass (Java's holder loop).
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 10, &HashMap::new());
        got.sort();
        assert_eq!(got, vec![(3, 1), (91, 2), (1000, 1)]);

        // Already knowing 91 at level 2 and the autoGet: only the still-missing
        // skill 3, and — at level 20 — 91's level-3 upgrade.
        let mut known = HashMap::new();
        known.insert(91, 2);
        known.insert(1000, 1);
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 20, &known);
        got.sort();
        assert_eq!(got, vec![(3, 1), (91, 3)]);
    }
}
