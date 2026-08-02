//! Port of `geoengine/GeoEngine` + `geoengine/geodata/GeoData` — geodata
//! loading from the stock `.l2j` region files and the position-validation
//! queries built on it (`canSeeTarget`, `canMoveToTarget`,
//! `getValidLocation`, spawn heights).
//!
//! Pathfinding (`CellPathFinding`) lives in [`path`] and runs on the
//! dedicated [`worker`] thread. Door collision lives in [`doors`] — closed
//! doors block LOS/movement via the segment checks Java runs at the head of
//! these queries (G12). Runtime NSWE editing (the `//geoenable*`/
//! `//geodisable*` admin commands) layers over the immutable regions through
//! the override map, and [`GeoEngine::save_region`] writes those edits back
//! out in the `.l2j` on-disk format. Not ported: fence checks (no fences
//! exist on the Rust side).

pub mod doors;
pub mod line;
pub mod path;
pub mod region;
pub mod worker;

use std::path::Path;

use line::{LinePointIterator, LinePointIterator3D};
use region::{REGION_CELLS_X, REGION_CELLS_Y, Region};
use tracing::{info, warn};

/// `geodata/Cell` NSWE bits.
pub const NSWE_EAST: u8 = 1 << 0;
pub const NSWE_WEST: u8 = 1 << 1;
pub const NSWE_SOUTH: u8 = 1 << 2;
pub const NSWE_NORTH: u8 = 1 << 3;
pub const NSWE_NORTH_EAST: u8 = NSWE_NORTH | NSWE_EAST;
pub const NSWE_NORTH_WEST: u8 = NSWE_NORTH | NSWE_WEST;
pub const NSWE_SOUTH_EAST: u8 = NSWE_SOUTH | NSWE_EAST;
pub const NSWE_SOUTH_WEST: u8 = NSWE_SOUTH | NSWE_WEST;
pub const NSWE_ALL: u8 = NSWE_EAST | NSWE_WEST | NSWE_SOUTH | NSWE_NORTH;

/// `GeoData.WORLD_MIN_X/Y`: geo-grid origin. Note these are *not*
/// `World.WORLD_X/Y_MIN` — the geo grid is anchored at map tile (0,0)
/// (world tile numbering has tile 20 at world x=0), so a region file's
/// `x_y` filename equals `geoX / REGION_CELLS_X` directly.
pub const WORLD_MIN_X: i32 = -655360;
pub const WORLD_MIN_Y: i32 = -589824;

/// `GeoData.GEO_REGIONS_X/Y`.
const GEO_REGIONS_X: i32 = 32;
const GEO_REGIONS_Y: i32 = 32;
const GEO_REGIONS: usize = (GEO_REGIONS_X * GEO_REGIONS_Y) as usize;

/// `World.TILE_X_MIN..TILE_X_MAX` / `TILE_Y_MIN..TILE_Y_MAX` — the region
/// filename ranges scanned at load.
pub const TILE_X_MIN: i32 = 11;
pub const TILE_X_MAX: i32 = 28;
pub const TILE_Y_MIN: i32 = 10;
pub const TILE_Y_MAX: i32 = 26;

/// `GeoEngine`'s LOS tuning constants.
const ELEVATED_SEE_OVER_DISTANCE: i32 = 2;
const MAX_SEE_OVER_HEIGHT: i32 = 48;
const SPAWN_Z_DELTA_LIMIT: i32 = 100;

pub struct GeoEngine {
    /// 32×32 grid; `None` = no geodata (Java `NullRegion`: everything passable
    /// and `worldZ` echoed back).
    regions: Vec<Option<Region>>,
    /// Door collision polygons + open flags (registered at boot before the
    /// engine is shared; open flags are atomics, flipped through `&self`).
    pub doors: doors::DoorGrid,
    /// Runtime NSWE edits (admin `//geoenable*`/`//geodisable*`), keyed by
    /// `(geo_x, geo_y, nearest_z)` → the cell's overridden NSWE bits. The base
    /// regions are immutable (mmap/shared), so edits layer on top here.
    /// [`has_overrides`](Self::has_overrides) gates the query hot path so an
    /// unedited engine never touches the lock.
    nswe_overrides: std::sync::RwLock<std::collections::HashMap<(i32, i32, i32), u8>>,
    has_overrides: std::sync::atomic::AtomicBool,
}

