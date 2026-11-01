//! `Routes.xml` — port of `WalkingManager`'s loader.
//!
//! 197 routes on this dist with 227 `<target>`s over 61 npc ids. A target
//! names a spawn point as well as an npc id (`spawnX/Y/Z`), and 19 ids walk
//! several routes — the Antharas' Lair patrols send one monster id down three
//! paths — so a route is chosen per spawn point, as Java's `NpcRoutesHolder`
//! does, not per npc id.
//!
//! `repeatStyle` is `back` (152), `cycle` (23), `none` (18) or `conveyor`
//! (4); `random` is parsed for shape but never selected by this datapack.

use crate::data::xml;
use quick_xml::events::Event;
use std::collections::HashMap;
use tracing::info;

/// Java `WalkingManager.REPEAT_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalkRoute {
    pub name: String,
    pub repeat: bool,
    pub repeat_style: RepeatStyle,
    pub nodes: Vec<WalkNode>,
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouteData {
    pub routes: Vec<WalkRoute>,
    /// Java `_routesToAttach`: npc id → spawn point → index into
    /// [`Self::routes`] (`NpcRoutesHolder._correspondences`, keyed there by
    /// the `"x-y-z"` string). A later target on the same point wins, as the
    /// Java map put does.
    by_npc: HashMap<i32, HashMap<(i32, i32, i32), usize>>,
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
        let mut by_npc: HashMap<i32, HashMap<(i32, i32, i32), usize>> = HashMap::new();
        let mut cur: Option<WalkRoute> = None;

        for event in xml::events(&content) {
            let e = match event {
                Event::Start(e) | Event::Empty(e) => e,
                Event::End(e) => {
                    if e.name().as_ref() == "route"
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
                "route" => {
                    let repeat = attr(&e, "repeat").as_deref() == Some("true");
                    let repeat_style = match attr(&e, "repeatStyle").as_deref() {
                        Some("back") => RepeatStyle::GoBack,
                        Some("cycle") => RepeatStyle::GoFirst,
                        Some("conveyor") => RepeatStyle::TeleportFirst,
                        Some("random") => RepeatStyle::Random,
                        _ => RepeatStyle::None,
                    };
                    cur = Some(WalkRoute {
                        name: attr(&e, "name").unwrap_or_default(),
                        repeat,
                        repeat_style,
                        nodes: Vec::new(),
                    });
                }
                "target" => {
                    // Java parses all four with `Integer.parseInt` and skips the
                    // target on any failure.
                    let num = |k: &str| attr(&e, k).and_then(|v| v.parse::<i32>().ok());
                    if let (Some(id), Some(x), Some(y), Some(z)) =
                        (num("id"), num("spawnX"), num("spawnY"), num("spawnZ"))
                    {
                        // The route is pushed at `</route>`, so its index is
                        // the current length.
                        by_npc
                            .entry(id)
                            .or_default()
                            .insert((x, y, z), routes.len());
                    }
                }
                "point" => {
                    if let Some(r) = cur.as_mut() {
                        r.nodes.push(WalkNode {
                            x: attr_i(&e, "X"),
                            y: attr_i(&e, "Y"),
                            z: attr_i(&e, "Z"),
                            delay: attr(&e, "delay").and_then(|v| v.parse().ok()).unwrap_or(0),
                            run: attr(&e, "run").as_deref() == Some("true"),
                            chat: attr(&e, "string").unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        }

        // Drop attachments whose route was discarded (empty node list).
        for points in by_npc.values_mut() {
            points.retain(|_, &mut idx| idx < routes.len());
        }
        by_npc.retain(|_, points| !points.is_empty());
        info!(
            "RouteData: Loaded {} walking routes for {} NPCs.",
            routes.len(),
            by_npc.len()
        );
        Self { routes, by_npc }
    }

    /// `WalkingManager.onSpawn` → `NpcRoutesHolder.getRouteName(npc)`: the
    /// route a spawn of `npc_id` at `spawn_loc` walks. An id that is targeted
    /// but spawned somewhere no target names walks nothing.
    pub fn route_for_spawn(
        &self,
        npc_id: i32,
        spawn_loc: (i32, i32, i32),
    ) -> Option<(usize, &WalkRoute)> {
        let idx = *self.by_npc.get(&npc_id)?.get(&spawn_loc)?;
        Some((idx, self.routes.get(idx)?))
    }

    /// `WalkingManager.isTargeted(npc)` — any route names this npc id.
    pub fn is_targeted(&self, npc_id: i32) -> bool {
        self.by_npc.contains_key(&npc_id)
    }

    pub fn get(&self, idx: usize) -> Option<&WalkRoute> {
        self.routes.get(idx)
    }

    /// Test hook: attach `npc_id` spawned at `spawn_loc` to a route index.
    #[doc(hidden)]
    pub fn attach_for_test(&mut self, npc_id: i32, spawn_loc: (i32, i32, i32), route_idx: usize) {
        self.by_npc
            .entry(npc_id)
            .or_default()
            .insert(spawn_loc, route_idx);
    }

    pub fn attached_npcs(&self) -> usize {
        self.by_npc.len()
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref().eq_ignore_ascii_case(key))
        .map(|a| a.value.into_owned())
}

fn attr_i(e: &quick_xml::events::BytesStart, key: &str) -> i32 {
    attr(e, key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_real_dist_routes() {
        let d = RouteData::load_from(DIST);
        assert_eq!(
            d.routes.len(),
            197,
            "expected the dist's 197 walking routes"
        );
        // 61 target ids. One, 80000 (`FPC_Giran_Evi`), has no npc template:
        // Java skips it at load, here it is kept and simply never spawns.
        assert_eq!(d.attached_npcs(), 61);
        assert!(d.is_targeted(22857));
        assert!(!d.is_targeted(20001));

        // Knoriks 22857 walks a different route from each spawn point:
        // knoriks_01 from (140633, 114472, -3725), knoriks_06 from
        // (146901, 116472, -3698).
        let (_, k1) = d
            .route_for_spawn(22857, (140633, 114472, -3725))
            .expect("knoriks_01 spawn point");
        assert_eq!(k1.name, "knoriks_01");
        assert_eq!(k1.repeat_style, RepeatStyle::GoBack);
        assert!(k1.repeat);
        assert_eq!(k1.nodes.len(), 7);
        let (_, k6) = d
            .route_for_spawn(22857, (146901, 116472, -3698))
            .expect("knoriks_06 spawn point");
        assert_eq!(k6.name, "knoriks_06");
        assert_eq!(k6.repeat_style, RepeatStyle::GoFirst);
        assert_eq!(k6.nodes.len(), 29);
        // A spawn point no target names walks nothing.
        assert!(d.route_for_spawn(22857, (0, 0, 0)).is_none());

        // `back` 152, `cycle` 23, `conveyor` 4, and 18 `none`.
        let by = |s: RepeatStyle| d.routes.iter().filter(|r| r.repeat_style == s).count();
        assert_eq!(by(RepeatStyle::GoBack), 152);
        assert_eq!(by(RepeatStyle::GoFirst), 23);
        assert_eq!(by(RepeatStyle::TeleportFirst), 4);
        assert_eq!(by(RepeatStyle::None), 18);
    }
}
