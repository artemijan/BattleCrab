//! Datapack sweep for spawn rows that land in a geodata "pocket" — a layer
//! underneath the floor players actually walk on, so the mob is invisible,
//! unaggroable and unhittable until its AI walks it out and
//! `getValidLocation` re-grounds it.
//!
//! Not a unit test: `#[ignore]`d, and it needs the real geodata. Run one area
//! at a time, the fill is bbox-bounded:
//!
//! ```text
//! SWEEP_BBOX=10000,105000,27000,122000 \
//!   cargo test --release --test spawn_pocket_sweep -- --ignored --nocapture
//! ```
//!
//! Other knobs: `SPAWNS_DIR` sweeps another datapack copy, `SWEEP_SEEDS`
//! ("x,y,z;…") adds player-reported standing positions as fill seeds,
//! `SWEEP_DEBUG` ("x,y[,radius]") dumps the metrics of every row near a point,
//! and `SWEEP_CSV=1` dumps them for every candidate row (used to calibrate the
//! thresholds below).
//!
//! # Why it is done this way
//!
//! `Spawn.initializeNpc` snaps a spawn z with `getHeight` = `getNearestZ`,
//! which picks the nearest layer by |dz| — *not* the surface at or below. Dungeon
//! cells carry spurious slabs 48-128 units under the floor, so a datapack z of
//! `floor - 50` lands under the floor wherever a slab exists.
//!
//! Two obvious detectors do not work:
//!
//! - **Component size.** A pocket is not always small: Cruma's slab spans the
//!   whole tower, flood filling to 800+ cells just like a real floor.
//! - **Reachability alone.** The engine's step rule has no vertical limit, so a
//!   fill leaks onto the slab wherever `getNearestZ` jumps, and the slab then
//!   looks "reachable" everywhere.
//!
//! What does work is asking the engine the question a player asks: seed a fill
//! from the coordinates teleporters actually drop players on, then for each
//! spawn row check whether a walker starting on the floor can *arrive* at the
//! mob's layer (`getValidLocation` bails to the origin when the walk lands on a
//! different layer than requested) and whether the floor can *see* it
//! (`canSeeTarget`).
//!
//! Thresholds are calibrated against the ten Cruma rows fixed and confirmed in
//! game by an earlier pass; the sweep recovers all ten.

use gameserver::geo::{GeoEngine, NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// A sub-floor slab sits tens of units under the floor; a real storey is
/// hundreds to thousands. Beyond this it is a different level, not a burial —
/// this is what keeps caves and lower floors out of the report.
const MAX_SLAB_GAP: i32 = 130;
/// Vantage points from which a walker may still reach the mob's own layer.
/// Not zero: `getValidLocation` walks a straight cell line, and from the odd
/// angle that line drags z down onto the slab (the in-game-verified Dicor
/// scores 1).
const MAX_WALKABLE_HITS: usize = 1;
/// The floor above must be walkable from most vantage points.
const MIN_FLOOR_WALKS: usize = 8;
/// A buried mob may still be visible from a stray angle (Dicor scored 2 and
/// was still unhittable in game).
const MAX_VISIBLE_BEFORE: usize = 2;
/// ...and must become widely visible once lifted onto the floor.
const MIN_VISIBLE_AFTER: usize = 8;
/// Vantage cells must be on the candidate floor's own storey.
const SAME_STOREY: i32 = 80;
/// Vantage points are sampled from the floor in this world-distance band.
const VANTAGE_MIN: i32 = 150;
const VANTAGE_MAX: i32 = 1100;
const VANTAGE_COUNT: usize = 24;
/// Fill ceiling. A full geo region is ~4M cells and several layers deep; this
/// is a runaway guard, not a working limit (a saturated fill silently loses
/// coverage, so the sweep reports how many rows it could not judge).
const FILL_CAP: usize = 24_000_000;

fn game_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/game")
}

#[derive(Debug, Clone)]
struct Row {
    file: String,
    line: usize,
    id: i32,
    x: i32,
    y: i32,
    z: i32,
}

fn attr(s: &str, name: &str) -> Option<i32> {
    let key = format!("{name}=\"");
    let start = s.find(&key)? + key.len();
    let rest = &s[start..];
    let end = rest.find('"')?;
    rest[..end].trim().parse().ok()
}