impl GeoEngine {
    /// No geodata at all — every query behaves like Java's `NullRegion`.
    pub fn empty() -> Self {
        Self {
            regions: (0..GEO_REGIONS).map(|_| None).collect(),
            doors: doors::DoorGrid::default(),
            nswe_overrides: std::sync::RwLock::new(std::collections::HashMap::new()),
            has_overrides: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The effective NSWE bits of the cell nearest `world_z` (runtime override
    /// if edited, else the base region; `NSWE_ALL` on unloaded geodata).
    pub fn nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u8 {
        if self
            .has_overrides
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let nz = self.get_nearest_z(geo_x, geo_y, world_z);
            if let Some(&ov) = self.nswe_overrides.read().unwrap().get(&(geo_x, geo_y, nz)) {
                return ov;
            }
        }
        self.region(geo_x, geo_y)
            .map_or(NSWE_ALL, |r| r.nearest_nswe(geo_x, geo_y, world_z))
    }

    /// Admin `//geoenable<dir>` — OR `nswe` into the nearest cell's flags.
    pub fn set_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) {
        self.edit_nswe(geo_x, geo_y, world_z, |cur| cur | nswe);
    }

    /// Admin `//geodisable<dir>` — clear `nswe` from the nearest cell's flags.
    pub fn unset_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) {
        self.edit_nswe(geo_x, geo_y, world_z, |cur| cur & !nswe);
    }

