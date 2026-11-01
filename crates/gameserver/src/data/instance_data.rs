//! Port of `data/xml/InstanceData` — the `data/instances/**/*.xml` templates
//! (Java `InstanceManager.load` + `InstanceTemplate`). A template describes a
//! private world: its doors, spawn groups, the enter/exit locations, and the
//! activity / empty-destroy timers.

use std::collections::HashMap;

use crate::data::xml;
use crate::data::xml::{attr_i32_trimmed as attr_i32, attr_str};
use quick_xml::events::{BytesStart, Event};
use tracing::info;

const INSTANCE_DIR: &str = "data/instances";

/// One NPC placement in a spawn group (`<npc id x y z heading/>`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateSpawn {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// `respawnTime`, in seconds (0 = no respawn).
    pub respawn_secs: i32,
}

/// A named spawn group (`<group>`); `spawn_by_default` groups populate on
/// creation, others are triggered by the instance's script.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpawnGroup {
    pub name: String,
    pub spawn_by_default: bool,
    pub npcs: Vec<TemplateSpawn>,
}

/// Where a party is sent on leaving (`<exit>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExitType {
    /// Back to where each member entered from (`type="ORIGIN"`).
    Origin,
    /// A fixed location.
    Fixed(i32, i32, i32),
}

/// `InstanceTeleportType` of the `<enter>` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EnterType {
    /// No enter location (Java warns and leaves the player where they are).
    #[default]
    None,
    /// The first `<location>`.
    Fixed,
    /// A random `<location>`.
    Random,
}

/// One `<conditions>/<condition>` (Java `instancezone.conditions.Condition`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EnterCondition {
    /// The `type` attribute (`Level`, `GroupMax`, …).
    pub kind: String,
    pub only_leader: bool,
    pub show_message_and_html: bool,
    /// The `<param name value/>` children.
    pub params: HashMap<String, String>,
}

impl EnterCondition {
    pub fn int_param(&self, name: &str) -> Option<i32> {
        self.params.get(name).and_then(|v| v.trim().parse().ok())
    }
}

/// A door in the template's `<doorlist>`, with its `<openStatus default>`
/// override (Java merges the instance node over the door template).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TemplateDoor {
    pub id: i32,
    pub open_by_default: Option<bool>,
}

/// A parsed instance template (Java `InstanceTemplate`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstanceTemplate {
    pub id: i32,
    /// The `name` attribute, or `None` when absent (Java's field defaults to
    /// "UnknownInstance"); cosmetic, shown only in the GM instance panel.
    pub name: Option<String>,
    /// `maxWorlds` — concurrent copies allowed, or -1 for unlimited.
    pub max_worlds: i32,
    /// `<time duration>` — how long the instance stays up, in minutes. Java's
    /// default is -1, never ending; 0 destroys it at once.
    pub duration_min: i32,
    /// `<time empty>` — how many minutes an emptied instance lingers before
    /// teardown (Java `TimeUnit.MINUTES`). -1 (the default) never destroys it
    /// for being empty.
    pub empty_destroy_min: i32,
    /// `<enter type>` and its `<location>`s.
    pub enter_type: EnterType,
    pub enter_locations: Vec<(i32, i32, i32)>,
    pub exit: ExitType,
    pub doors: Vec<TemplateDoor>,
    pub groups: Vec<SpawnGroup>,
    pub conditions: Vec<EnterCondition>,
}

impl InstanceTemplate {
    /// `InstanceTemplate.getEnterLocation`: the first location, or a random
    /// one (`roll(n)` picks the index).
    pub fn enter_location(&self, roll: impl FnOnce(i32) -> i32) -> Option<(i32, i32, i32)> {
        match self.enter_type {
            EnterType::None => None,
            EnterType::Fixed => self.enter_locations.first().copied(),
            EnterType::Random if self.enter_locations.is_empty() => None,
            EnterType::Random => {
                let i = roll(self.enter_locations.len() as i32);
                self.enter_locations.get(i as usize).copied()
            }
        }
    }
}

/// Every instance template, by id (Java `InstanceManager._instanceTemplates`).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstanceData {
    by_id: HashMap<i32, InstanceTemplate>,
}

