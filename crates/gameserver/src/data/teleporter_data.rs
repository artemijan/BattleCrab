//! Port of `data/xml/TeleporterData` + `model/teleporter/{TeleportHolder,
//! TeleportLocation}` (G15.5): every gatekeeper's named teleport lists from
//! `data/teleporters/**` (town/castle/clanhall/fortress/others). The runtime
//! flow (`showTeleports`/`teleport` bypasses, fees) lives in
//! `game_loop/teleporter.rs`.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::{info, warn};

use super::item_data::ADENA_ID;
use crate::data::xml::attr_str;

pub const TELEPORTERS_DIR: &str = "data/teleporters";

/// Java `enums/TeleportType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportType {
    Normal,
    Hunting,
    NoblesToken,
    NoblesAdena,
    Other,
}

impl TeleportType {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "NORMAL" => Self::Normal,
            "HUNTING" => Self::Hunting,
            "NOBLES_TOKEN" => Self::NoblesToken,
            "NOBLES_ADENA" => Self::NoblesAdena,
            "OTHER" => Self::Other,
            _ => return None,
        })
    }

    /// The list's default name when the XML gives none (`type.name()`).
    pub fn default_name(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Hunting => "HUNTING",
            Self::NoblesToken => "NOBLES_TOKEN",
            Self::NoblesAdena => "NOBLES_ADENA",
            Self::Other => "OTHER",
        }
    }
}

/// Java `TeleportLocation` — one destination line. `id` is the position in
/// the holder's list (Java `registerLocation`'s running index), which is what
/// the html buttons send back.
#[derive(Debug, Clone)]
pub struct TeleportLocation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Display name — most dist lines use `npcStringId` instead.
    pub name: Option<String>,
    /// Client fstring id for the destination name (-1 = none, use `name`).
    pub npc_string_id: i32,
    pub fee_id: i32,
    pub fee_count: i64,
    /// `castleId` — the castle(s) whose siege blocks this destination
    /// (Java `TeleportLocation._castleId`, a `;`-separated list). Empty for
    /// most lines.
    pub castle_ids: Vec<i32>,
}

/// Java `TeleportHolder` minus the html/teleport behavior (game-loop side).
#[derive(Debug, Clone)]
pub struct TeleportHolder {
    pub name: String,
    pub teleport_type: TeleportType,
    pub locations: Vec<TeleportLocation>,
}

impl TeleportHolder {
    /// `TeleportHolder.isNoblesse()`.
    pub fn is_noblesse(&self) -> bool {
        matches!(
            self.teleport_type,
            TeleportType::NoblesToken | TeleportType::NoblesAdena
        )
    }

    /// `TeleportHolder.isNormalTeleport()` — the lists whose fee waives below
    /// the free-teleport level.
    pub fn is_normal_teleport(&self) -> bool {
        matches!(
            self.teleport_type,
            TeleportType::Normal | TeleportType::Hunting
        )
    }
}

/// npc template id → list name → holder.
pub struct TeleporterData {
    teleporters: HashMap<i32, HashMap<String, TeleportHolder>>,
}

impl TeleporterData {
    pub fn load_from(file_path: &str) -> Self {
        let mut teleporters = HashMap::new();
        for path in &super::xml::xml_files_under(format!("{file_path}{TELEPORTERS_DIR}")) {
            parse_file(path, &mut teleporters);
        }
        info!(
            "TeleporterData: Loaded {} npc teleporters.",
            teleporters.len()
        );
        Self { teleporters }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            teleporters: HashMap::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, npc_id: i32, holder: TeleportHolder) {
        self.teleporters
            .entry(npc_id)
            .or_default()
            .insert(holder.name.clone(), holder);
    }

    /// `TeleporterData.getHolder(npcId, listName)`.
    pub fn holder(&self, npc_id: i32, list_name: &str) -> Option<&TeleportHolder> {
        self.teleporters.get(&npc_id)?.get(list_name)
    }

    pub fn teleporter_count(&self) -> usize {
        self.teleporters.len()
    }
}

