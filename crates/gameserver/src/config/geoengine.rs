//! `GeoEngine.ini` — port of the `GEOENGINE_CONFIG_FILE` block of `Config.java`.
//! Only the keys the ported engine uses are loaded; the pathfinding tuning
//! knobs (buffers, weights, postfilter) wait for the CellPathFinding port.

use commons::config::PropertiesParser;

pub const GEOENGINE_CONFIG_FILE: &str = "config/GeoEngine.ini";

pub struct GeoEngineConfig {
    /// `PathFinding`: 0 = geodata checks disabled, 1 = path node files,
    /// 2 = cell pathfinding. The Rust port treats any non-zero value as
    /// "geodata movement checks on" (the pathfinder itself is not ported yet).
    pub path_finding: i32,
    /// `GeoDataPath`: directory scanned for `{x}_{y}.l2j` region files.
    pub geodata_path: String,
}

impl GeoEngineConfig {
    pub fn load() -> Self {
        let p = PropertiesParser::load(GEOENGINE_CONFIG_FILE);
        Self {
            path_finding: p.get_int("PathFinding", 0),
            geodata_path: p.get_string("GeoDataPath", "./data/geodata/"),
        }
    }
}
