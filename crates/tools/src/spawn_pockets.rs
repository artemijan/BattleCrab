//! Finds spawn rows that land in a geodata "pocket" — a layer underneath the
//! floor players actually walk on, where the mob is invisible, unaggroable and
//! unhittable until its AI walks it out and `getValidLocation` re-grounds it.
//!
//! # The defect
//!
//! `Spawn.initializeNpc` snaps a spawn z with `getHeight` = `getNearestZ`,
//! which picks the nearest layer by |dz| — *not* the surface at or below.
//! Dungeon cells carry spurious slabs 48-128 units under the floor, so a
//! datapack z of `floor - 50` lands under the floor wherever a slab exists:
//!
//! ```text
//! z="-12131" -> -12176 instead of -12080   (Cruma Tower)
//! z="-9092"  -> -9096  instead of -9040
//! ```
//!
//! # Why it is detected this way
//!
//! Two detectors that look obvious do not work:
//!
//! - **Component size.** A pocket is not always small: Cruma's slab spans the
//!   whole tower, flood filling to 800+ cells exactly like a real floor.
//! - **Reachability alone.** The engine's step rule has no vertical limit, so a
//!   fill leaks onto the slab wherever `getNearestZ` jumps, and the slab then
//!   looks reachable everywhere.
//!
//! What works is asking the engine the question a player asks: seed a fill from
//! the coordinates teleporters actually drop players on, then per row check
//! whether a walker starting on the floor can *arrive* at the mob's layer
//! ([`GeoEngine::get_valid_location`] bails back to the origin when the walk
//! ends on a different layer than requested) and whether the floor can *see*
//! it ([`GeoEngine::can_see_target`]).
//!
//! Thresholds are calibrated, not guessed: [`Candidate`] carries the raw
//! metrics of every row considered, and the cut-offs below come from ten Cruma
//! rows fixed and confirmed in game. The sweep recovers all ten.

use crate::datapack::{Bbox, SpawnRow};
use gameserver::geo;

use std::collections::{HashSet, VecDeque};

/// A sub-floor slab sits tens of units under the floor; a real storey is
/// hundreds to thousands. Beyond this it is a different level, not a burial —
/// this is what keeps caves and lower floors out of the report.
pub const MAX_SLAB_GAP: i32 = 130;
/// Vantage points from which a walker may still reach the mob's own layer.
/// Not zero: `get_valid_location` walks a straight cell line, and from the odd
/// angle that line drags z down onto the slab (the in-game-verified Cruma
/// Dicor scores 1).
pub const MAX_WALKABLE_HITS: usize = 1;
/// The floor above must be walkable from most vantage points.
pub const MIN_FLOOR_WALKS: usize = 8;
/// A buried mob may still be visible from a stray angle (Dicor scored 2 and
/// was still unhittable in game).
pub const MAX_VISIBLE_BEFORE: usize = 2;
/// ...and must become widely visible once lifted onto the floor.
pub const MIN_VISIBLE_AFTER: usize = 8;
/// Vantage cells must be on the candidate floor's own storey.
const SAME_STOREY: i32 = 80;
/// Vantage points are sampled from the floor in this world-distance band.
const VANTAGE_MIN: i32 = 150;
const VANTAGE_MAX: i32 = 1100;
const VANTAGE_COUNT: usize = 24;
/// Fill ceiling. A full geo region is ~4M cells and several layers deep; this
/// is a runaway guard, not a working limit — a saturated fill silently loses
/// coverage, which is why [`Report::uncovered`] is reported alongside.
const FILL_CAP: usize = 24_000_000;

/// What to sweep. Nothing here is read from the environment.
pub struct Config<'a> {
    /// Spawn rows to judge (already loaded, so a GUI can filter them first).
    pub rows: &'a [SpawnRow],
    /// Coordinates players demonstrably stand on — teleport destinations, plus
    /// anything else known-good (a player-reported position, say).
    pub seeds: &'a [(i32, i32, i32)],
    /// Area to fill and judge. The fill is area-bounded, so sweeping the world
    /// means sweeping region by region.
    pub bbox: Bbox,
}

/// One row that had a candidate floor above it, with the metrics behind the
/// verdict. Kept even when not buried so thresholds can be re-calibrated
/// against real numbers.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub row: SpawnRow,
    /// Where `getHeight` actually puts the mob.
    pub snapped_z: i32,
    /// Lowest walkable layer above it.
    pub floor_z: i32,
    /// Datapack z that would snap onto `floor_z`.
    pub suggested_z: i32,
    pub vantage_points: usize,
    /// Vantage points from which a walker can arrive at `snapped_z`.
    pub walk_to_snapped: usize,
    /// ...and at `floor_z`.
    pub walk_to_floor: usize,
    /// Vantage points that can see the mob where it is now.
    pub visible_before: usize,
    /// ...and where it would be.
    pub visible_after: usize,
    /// Whether this row meets every burial threshold.
    pub buried: bool,
}

