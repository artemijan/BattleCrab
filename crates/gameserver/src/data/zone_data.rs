//! Port of `instancemanager/ZoneManager` narrowed to the G12 slice: only
//! `data/zones/peace.xml`, `water.xml` and `no_restart.xml` load (the three
//! zone families whose consumers exist — the other ~37 files/32 types are
//! gated on systems like sieges/castles/olympiad and stay in G14). Shapes
//! reuse the `ZoneForm`/`Territory` geometry already ported for spawn
//! territories (Java shares `ZoneForm` between `ZoneManager` and `SpawnData`
//! the same way).
//!
//! Spatial index: Java buckets zones into a `ZoneRegion[][]` grid of
//! `SHIFT_BY = 15` cells (32768 units — one map tile), each region holding
//! every zone whose bounding box overlaps it; a point query walks its cell's
//! (few) zones. Same structure here as a HashMap keyed by cell coordinate.
//!
//! `NoRestartZone`'s `restartTime`/`restartAllowedTime` `<stat>`s are not
//! stored: their only Java consumer is `onPlayerLoginInside`'s
//! "teleport home if you were away too long" — deferred with the flag's
//! other consumers (nothing in this Mobius version reads `ZoneId.NO_RESTART`
//! after it's set), so the zone only tracks membership for now.

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

use super::spawn_data::{Territory, ZoneForm};

/// Java `ZoneManager.SHIFT_BY` (15) — zone-grid cells are one map tile, not
/// the 2048-unit visibility cells (`World::REGION_SHIFT` = 11).
const ZONE_SHIFT: i32 = 15;

/// The zone families this slice loads (3 of Java's 35 `ZoneType` classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Peace,
    Water,
    NoRestart,
    /// Java `ArenaZone` → `ZoneId.PVP`: free-for-all areas where players are
    /// auto-attackable and hostile actions don't raise a flag.
    Pvp,
}

impl ZoneKind {
    /// Bit in a [`ZoneData::mask_at`] / `ZoneFlags` membership mask.
    pub fn bit(self) -> u8 {
        match self {
            ZoneKind::Peace => 1,
            ZoneKind::Water => 2,
            ZoneKind::NoRestart => 4,
            ZoneKind::Pvp => 8,
        }
    }
}

pub struct Zone {
    pub name: String,
    pub kind: ZoneKind,
    pub territory: Territory,
}

