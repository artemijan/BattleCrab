//! `GeoEngine.ini` — port of the `GEOENGINE_CONFIG_FILE` block of `Config.java`.

use commons::config::PropertiesParser;

use crate::geo::path::{PathConfig, parse_buffer_sizes};

pub const GEOENGINE_CONFIG_FILE: &str = "config/GeoEngine.ini";

pub struct GeoEngineConfig {
    /// `PathFinding`: 0 = geodata checks disabled, 1 = path node files,
    /// 2 = cell pathfinding. The Rust port treats any non-zero value as
    /// "geodata movement checks on + cell pathfinding" (the geo-node file
    /// variant of mode 1 was never ported — Java's own default is 2).
    pub path_finding: i32,
    /// `GeoDataPath`: directory scanned for `{x}_{y}.l2j` region files.
    pub geodata_path: String,
    /// `GeoEditPath`: where `//geosave`/`//geosaveall` write edited regions
    /// (`Config.GEOEDIT_PATH`). Never read back — the server always loads from
    /// `geodata_path`, so a save is an export for the GM to move over.
    pub geoedit_path: String,
    /// The `CellPathFinding` tuning knobs (buffers, weights, postfilter),
    /// handed to the path worker.
    pub path: PathConfig,
}

impl GeoEngineConfig {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(root: &str) -> Self {
        let p = PropertiesParser::load_rel(root, GEOENGINE_CONFIG_FILE);
        Self {
            path_finding: p.get_int("PathFinding", 0),
            geodata_path: super::datapack_path(
                root,
                &p.get_string("GeoDataPath", "./data/geodata/"),
            ),
            geoedit_path: super::datapack_path(root, &p.get_string("GeoEditPath", "saves")),
            path: PathConfig {
                buffer_sizes: parse_buffer_sizes(&p.get_string(
                    "PathFindBuffers",
                    "100x6;128x6;192x6;256x4;320x4;384x4;500x2",
                )),
                low_weight: p.get_float("LowWeight", 0.5) as f64,
                medium_weight: p.get_float("MediumWeight", 2.0) as f64,
                high_weight: p.get_float("HighWeight", 3.0) as f64,
                advanced_diagonal_strategy: p.get_bool("AdvancedDiagonalStrategy", true),
                diagonal_weight: p.get_float("DiagonalWeight", 0.707) as f64,
                max_postfilter_passes: p.get_int("MaxPostfilterPasses", 3),
            },
        }
    }
}
