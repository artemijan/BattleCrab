//! Port of `data/xml/InitialEquipmentData` — the starting gear a freshly
//! created character of a given class receives (`data/stats/initialEquipment.xml`).

use std::collections::HashMap;

use crate::data::xml::{attr_i32, attr_str};
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const INITIAL_EQUIPMENT_FILE: &str = "data/stats/initialEquipment.xml";

#[derive(Debug, Clone, Copy)]
pub struct InitialEquipmentItem {
    pub item_id: i32,
    pub count: i64,
    pub equipped: bool,
}

pub struct InitialEquipmentData {
    by_class: HashMap<i32, Vec<InitialEquipmentItem>>,
}

impl InitialEquipmentData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_class: HashMap<i32, Vec<InitialEquipmentItem>> = HashMap::new();
        let full_path = format!("{file_path}{INITIAL_EQUIPMENT_FILE}");
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let mut reader = Reader::from_str(&content);
            let mut cur_class: Option<i32> = None;
            loop {
                match reader.read_event() {
                    Ok(Event::Start(e)) if e.name().as_ref() == b"equipment" => {
                        cur_class = attr_i32(&e, b"classId");
                    }
                    Ok(Event::End(e)) if e.name().as_ref() == b"equipment" => {
                        cur_class = None;
                    }
                    Ok(Event::Empty(e)) if e.name().as_ref() == b"item" => {
                        let Some(class_id) = cur_class else { continue };
                        let Some(item_id) = attr_i32(&e, b"id") else {
                            continue;
                        };
                        let count = attr_i32(&e, b"count").unwrap_or(1) as i64;
                        let equipped = attr_str(&e, b"equipped").as_deref() == Some("true");
                        by_class
                            .entry(class_id)
                            .or_default()
                            .push(InitialEquipmentItem {
                                item_id,
                                count,
                                equipped,
                            });
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
        let total: usize = by_class.values().map(|v| v.len()).sum();
        info!(
            "InitialEquipmentData: Loaded gear for {} classes ({total} item entries).",
            by_class.len()
        );
        Self { by_class }
    }

    pub fn get(&self, class_id: i32) -> &[InitialEquipmentItem] {
        self.by_class
            .get(&class_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            by_class: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_human_fighter_starting_gear() {
        let data = InitialEquipmentData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        let items = data.get(0); // Human Fighter
        assert!(!items.is_empty());
        let sword = items
            .iter()
            .find(|i| i.item_id == 2369)
            .expect("Squire's Sword");
        assert!(sword.equipped);
        let dagger = items.iter().find(|i| i.item_id == 10).expect("Dagger");
        assert!(!dagger.equipped);
    }
}