#[derive(Default)]
pub struct ZoneData {
    pub zones: Vec<Zone>,
    /// Zone-grid cell → indexes into `zones` whose bounds overlap the cell.
    grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl ZoneData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut zones = Vec::new();
        for (file, kind) in [
            ("peace.xml", ZoneKind::Peace),
            ("water.xml", ZoneKind::Water),
            ("no_restart.xml", ZoneKind::NoRestart),
            // `pvp.xml` is uniformly `ArenaZone`, so the filename→kind mapping
            // is correct. `underground_coliseum.xml` mixes zone types and needs
            // per-zone `type=` parsing before it can be loaded — deferred.
            ("pvp.xml", ZoneKind::Pvp),
        ] {
            parse_file(&format!("{file_path}data/zones/{file}"), kind, &mut zones);
        }
        let mut data = Self { zones, grid: Default::default() };
        data.build_grid();
        info!("ZoneData: Loaded {} zones (peace/water/no_restart/pvp).", data.zones.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test/synthetic hook: register a zone directly and re-index.
    pub fn insert(&mut self, zone: Zone) {
        self.zones.push(zone);
        self.build_grid();
    }

    /// Java `ZoneManager` registration: each zone lands in every grid cell
    /// its bounding box overlaps.
    fn build_grid(&mut self) {
        self.grid.clear();
        for (i, zone) in self.zones.iter().enumerate() {
            let (x1, x2, y1, y2) = zone.territory.bounds();
            for cx in (x1 >> ZONE_SHIFT)..=(x2 >> ZONE_SHIFT) {
                for cy in (y1 >> ZONE_SHIFT)..=(y2 >> ZONE_SHIFT) {
                    self.grid.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
    }

    /// Every loaded zone containing the point (`ZoneRegion.getZones` +
    /// `ZoneType.isInsideZone(x, y, z)` — 2D shape + the z band).
    pub fn zones_at(&self, x: i32, y: i32, z: i32) -> impl Iterator<Item = &Zone> {
        self.grid
            .get(&(x >> ZONE_SHIFT, y >> ZONE_SHIFT))
            .into_iter()
            .flatten()
            .map(|&i| &self.zones[i as usize])
            .filter(move |zn| z >= zn.territory.min_z && z <= zn.territory.max_z && zn.territory.contains_2d(x, y))
    }

    /// `ZoneKind` membership bits at a point — what `revalidate_zones` diffs.
    pub fn mask_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.zones_at(x, y, z).fold(0, |m, zn| m | zn.kind.bit())
    }
}

fn parse_file(path: &str, kind: ZoneKind, out: &mut Vec<Zone>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let mut reader = Reader::from_str(&content);

    struct Pending {
        name: String,
        shape: String,
        min_z: i32,
        max_z: i32,
        rad: Option<i32>,
        xs: Vec<i32>,
        ys: Vec<i32>,
    }
    let mut cur: Option<Pending> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone" {
                    if let Some(p) = cur.take() {
                        if let Some(form) = build_form(&p.shape, p.xs, p.ys, p.rad) {
                            out.push(Zone {
                                name: p.name,
                                kind,
                                territory: Territory { form, min_z: p.min_z, max_z: p.max_z },
                            });
                        }
                    }
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"zone" => {
                cur = Some(Pending {
                    name: attr_str(&e, b"name").unwrap_or_default(),
                    shape: attr_str(&e, b"shape").unwrap_or_else(|| "NPoly".to_string()),
                    min_z: attr_i32(&e, b"minZ").unwrap_or(i32::MIN),
                    max_z: attr_i32(&e, b"maxZ").unwrap_or(i32::MAX),
                    rad: attr_i32(&e, b"rad"),
                    xs: Vec::new(),
                    ys: Vec::new(),
                });
            }
            // Zone files capitalize the node attributes (`X`/`Y`), unlike
            // the spawn territories' lowercase — accept either.
            b"node" => {
                if let Some(p) = cur.as_mut() {
                    p.xs.push(attr_i32(&e, b"X").or_else(|| attr_i32(&e, b"x")).unwrap_or(0));
                    p.ys.push(attr_i32(&e, b"Y").or_else(|| attr_i32(&e, b"y")).unwrap_or(0));
                }
            }
            _ => {} // `<stat>` params skipped (see module docs)
        }
    }
}

/// Same shape-construction rules as `spawn_data::finish_territory`.
fn build_form(shape: &str, xs: Vec<i32>, ys: Vec<i32>, rad: Option<i32>) -> Option<ZoneForm> {
    match shape {
        "Cuboid" if xs.len() >= 2 && ys.len() >= 2 => Some(ZoneForm::Cuboid {
            x1: xs[0].min(xs[1]),
            x2: xs[0].max(xs[1]),
            y1: ys[0].min(ys[1]),
            y2: ys[0].max(ys[1]),
        }),
        "Cylinder" if !xs.is_empty() && !ys.is_empty() => {
            Some(ZoneForm::Cylinder { x: xs[0], y: ys[0], rad: rad.unwrap_or(0) })
        }
        _ if xs.len() >= 3 => Some(ZoneForm::NPoly { xs, ys }),
        _ => None,
    }
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_files() {
        let data = ZoneData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // 127 peace + 423 water + 40 no_restart + 6 pvp.
        assert_eq!(data.zones.len(), 596);

        // Talking Island town center sits in talking_island_town_peace_zone1
        // (NPoly, z band [-3966, -3466]).
        let mask = data.mask_at(-84300, 243000, -3700);
        assert_eq!(mask & ZoneKind::Peace.bit(), ZoneKind::Peace.bit());
        // Same x/y far outside the z band: not in the zone.
        assert_eq!(data.mask_at(-84300, 243000, 0), 0);

        // A point inside water cuboid 11_23_water1 ([-294912..-262144] ×
        // [163839..196607], z [-4779, -3779]).
        let mask = data.mask_at(-280000, 180000, -4000);
        assert_eq!(mask & ZoneKind::Water.bit(), ZoneKind::Water.bit());

        // Zaken's deck is a no-restart NPoly.
        let mask = data.mask_at(54000, 217000, -3000);
        assert_eq!(mask & ZoneKind::NoRestart.bit(), ZoneKind::NoRestart.bit());

        // gludin_pvp ArenaZone (NPoly, z band [-3752, -352]).
        let mask = data.mask_at(-88000, 142000, -1000);
        assert_eq!(mask & ZoneKind::Pvp.bit(), ZoneKind::Pvp.bit());
    }

    #[test]
    fn grid_and_mask_on_synthetic_zone() {
        let mut data = ZoneData::empty();
        data.insert(Zone {
            name: "test_peace".into(),
            kind: ZoneKind::Peace,
            territory: Territory {
                form: ZoneForm::Cuboid { x1: 0, x2: 1000, y1: 0, y2: 1000 },
                min_z: -100,
                max_z: 100,
            },
        });
        assert_eq!(data.mask_at(500, 500, 0), ZoneKind::Peace.bit());
        assert_eq!(data.mask_at(1500, 500, 0), 0);
        assert_eq!(data.mask_at(500, 500, 200), 0);
    }
}