    fn edit_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, f: impl FnOnce(u8) -> u8) {
        let cur = self.nearest_nswe(geo_x, geo_y, world_z);
        let nz = self.get_nearest_z(geo_x, geo_y, world_z);
        self.nswe_overrides
            .write()
            .unwrap()
            .insert((geo_x, geo_y, nz), f(cur));
        self.has_overrides
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Number of runtime NSWE edits currently applied (admin `//geosave`).
    pub fn override_count(&self) -> usize {
        self.nswe_overrides.read().unwrap().len()
    }

    /// The region-file tiles that carry geodata, in `x_y` order — Java's
    /// `//geosaveall` sweep, which walks every tile and skips `NullRegion`s.
    pub fn loaded_tiles(&self) -> Vec<(i32, i32)> {
        (0..GEO_REGIONS_X)
            .flat_map(|x| (0..GEO_REGIONS_Y).map(move |y| (x, y)))
            .filter(|&(x, y)| self.regions[(x * GEO_REGIONS_Y + y) as usize].is_some())
            .collect()
    }

    /// Java `Region.saveToFile` for one tile: write `{x}_{y}.l2j` into `dir`
    /// with this tile's runtime NSWE edits folded into the cells they changed.
    /// `None` when the tile has no geodata (Java's `NullRegion` branch —
    /// "Could not find region"), else whether the write succeeded.
    pub fn save_region(&self, tile_x: i32, tile_y: i32, dir: &Path) -> Option<bool> {
        // Out-of-grid tiles read as "no region", like `region()` — a GM standing
        // at an absurd coordinate must not index past the tile array.
        if !(0..GEO_REGIONS_X).contains(&tile_x) || !(0..GEO_REGIONS_Y).contains(&tile_y) {
            return None;
        }
        let region = self.regions[(tile_x * GEO_REGIONS_Y + tile_y) as usize].as_ref()?;
        let (min_x, min_y) = (tile_x * REGION_CELLS_X, tile_y * REGION_CELLS_Y);
        let edits: std::collections::HashMap<(i32, i32, i32), u8> = self
            .nswe_overrides
            .read()
            .unwrap()
            .iter()
            .filter(|((gx, gy, _), _)| {
                (min_x..min_x + REGION_CELLS_X).contains(gx)
                    && (min_y..min_y + REGION_CELLS_Y).contains(gy)
            })
            .map(|(&k, &v)| (k, v))
            .collect();
        Some(region.save_to_file(&dir.join(format!("{tile_x}_{tile_y}.l2j")), &edits))
    }

    /// Admin `//geomap` — the geodata tile (region file) coords a world position
    /// falls in, plus that tile's world bounds: `((tile_x, tile_y), (min_x,
    /// min_y), (max_x, max_y))`.
    pub fn geomap_tile(&self, world_x: i32, world_y: i32) -> ((i32, i32), (i32, i32), (i32, i32)) {
        let tile_x = self.get_geo_x(world_x) / REGION_CELLS_X;
        let tile_y = self.get_geo_y(world_y) / REGION_CELLS_Y;
        let (size_x, size_y) = (REGION_CELLS_X * 16, REGION_CELLS_Y * 16);
        let min_x = tile_x * size_x + WORLD_MIN_X;
        let min_y = tile_y * size_y + WORLD_MIN_Y;
        (
            (tile_x, tile_y),
            (min_x, min_y),
            (min_x + size_x - 1, min_y + size_y - 1),
        )
    }

    /// Java `GeoEngine()` constructor: scan `geodata_path` for
    /// `{regionX}_{regionY}.l2j` over the world's tile range and mmap each
    /// file found. A missing directory or file is not an error (regions
    /// simply stay `None`); a corrupt file is skipped with a warning.
    pub fn load(geodata_path: &Path) -> Self {
        let mut engine = Self::empty();
        let mut loaded = 0;
        for region_x in TILE_X_MIN..=TILE_X_MAX {
            for region_y in TILE_Y_MIN..=TILE_Y_MAX {
                let file = geodata_path.join(format!("{region_x}_{region_y}.l2j"));
                if !file.exists() {
                    continue;
                }
                match Region::load(&file) {
                    Ok(region) => {
                        engine.regions[(region_x * GEO_REGIONS_Y + region_y) as usize] =
                            Some(region);
                        loaded += 1;
                    }
                    Err(e) => warn!("GeoEngine: failed to load {}: {e}", file.display()),
                }
            }
        }
        info!("GeoEngine: Loaded {loaded} regions.");
        engine
    }

    /// Install a region directly (tests / geo editor).
    pub fn set_region(&mut self, region_x: i32, region_y: i32, region: Region) {
        self.regions[(region_x * GEO_REGIONS_Y + region_y) as usize] = Some(region);
    }

    fn region(&self, geo_x: i32, geo_y: i32) -> Option<&Region> {
        if geo_x < 0 || geo_y < 0 {
            return None;
        }
        let idx = (geo_x / REGION_CELLS_X) * GEO_REGIONS_Y + (geo_y / REGION_CELLS_Y);
        self.regions.get(idx as usize)?.as_ref()
    }

    // --- Coordinate transforms (`GeoData`) ---

    pub fn get_geo_x(&self, world_x: i32) -> i32 {
        (world_x - WORLD_MIN_X) / 16
    }

    pub fn get_geo_y(&self, world_y: i32) -> i32 {
        (world_y - WORLD_MIN_Y) / 16
    }

    pub fn get_world_x(&self, geo_x: i32) -> i32 {
        (geo_x * 16) + WORLD_MIN_X + 8
    }

    pub fn get_world_y(&self, geo_y: i32) -> i32 {
        (geo_y * 16) + WORLD_MIN_Y + 8
    }

    // --- Cell queries (`GeoData`, `NullRegion` behaviour when unloaded) ---

    pub fn has_geo_pos(&self, geo_x: i32, geo_y: i32) -> bool {
        self.region(geo_x, geo_y).is_some()
    }

    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        if self
            .has_overrides
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let nz = self.get_nearest_z(geo_x, geo_y, world_z);
            if let Some(&ov) = self.nswe_overrides.read().unwrap().get(&(geo_x, geo_y, nz)) {
                return (ov & nswe) == nswe;
            }
        }
        self.region(geo_x, geo_y)
            .is_none_or(|r| r.check_nearest_nswe(geo_x, geo_y, world_z, nswe))
    }

    pub fn get_nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.region(geo_x, geo_y)
            .map_or(world_z, |r| r.nearest_z(geo_x, geo_y, world_z))
    }

    pub fn get_next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.region(geo_x, geo_y)
            .map_or(world_z, |r| r.next_lower_z(geo_x, geo_y, world_z))
    }

    pub fn get_next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        self.region(geo_x, geo_y)
            .map_or(world_z, |r| r.next_higher_z(geo_x, geo_y, world_z))
    }

    /// Java `checkNearestNsweAntiCornerCut`: a diagonal step also requires
    /// both adjacent cardinal cells to be enterable. Ported verbatim,
    /// including the NW branch checking `(geoX, geoY-1)` twice where the
    /// commented-out original intent was `(geoX-1, geoY)` — kept for parity.
    pub fn check_nearest_nswe_anti_corner_cut(
        &self,
        geo_x: i32,
        geo_y: i32,
        world_z: i32,
        nswe: u8,
    ) -> bool {
        let mut can = true;
        if (nswe & NSWE_NORTH_EAST) == NSWE_NORTH_EAST {
            can = self.check_nearest_nswe(geo_x, geo_y - 1, world_z, NSWE_EAST)
                && self.check_nearest_nswe(geo_x + 1, geo_y, world_z, NSWE_NORTH);
        }
        if can && (nswe & NSWE_NORTH_WEST) == NSWE_NORTH_WEST {
            can = self.check_nearest_nswe(geo_x, geo_y - 1, world_z, NSWE_WEST)
                && self.check_nearest_nswe(geo_x, geo_y - 1, world_z, NSWE_NORTH);
        }
        if can && (nswe & NSWE_SOUTH_EAST) == NSWE_SOUTH_EAST {
            can = self.check_nearest_nswe(geo_x, geo_y + 1, world_z, NSWE_EAST)
                && self.check_nearest_nswe(geo_x + 1, geo_y, world_z, NSWE_SOUTH);
        }
        if can && (nswe & NSWE_SOUTH_WEST) == NSWE_SOUTH_WEST {
            can = self.check_nearest_nswe(geo_x, geo_y + 1, world_z, NSWE_WEST)
                && self.check_nearest_nswe(geo_x - 1, geo_y, world_z, NSWE_SOUTH);
        }
        can && self.check_nearest_nswe(geo_x, geo_y, world_z, nswe)
    }

    // --- World-coordinate helpers ---

    /// Java `hasGeo(x, y)`.
    pub fn has_geo(&self, world_x: i32, world_y: i32) -> bool {
        self.has_geo_pos(self.get_geo_x(world_x), self.get_geo_y(world_y))
    }

    /// Java `getHeight`: nearest geodata z at a world position.
    pub fn get_height(&self, world_x: i32, world_y: i32, world_z: i32) -> i32 {
        self.get_nearest_z(self.get_geo_x(world_x), self.get_geo_y(world_y), world_z)
    }

    /// Java `getSpawnHeight`: snap a spawn z down onto the geodata surface,
    /// unless it is implausibly far (> 100 units) from the requested z.
    pub fn get_spawn_height(&self, world_x: i32, world_y: i32, world_z: i32) -> i32 {
        let geo_x = self.get_geo_x(world_x);
        let geo_y = self.get_geo_y(world_y);
        if !self.has_geo_pos(geo_x, geo_y) {
            return world_z;
        }
        let next_lower_z = self.get_next_lower_z(geo_x, geo_y, world_z + 20);
        if (next_lower_z - world_z).abs() <= SPAWN_Z_DELTA_LIMIT {
            next_lower_z
        } else {
            world_z
        }
    }

    /// Java `getLosGeoZ`: the z the sight line is forced to at `(cur_x,
    /// cur_y)` — ground level if the step is enterable, otherwise the next
    /// obstacle top.
    fn get_los_geo_z(
        &self,
        prev_x: i32,
        prev_y: i32,
        prev_geo_z: i32,
        cur_x: i32,
        cur_y: i32,
        nswe: u8,
    ) -> i32 {
        if self.check_nearest_nswe_anti_corner_cut(prev_x, prev_y, prev_geo_z, nswe) {
            self.get_nearest_z(cur_x, cur_y, prev_geo_z)
        } else {
            self.get_next_higher_z(cur_x, cur_y, prev_geo_z)
        }
    }

    /// Java `canSeeTarget(x, y, z, tx, ty, tz)` — line of sight along the
    /// geo-cell bee line, allowing sight over obstacles up to 48 units above
    /// the line. Closed doors occlude first (`checkIfDoorsBetween`, the
    /// double-face variant); fence checks are not ported (none exist yet).
    pub fn can_see_target(&self, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> bool {
        if self.doors.check_doors_between(x, y, z, tx, ty, tz, true) {
            return false;
        }
        let mut geo_x = self.get_geo_x(x);
        let mut geo_y = self.get_geo_y(y);
        let mut t_geo_x = self.get_geo_x(tx);
        let mut t_geo_y = self.get_geo_y(ty);
        let mut nearest_from_z = self.get_nearest_z(geo_x, geo_y, z);
        let mut nearest_to_z = self.get_nearest_z(t_geo_x, t_geo_y, tz);

        // Fastpath: same cell.
        if geo_x == t_geo_x && geo_y == t_geo_y {
            return !self.has_geo_pos(t_geo_x, t_geo_y) || nearest_from_z == nearest_to_z;
        }

        // Always trace from the higher end downwards.
        if nearest_to_z > nearest_from_z {
            std::mem::swap(&mut nearest_from_z, &mut nearest_to_z);
            std::mem::swap(&mut geo_x, &mut t_geo_x);
            std::mem::swap(&mut geo_y, &mut t_geo_y);
        }

        let mut iter =
            LinePointIterator3D::new(geo_x, geo_y, nearest_from_z, t_geo_x, t_geo_y, nearest_to_z);
        // First point is guaranteed available — our own position.
        iter.next();
        let mut prev_x = iter.x();
        let mut prev_y = iter.y();
        let mut prev_geo_z = iter.z();
        let mut pt_index = 0;

        while iter.next() {
            let cur_x = iter.x();
            let cur_y = iter.y();
            if cur_x == prev_x && cur_y == prev_y {
                continue;
            }
            let bee_cur_z = iter.z();
            let mut cur_geo_z = prev_geo_z;

            if self.has_geo_pos(cur_x, cur_y) {
                let nswe = compute_nswe(prev_x, prev_y, cur_x, cur_y);
                cur_geo_z = self.get_los_geo_z(prev_x, prev_y, prev_geo_z, cur_x, cur_y, nswe);
                let max_height = if pt_index < ELEVATED_SEE_OVER_DISTANCE {
                    nearest_from_z + MAX_SEE_OVER_HEIGHT
                } else {
                    bee_cur_z + MAX_SEE_OVER_HEIGHT
                };
                let mut can_see_through = false;
                if cur_geo_z <= max_height {
                    if (nswe & NSWE_NORTH_EAST) == NSWE_NORTH_EAST {
                        let north = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x,
                            prev_y - 1,
                            NSWE_EAST,
                        );
                        let east = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x + 1,
                            prev_y,
                            NSWE_NORTH,
                        );
                        can_see_through = north <= max_height
                            && east <= max_height
                            && north <= self.get_nearest_z(prev_x, prev_y - 1, bee_cur_z)
                            && east <= self.get_nearest_z(prev_x + 1, prev_y, bee_cur_z);
                    } else if (nswe & NSWE_NORTH_WEST) == NSWE_NORTH_WEST {
                        let north = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x,
                            prev_y - 1,
                            NSWE_WEST,
                        );
                        let west = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x - 1,
                            prev_y,
                            NSWE_NORTH,
                        );
                        can_see_through = north <= max_height
                            && west <= max_height
                            && north <= self.get_nearest_z(prev_x, prev_y - 1, bee_cur_z)
                            && west <= self.get_nearest_z(prev_x - 1, prev_y, bee_cur_z);
                    } else if (nswe & NSWE_SOUTH_EAST) == NSWE_SOUTH_EAST {
                        let south = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x,
                            prev_y + 1,
                            NSWE_EAST,
                        );
                        let east = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x + 1,
                            prev_y,
                            NSWE_SOUTH,
                        );
                        can_see_through = south <= max_height
                            && east <= max_height
                            && south <= self.get_nearest_z(prev_x, prev_y + 1, bee_cur_z)
                            && east <= self.get_nearest_z(prev_x + 1, prev_y, bee_cur_z);
                    } else if (nswe & NSWE_SOUTH_WEST) == NSWE_SOUTH_WEST {
                        let south = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x,
                            prev_y + 1,
                            NSWE_WEST,
                        );
                        let west = self.get_los_geo_z(
                            prev_x,
                            prev_y,
                            prev_geo_z,
                            prev_x - 1,
                            prev_y,
                            NSWE_SOUTH,
                        );
                        can_see_through = south <= max_height
                            && west <= max_height
                            && south <= self.get_nearest_z(prev_x, prev_y + 1, bee_cur_z)
                            && west <= self.get_nearest_z(prev_x - 1, prev_y, bee_cur_z);
                    } else {
                        can_see_through = true;
                    }
                }
                if !can_see_through {
                    return false;
                }
            }

            prev_x = cur_x;
            prev_y = cur_y;
            prev_geo_z = cur_geo_z;
            pt_index += 1;
        }
        true
    }

    /// Java `getValidLocation` (fence checks omitted — none exist yet):
    /// walk the cell line towards the destination and return the last
    /// walkable location — the destination itself if nothing blocks. A
    /// closed door across the line keeps the mover at the origin.
    pub fn get_valid_location(
        &self,
        x: i32,
        y: i32,
        z: i32,
        tx: i32,
        ty: i32,
        tz: i32,
    ) -> (i32, i32, i32) {
        let geo_x = self.get_geo_x(x);
        let geo_y = self.get_geo_y(y);
        let nearest_from_z = self.get_nearest_z(geo_x, geo_y, z);
        let t_geo_x = self.get_geo_x(tx);
        let t_geo_y = self.get_geo_y(ty);
        let nearest_to_z = self.get_nearest_z(t_geo_x, t_geo_y, tz);

        if self
            .doors
            .check_doors_between(x, y, nearest_from_z, tx, ty, nearest_to_z, false)
        {
            return (x, y, self.get_height(x, y, nearest_from_z));
        }

        let mut iter = LinePointIterator::new(geo_x, geo_y, t_geo_x, t_geo_y);
        iter.next(); // first point is our own cell
        let mut prev_x = iter.x();
        let mut prev_y = iter.y();
        let mut prev_z = nearest_from_z;

        while iter.next() {
            let cur_x = iter.x();
            let cur_y = iter.y();
            let cur_z = self.get_nearest_z(cur_x, cur_y, prev_z);
            if self.has_geo_pos(prev_x, prev_y)
                && !self.check_nearest_nswe_anti_corner_cut(
                    prev_x,
                    prev_y,
                    prev_z,
                    compute_nswe(prev_x, prev_y, cur_x, cur_y),
                )
            {
                // Can't continue — return the last walkable cell.
                return (self.get_world_x(prev_x), self.get_world_y(prev_y), prev_z);
            }
            prev_x = cur_x;
            prev_y = cur_y;
            prev_z = cur_z;
        }
        if self.has_geo_pos(prev_x, prev_y) && prev_z != nearest_to_z {
            // Different floor reached than requested — stay at the origin.
            (x, y, nearest_from_z)
        } else {
            (tx, ty, nearest_to_z)
        }
    }

    /// Java `canMoveToTarget` (fence checks omitted): can a character walk
    /// the straight cell line from one location to the other? Closed doors
    /// block (this is also the path worker's postfilter, so routes never
    /// thread a closed door).
    pub fn can_move_to_target(&self, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> bool {
        let geo_x = self.get_geo_x(x);
        let geo_y = self.get_geo_y(y);
        let nearest_from_z = self.get_nearest_z(geo_x, geo_y, z);
        let t_geo_x = self.get_geo_x(tx);
        let t_geo_y = self.get_geo_y(ty);
        let nearest_to_z = self.get_nearest_z(t_geo_x, t_geo_y, tz);

        if self
            .doors
            .check_doors_between(x, y, nearest_from_z, tx, ty, nearest_to_z, false)
        {
            return false;
        }

        let mut iter = LinePointIterator::new(geo_x, geo_y, t_geo_x, t_geo_y);
        iter.next();
        let mut prev_x = iter.x();
        let mut prev_y = iter.y();
        let mut prev_z = nearest_from_z;

        while iter.next() {
            let cur_x = iter.x();
            let cur_y = iter.y();
            let cur_z = self.get_nearest_z(cur_x, cur_y, prev_z);
            if self.has_geo_pos(prev_x, prev_y)
                && !self.check_nearest_nswe_anti_corner_cut(
                    prev_x,
                    prev_y,
                    prev_z,
                    compute_nswe(prev_x, prev_y, cur_x, cur_y),
                )
            {
                return false;
            }
            prev_x = cur_x;
            prev_y = cur_y;
            prev_z = cur_z;
        }
        !self.has_geo_pos(prev_x, prev_y) || prev_z == nearest_to_z
    }
}

