//! Port of `data/xml/ActionData` — the action-id list the client needs for
//! `ExBasicActionList` (the default action bar), plus each row's
//! `handler`/`option` pair, which is what `RequestActionUse` dispatches on
//! (Java `ActionDataHolder`).
//!
//! **Departure from the `ItemHandler` pattern.** `data/item_data.rs` resolves
//! `<set name="handler">` to a typed enum at load time; this loader keeps the
//! raw string. The dispatcher in `game_loop/player_actions.rs` *is* the
//! registry — Java's `PlayerActionHandler` map — and it is cold, one lookup per
//! action-bar press, so a second name table buys nothing. Keeping the string is
//! what lets a handler the port has no arm for name itself in the log instead
//! of vanishing, which is the failure this loader used to enable: only
//! `ServitorSkillUse` was read, and every other row fell off an allow-list in
//! silence.

use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

pub const ACTION_DATA_FILE: &str = "data/ActionData.xml";

/// One `<action id= handler= option=>` row — Java's `ActionDataHolder`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionRow {
    /// `handler="…"`. **`None` is a real value in this file** (23 rows carry
    /// it), naming an action the client resolves by itself. Java registers no
    /// handler under that name either, so it takes `RequestActionUse`'s
    /// "couldn't find handler" branch and does nothing.
    pub handler: String,
    /// `option="…"` — Java's `ActionDataHolder.getOptionId()`. Absent means 0.
    /// Its meaning is the handler's: a social id for `SocialAction`, a skill id
    /// for `ServitorSkillUse`/`PetSkillUse`, a store type for `PrivateStore`.
    pub option: i32,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionData {
    action_ids: Vec<i32>,
    rows: HashMap<i32, ActionRow>,
}

impl ActionData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut action_ids = Vec::new();
        let mut rows: HashMap<i32, ActionRow> = HashMap::new();
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
                    rows.insert(
                        id,
                        ActionRow {
                            handler: attr(b"handler").unwrap_or_default(),
                            option: attr(b"option")
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(0),
                        },
                    );
                }
                _ => {}
            }
        }

        info!(
            "ActionData: Loaded {} action ids ({} servitor skills).",
            action_ids.len(),
            rows.values()
                .filter(|r| r.handler == "ServitorSkillUse")
                .count()
        );
        Self { action_ids, rows }
    }

    pub fn action_ids(&self) -> &[i32] {
        &self.action_ids
    }

    /// The row an action id dispatches through, if the file declares one.
    /// `None` here is Java's `actionHolder == null` — the fallback switch in
    /// `RequestActionUse.runImpl`.
    pub fn row(&self, action_id: i32) -> Option<&ActionRow> {
        self.rows.get(&action_id)
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            action_ids: Vec::new(),
            rows: HashMap::new(),
        }
    }

    /// The skill an action id tells the servitor to cast, if it is one of the
    /// `ServitorSkillUse` bindings. 105 rows ship on this dist; 13 name a skill
    /// one of the six summonable servitors actually has, which is what makes
    /// them reachable.
    pub fn servitor_skill(&self, action_id: i32) -> Option<i32> {
        self.row(action_id)
            .filter(|r| r.handler == "ServitorSkillUse")
            .map(|r| r.option)
    }

    /// The skill an action id tells the **pet** to cast (`PetSkillUse`, 57
    /// rows). Unlike the servitor bindings the option is a skill id whose
    /// *level* comes from the pet's own `PetData`, not from the row.
    pub fn pet_skill(&self, action_id: i32) -> Option<i32> {
        self.row(action_id)
            .filter(|r| r.handler == "PetSkillUse")
            .map(|r| r.option)
    }

    #[cfg(test)]
    pub fn insert_row_for_test(&mut self, action_id: i32, handler: &str, option: i32) {
        self.action_ids.push(action_id);
        self.rows.insert(
            action_id,
            ActionRow {
                handler: handler.to_string(),
                option,
            },
        );
    }

    #[cfg(test)]
    pub fn insert_servitor_skill_for_test(&mut self, action_id: i32, skill_id: i32) {
        self.insert_row_for_test(action_id, "ServitorSkillUse", skill_id);
    }
}