fn parse_file(path: &std::path::Path, out: &mut HashMap<i32, HashMap<String, TeleportHolder>>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    // Per `<npc>` element: the named lists being built plus the alias ids from
    // a nested `<npcs><npc id=…/></npcs>` block (which shares the same lists).
    let mut npc_id: Option<i32> = None;
    let mut lists: HashMap<String, TeleportHolder> = HashMap::new();
    let mut alias_ids: Vec<i32> = Vec::new();
    let mut in_npcs = false;
    let mut current: Option<TeleportHolder> = None;

    loop {
        let event = match reader.read_event() {
            Ok(e) => e,
            Err(_) => break,
        };
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let attr = |key: &[u8]| attr_str(&e, key);
                match e.name().as_ref() {
                    b"npcs" => in_npcs = true,
                    b"npc" if in_npcs => {
                        if let Some(id) = attr(b"id").and_then(|v| v.parse().ok()) {
                            alias_ids.push(id);
                        }
                    }
                    b"npc" => {
                        npc_id = attr(b"id").and_then(|v| v.parse().ok());
                        lists.clear();
                        alias_ids.clear();
                    }
                    b"teleport" => {
                        let Some(teleport_type) =
                            attr(b"type").as_deref().and_then(TeleportType::parse)
                        else {
                            warn!("TeleporterData: bad teleport type in {}.", path.display());
                            continue;
                        };
                        let name = attr(b"name")
                            .unwrap_or_else(|| teleport_type.default_name().to_string());
                        current = Some(TeleportHolder {
                            name,
                            teleport_type,
                            locations: Vec::new(),
                        });
                    }
                    b"location" => {
                        if let Some(holder) = current.as_mut() {
                            let num = |key: &[u8]| attr(key).and_then(|v| v.parse::<i64>().ok());
                            let (Some(x), Some(y), Some(z)) = (num(b"x"), num(b"y"), num(b"z"))
                            else {
                                continue;
                            };
                            holder.locations.push(TeleportLocation {
                                x: x as i32,
                                y: y as i32,
                                z: z as i32,
                                name: attr(b"name"),
                                npc_string_id: num(b"npcStringId").unwrap_or(-1) as i32,
                                castle_ids: attr(b"castleId")
                                    .map(|v| {
                                        v.split(';')
                                            .filter_map(|p| p.trim().parse::<i32>().ok())
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                fee_id: num(b"feeId").unwrap_or(ADENA_ID as i64) as i32,
                                fee_count: num(b"feeCount").unwrap_or(0),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"npcs" => in_npcs = false,
                b"teleport" => {
                    if let Some(holder) = current.take() {
                        // Java warns on duplicates and keeps the first
                        // (`putIfAbsent`).
                        if lists.contains_key(&holder.name) {
                            warn!(
                                "TeleporterData: duplicate teleport list ({}) in {}.",
                                holder.name,
                                path.display()
                            );
                        } else {
                            lists.insert(holder.name.clone(), holder);
                        }
                    }
                }
                b"npc" if !in_npcs => {
                    if let Some(id) = npc_id.take() {
                        for alias in alias_ids.drain(..) {
                            out.insert(alias, lists.clone());
                        }
                        out.insert(id, std::mem::take(&mut lists));
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_real_dist_roxxy() {
        let data = TeleporterData::load_from(DIST);
        // Roxxy (Talking Island gatekeeper) — 9 NORMAL destinations.
        let holder = data.holder(30006, "NORMAL").expect("Roxxy NORMAL list");
        assert_eq!(holder.teleport_type, TeleportType::Normal);
        assert!(holder.is_normal_teleport());
        assert!(!holder.is_noblesse());
        assert_eq!(holder.locations.len(), 9);
        // First line: The Village of Gludin, hand-checked from Roxxy.xml.
        let gludin = &holder.locations[0];
        assert_eq!((gludin.x, gludin.y, gludin.z), (-80684, 149770, -3040));
        assert_eq!(gludin.npc_string_id, 1010004);
        assert_eq!(gludin.fee_id, ADENA_ID);
        assert_eq!(gludin.fee_count, 9400);
        // Unknown list name misses.
        assert!(data.holder(30006, "HUNTING").is_none());
    }

    #[test]
    fn loads_every_dist_file() {
        let data = TeleporterData::load_from(DIST);
        // The dist ships teleport lists for a few hundred NPCs across the
        // town/castle/clanhall/fortress/others subdirectories.
        assert!(
            data.teleporter_count() > 100,
            "unexpectedly few teleporters: {}",
            data.teleporter_count()
        );
    }
}