impl InstanceData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let mut by_id = HashMap::new();
        for path in crate::data::xml::xml_files_under(format!("{root}{INSTANCE_DIR}")) {
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Some(t) = parse(&content)
            {
                by_id.insert(t.id, t);
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

    /// Every template (unordered) — the GM instance panel sorts/filters these.
    pub fn iter(&self) -> impl Iterator<Item = &InstanceTemplate> {
        self.by_id.values()
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

fn parse(content: &str) -> Option<InstanceTemplate> {
    let mut id = None;
    let mut name = None;
    let mut max_worlds = -1;
    let mut duration_min = -1;
    let mut empty_destroy_min = -1;
    let mut enter_type = EnterType::None;
    let mut enter_locations = Vec::new();
    let mut exit = ExitType::Origin;
    let mut doors = Vec::new();
    let mut groups = Vec::new();
    let mut group: Option<SpawnGroup> = None;
    let mut conditions: Vec<EnterCondition> = Vec::new();
    let mut in_enter = false;
    let mut in_exit = false;
    let mut in_condition = false;

    for event in xml::events(content) {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                handle_open(
                    &e,
                    &mut Parsed {
                        id: &mut id,
                        name: &mut name,
                        max_worlds: &mut max_worlds,
                        duration_min: &mut duration_min,
                        empty_destroy_min: &mut empty_destroy_min,
                        enter_type: &mut enter_type,
                        enter_locations: &mut enter_locations,
                        exit: &mut exit,
                        doors: &mut doors,
                        group: &mut group,
                        conditions: &mut conditions,
                        in_enter: &mut in_enter,
                        in_exit: &mut in_exit,
                        in_condition: &mut in_condition,
                    },
                );
            }
            Event::End(e) => match e.name().as_ref() {
                "condition" => in_condition = false,
                "enter" => in_enter = false,
                "exit" => in_exit = false,
                "group" => {
                    if let Some(g) = group.take() {
                        groups.push(g);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Some(InstanceTemplate {
        id: id?,
        name,
        max_worlds,
        duration_min,
        empty_destroy_min,
        enter_type,
        enter_locations,
        exit,
        doors,
        groups,
        conditions,
    })
}

/// The parse state `handle_open` writes into.
struct Parsed<'a> {
    id: &'a mut Option<i32>,
    name: &'a mut Option<String>,
    max_worlds: &'a mut i32,
    duration_min: &'a mut i32,
    empty_destroy_min: &'a mut i32,
    enter_type: &'a mut EnterType,
    enter_locations: &'a mut Vec<(i32, i32, i32)>,
    exit: &'a mut ExitType,
    doors: &'a mut Vec<TemplateDoor>,
    group: &'a mut Option<SpawnGroup>,
    conditions: &'a mut Vec<EnterCondition>,
    in_enter: &'a mut bool,
    in_exit: &'a mut bool,
    in_condition: &'a mut bool,
}

fn handle_open(e: &BytesStart, p: &mut Parsed) {
    match e.name().as_ref() {
        "instance" => {
            *p.id = attr_i32(e, "id");
            *p.name = attr_str(e, "name").filter(|s| !s.is_empty());
            *p.max_worlds = attr_i32(e, "maxWorlds").unwrap_or(-1);
        }
        "time" => {
            *p.duration_min = attr_i32(e, "duration").unwrap_or(-1);
            // `setEmptyDestroyTime` ignores a negative value.
            if let Some(empty) = attr_i32(e, "empty").filter(|&v| v >= 0) {
                *p.empty_destroy_min = empty;
            }
        }
        "enter" => {
            *p.in_enter = true;
            *p.enter_type = match attr_str(e, "type").as_deref() {
                Some("FIXED") => EnterType::Fixed,
                Some("RANDOM") => EnterType::Random,
                _ => EnterType::None,
            };
        }
        "exit" => {
            *p.in_exit = true;
            // A self-closing `<exit type="ORIGIN"/>` has no location child.
            if attr_str(e, "type").as_deref() == Some("ORIGIN") {
                *p.exit = ExitType::Origin;
            }
        }
        "location" => {
            if let (Some(x), Some(y), Some(z)) =
                (attr_i32(e, "x"), attr_i32(e, "y"), attr_i32(e, "z"))
            {
                if *p.in_enter {
                    p.enter_locations.push((x, y, z));
                } else if *p.in_exit && !matches!(*p.exit, ExitType::Fixed(..)) {
                    *p.exit = ExitType::Fixed(x, y, z);
                }
            }
        }
        "door" => {
            if let Some(id) = attr_i32(e, "id") {
                p.doors.push(TemplateDoor {
                    id,
                    open_by_default: None,
                });
            }
        }
        "openStatus" => {
            if let Some(door) = p.doors.last_mut() {
                door.open_by_default = attr_str(e, "default").map(|v| v == "open");
            }
        }
        "condition" => {
            *p.in_condition = true;
            p.conditions.push(EnterCondition {
                kind: attr_str(e, "type").unwrap_or_default(),
                only_leader: attr_str(e, "onlyLeader").as_deref() == Some("true"),
                show_message_and_html: attr_str(e, "showMessageAndHtml").as_deref() == Some("true"),
                params: HashMap::new(),
            });
        }
        "param" if *p.in_condition => {
            if let (Some(c), Some(name), Some(value)) = (
                p.conditions.last_mut(),
                attr_str(e, "name"),
                attr_str(e, "value"),
            ) {
                c.params.insert(name, value);
            }
        }
        "group" => {
            *p.group = Some(SpawnGroup {
                name: attr_str(e, "name").unwrap_or_default(),
                // Java defaults `spawnByDefault` to true.
                spawn_by_default: attr_str(e, "spawnByDefault").as_deref() != Some("false"),
                npcs: Vec::new(),
            });
        }
        "npc" => {
            if let (Some(g), Some(npc_id)) = (p.group.as_mut(), attr_i32(e, "id")) {
                g.npcs.push(TemplateSpawn {
                    npc_id,
                    x: attr_i32(e, "x").unwrap_or(0),
                    y: attr_i32(e, "y").unwrap_or(0),
                    z: attr_i32(e, "z").unwrap_or(0),
                    heading: attr_i32(e, "heading").unwrap_or(0),
                    respawn_secs: attr_str(e, "respawnTime")
                        .and_then(|v| crate::data::spawn_data::parse_duration_secs(&v))
                        .unwrap_or(0),
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_the_real_instance_templates() {
        let data = InstanceData::load_from(ROOT);
        assert!(data.len() >= 5, "the Olympiad arenas + tomb load");

        // The Grassy Arena (147): two doors, one default group of two NPCs.
        let grassy = data.get(147).expect("grassy arena");
        assert_eq!(
            grassy.doors.iter().map(|d| d.id).collect::<Vec<_>>(),
            [17100001, 17100002]
        );
        assert_eq!(grassy.groups.len(), 1);
        assert!(grassy.groups[0].spawn_by_default, "default group");
        assert_eq!(grassy.groups[0].npcs.len(), 2);
        assert_eq!(grassy.groups[0].npcs[0].npc_id, 36402);
        assert_eq!(grassy.groups[0].npcs[0].x, -89400);

        // Frintezza's Last Imperial Tomb (136): a fixed enter location, an
        // ORIGIN exit, a duration, and its 22-door list.
        let tomb = data.get(136).expect("last imperial tomb");
        assert_eq!(tomb.max_worlds, 5);
        assert_eq!(tomb.enter_type, EnterType::Fixed);
        assert_eq!(tomb.enter_locations, [(-88015, -141153, -9168)]);
        assert_eq!(tomb.exit, ExitType::Origin);
        assert_eq!(tomb.duration_min, 120);
        assert_eq!(tomb.doors.len(), 22);
        // A non-default group is present (spawnByDefault="false").
        assert!(tomb.groups.iter().any(|g| !g.spawn_by_default));

        // Labyrinth of Belis (178): a Level condition, one door forced open,
        // and the Handymen's 15-second respawn.
        let belis = data.get(178).expect("labyrinth of belis");
        assert_eq!(belis.duration_min, 60);
        assert_eq!(belis.empty_destroy_min, 0);
        assert_eq!(belis.conditions.len(), 1);
        assert_eq!(belis.conditions[0].kind, "Level");
        assert_eq!(belis.conditions[0].int_param("min"), Some(18));
        assert_eq!(
            belis.doors[0],
            TemplateDoor {
                id: 16240001,
                open_by_default: Some(true)
            }
        );
        assert_eq!(belis.doors[1].open_by_default, None);
        let handyman = belis.groups[0]
            .npcs
            .iter()
            .find(|n| n.npc_id == 22997)
            .expect("handyman");
        assert_eq!(handyman.respawn_secs, 15);
    }
}
