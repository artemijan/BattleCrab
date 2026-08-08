//! `data/ResidenceFunctions.xml` — the clan-hall function upgrade catalogue
//! (Java `ResidenceFunctionsData`). Each function (HP_REGEN, TELEPORT, …) has a
//! ladder of levels, each with an item cost, a rental duration and a benefit
//! `value`.

use std::collections::HashMap;

use crate::data::xml::attr_str as attr;
use crate::data::xml::{attr_i32, attr_i64};
use quick_xml::events::Event;

/// One purchasable level of a function.
#[derive(Debug, Clone, Copy)]
pub struct FunctionLevel {
    pub cost_id: i32,
    pub cost_count: i64,
    /// The rental period in millis (`1days`/`2days`/`3days`).
    pub duration_ms: i64,
    /// The benefit magnitude (a regen multiplier, a teleport tier, …).
    pub value: f64,
}

/// One function id and its level ladder.
#[derive(Debug, Clone)]
pub struct ResidenceFunctionDef {
    pub func_id: i32,
    /// Java `ResidenceFunctionType` name (HP_REGEN, TELEPORT, …).
    pub func_type: String,
    pub levels: HashMap<i32, FunctionLevel>,
}

#[derive(Debug, Clone, Default)]
pub struct ResidenceFunctionData {
    by_id: HashMap<i32, ResidenceFunctionDef>,
}

impl ResidenceFunctionData {
    /// The template for a `(func_id, level)`, if defined.
    pub fn level(&self, func_id: i32, level: i32) -> Option<&FunctionLevel> {
        self.by_id.get(&func_id).and_then(|d| d.levels.get(&level))
    }

    /// A function's `ResidenceFunctionType` name.
    pub fn type_of(&self, func_id: i32) -> Option<&str> {
        self.by_id.get(&func_id).map(|d| d.func_type.as_str())
    }

    /// The function id for a `ResidenceFunctionType` name (`removeFunction` names
    /// the type, not the id).
    pub fn id_of_type(&self, type_name: &str) -> Option<i32> {
        self.by_id
            .values()
            .find(|d| d.func_type == type_name)
            .map(|d| d.func_id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id = HashMap::new();
        let path = format!("{file_path}data/ResidenceFunctions.xml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self { by_id };
        };
        let mut reader = quick_xml::Reader::from_str(&content);
        reader.config_mut().trim_text(true);
        let mut current: Option<ResidenceFunctionDef> = None;
        loop {
            match reader.read_event() {
                Ok(Event::Eof) | Err(_) => break,
                Ok(Event::Start(e)) if e.name().as_ref() == b"function" => {
                    // The outer <function id type> — start a new def.
                    current = Some(ResidenceFunctionDef {
                        func_id: attr_i32(&e, b"id").unwrap_or(0),
                        func_type: attr(&e, b"type").unwrap_or_default(),
                        levels: HashMap::new(),
                    });
                }
                Ok(Event::Empty(e)) if e.name().as_ref() == b"function" => {
                    // An inner <function level costId costCount duration value>.
                    if let Some(def) = current.as_mut()
                        && let Some(level) = attr_i32(&e, b"level")
                    {
                        def.levels.insert(
                            level,
                            FunctionLevel {
                                cost_id: attr_i32(&e, b"costId").unwrap_or(0),
                                cost_count: attr_i64(&e, b"costCount").unwrap_or(0),
                                duration_ms: parse_duration(
                                    &attr(&e, b"duration").unwrap_or_default(),
                                ),
                                value: attr(&e, b"value")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0.0),
                            },
                        );
                    }
                }
                Ok(Event::End(e)) if e.name().as_ref() == b"function" => {
                    if let Some(def) = current.take() {
                        by_id.insert(def.func_id, def);
                    }
                }
                _ => {}
            }
        }
        Self { by_id }
    }
}

/// `StatSet.getDuration` for the forms this file uses: `<n>days`.
fn parse_duration(s: &str) -> i64 {
    if let Some(days) = s.strip_suffix("days").and_then(|n| n.parse::<i64>().ok()) {
        days * 86_400_000
    } else {
        0
    }
}