/// Java `GeoUtils.computeNswe` — the direction mask of a single-cell step.
/// Callers never pass identical points (the 2D iterator always advances an
/// axis; the LOS loop skips z-only steps).
fn compute_nswe(last_x: i32, last_y: i32, x: i32, y: i32) -> u8 {
    use std::cmp::Ordering::*;
    match (x.cmp(&last_x), y.cmp(&last_y)) {
        (Greater, Greater) => NSWE_SOUTH_EAST,
        (Greater, Less) => NSWE_NORTH_EAST,
        (Greater, Equal) => NSWE_EAST,
        (Less, Greater) => NSWE_SOUTH_WEST,
        (Less, Less) => NSWE_NORTH_WEST,
        (Less, Equal) => NSWE_WEST,
        (Equal, Greater) => NSWE_SOUTH,
        (Equal, Less) => NSWE_NORTH,
        (Equal, Equal) => unreachable!("computeNswe called with identical points"),
    }
}

/// Build an all-complex synthetic region where each cell's (height, nswe)
/// comes from `f(geo_x_in_region, geo_y_in_region)`. Test/tooling helper —
/// production regions come from `Region::load`.
pub fn synthetic_region(f: impl Fn(i32, i32) -> (i16, u8)) -> Region {
    use region::encode_cell;
    let mut img =
        Vec::with_capacity(region::REGION_BLOCKS as usize * (1 + region::BLOCK_CELLS as usize * 2));
    for bx in 0..region::REGION_BLOCKS_X {
        for by in 0..region::REGION_BLOCKS_Y {
            img.push(1u8); // complex
            for cx in 0..region::BLOCK_CELLS_X {
                for cy in 0..region::BLOCK_CELLS_Y {
                    let (h, nswe) = f(bx * 8 + cx, by * 8 + cy);
                    img.extend_from_slice(&encode_cell(h, nswe).to_le_bytes());
                }
            }
        }
    }
    Region::from_owned(img).expect("synthetic region image is well-formed")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Region (11, 10) — the world's lowest tile — so local cell (0,0) is at
    /// geo (11*2048, 10*2048).
    const BASE_GEO_X: i32 = 11 * REGION_CELLS_X;
    const BASE_GEO_Y: i32 = 10 * REGION_CELLS_Y;

    fn engine_with(f: impl Fn(i32, i32) -> (i16, u8)) -> GeoEngine {
        let mut engine = GeoEngine::empty();
        engine.set_region(11, 10, synthetic_region(f));
        engine
    }

    /// World coords of the centre of local cell (cx, cy) of the test region.
    fn world(g: &GeoEngine, cx: i32, cy: i32) -> (i32, i32) {
        (
            g.get_world_x(BASE_GEO_X + cx),
            g.get_world_y(BASE_GEO_Y + cy),
        )
    }

    /// Runtime NSWE edits (admin `//geodisable*`/`//geoenable*`) layer over the
    /// immutable base region and change what `check_nearest_nswe` reports — so a
    /// disabled direction actually blocks movement.
    #[test]
    fn runtime_nswe_override_edits_passability() {
        let g = engine_with(|_, _| (0, NSWE_ALL));
        let (gx, gy, z) = (BASE_GEO_X + 5, BASE_GEO_Y + 5, 0);
        assert!(g.has_geo_pos(gx, gy));
        assert!(
            g.check_nearest_nswe(gx, gy, z, NSWE_NORTH),
            "open before edit"
        );

        g.unset_nearest_nswe(gx, gy, z, NSWE_NORTH);
        assert!(
            !g.check_nearest_nswe(gx, gy, z, NSWE_NORTH),
            "north blocked after //geodisablenorth"
        );
        assert!(
            g.check_nearest_nswe(gx, gy, z, NSWE_SOUTH),
            "other directions untouched"
        );
        assert_eq!(g.override_count(), 1, "one cell edited");

        g.set_nearest_nswe(gx, gy, z, NSWE_NORTH);
        assert!(
            g.check_nearest_nswe(gx, gy, z, NSWE_NORTH),
            "north re-enabled after //geoenablenorth"
        );

        // An unedited cell still reads from the base region (no override lookup
        // false-positives).
        assert!(
            g.check_nearest_nswe(gx + 1, gy, z, NSWE_ALL),
            "neighbour cell unaffected"
        );
    }

    #[test]
    fn coordinate_transforms_round_trip() {
        let g = GeoEngine::empty();
        // Giran: 82698, 148638 → region file 22_22 (world tile 20_18 is the
        // grid origin; 82698 >> 15 = 2 tiles east, 148638 >> 15 = 4 south).
        let gx = g.get_geo_x(82698);
        let gy = g.get_geo_y(148638);
        assert_eq!(gx / REGION_CELLS_X, 22);
        assert_eq!(gy / REGION_CELLS_Y, 22);
        // World→geo→world lands on the cell centre, within one cell (16 units).
        assert!((g.get_world_x(gx) - 82698).abs() <= 16);
        assert!((g.get_world_y(gy) - 148638).abs() <= 16);
    }

    /// Loads the real datapack geodata (mmap, so cheap) and sanity-checks
    /// queries around Giran against known world geometry — same "test against
    /// the real dist files" convention as the item/skill loaders.
    #[test]
    fn loads_real_dist_geodata() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/geodata"
        ));
        let g = GeoEngine::load(path);
        // Giran town square: ground height must be within a step of retail z.
        assert!(
            g.has_geo(82698, 148638),
            "Giran region 22_22 should be loaded"
        );
        let height = g.get_height(82698, 148638, -3473);
        assert!((height - -3473).abs() <= 32, "Giran ground z, got {height}");
        // Open square: adjacent points see each other and are walkable.
        assert!(g.can_see_target(82698, 148638, height, 82900, 148700, height));
        assert!(g.can_move_to_target(82698, 148638, height, 82900, 148700, height));
        // Spawn snapping pulls a mid-air z back to the ground.
        assert_eq!(g.get_spawn_height(82698, 148638, height + 60), height);
    }

    /// A wall column (tall, non-enterable, approach cells blocked east) must
    /// stop both sight and movement; `get_valid_location` walks up to it.
    #[test]
    fn wall_blocks_los_and_movement() {
        let g = engine_with(|x, _y| {
            if x == 10 {
                (200, 0) // the wall itself: 200 units high, no exits
            } else if x == 9 {
                (0, NSWE_ALL & !NSWE_EAST) // ground before it: can't step east
            } else {
                (0, NSWE_ALL)
            }
        });
        let (x0, y0) = world(&g, 5, 5);
        let (x1, y1) = world(&g, 15, 5);
        assert!(
            !g.can_see_target(x0, y0, 0, x1, y1, 0),
            "wall must block LOS"
        );
        assert!(!g.can_move_to_target(x0, y0, 0, x1, y1, 0));
        // Last walkable cell before the wall is x = 9.
        let (vx, vy, vz) = g.get_valid_location(x0, y0, 0, x1, y1, 0);
        assert_eq!((vx, vy, vz), (g.get_world_x(BASE_GEO_X + 9), y0, 0));
        // Same side of the wall: everything passes.
        let (x2, y2) = world(&g, 8, 5);
        assert!(g.can_see_target(x0, y0, 0, x2, y2, 0));
        assert!(g.can_move_to_target(x0, y0, 0, x2, y2, 0));
        assert_eq!(g.get_valid_location(x0, y0, 0, x2, y2, 0), (x2, y2, 0));
    }

    /// A low fence (≤ 48 units, `MAX_SEE_OVER_HEIGHT`) can be seen over but
    /// not walked through.
    #[test]
    fn low_fence_allows_sight_but_not_movement() {
        let g = engine_with(|x, _y| {
            if x == 10 {
                (32, 0)
            } else if x == 9 {
                (0, NSWE_ALL & !NSWE_EAST)
            } else {
                (0, NSWE_ALL)
            }
        });
        let (x0, y0) = world(&g, 5, 5);
        let (x1, y1) = world(&g, 15, 5);
        assert!(g.can_see_target(x0, y0, 0, x1, y1, 0), "48-unit see-over");
        assert!(!g.can_move_to_target(x0, y0, 0, x1, y1, 0));
    }

    #[test]
    fn empty_engine_behaves_like_null_region() {
        let g = GeoEngine::empty();
        assert!(!g.has_geo(82698, 148638));
        assert_eq!(g.get_height(82698, 148638, -3473), -3473);
        assert!(g.can_see_target(0, 0, 0, 5000, 5000, 500));
        assert!(g.can_move_to_target(0, 0, 0, 5000, 5000, 500));
        assert_eq!(
            g.get_valid_location(0, 0, 0, 5000, 5000, 500),
            (5000, 5000, 500)
        );
        assert_eq!(g.get_spawn_height(82698, 148638, -3473), -3473);
    }
}
