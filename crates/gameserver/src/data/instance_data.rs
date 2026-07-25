//! Port of `data/xml/InstanceData` — the `data/instances/**/*.xml` templates
//! (Java `InstanceManager.load` + `InstanceTemplate`). A template describes a
//! private world: its doors, spawn groups, the enter/exit locations, and the
//! activity / empty-destroy timers.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use tracing::info;

const INSTANCE_DIR: &str = "data/instances";

/// One NPC placement in a spawn group (`<npc id x y z heading/>`).
#[derive(Debug, Clone)]
pub struct TemplateSpawn {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// A named spawn group (`<group>`); `spawn_by_default` groups populate on
/// creation, others are triggered by the instance's script.
#[derive(Debug, Clone)]
pub struct SpawnGroup {
    pub name: String,
    pub spawn_by_default: bool,
    pub npcs: Vec<TemplateSpawn>,
}

/// Where a party is sent on leaving (`<exit>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitType {
    /// Back to where each member entered from (`type="ORIGIN"`).
    Origin,
    /// A fixed location.
    Fixed(i32, i32, i32),
}

/// A parsed instance template (Java `InstanceTemplate`).
#[derive(Debug, Clone)]
pub struct InstanceTemplate {
    pub id: i32,
    /// `maxWorlds` — concurrent copies allowed, or -1 for unlimited.
    pub max_worlds: i32,
    /// `<time duration>` — how long the instance stays up, in minutes (0 = none).
    pub duration_min: i32,
    /// `<time empty>` — how many minutes an emptied instance lingers before
    /// teardown (Java `TimeUnit.MINUTES`).
    pub empty_destroy_min: i32,
    /// `<enter>` location the party is teleported to.
    pub enter: Option<(i32, i32, i32)>,
    pub exit: ExitType,
    pub doors: Vec<i32>,
    pub groups: Vec<SpawnGroup>,
}

/// Every instance template, by id (Java `InstanceManager._instanceTemplates`).
#[derive(Debug, Default)]
pub struct InstanceData {
    by_id: HashMap<i32, InstanceTemplate>,
}

impl InstanceData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let mut by_id = HashMap::new();
        let mut files = Vec::new();
        collect_xml(&format!("{root}{INSTANCE_DIR}"), &mut files);
        for path in files {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(t) = parse(&content) {
                    by_id.insert(t.id, t);
                }
            }
        }
        info!("InstanceData: Loaded {} instance templates.", by_id.len());
        Self { by_id }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, id: i32) -> Option<&InstanceTemplate> {
        self.by_id.get(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, t: InstanceTemplate) {
        self.by_id.insert(t.id, t);
    }
}

/// Recursively gather every `.xml` under `dir` (instance templates live in
/// per-content subdirectories: `Bosses/`, `Olympiad/`, `custom/`).
fn collect_xml(dir: &str, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_xml(&path.to_string_lossy(), out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("xml") {
            out.push(path);
        }
    }
}