/// Every `<npc … x= y= z=>` row under `dir`, with its file and line so the
/// report can be applied back to the datapack.
fn collect_rows(dir: &Path, root: &Path, out: &mut Vec<Row>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rows(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "xml") {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let short = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            for (i, line) in text.lines().enumerate() {
                let t = line.trim_start();
                if !t.starts_with("<npc ") {
                    continue;
                }
                let (Some(id), Some(x), Some(y), Some(z)) =
                    (attr(t, "id"), attr(t, "x"), attr(t, "y"), attr(t, "z"))
                else {
                    continue;
                };
                out.push(Row {
                    file: short.clone(),
                    line: i + 1,
                    id,
                    x,
                    y,
                    z,
                });
            }
        }
    }
}

/// Layers at a cell, ascending, walking `getNextHigherZ` from the bottom.
fn layers(geo: &GeoEngine, gx: i32, gy: i32) -> Vec<i32> {
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

fn seed_locations(dir: &Path, out: &mut Vec<(i32, i32, i32)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            seed_locations(&path, out);
        } else if path.extension().is_some_and(|e| e == "xml") {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                let t = line.trim_start();
                if !t.starts_with("<location ") {
                    continue;
                }
                if let (Some(x), Some(y), Some(z)) = (attr(t, "x"), attr(t, "y"), attr(t, "z")) {
                    out.push((x, y, z));
                }
            }
        }
    }
}

