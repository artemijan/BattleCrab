//! Port of `data/xml/CategoryData` — `data/CategoryData.xml`'s named
//! class-id (and occasionally npc-id) sets, backing Java's
//! `player.isInCategory(CategoryType.X)` checks (the village-master
//! class-transfer gates are the first consumer).

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const CATEGORY_FILE: &str = "data/CategoryData.xml";

#[derive(Default)]
pub struct CategoryData {
    by_name: HashMap<String, HashSet<i32>>,
}

impl CategoryData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_name: HashMap<String, HashSet<i32>> = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{CATEGORY_FILE}")) {
            let mut reader = Reader::from_str(&content);
            let mut current: Option<String> = None;
            let mut in_id = false;
            while let Ok(event) = reader.read_event() {
                match event {
                    Event::Start(e) => match e.name().as_ref() {
                        b"category" => {
                            current = e
                                .attributes()
                                .flatten()
                                .find(|a| a.key.as_ref() == b"name")
                                .map(|a| String::from_utf8_lossy(&a.value).into_owned());
                        }
                        b"id" => in_id = true,
                        _ => {}
                    },
                    Event::Text(t) if in_id => {
                        if let (Some(name), Ok(id)) =
                            (current.as_ref(), String::from_utf8_lossy(&t.into_inner()).trim().parse::<i32>())
                        {
                            by_name.entry(name.clone()).or_default().insert(id);
                        }
                    }
                    Event::End(e) => match e.name().as_ref() {
                        b"category" => current = None,
                        b"id" => in_id = false,
                        _ => {}
                    },
                    Event::Eof => break,
                    _ => {}
                }
            }
        }
        info!("CategoryData: Loaded {} categories.", by_name.len());
        Self { by_name }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test hook.
    pub fn insert_for_test(&mut self, name: &str, ids: &[i32]) {
        self.by_name.insert(name.to_string(), ids.iter().copied().collect());
    }

    /// `CategoryData.isInCategory(type, id)`.
    pub fn contains(&self, category: &str, id: i32) -> bool {
        self.by_name.get(category).is_some_and(|s| s.contains(&id))
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_file() {
        let data = CategoryData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(data.len() > 50, "expected many categories, got {}", data.len());
        // Orc Fighter (44) is a fighter, not yet second class.
        assert!(data.contains("FIGHTER_GROUP", 44));
        assert!(!data.contains("SECOND_CLASS_GROUP", 44));
        // Orc Raider (45) has done the first transfer.
        assert!(data.contains("SECOND_CLASS_GROUP", 45));
        // Orc Mystic (49) is a mage.
        assert!(data.contains("MAGE_GROUP", 49));
    }
}
