//! Minimal port of `data/xml/SkillTreeData` — **only** what character creation
//! needs: the level-1 auto-get skills a new character starts with
//! (`initNewChar` → `getAvailableSkills(..., includeAutoGet=true)`).
//!
//! We read the base-class trees under `data/skillTrees/StartingClass/*.xml` and
//! keep the skills with `getLevel <= 1` and `skillLevel == 1` (these are all
//! `autoGet`). The full skill tree (learning skills while leveling, FS skills,
//! parent-class trees) belongs to the skill milestone (G7).
//! TODO(G7): full SkillTreeData (class progression, learn packets, sub-classes).

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const STARTING_CLASS_DIR: &str = "data/skillTrees/StartingClass";

/// A skill a character knows: `(skill_id, skill_level)`.
pub type Skill = (i32, i32);

pub struct SkillTreeData {
    /// Initial (level-1 auto-get) skills per base class id.
    initial: HashMap<i32, Vec<Skill>>,
}

impl SkillTreeData {
    pub fn load() -> Self {
        let mut initial: HashMap<i32, Vec<Skill>> = HashMap::new();
        if let Ok(dir) = std::fs::read_dir(STARTING_CLASS_DIR) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_tree(&path, &mut initial);
            }
        }
        let total: usize = initial.values().map(|v| v.len()).sum();
        info!("SkillTreeData: Loaded initial skills for {} classes ({total} skills).", initial.len());
        Self { initial }
    }

    /// The skills a freshly created character of `class_id` starts with.
    pub fn initial_skills(&self, class_id: i32) -> Vec<Skill> {
        self.initial.get(&class_id).cloned().unwrap_or_default()
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { initial: HashMap::new() }
    }
}

fn parse_tree(path: &std::path::Path, out: &mut HashMap<i32, Vec<Skill>>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let mut reader = Reader::from_str(&content);
    let mut current_class: Option<i32> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"skillTree" => {
                    current_class = attr_i32(&e, b"classId");
                }
                b"skill" => {
                    let Some(class_id) = current_class else { continue };
                    let skill_id = attr_i32(&e, b"skillId").unwrap_or(-1);
                    let skill_level = attr_i32(&e, b"skillLevel").unwrap_or(0);
                    let get_level = attr_i32(&e, b"getLevel").unwrap_or(99);
                    // Level-1 first-rank skills only (all such entries are autoGet).
                    if get_level <= 1 && skill_level == 1 && skill_id > 0 {
                        out.entry(class_id).or_default().push((skill_id, skill_level));
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

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| String::from_utf8_lossy(&a.value).parse().ok())
}
