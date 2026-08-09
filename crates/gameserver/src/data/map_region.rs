//! Port of `instancemanager/MapRegionManager` + `model/MapRegion`, scoped to
//! the death→"to village" flow: map every world position to its region and
//! hand out that region's town respawn points.
//!
//! Java `getTeleToLocation(TOWN)` resolves the region in two steps, and both
//! are ported here: an enclosing **`RespawnZone`** (`data/zones/respawn.xml`)
//! wins first (its per-race `<race point=…>` names the target region — this is
//! what pulls e.g. Elven Ruins back to Talking Island even though its coarse
//! 32768-unit map tile is shared with Giran Harbour); only outside every
//! RespawnZone does it fall back to the map-tile lookup (`getMapRegion`).
//!
//! Not ported: chaotic (karma) points, the map-region `<banned>` race
//! redirects, castle/fort/clan-hall teleport targets — none of those systems
//! exist yet. (RespawnZones already carry their own per-race redirects, so the
//! common cross-race case — a Dark Elf in Elf territory, etc. — is covered.)

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::enums::Race;

use super::spawn_data::Territory;
use crate::data::xml::attr_str;

pub const MAPREGION_DIR: &str = "data/mapregion";
/// `data/zones/respawn.xml` — the `RespawnZone` polygons consulted before the
/// map-tile fallback.
pub const RESPAWN_ZONE_FILE: &str = "data/zones/respawn.xml";

/// Java `MapRegionManager.DEFAULT_RESPAWN`.
const DEFAULT_RESPAWN: &str = "talking_island_town";

#[derive(Debug, Clone)]
pub struct MapRegion {
    pub name: String,
    /// `locId`: the client "Current location: … (near X)" system-message id
    /// for this region (`MapRegion.getLocId`, the `/loc` user command). 0
    /// when the XML has none.
    pub loc_id: i32,
    /// `bbs`: the community-board region code (`MapRegion.getBbs`). This is
    /// what Java's `MapRegionManager.getBBs(loc)` returns, and it is the
    /// "location" a party matching room is created in and filtered by (G30).
    pub bbs: i32,
    /// Ordinary (non-chaotic, non-other) respawn points.
    pub respawn_points: Vec<(i32, i32, i32)>,
    /// `<map X= Y=/>` tile coordinates this region covers.
    pub tiles: Vec<(i32, i32)>,
}

/// Port of `zone/type/RespawnZone`: a polygon (+ z band) that overrides the
/// map-tile lookup, mapping the dying player's race to a target region name
/// (`_raceRespawnPoint`).
pub struct RespawnZone {
    #[allow(dead_code)]
    pub name: String,
    pub territory: Territory,
    /// `<race name= point=/>`: race → target region name.
    pub race_points: HashMap<Race, String>,
}

pub struct MapRegionData {
    regions: Vec<MapRegion>,
    respawn_zones: Vec<RespawnZone>,
}

/// `getMapRegionX/Y`: world coordinate → 32768-unit map tile index.
fn map_tile_x(x: i32) -> i32 {
    (x >> 15) + 9 + 11
}
fn map_tile_y(y: i32) -> i32 {
    (y >> 15) + 10 + 8
}

