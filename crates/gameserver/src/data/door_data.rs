//! Port of `data/xml/DoorData` — parses `data/DoorData.xml` (1180 doors on
//! this dist). Java flattens every child element's attributes into one
//! `StatSet` (`parseDoor`), so `openMethod`/`closeTime` living on
//! `<openStatus>`, `height` on `<location>`, `targetable` on `<status>` all
//! land in the same bag; this parser does the same.
//!
//! Not carried: `masterClose` events (80 doors — the "closing door X also
//! closes door Y" cascade waits for a consumer), `isWall` (20 doors — only
//! read by siege attackability), and the group/childId/emitter machinery
//! Java parses but this dist's XML never uses.

use std::collections::HashMap;

use crate::data::xml::attr_i32;
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const DOOR_FILE: &str = "data/DoorData.xml";

/// `enums/DoorOpenType` narrowed to the methods this dist declares
/// (`BY_CYCLE` exists in the enum but appears in no door).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoorOpenMethod {
    /// No `openMethod` attribute — the door only moves when a script/system
    /// calls open/close (most of the 1180).
    #[default]
    None,
    ByClick,
    ByItem,
    BySkill,
    ByTime,
}

/// Java `DoorTemplate` narrowed to the fields with data behind them.
#[derive(Debug, Clone)]
pub struct DoorTemplate {
    pub id: i32,
    pub name: String,
    /// Collision polygon (`<nodes>` — always 4 points in this dist).
    pub node_x: [i32; 4],
    pub node_y: [i32; 4],
    pub node_z: i32,
    /// Collision height above `node_z` (`<location height>`, default 150).
    pub height: i32,
    /// Spawn location; Java clamps `z` to `min(z, nodeZ)`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp_max: i32,
    pub p_def: i32,
    pub m_def: i32,
    pub targetable: bool,
    pub show_hp: bool,
    /// `<openStatus default>` — "open" or "close" (every door declares it).
    pub open_by_default: bool,
    pub open_method: DoorOpenMethod,
    /// BY_TIME cycle (seconds): stays open `open_time`, closed `close_time`,
    /// plus a random 0..`random_time` spread. `close_time` doubles as the
    /// auto-close delay for script-opened doors (−1 = never).
    pub open_time: i32,
    pub close_time: i32,
    pub random_time: i32,
}

impl DoorTemplate {
    pub fn z_min(&self) -> i32 {
        self.node_z
    }
    pub fn z_max(&self) -> i32 {
        self.node_z + self.height
    }
}

#[derive(Default)]
pub struct DoorData {
    pub doors: Vec<DoorTemplate>,
    by_id: HashMap<i32, u32>,
}

impl DoorData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::default();
        parse_file(&format!("{file_path}{DOOR_FILE}"), &mut data.doors);
        data.rebuild_index();
        info!("DoorData: Loaded {} doors.", data.doors.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test hook: register a door directly.
    pub fn insert_for_test(&mut self, door: DoorTemplate) {
        self.doors.push(door);
        self.rebuild_index();
    }

    fn rebuild_index(&mut self) {
        self.by_id = self
            .doors
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id, i as u32))
            .collect();
    }

    pub fn get(&self, door_id: i32) -> Option<&DoorTemplate> {
        self.by_id.get(&door_id).map(|&i| &self.doors[i as usize])
    }
}

fn parse_file(path: &str, out: &mut Vec<DoorTemplate>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    // Flat per-door attribute bag (Java's StatSet) + the node list.
    let mut attrs: HashMap<String, String> = HashMap::new();
    let mut xs: Vec<i32> = Vec::new();
    let mut ys: Vec<i32> = Vec::new();
    let mut in_door = false;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"door" && in_door {
                    in_door = false;
                    if let Some(door) = build_door(&attrs, &xs, &ys) {
                        out.push(door);
                    }
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"door" => {
                in_door = true;
                attrs.clear();
                xs.clear();
                ys.clear();
                absorb_attrs(&e, &mut attrs);
            }
            b"node" if in_door => {
                xs.push(attr_i32(&e, b"x").unwrap_or(0));
                ys.push(attr_i32(&e, b"y").unwrap_or(0));
            }
            // <nodes>/<location>/<stats>/<status>/<openStatus>/<event> — all
            // attributes flatten into the same bag, like Java's parseDoor.
            _ if in_door => absorb_attrs(&e, &mut attrs),
            _ => {}
        }
    }
}

