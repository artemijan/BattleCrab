//! `data/residences/castles/*.xml` `<siegeGuards>` — the mercenary posting
//! tickets per castle (Java `CastleData._siegeGuards`). Only the
//! itemId → castle mapping is read: the port models no mercenary *hiring*,
//! but `ItemAction`'s pickup refusal must recognize a ticket lying on the
//! ground inside its castle's siege zone (a player can buy one from the
//! mercenary manager's list and drop it).

use crate::data::xml;
use std::collections::HashMap;

use quick_xml::events::Event;

const CASTLES_DIR: &str = "data/residences/castles";

/// Mercenary ticket item id → the castle it posts a guard for.
#[derive(Default, Clone)]
pub struct CastleSiegeGuards {
    tickets: HashMap<i32, i32>,
}

impl CastleSiegeGuards {
    pub fn load_from(file_path: &str) -> Self {
        let mut tickets = HashMap::new();
        let root = format!("{file_path}{CASTLES_DIR}");
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Self { tickets };
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "xml") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            parse_file(&content, &mut tickets);
        }
        Self { tickets }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, item_id: i32, castle_id: i32) {
        self.tickets.insert(item_id, castle_id);
    }

    /// Java `SiegeGuardManager.getSiegeGuardByItem(castleId, itemId) != null`.
    pub fn is_ticket_of(&self, castle_id: i32, item_id: i32) -> bool {
        self.tickets.get(&item_id) == Some(&castle_id)
    }
}

fn parse_file(content: &str, out: &mut HashMap<i32, i32>) {
    let mut castle_id = 0;
    for event in xml::events(content) {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name();
                let attr = |key: &[u8]| super::xml::attr_i32(&e, key);
                match name.as_ref() {
                    b"castle" => castle_id = attr(b"id").unwrap_or(0),
                    b"guard" => {
                        if castle_id > 0
                            && let Some(item_id) = attr(b"itemId")
                        {
                            out.insert(item_id, castle_id);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