impl MapRegionData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut regions = Vec::new();
        let dir = format!("{file_path}{MAPREGION_DIR}");
        {
            for path in super::xml::xml_files_in(&dir) {
                parse_file(&path, &mut regions);
            }
        }
        let respawn_zones = parse_respawn_zones(&format!("{file_path}{RESPAWN_ZONE_FILE}"));
        info!(
            "MapRegionData: Loaded {} map regions, {} respawn zones.",
            regions.len(),
            respawn_zones.len()
        );
        Self {
            regions,
            respawn_zones,
        }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            regions: Vec::new(),
            respawn_zones: Vec::new(),
        }
    }

    /// Synthetic regions for unit tests.
    #[doc(hidden)]
    pub fn from_regions(regions: Vec<MapRegion>) -> Self {
        Self {
            regions,
            respawn_zones: Vec::new(),
        }
    }

    /// Synthetic regions + respawn zones for unit tests.
    #[doc(hidden)]
    pub fn from_parts(regions: Vec<MapRegion>, respawn_zones: Vec<RespawnZone>) -> Self {
        Self {
            regions,
            respawn_zones,
        }
    }

    /// `getMapRegion(locX, locY)`.
    pub fn region_at(&self, x: i32, y: i32) -> Option<&MapRegion> {
        let tile = (map_tile_x(x), map_tile_y(y));
        self.regions.iter().find(|r| r.tiles.contains(&tile))
    }

    fn region_by_name(&self, name: &str) -> Option<&MapRegion> {
        self.regions.iter().find(|r| r.name == name)
    }

    /// `MapRegionManager.getBBs(loc)` — the community-board region code at a
    /// point, falling back to the default respawn region's code exactly like
    /// Java (`REGIONS.get(DEFAULT_RESPAWN).getBbs()`), then to 0.
    pub fn bbs_at(&self, x: i32, y: i32) -> i32 {
        self.region_at(x, y)
            .or_else(|| self.region_by_name(DEFAULT_RESPAWN))
            .map_or(0, |r| r.bbs)
    }

    /// `ZoneManager.getZone(loc, RespawnZone.class)`: the first RespawnZone
    /// whose polygon and z band contain the point.
    fn respawn_zone_at(&self, x: i32, y: i32, z: i32) -> Option<&RespawnZone> {
        self.respawn_zones.iter().find(|zn| {
            z >= zn.territory.min_z && z <= zn.territory.max_z && zn.territory.contains_2d(x, y)
        })
    }

    /// `getTeleToLocation(creature, TeleportWhereType.TOWN)` narrowed to the
    /// no-clan/no-karma path. Java resolves the region in two steps:
    ///
    /// 1. an enclosing `RespawnZone` (`getRespawnPoint(race)` → region name,
    ///    then `getRestartRegion` which itself defaults to `talking_island_town`
    ///    when the name is missing/unknown), else
    /// 2. the map-tile region (`getMapRegion`), defaulting to
    ///    `talking_island_town` on a miss (Java's catch-all).
    ///
    /// `pick` indexes into the chosen region's ordinary spawn points
    /// (`RandomRespawnInTownEnabled` — the caller rolls).
    pub fn town_respawn(
        &self,
        x: i32,
        y: i32,
        z: i32,
        race: Race,
        pick: usize,
    ) -> Option<(i32, i32, i32)> {
        let region = if let Some(zone) = self.respawn_zone_at(x, y, z) {
            // getRestartRegion(getRespawnPoint(race)): the named region, or the
            // default when the race isn't mapped / the name doesn't resolve.
            zone.race_points
                .get(&race)
                .and_then(|name| self.region_by_name(name))
                .or_else(|| self.region_by_name(DEFAULT_RESPAWN))
        } else {
            self.region_at(x, y)
                .or_else(|| self.region_by_name(DEFAULT_RESPAWN))
        }?;
        if region.respawn_points.is_empty() {
            return None;
        }
        Some(region.respawn_points[pick % region.respawn_points.len()])
    }
}

fn parse_file(path: &std::path::Path, out: &mut Vec<MapRegion>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);
    let mut cur: Option<MapRegion> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"region"
                    && let Some(r) = cur.take()
                {
                    out.push(r);
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let attr = |key: &[u8]| attr_str(&e, key);
        match e.name().as_ref() {
            b"region" => {
                cur = Some(MapRegion {
                    name: attr(b"name").unwrap_or_default(),
                    loc_id: attr(b"locId").and_then(|v| v.parse().ok()).unwrap_or(0),
                    bbs: attr(b"bbs").and_then(|v| v.parse().ok()).unwrap_or(0),
                    respawn_points: Vec::new(),
                    tiles: Vec::new(),
                });
            }
            b"respawnPoint" => {
                if let Some(r) = cur.as_mut() {
                    // Chaotic (karma) and "other" points are skipped — the
                    // callers that use them aren't ported.
                    let special = ["isChaotic", "isOther"]
                        .iter()
                        .any(|k| attr(k.as_bytes()).is_some_and(|v| v == "true"));
                    if !special
                        && let (Some(x), Some(y), Some(z)) = (attr(b"X"), attr(b"Y"), attr(b"Z"))
                        && let (Ok(x), Ok(y), Ok(z)) = (x.parse(), y.parse(), z.parse())
                    {
                        r.respawn_points.push((x, y, z));
                    }
                }
            }
            b"map" => {
                if let Some(r) = cur.as_mut()
                    && let (Some(x), Some(y)) = (attr(b"X"), attr(b"Y"))
                    && let (Ok(x), Ok(y)) = (x.parse(), y.parse())
                {
                    r.tiles.push((x, y));
                }
            }
            _ => {}
        }
    }
}