impl Candidate {
    /// How far under the floor the mob sits.
    pub fn gap(&self) -> i32 {
        self.floor_z - self.snapped_z
    }
}

pub struct Report {
    pub candidates: Vec<Candidate>,
    /// Rows inside the bbox that had geodata.
    pub rows_judged: usize,
    /// Rows on cells the fill never reached — no seed leads there, so they get
    /// no verdict either way. A non-zero count is a coverage gap, not a pass.
    pub uncovered: usize,
    pub walkable_cells: usize,
}

impl Report {
    pub fn buried(&self) -> impl Iterator<Item = &Candidate> {
        self.candidates.iter().filter(|c| c.buried)
    }
}

/// Layers at a cell, ascending, walking `getNextHigherZ` from the bottom.
fn layers(geo: &geo::GeoEngine, gx: i32, gy: i32) -> Vec<i32> {
    let mut out = Vec::new();
    let mut z = -32000;
    for _ in 0..64 {
        let next = geo.get_next_higher_z(gx, gy, z);
        if out.last() == Some(&next) || (next == z && !out.is_empty()) {
            break;
        }
        out.push(next);
        z = next + 1;
    }
    out
}

/// Flood fill the walkable surface from `seeds` with the engine's own step
/// rule (`checkNearestNswe` + `getNearestZ(neighbour, prevZ)`), clipped to the
/// bbox.
fn reachable(
    geo: &geo::GeoEngine,
    seeds: &[(i32, i32, i32)],
    bbox: Bbox,
) -> HashSet<(i32, i32, i32)> {
    let (gminx, gminy) = (geo.get_geo_x(bbox.0), geo.get_geo_y(bbox.1));
    let (gmaxx, gmaxy) = (geo.get_geo_x(bbox.2), geo.get_geo_y(bbox.3));
    let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
    let mut queue = VecDeque::new();
    for &(sx, sy, sz) in seeds {
        let (gx, gy) = (geo.get_geo_x(sx), geo.get_geo_y(sy));
        if !geo.has_geo_pos(gx, gy) || gx < gminx || gx > gmaxx || gy < gminy || gy > gmaxy {
            continue;
        }
        let z = geo.get_nearest_z(gx, gy, sz);
        if seen.insert((gx, gy, z)) {
            queue.push_back((gx, gy, z));
        }
    }
    while let Some((cx, cy, cz)) = queue.pop_front() {
        if seen.len() >= FILL_CAP {
            break;
        }
        for (dx, dy, bit) in [
            (0, -1, geo::NSWE_NORTH),
            (0, 1, geo::NSWE_SOUTH),
            (1, 0, geo::NSWE_EAST),
            (-1, 0, geo::NSWE_WEST),
        ] {
            if !geo.check_nearest_nswe(cx, cy, cz, bit) {
                continue;
            }
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < gminx || nx > gmaxx || ny < gminy || ny > gmaxy || !geo.has_geo_pos(nx, ny) {
                continue;
            }
            let nz = geo.get_nearest_z(nx, ny, cz);
            if seen.insert((nx, ny, nz)) {
                queue.push_back((nx, ny, nz));
            }
        }
    }
    seen
}

/// Vantage cells inside `walkable`, on `floor_z`'s own storey, spread over the
/// distance band around `(x, y)`.
fn vantages(
    geo: &geo::GeoEngine,
    walkable: &HashSet<(i32, i32, i32)>,
    floor_z: i32,
    x: i32,
    y: i32,
) -> Vec<(i32, i32, i32)> {
    let mut candidates: Vec<(i32, i32, i32, i32)> = walkable
        .iter()
        .filter(|&&(_, _, gz)| (gz - floor_z).abs() <= SAME_STOREY)
        .filter_map(|&(gx, gy, gz)| {
            let (wx, wy) = (geo.get_world_x(gx), geo.get_world_y(gy));
            let d = ((((wx - x) as i64).pow(2) + ((wy - y) as i64).pow(2)) as f64).sqrt() as i32;
            (VANTAGE_MIN..=VANTAGE_MAX)
                .contains(&d)
                .then_some((d, wx, wy, gz))
        })
        .collect();
    // Total order, not just by distance: the source is a HashSet, so anything
    // less makes the sampled vantage points — and the verdicts — run-dependent.
    candidates.sort_unstable();
    let step = (candidates.len() / VANTAGE_COUNT).max(1);
    candidates
        .iter()
        .step_by(step)
        .take(VANTAGE_COUNT)
        .map(|&(_, wx, wy, gz)| (wx, wy, gz))
        .collect()
}

