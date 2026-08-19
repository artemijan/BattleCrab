//! Minimal readers for the bits of the datapack the tools need.
//!
//! Deliberately not the server's XML parsers: a tool wants the *source
//! location* of every row (file and line) so its report can be applied back to
//! the datapack by hand or by script, and the server's parsers throw that away.

use gameserver::geo;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// World units spanned by one geodata region file (`x_y.l2j`).
pub const REGION_SPAN: i32 = geo::region::REGION_CELLS_X * 16;

/// An axis-aligned world-coordinate box, `(min_x, min_y, max_x, max_y)`.
pub type Bbox = (i32, i32, i32, i32);

/// One `<npc …/>` spawn row, with where it came from.
#[derive(Debug, Clone)]
pub struct SpawnRow {
    /// Path relative to the scanned root, e.g. `Dion/DionMonsterSpawns.xml`.
    pub file: String,
    /// 1-based line number, so `file:line` is clickable.
    pub line: usize,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Value of an integer XML attribute on a single-line element.
fn attr(s: &str, name: &str) -> Option<i32> {
    let key = format!("{name}=\"");
    let start = s.find(&key)? + key.len();
    let rest = &s[start..];
    let end = rest.find('"')?;
    rest[..end].trim().parse().ok()
}

fn walk_xml(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort(); // stable report order regardless of directory iteration
    for path in paths {
        if path.is_dir() {
            walk_xml(&path, f);
        } else if path.extension().is_some_and(|e| e == "xml")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            f(&path, &text);
        }
    }
}

/// Every `<npc … x= y= z=>` row under `dir`. Rows without explicit
/// coordinates (territory spawns) have nothing to judge and are skipped.
pub fn spawn_rows(dir: &Path) -> Vec<SpawnRow> {
    let mut out = Vec::new();
    walk_xml(dir, &mut |path, text| {
        let file = path.strip_prefix(dir).unwrap_or(path).display().to_string();
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
            out.push(SpawnRow {
                file: file.clone(),
                line: i + 1,
                id,
                x,
                y,
                z,
            });
        }
    });
    out
}

/// Every `<location x= y= z=>` under `dir` — teleporter destinations, i.e.
/// coordinates players demonstrably arrive standing on.
pub fn teleport_locations(dir: &Path) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    walk_xml(dir, &mut |_, text| {
        for line in text.lines() {
            let t = line.trim_start();
            if !t.starts_with("<location ") {
                continue;
            }
            if let (Some(x), Some(y), Some(z)) = (attr(t, "x"), attr(t, "y"), attr(t, "z")) {
                out.push((x, y, z));
            }
        }
    });
    out
}

/// The geodata region tile a world position falls in.
pub fn region_of(x: i32, y: i32) -> (i32, i32) {
    (
        (x - geo::WORLD_MIN_X).div_euclid(REGION_SPAN),
        (y - geo::WORLD_MIN_Y).div_euclid(REGION_SPAN),
    )
}

/// The world bbox covered by a region tile.
pub fn region_bbox(tile_x: i32, tile_y: i32) -> Bbox {
    let x0 = tile_x * REGION_SPAN + geo::WORLD_MIN_X;
    let y0 = tile_y * REGION_SPAN + geo::WORLD_MIN_Y;
    (x0, y0, x0 + REGION_SPAN - 1, y0 + REGION_SPAN - 1)
}

/// Region tiles that actually contain spawn rows, in scan order. Sweeping
/// these instead of all 32x32 skips the ~2/3 of the grid with nothing in it.
pub fn regions_with_spawns(rows: &[SpawnRow]) -> Vec<(i32, i32)> {
    rows.iter()
        .map(|r| region_of(r.x, r.y))
        .filter(|&(tx, ty)| {
            (geo::TILE_X_MIN..=geo::TILE_X_MAX).contains(&tx)
                && (geo::TILE_Y_MIN..=geo::TILE_Y_MAX).contains(&ty)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
