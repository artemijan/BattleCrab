//! `data/residences/castles/*.xml` `<siegeGuards>` — the mercenary posting
//! tickets per castle (Java `CastleData._siegeGuards` +
//! `SiegeGuardHolder`). 499 `<guard>` rows ship across the nine castles.
//!
//! Each row binds a **ticket item** to the **guard npc** it posts, how many of
//! that guard a castle may field, and whether it holds its ground. The loader
//! read only `itemId → castleId` until the mercenary system landed, because
//! that was all `ItemAction`'s pickup refusal needed.

use crate::data::xml;
use std::collections::HashMap;

use quick_xml::events::Event;

const CASTLES_DIR: &str = "data/residences/castles";

/// One `<guard>` row — Java `SiegeGuardHolder`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SiegeGuardHolder {
    pub castle_id: i32,
    pub item_id: i32,
    pub npc_id: i32,
    /// `npcMaxAmount` — how many of *this* guard the castle may have posted at
    /// once (Java `isAtNpcLimit`).
    pub max_npc_amount: i32,
    /// `stationary="true"` — the mercenary holds its post instead of roaming
    /// (Java `npc.setImmobilized(holder.isStationary())`).
    pub stationary: bool,
}

/// Mercenary ticket item id → the guard it posts.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CastleSiegeGuards {
    tickets: HashMap<i32, SiegeGuardHolder>,
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
        self.tickets.insert(
            item_id,
            SiegeGuardHolder {
                castle_id,
                item_id,
                npc_id: 35030,
                max_npc_amount: 2,
                stationary: true,
            },
        );
    }

    #[cfg(test)]
    pub fn insert_holder_for_test(&mut self, holder: SiegeGuardHolder) {
        self.tickets.insert(holder.item_id, holder);
    }

    /// Java `SiegeGuardManager.getSiegeGuardByItem(castleId, itemId)`.
    pub fn by_item(&self, castle_id: i32, item_id: i32) -> Option<&SiegeGuardHolder> {
        self.tickets
            .get(&item_id)
            .filter(|h| h.castle_id == castle_id)
    }

    /// Java `SiegeGuardManager.getSiegeGuardByNpc(castleId, npcId)`.
    pub fn by_npc(&self, castle_id: i32, npc_id: i32) -> Option<&SiegeGuardHolder> {
        self.tickets
            .values()
            .find(|h| h.castle_id == castle_id && h.npc_id == npc_id)
    }

    /// Java `SiegeGuardManager.getSiegeGuardByItem(castleId, itemId) != null`.
    pub fn is_ticket_of(&self, castle_id: i32, item_id: i32) -> bool {
        self.by_item(castle_id, item_id).is_some()
    }
}

fn parse_file(content: &str, out: &mut HashMap<i32, SiegeGuardHolder>) {
    let mut castle_id = 0;
    for event in xml::events(content) {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name();
                let attr = |key: &[u8]| xml::attr_i32(&e, key);
                match name.as_ref() {
                    b"castle" => castle_id = attr(b"id").unwrap_or(0),
                    b"guard" => {
                        if castle_id > 0
                            && let Some(item_id) = attr(b"itemId")
                        {
                            out.insert(
                                item_id,
                                SiegeGuardHolder {
                                    castle_id,
                                    item_id,
                                    npc_id: attr(b"npcId").unwrap_or(0),
                                    max_npc_amount: attr(b"npcMaxAmount").unwrap_or(0),
                                    stationary: xml::attr_str(&e, b"stationary").as_deref()
                                        == Some("true"),
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
