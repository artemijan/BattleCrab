//! Port of `data/xml/SayuneData` — the Sayune jump maps from
//! `data/SayuneData.xml`.
//!
//! A `<map>` holds a tree of points: a `<loc>` is one hop, a `<selector>` is a
//! junction whose `<choice>` children the player picks between, and any of the
//! three may carry further points inside it (the path that follows a choice).

use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

pub const SAYUNE_DATA_FILE: &str = "data/SayuneData.xml";

/// Java `SayuneEntry` — a map (`id` only, no position) or one point on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SayuneEntry {
    /// `<selector>`: the player chooses among `inner` from here.
    pub selector: bool,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Java `_innerEntries`.
    pub inner: Vec<SayuneEntry>,
}

impl SayuneEntry {
    pub fn xyz(&self) -> (i32, i32, i32) {
        (self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SayuneData {
    maps: HashMap<i32, SayuneEntry>,
}

impl SayuneData {
    pub fn load_from(file_path: &str) -> Self {
        let data = std::fs::read_to_string(format!("{file_path}{SAYUNE_DATA_FILE}"))
            .map(|content| Self::parse(&content))
            .unwrap_or_default();
        info!("SayuneData: Loaded {} maps.", data.maps.len());
        data
    }

    fn parse(content: &str) -> Self {
        let mut maps = HashMap::new();
        // The open `<map>` and, above it, every open point: a finished
        // element is added to the entry below it on the stack.
        let mut stack: Vec<SayuneEntry> = Vec::new();
        for event in xml::events(content) {
            let (e, open) = match &event {
                Event::Start(e) => (e, true),
                Event::Empty(e) => (e, false),
                Event::End(end) => {
                    if matches!(end.name().as_ref(), "map" | "selector" | "choice" | "loc")
                        && let Some(done) = stack.pop()
                    {
                        match stack.last_mut() {
                            Some(parent) => parent.inner.push(done),
                            None => {
                                maps.insert(done.id, done);
                            }
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            let name = e.name();
            let entry = match name.as_ref() {
                "map" if stack.is_empty() => SayuneEntry {
                    id: xml::attr_i32(e, "id").unwrap_or(0),
                    ..Default::default()
                },
                "selector" | "choice" | "loc" if !stack.is_empty() => SayuneEntry {
                    selector: name.as_ref() == "selector",
                    id: xml::attr_i32(e, "id").unwrap_or(0),
                    x: xml::attr_i32(e, "x").unwrap_or(0),
                    y: xml::attr_i32(e, "y").unwrap_or(0),
                    z: xml::attr_i32(e, "z").unwrap_or(0),
                    inner: Vec::new(),
                },
                _ => continue,
            };
            if open {
                stack.push(entry);
            } else {
                match stack.last_mut() {
                    Some(parent) => parent.inner.push(entry),
                    None => {
                        maps.insert(entry.id, entry);
                    }
                }
            }
        }
        Self { maps }
    }

    /// Java `getMap(id)`.
    pub fn map(&self, id: i32) -> Option<&SayuneEntry> {
        self.maps.get(&id)
    }
}