fn build_door(attrs: &HashMap<String, String>, xs: &[i32], ys: &[i32]) -> Option<DoorTemplate> {
    if xs.len() < 4 || ys.len() < 4 {
        return None; // Java assumes exactly 4 collision nodes, as does the dist.
    }
    let get_i32 = |k: &str| attrs.get(k).and_then(|v| v.parse::<i32>().ok());
    let get_bool = |k: &str, dflt: bool| attrs.get(k).map(|v| v == "true").unwrap_or(dflt);
    let id = get_i32("id")?;
    let node_z = get_i32("nodeZ")?;
    let z = get_i32("z")?;
    Some(DoorTemplate {
        id,
        name: attrs.get("name").cloned().unwrap_or_default(),
        node_x: [xs[0], xs[1], xs[2], xs[3]],
        node_y: [ys[0], ys[1], ys[2], ys[3]],
        node_z,
        height: get_i32("height").unwrap_or(150),
        x: get_i32("x")?,
        y: get_i32("y")?,
        z: z.min(node_z),
        hp_max: get_i32("baseHpMax").unwrap_or(1),
        p_def: get_i32("basePDef").unwrap_or(0),
        m_def: get_i32("baseMDef").unwrap_or(0),
        targetable: get_bool("targetable", true),
        show_hp: get_bool("showHp", true),
        open_by_default: attrs.get("default").map(|v| v == "open").unwrap_or(false),
        open_method: match attrs.get("openMethod").map(String::as_str) {
            Some("BY_CLICK") => DoorOpenMethod::ByClick,
            Some("BY_ITEM") => DoorOpenMethod::ByItem,
            Some("BY_SKILL") => DoorOpenMethod::BySkill,
            Some("BY_TIME") => DoorOpenMethod::ByTime,
            _ => DoorOpenMethod::None,
        },
        open_time: get_i32("openTime").unwrap_or(0),
        close_time: get_i32("closeTime").unwrap_or(-1),
        random_time: get_i32("randomTime").unwrap_or(-1),
    })
}

fn absorb_attrs(e: &quick_xml::events::BytesStart, out: &mut HashMap<String, String>) {
    for a in e.attributes().flatten() {
        out.insert(
            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
            String::from_utf8_lossy(&a.value).into_owned(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_file() {
        let data = DoorData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert_eq!(data.doors.len(), 1180);

        // First door in the file: Embryo fort raid door 12220001.
        let d = data.get(12220001).expect("door 12220001");
        assert_eq!(d.node_z, 4624);
        assert_eq!(d.node_x, [-245777, -245473, -245472, -245776]);
        assert_eq!((d.x, d.y), (-245489, 148819));
        assert_eq!(d.z, 4624, "z clamped to min(4640, nodeZ 4624)");
        assert_eq!(d.hp_max, 3258432);
        assert_eq!(d.p_def, 100000);
        assert_eq!(d.m_def, 5000);
        assert!(!d.targetable);
        assert!(!d.show_hp);
        assert!(!d.open_by_default);
        assert_eq!(d.open_method, DoorOpenMethod::None);
        assert_eq!(d.height, 150, "default height");
        assert_eq!(d.z_max(), 4624 + 150);

        // A BY_TIME cycler (Valos dungeon).
        let t = data
            .doors
            .iter()
            .find(|d| d.open_method == DoorOpenMethod::ByTime)
            .expect("a BY_TIME door");
        assert!(t.open_time > 0 && t.close_time > 0);

        // The Gludin clan-hall doors are BY_CLICK with default targetable.
        let c = data.get(17220001).expect("door 17220001");
        assert_eq!(c.open_method, DoorOpenMethod::ByClick);
        assert!(c.targetable, "targetable defaults true when absent");

        // Method histogram matches the raw attribute counts.
        let by = |m: DoorOpenMethod| data.doors.iter().filter(|d| d.open_method == m).count();
        assert_eq!(by(DoorOpenMethod::ByClick), 160);
        assert_eq!(by(DoorOpenMethod::ByItem), 13);
        assert_eq!(by(DoorOpenMethod::BySkill), 34);
        assert_eq!(by(DoorOpenMethod::ByTime), 111);
        assert_eq!(data.doors.iter().filter(|d| d.open_by_default).count(), 96);
    }
}