fn parse(content: &str) -> Option<InstanceTemplate> {
    let mut reader = Reader::from_str(content);
    let mut id = None;
    let mut max_worlds = -1;
    let mut duration_min = 0;
    let mut empty_destroy_min = 0;
    let mut enter = None;
    let mut exit = ExitType::Origin;
    let mut doors = Vec::new();
    let mut groups = Vec::new();
    let mut group: Option<SpawnGroup> = None;
    let mut in_enter = false;
    let mut in_exit = false;

    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                handle_open(
                    &e,
                    &mut id,
                    &mut max_worlds,
                    &mut duration_min,
                    &mut empty_destroy_min,
                    &mut enter,
                    &mut exit,
                    &mut doors,
                    &mut group,
                    &mut in_enter,
                    &mut in_exit,
                );
            }
            Event::End(e) => match e.name().as_ref() {
                b"enter" => in_enter = false,
                b"exit" => in_exit = false,
                b"group" => {
                    if let Some(g) = group.take() {
                        groups.push(g);
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Some(InstanceTemplate {
        id: id?,
        max_worlds,
        duration_min,
        empty_destroy_min,
        enter,
        exit,
        doors,
        groups,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_open(
    e: &BytesStart,
    id: &mut Option<i32>,
    max_worlds: &mut i32,
    duration_min: &mut i32,
    empty_destroy_min: &mut i32,
    enter: &mut Option<(i32, i32, i32)>,
    exit: &mut ExitType,
    doors: &mut Vec<i32>,
    group: &mut Option<SpawnGroup>,
    in_enter: &mut bool,
    in_exit: &mut bool,
) {
    match e.name().as_ref() {
        b"instance" => {
            *id = attr_i32(e, b"id");
            *max_worlds = attr_i32(e, b"maxWorlds").unwrap_or(-1);
        }
        b"time" => {
            *duration_min = attr_i32(e, b"duration").unwrap_or(0);
            *empty_destroy_min = attr_i32(e, b"empty").unwrap_or(0);
        }
        b"enter" => *in_enter = true,
        b"exit" => {
            *in_exit = true;
            // A self-closing `<exit type="ORIGIN"/>` has no location child.
            if attr_str(e, b"type").as_deref() == Some("ORIGIN") {
                *exit = ExitType::Origin;
            }
        }
        b"location" => {
            if let (Some(x), Some(y), Some(z)) =
                (attr_i32(e, b"x"), attr_i32(e, b"y"), attr_i32(e, b"z"))
            {
                if *in_enter {
                    *enter = Some((x, y, z));
                } else if *in_exit {
                    *exit = ExitType::Fixed(x, y, z);
                }
            }
        }
        b"door" => {
            if let Some(door_id) = attr_i32(e, b"id") {
                doors.push(door_id);
            }
        }
        b"group" => {
            *group = Some(SpawnGroup {
                name: attr_str(e, b"name").unwrap_or_default(),
                // Java defaults `spawnByDefault` to true.
                spawn_by_default: attr_str(e, b"spawnByDefault").as_deref() != Some("false"),
                npcs: Vec::new(),
            });
        }
        b"npc" => {
            if let (Some(g), Some(npc_id)) = (group.as_mut(), attr_i32(e, b"id")) {
                g.npcs.push(TemplateSpawn {
                    npc_id,
                    x: attr_i32(e, b"x").unwrap_or(0),
                    y: attr_i32(e, b"y").unwrap_or(0),
                    z: attr_i32(e, b"z").unwrap_or(0),
                    heading: attr_i32(e, b"heading").unwrap_or(0),
                });
            }
        }
        _ => {}
    }
}

fn attr_str(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    #[test]
    fn loads_the_real_instance_templates() {
        let data = InstanceData::load_from(ROOT);
        assert!(data.len() >= 5, "the Olympiad arenas + tomb load");

        // The Grassy Arena (147): two doors, one default group of two NPCs.
        let grassy = data.get(147).expect("grassy arena");
        assert_eq!(grassy.doors, vec![17100001, 17100002]);
        assert_eq!(grassy.groups.len(), 1);
        assert!(grassy.groups[0].spawn_by_default, "default group");
        assert_eq!(grassy.groups[0].npcs.len(), 2);
        assert_eq!(grassy.groups[0].npcs[0].npc_id, 36402);
        assert_eq!(grassy.groups[0].npcs[0].x, -89400);

        // Frintezza's Last Imperial Tomb (136): a fixed enter location, an
        // ORIGIN exit, a duration, and its 22-door list.
        let tomb = data.get(136).expect("last imperial tomb");
        assert_eq!(tomb.max_worlds, 5);
        assert_eq!(tomb.enter, Some((-88015, -141153, -9168)));
        assert_eq!(tomb.exit, ExitType::Origin);
        assert_eq!(tomb.duration_min, 120);
        assert_eq!(tomb.doors.len(), 22);
        // A non-default group is present (spawnByDefault="false").
        assert!(tomb.groups.iter().any(|g| !g.spawn_by_default));
    }
}
