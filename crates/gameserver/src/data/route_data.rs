//! `Routes.xml` — port of `WalkingManager`'s loader.
//!
//! 13 routes on this dist, attached to 13 NPC ids and spawned from
//! `TownNpcWalkers.xml`: Giran's porters and scribes pacing their circuits,
//! the running boy, and Gordon (a raid boss on a 67-node patrol).
//!
//! Only two `repeatStyle`s occur here — `cycle` and `back`. `conveyor`
//! (teleport to the first node) and `random` are parsed for shape but never
//! selected by this datapack.

use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

/// Java `WalkingManager.REPEAT_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatStyle {
    /// `back` — walk to the last node, then retrace to the first.
    GoBack,
    /// `cycle` — walk to the last node, then head straight for the first.
    GoFirst,
    /// `conveyor` — like `cycle`, but *teleport* back to the first node.
    TeleportFirst,
    /// `random` — hop between nodes in no order.
    Random,
    /// No `repeat` — stop at the last node.
    None,
}

#[derive(Debug, Clone)]
pub struct WalkNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Seconds to pause on arrival before setting off again.
    pub delay: i32,
    /// Walk or run this leg.
    pub run: bool,
    /// `<point ... string="…">` — shouted on arrival. Empty for most nodes.
    pub chat: String,
}

#[derive(Debug, Clone)]
pub struct WalkRoute {
    pub name: String,
    pub repeat: bool,
    pub repeat_style: RepeatStyle,
    pub nodes: Vec<WalkNode>,
}

#[derive(Default, Clone)]
pub struct RouteData {
    pub routes: Vec<WalkRoute>,
    /// npc id → index into [`Self::routes`]. No npc id carries two routes on
    /// this dist, so Java's per-spawn-point disambiguation isn't needed.
    by_npc: HashMap<i32, usize>,
}

impl RouteData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let path = format!("{file_path}data/Routes.xml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut routes: Vec<WalkRoute> = Vec::new();
        let mut by_npc = HashMap::new();
        let mut cur: Option<WalkRoute> = None;

        for event in xml::events(&content) {
            let e = match event {
                Event::Start(e) | Event::Empty(e) => e,
                Event::End(e) => {
                    if e.name().as_ref() == b"route"
                        && let Some(r) = cur.take()
                        && !r.nodes.is_empty()
                    {
                        routes.push(r);
                    }
                    continue;
                }
                _ => continue,
            };
            match e.name().as_ref() {
                b"route" => {
                    let repeat = attr(&e, b"repeat").as_deref() == Some("true");
                    let repeat_style = match attr(&e, b"repeatStyle").as_deref() {
                        Some("back") => RepeatStyle::GoBack,
                        Some("cycle") => RepeatStyle::GoFirst,
                        Some("conveyor") => RepeatStyle::TeleportFirst,
                        Some("random") => RepeatStyle::Random,
                        _ => RepeatStyle::None,
                    };
                    cur = Some(WalkRoute {
                        name: attr(&e, b"name").unwrap_or_default(),
                        repeat,
                        repeat_style,
                        nodes: Vec::new(),
                    });
                }
                b"target" => {
                    if let Some(id) = attr(&e, b"id").and_then(|v| v.parse().ok()) {
                        // The route is pushed at `</route>`, so its index is
                        // the current length.
                        by_npc.insert(id, routes.len());
                    }
                }
                b"point" => {
                    if let Some(r) = cur.as_mut() {
                        r.nodes.push(WalkNode {
                            x: attr_i(&e, b"X"),
                            y: attr_i(&e, b"Y"),
                            z: attr_i(&e, b"Z"),
                            delay: attr(&e, b"delay").and_then(|v| v.parse().ok()).unwrap_or(0),
                            run: attr(&e, b"run").as_deref() == Some("true"),
                            chat: attr(&e, b"string").unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        }

        // Drop attachments whose route was discarded (empty node list).
        by_npc.retain(|_, &mut idx| idx < routes.len());
        info!(
            "RouteData: Loaded {} walking routes for {} NPCs.",
            routes.len(),
            by_npc.len()
        );
        Self { routes, by_npc }
    }

    pub fn route_for_npc(&self, npc_id: i32) -> Option<(usize, &WalkRoute)> {
        let idx = *self.by_npc.get(&npc_id)?;
        Some((idx, self.routes.get(idx)?))
    }

    pub fn get(&self, idx: usize) -> Option<&WalkRoute> {
        self.routes.get(idx)
    }

    /// Test hook: attach `npc_id` to a route index directly.
    #[doc(hidden)]
    pub fn attach_for_test(&mut self, npc_id: i32, route_idx: usize) {
        self.by_npc.insert(npc_id, route_idx);
    }

    pub fn attached_npcs(&self) -> usize {
        self.by_npc.len()
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref().eq_ignore_ascii_case(key))
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

fn attr_i(e: &quick_xml::events::BytesStart, key: &[u8]) -> i32 {
    attr(e, key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_real_dist_routes() {
        let d = RouteData::load_from(DIST);
        assert_eq!(d.routes.len(), 13, "expected the dist's 13 walking routes");
        // 13 targets across those routes: `running_boy` names two NPCs and
        // `FPC_Giran_Evi` names 80000, which has no template.
        assert_eq!(d.attached_npcs(), 14);

        // Porter Remy 31356 paces an 18-node cycle.
        let (_, remy) = d.route_for_npc(31356).expect("31356 has a route");
        assert_eq!(remy.name, "porter_remy");
        assert_eq!(remy.repeat_style, RepeatStyle::GoFirst);
        assert!(remy.repeat);
        assert_eq!(remy.nodes.len(), 18);

        // Scribe Leandro retraces his path.
        let (_, leandro) = d.route_for_npc(31357).expect("31357 has a route");
        assert_eq!(leandro.repeat_style, RepeatStyle::GoBack);

        // Only these two styles occur here.
        for r in &d.routes {
            assert!(
                matches!(r.repeat_style, RepeatStyle::GoBack | RepeatStyle::GoFirst),
                "unexpected repeat style on {}",
                r.name
            );
        }
    }
}
