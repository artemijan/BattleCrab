//! Port of `instancemanager/MapRegionManager` + `model/MapRegion`, scoped to
//! the death→"to village" flow: map every world position to its region via
//! the 32768-unit map tiles and hand out the region's town respawn points.
//! Not ported: chaotic (karma) points, banned-race redirects, castle/fort/
//! clan-hall teleport targets — none of those systems exist yet.

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const MAPREGION_DIR: &str = "data/mapregion";

/// Java `MapRegionManager.DEFAULT_RESPAWN`.
const DEFAULT_RESPAWN: &str = "talking_island_town";

#[derive(Debug, Clone)]
pub struct MapRegion {
    pub name: String,
    /// Ordinary (non-chaotic, non-other) respawn points.
    pub respawn_points: Vec<(i32, i32, i32)>,
    /// `<map X= Y=/>` tile coordinates this region covers.
    pub tiles: Vec<(i32, i32)>,
}

pub struct MapRegionData {
    regions: Vec<MapRegion>,
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
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut paths: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xml"))
                .collect();
            paths.sort();
            for path in paths {
                parse_file(&path, &mut regions);
            }
        }
        info!("MapRegionData: Loaded {} map regions.", regions.len());
        Self { regions }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { regions: Vec::new() }
    }

    /// Synthetic regions for unit tests.
    #[doc(hidden)]
    pub fn from_regions(regions: Vec<MapRegion>) -> Self {
        Self { regions }
    }

    /// `getMapRegion(locX, locY)`.
    pub fn region_at(&self, x: i32, y: i32) -> Option<&MapRegion> {
        let tile = (map_tile_x(x), map_tile_y(y));
        self.regions.iter().find(|r| r.tiles.contains(&tile))
    }

    /// `getTeleToLocation(creature, TeleportWhereType.TOWN)` narrowed to the
    /// no-clan/no-karma path: the enclosing region's spawn points, falling
    /// back to `talking_island_town` like Java's catch-all. `pick` indexes
    /// into the point list (`RandomRespawnInTownEnabled` — the caller rolls).
    pub fn town_respawn(&self, x: i32, y: i32, pick: usize) -> Option<(i32, i32, i32)> {
        let region = self
            .region_at(x, y)
            .or_else(|| self.regions.iter().find(|r| r.name == DEFAULT_RESPAWN))?;
        if region.respawn_points.is_empty() {
            return None;
        }
        Some(region.respawn_points[pick % region.respawn_points.len()])
    }
}

fn parse_file(path: &std::path::Path, out: &mut Vec<MapRegion>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let mut reader = Reader::from_str(&content);
    let mut cur: Option<MapRegion> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"region" {
                    if let Some(r) = cur.take() {
                        out.push(r);
                    }
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let attr = |key: &[u8]| -> Option<String> {
            e.attributes()
                .flatten()
                .find(|a| a.key.as_ref() == key)
                .map(|a| String::from_utf8_lossy(&a.value).into_owned())
        };
        match e.name().as_ref() {
            b"region" => {
                cur = Some(MapRegion {
                    name: attr(b"name").unwrap_or_default(),
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
                    if !special {
                        if let (Some(x), Some(y), Some(z)) = (attr(b"X"), attr(b"Y"), attr(b"Z")) {
                            if let (Ok(x), Ok(y), Ok(z)) = (x.parse(), y.parse(), z.parse()) {
                                r.respawn_points.push((x, y, z));
                            }
                        }
                    }
                }
            }
            b"map" => {
                if let Some(r) = cur.as_mut() {
                    if let (Some(x), Some(y)) = (attr(b"X"), attr(b"Y")) {
                        if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                            r.tiles.push((x, y));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_dist_and_maps_giran() {
        let data = MapRegionData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Giran town center must resolve to the Giran region.
        let region = data.region_at(83000, 148000).expect("region at Giran");
        assert_eq!(region.name, "giran_castle_town");
        // Its first ordinary respawn point (hand-checked from the XML).
        assert_eq!(data.town_respawn(83000, 148000, 0), Some((82480, 149087, -3350)));
    }

    #[test]
    fn unknown_position_falls_back_to_talking_island() {
        let data = MapRegionData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        let (x, y, _z) = data.town_respawn(-800_000, -800_000, 0).expect("default respawn");
        // talking_island_town's first respawn point is on Talking Island.
        assert!(x > -130_000 && x < 0 && y > 200_000, "unexpected default respawn: {x},{y}");
    }
}