/// Parse the `type="RespawnZone"` entries of `respawn.xml`. Same geometry rules
/// as `zone_data`/`spawn_data` (`<node X= Y=/>` + `minZ`/`maxZ`, all NPoly in
/// this dist), plus the per-race `<race name= point=/>` region map.
fn parse_respawn_zones(path: &str) -> Vec<RespawnZone> {
    let mut out = Vec::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return out;
    };
    let mut reader = Reader::from_str(&content);

    struct Pending {
        name: String,
        is_respawn: bool,
        shape: String,
        min_z: i32,
        max_z: i32,
        rad: Option<i32>,
        xs: Vec<i32>,
        ys: Vec<i32>,
        race_points: HashMap<Race, String>,
    }
    let mut cur: Option<Pending> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone"
                    && let Some(p) = cur.take()
                    && p.is_respawn
                    && let Some(form) =
                        super::spawn_data::build_zone_form(&p.shape, p.xs, p.ys, p.rad)
                {
                    out.push(RespawnZone {
                        name: p.name,
                        territory: Territory {
                            form,
                            min_z: p.min_z,
                            max_z: p.max_z,
                        },
                        race_points: p.race_points,
                    });
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let attr = |key: &[u8]| attr_str(&e, key);
        match e.name().as_ref() {
            b"zone" => {
                cur = Some(Pending {
                    name: attr(b"name").unwrap_or_default(),
                    is_respawn: attr(b"type").as_deref() == Some("RespawnZone"),
                    shape: attr(b"shape").unwrap_or_else(|| "NPoly".to_string()),
                    min_z: attr(b"minZ")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(i32::MIN),
                    max_z: attr(b"maxZ")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(i32::MAX),
                    rad: attr(b"rad").and_then(|v| v.parse().ok()),
                    xs: Vec::new(),
                    ys: Vec::new(),
                    race_points: HashMap::new(),
                });
            }
            b"node" => {
                if let Some(p) = cur.as_mut() {
                    p.xs.push(
                        attr(b"X")
                            .or_else(|| attr(b"x"))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0),
                    );
                    p.ys.push(
                        attr(b"Y")
                            .or_else(|| attr(b"y"))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0),
                    );
                }
            }
            b"race" => {
                if let Some(p) = cur.as_mut()
                    && let (Some(name), Some(point)) = (attr(b"name"), attr(b"point"))
                    && let Some(race) = Race::from_name(&name)
                {
                    p.race_points.insert(race, point);
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_real_dist_and_maps_giran() {
        let data = MapRegionData::load_from(DIST);
        // Giran town center must resolve to the Giran region.
        let region = data.region_at(83000, 148000).expect("region at Giran");
        assert_eq!(region.name, "giran_castle_town");
        // Its first ordinary respawn point (hand-checked from the XML). Giran
        // town proper isn't inside any RespawnZone at this z, so this is the
        // pure map-tile path. (z picked inside the town peace-zone band.)
        assert_eq!(
            data.town_respawn(83000, 148000, -3350, Race::Human, 0),
            Some((82480, 149087, -3350))
        );
    }

    #[test]
    fn unknown_position_falls_back_to_talking_island() {
        let data = MapRegionData::load_from(DIST);
        let (x, y, _z) = data
            .town_respawn(-800_000, -800_000, -3000, Race::Human, 0)
            .expect("default respawn");
        // talking_island_town's first respawn point is on Talking Island.
        assert!(
            x > -130_000 && x < 0 && y > 200_000,
            "unexpected default respawn: {x},{y}"
        );
    }

    #[test]
    fn elven_ruins_respawn_zone_beats_shared_giran_harbour_tile() {
        let data = MapRegionData::load_from(DIST);
        // Elven Ruins (Elf-side Siif teleport) at (48765, 248461, -6160).
        // Its bare map tile (21,25) is claimed by giran_habor, but the death
        // sits inside RespawnZone `talking_island_town_21_25`, which sends
        // every race to talking_island_town.
        let (x, y, _z) = data
            .town_respawn(48765, 248461, -6160, Race::Elf, 0)
            .expect("respawn");
        let ti = data.region_by_name("talking_island_town").unwrap();
        assert!(
            ti.respawn_points.contains(&(x, y, _z)),
            "Elven Ruins death should respawn in Talking Island, got {x},{y}"
        );

        // Sanity: the raw map tile really is Giran Harbour, so without the
        // RespawnZone override this position would have gone there.
        assert_eq!(
            data.region_at(48765, 248461).map(|r| r.name.as_str()),
            Some("giran_habor")
        );
    }

    #[test]
    fn respawn_zone_redirects_dark_elf_out_of_elf_territory() {
        let data = MapRegionData::load_from(DIST);
        // `elf_town_21_18` maps DARK_ELF → darkelf_town (per-race redirect),
        // everyone else → elf_town. Pick a point inside that polygon/z band.
        let (ex, ey, _z) = data
            .town_respawn(40000, 31000, -3000, Race::Elf, 0)
            .expect("elf respawn");
        let (dx, dy, _z2) = data
            .town_respawn(40000, 31000, -3000, Race::DarkElf, 0)
            .expect("de respawn");
        assert!(
            data.region_by_name("elf_town")
                .unwrap()
                .respawn_points
                .contains(&(ex, ey, _z))
        );
        assert!(
            data.region_by_name("darkelf_town")
                .unwrap()
                .respawn_points
                .contains(&(dx, dy, _z2))
        );
    }
}
