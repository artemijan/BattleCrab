//! Port of `data/xml/StaticObjectData` — `data/StaticObjects.xml` (159
//! entries: town map panels, castle thrones/chairs, and a few flagpoles).
//! Pure decoration in this slice: they spawn, render via `StaticObjectInfo`,
//! and do nothing on click (the town-map dialog needs the community-board
//! plumbing, thrones need castles — both G14+). The `texture`/`map_x/y`
//! attributes only feed that click behavior, so they are not stored.

use crate::data::xml::attr_str;
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

pub const STATIC_OBJECT_FILE: &str = "data/StaticObjects.xml";

#[derive(Debug, Clone)]
pub struct StaticObjectTemplate {
    pub id: i32,
    pub name: String,
    /// 0 = town map, 1 = throne/chair, 2/3 = other decorations — only ever
    /// compared for the (unported) click behaviors; the info packet always
    /// writes type 0 like Java's `StaticObjectInfo(StaticObject)` ctor.
    pub kind: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Default)]
pub struct StaticObjectData {
    pub objects: Vec<StaticObjectTemplate>,
}

impl StaticObjectData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut objects = Vec::new();
        if let Ok(content) = std::fs::read_to_string(format!("{file_path}{STATIC_OBJECT_FILE}")) {
            let mut reader = Reader::from_str(&content);
            while let Ok(event) = reader.read_event() {
                let e = match event {
                    Event::Start(e) | Event::Empty(e) => e,
                    Event::Eof => break,
                    _ => continue,
                };
                if e.name().as_ref() != b"object" {
                    continue;
                }
                let attr = |key: &[u8]| attr_str(&e, key);
                let get_i32 = |key: &[u8]| attr(key).and_then(|v| v.parse::<i32>().ok());
                let (Some(id), Some(x), Some(y), Some(z)) =
                    (get_i32(b"id"), get_i32(b"x"), get_i32(b"y"), get_i32(b"z"))
                else {
                    continue;
                };
                objects.push(StaticObjectTemplate {
                    id,
                    name: attr(b"name").unwrap_or_default(),
                    kind: get_i32(b"type").unwrap_or(0),
                    x,
                    y,
                    z,
                });
            }
        }
        info!("StaticObjectData: Loaded {} static objects.", objects.len());
        Self { objects }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_file() {
        let data =
            StaticObjectData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // 159 <object> lines, 73 of them commented out ("custom static
        // objects" missing from the NA/EU client) — Java's DOM parse loads 86.
        assert_eq!(data.objects.len(), 86);
        let map = data
            .objects
            .iter()
            .find(|o| o.id == 17250001)
            .expect("talking island town map");
        assert_eq!(map.kind, 0);
        assert_eq!((map.x, map.y, map.z), (-82873, 244808, -3717));
        assert_eq!(map.name, "town_map_talking_1");
        assert_eq!(data.objects.iter().filter(|o| o.kind == 1).count(), 44);
    }
}