/// Flood fill the walkable surface from `seeds` with the engine's own step
/// rule (`checkNearestNswe` + `getNearestZ(neighbour, prevZ)`), clipped to the
/// world bbox.
fn reachable(
    geo: &GeoEngine,
    seeds: &[(i32, i32, i32)],
    bbox: (i32, i32, i32, i32),
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
            (0, -1, NSWE_NORTH),
            (0, 1, NSWE_SOUTH),
            (1, 0, NSWE_EAST),
            (-1, 0, NSWE_WEST),
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
/// distance band around the mob.
fn vantages(
    geo: &GeoEngine,
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

fn visible_from(geo: &GeoEngine, points: &[(i32, i32, i32)], x: i32, y: i32, z: i32) -> usize {
    points
        .iter()
        .filter(|&&(vx, vy, vz)| geo.can_see_target(vx, vy, vz, x, y, z))
        .count()
}

/// How many vantage points can walk to `(x, y, z)`. `getValidLocation` returns
/// the origin when the walk ends on a layer other than the one asked for, so
/// "arrived, at the requested z" is the test.
fn walkable_to(geo: &GeoEngine, points: &[(i32, i32, i32)], x: i32, y: i32, z: i32) -> usize {
    points
        .iter()
        .filter(|&&(vx, vy, vz)| {
            let arrived = geo.get_valid_location(vx, vy, vz, x, y, z);
            arrived != (vx, vy, vz) && arrived.2 == z
        })
        .count()
}

#[test]
#[ignore = "datapack sweep, needs the real geodata; run with --ignored --nocapture"]
fn spawn_pocket_sweep() {
    let game = game_dir();
    let geo = GeoEngine::load(&game.join("data/geodata"));

    let bbox: Vec<i32> = std::env::var("SWEEP_BBOX")
        .expect("set SWEEP_BBOX=minx,miny,maxx,maxy")
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    let bbox = (bbox[0], bbox[1], bbox[2], bbox[3]);
    let in_box = |x: i32, y: i32| x >= bbox.0 && x <= bbox.2 && y >= bbox.1 && y <= bbox.3;

    let spawns = std::env::var("SPAWNS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| game.join("data/spawns"));
    let root = spawns.parent().and_then(|p| p.parent()).unwrap_or(&spawns);
    let mut rows = Vec::new();
    collect_rows(&spawns, root, &mut rows);

    // Where players demonstrably stand: teleport destinations...
    let mut seeds = Vec::new();
    seed_locations(&game.join("data/teleporters"), &mut seeds);
    if let Ok(extra) = std::env::var("SWEEP_SEEDS") {
        for part in extra.split(';').filter(|p| !p.trim().is_empty()) {
            let v: Vec<i32> = part
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            seeds.push((v[0], v[1], v[2]));
        }
    }
    // ...plus spawn rows on single-layer cells, since most dungeons are
    // entered on foot and have no teleport destination inside them. One
    // surface at a cell means no ambiguity about what the ground is there.
    let ground_seeds = rows
        .iter()
        .filter(|r| in_box(r.x, r.y))
        .filter(|r| {
            let (gx, gy) = (geo.get_geo_x(r.x), geo.get_geo_y(r.y));
            geo.has_geo_pos(gx, gy) && layers(&geo, gx, gy).len() == 1
        })
        .map(|r| (r.x, r.y, r.z))
        .collect::<Vec<_>>();
    println!(
        "{} single-layer spawn cells added as ground seeds",
        ground_seeds.len()
    );
    seeds.extend(ground_seeds);

    let walkable = reachable(&geo, &seeds, bbox);
    rows.retain(|r| in_box(r.x, r.y));
    println!(
        "walkable surface: {} cells; {} spawn rows inside the bbox",
        walkable.len(),
        rows.len()
    );

    let debug_at: Option<(i32, i32, i64)> = std::env::var("SWEEP_DEBUG").ok().map(|s| {
        let v: Vec<i64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (v[0] as i32, v[1] as i32, *v.get(2).unwrap_or(&400))
    });
    let csv = std::env::var("SWEEP_CSV").is_ok();

    let mut buried = 0usize;
    let mut uncovered = 0usize;
    for row in &rows {
        let (gx, gy) = (geo.get_geo_x(row.x), geo.get_geo_y(row.y));
        if !geo.has_geo_pos(gx, gy) {
            continue;
        }
        let snapped = geo.get_nearest_z(gx, gy, row.z);

        // Which layers here are reachable at all? The fill leaks between
        // storeys wherever `getNearestZ` jumps, so this only narrows the
        // candidate floors — the walk test below is what decides.
        let reach: Vec<i32> = layers(&geo, gx, gy)
            .into_iter()
            .filter(|&l| walkable.contains(&(gx, gy, l)))
            .collect();
        if reach.is_empty() {
            uncovered += 1; // no seed reached this cell: no verdict either way
            continue;
        }
        // The floor this mob belongs to: the lowest reachable layer above it.
        let Some(&floor_z) = reach.iter().filter(|&&l| l > snapped).min() else {
            continue;
        };
        if floor_z - snapped > MAX_SLAB_GAP {
            continue;
        }

        let points = vantages(&geo, &walkable, floor_z, row.x, row.y);
        let walk_snapped = walkable_to(&geo, &points, row.x, row.y, snapped);
        let walk_floor = walkable_to(&geo, &points, row.x, row.y, floor_z);
        let before = visible_from(&geo, &points, row.x, row.y, snapped);
        let after = visible_from(&geo, &points, row.x, row.y, floor_z);

        let dbg = debug_at.is_some_and(|(dx, dy, r)| {
            ((row.x - dx) as i64).pow(2) + ((row.y - dy) as i64).pow(2) <= r * r
        });
        if csv || dbg {
            println!(
                "CSV\t{}\t{}\t{}\t{}\t{}\t{}\t{snapped}\t{floor_z}\t{}\t{}\t{walk_snapped}\t\
                 {walk_floor}\t{before}/{after}",
                row.file,
                row.line,
                row.id,
                row.x,
                row.y,
                row.z,
                floor_z - snapped,
                points.len(),
            );
        }

        if walk_snapped > MAX_WALKABLE_HITS
            || walk_floor < MIN_FLOOR_WALKS
            || before > MAX_VISIBLE_BEFORE
            || after < MIN_VISIBLE_AFTER
        {
            continue;
        }

        // Recommend a datapack z that snaps to the floor layer.
        let mut suggest = floor_z - 4;
        if geo.get_nearest_z(gx, gy, suggest) != floor_z {
            suggest = floor_z;
        }
        buried += 1;
        println!(
            "BURIED {}:{} id={} at ({},{}) z=\"{}\" -> snapped {snapped}, walkable floor \
             {floor_z} | suggest z=\"{suggest}\" | visible {before}/{} before, {after}/{} after",
            row.file,
            row.line,
            row.id,
            row.x,
            row.y,
            row.z,
            points.len(),
            points.len(),
        );
    }
    println!("{buried} buried rows, {uncovered} rows on cells the fill never reached");
}
