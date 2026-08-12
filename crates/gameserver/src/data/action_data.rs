//! Port of `data/xml/ActionData` — the action-id list the client needs for
//! `ExBasicActionList` (the default action bar), plus the `ServitorSkillUse`
//! bindings.
//!
//! An action row is `id → handler(option)`. Only the servitor-skill handler is
//! resolved here; the rest are dispatched by hard-coded id in the game loop,
//! because they are one-offs (sit, private store, emotes) rather than a table.

use crate::data::xml;
use quick_xml::events::Event;
use tracing::info;

pub const ACTION_DATA_FILE: &str = "data/ActionData.xml";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionData {
    action_ids: Vec<i32>,
    /// `handler="ServitorSkillUse" option="<skillId>"` — the summon's
    /// action-bar buttons. 105 rows ship on this dist; 13 name a skill the six
    /// summonable servitors actually have, which is what makes them reachable.
    servitor_skills: std::collections::HashMap<i32, i32>,
}

impl ActionData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut action_ids = Vec::new();
        let mut servitor_skills = std::collections::HashMap::new();
        let full_path = format!("{file_path}{ACTION_DATA_FILE}");
        let content = std::fs::read_to_string(&full_path).unwrap_or_else(|e| {
            let cwd = std::env::current_dir().unwrap();
            panic!("ActionData: cannot read {full_path}: {e}, CWD: {cwd:?}")
        });
        for event in xml::events(&content) {
            match event {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"action" => {
                    let attr = |key: &[u8]| super::xml::attr_str(&e, key);
                    let Some(id) = attr(b"id").and_then(|v| v.parse::<i32>().ok()) else {
                        continue;
                    };
                    action_ids.push(id);
                    if attr(b"handler").as_deref() == Some("ServitorSkillUse")
                        && let Some(skill_id) = attr(b"option").and_then(|v| v.parse::<i32>().ok())
                    {
                        servitor_skills.insert(id, skill_id);
                    }
                }
                _ => {}
            }
        }

        info!(
            "ActionData: Loaded {} action ids ({} servitor skills).",
            action_ids.len(),
            servitor_skills.len()
        );
        Self {
            action_ids,
            servitor_skills,
        }
    }

    pub fn action_ids(&self) -> &[i32] {
        &self.action_ids
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            action_ids: Vec::new(),
            servitor_skills: std::collections::HashMap::new(),
        }
    }

    /// The skill an action id tells the servitor to cast, if it is one of the
    /// `ServitorSkillUse` bindings.
    pub fn servitor_skill(&self, action_id: i32) -> Option<i32> {
        self.servitor_skills.get(&action_id).copied()
    }

    #[cfg(test)]
    pub fn insert_servitor_skill_for_test(&mut self, action_id: i32, skill_id: i32) {
        self.servitor_skills.insert(action_id, skill_id);
    }
}