fn visible_from(geo: &geo::GeoEngine, points: &[(i32, i32, i32)], x: i32, y: i32, z: i32) -> usize {
    points
        .iter()
        .filter(|&&(vx, vy, vz)| geo.can_see_target(vx, vy, vz, x, y, z))
        .count()
}

/// How many vantage points can walk to `(x, y, z)`. `get_valid_location`
/// returns the origin when the walk ends on a layer other than the one asked
/// for, so "moved, and landed at the requested z" is the test.
fn walkable_to(geo: &geo::GeoEngine, points: &[(i32, i32, i32)], x: i32, y: i32, z: i32) -> usize {
    points
        .iter()
        .filter(|&&(vx, vy, vz)| {
            let arrived = geo.get_valid_location(vx, vy, vz, x, y, z);
            arrived != (vx, vy, vz) && arrived.2 == z
        })
        .count()
}

/// Sweep one area. `geo` must already have the region(s) covering
/// `cfg.bbox` loaded.
pub fn sweep(geo: &geo::GeoEngine, cfg: &Config) -> Report {
    let (min_x, min_y, max_x, max_y) = cfg.bbox;
    let in_box = |x: i32, y: i32| x >= min_x && x <= max_x && y >= min_y && y <= max_y;

    // Most dungeons are entered on foot and have no teleport destination
    // inside them. Seed those from spawn rows on single-layer cells: one
    // surface at a cell means no ambiguity about what the ground is there.
    let mut seeds = cfg.seeds.to_vec();
    seeds.extend(
        cfg.rows
            .iter()
            .filter(|r| in_box(r.x, r.y))
            .filter(|r| {
                let (gx, gy) = (geo.get_geo_x(r.x), geo.get_geo_y(r.y));
                geo.has_geo_pos(gx, gy) && layers(geo, gx, gy).len() == 1
            })
            .map(|r| (r.x, r.y, r.z)),
    );

    let walkable = reachable(geo, &seeds, cfg.bbox);
    let mut report = Report {
        candidates: Vec::new(),
        rows_judged: 0,
        uncovered: 0,
        walkable_cells: walkable.len(),
    };

    for row in cfg.rows.iter().filter(|r| in_box(r.x, r.y)) {
        let (gx, gy) = (geo.get_geo_x(row.x), geo.get_geo_y(row.y));
        if !geo.has_geo_pos(gx, gy) {
            continue;
        }
        report.rows_judged += 1;
        let snapped_z = geo.get_nearest_z(gx, gy, row.z);

        // Which layers here are reachable at all? The fill leaks between
        // storeys wherever `getNearestZ` jumps, so this only narrows the
        // candidate floors — the walk test below is what decides.
        let reach: Vec<i32> = layers(geo, gx, gy)
            .into_iter()
            .filter(|&l| walkable.contains(&(gx, gy, l)))
            .collect();
        if reach.is_empty() {
            report.uncovered += 1;
            continue;
        }
        // The floor this mob belongs to: the lowest reachable layer above it.
        let Some(&floor_z) = reach.iter().filter(|&&l| l > snapped_z).min() else {
            continue;
        };
        if floor_z - snapped_z > MAX_SLAB_GAP {
            continue;
        }

        let points = vantages(geo, &walkable, floor_z, row.x, row.y);
        let walk_to_snapped = walkable_to(geo, &points, row.x, row.y, snapped_z);
        let walk_to_floor = walkable_to(geo, &points, row.x, row.y, floor_z);
        let visible_before = visible_from(geo, &points, row.x, row.y, snapped_z);
        let visible_after = visible_from(geo, &points, row.x, row.y, floor_z);

        let mut suggested_z = floor_z - 4;
        if geo.get_nearest_z(gx, gy, suggested_z) != floor_z {
            suggested_z = floor_z;
        }

        let buried = walk_to_snapped <= MAX_WALKABLE_HITS
            && walk_to_floor >= MIN_FLOOR_WALKS
            && visible_before <= MAX_VISIBLE_BEFORE
            && visible_after >= MIN_VISIBLE_AFTER;

        report.candidates.push(Candidate {
            row: row.clone(),
            snapped_z,
            floor_z,
            suggested_z,
            vantage_points: points.len(),
            walk_to_snapped,
            walk_to_floor,
            visible_before,
            visible_after,
            buried,
        });
    }
    report
}
