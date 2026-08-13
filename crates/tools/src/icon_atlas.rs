//! One sprite sheet for every icon the item datapack references.
//!
//! The web dashboard shows item icons; serving ~7k tiny PNGs would mean ~7k
//! requests (or ~7k files embedded into the API binary), so they are packed
//! into a single PNG grid plus a JSON map from icon reference to cell index.
//! The set of icons is *driven by the datapack*, not by the client's texture
//! packages: `dist/game/data/stats/items` names exactly what the UI can ever
//! be asked to draw, and everything else in `Icon.utx` (skill icons, UI
//! furniture, 1x1 placeholders) stays out of the sheet.
//!
//! References are matched case-insensitively and recorded lowercased — the
//! datapack spells the same package `BranchSys3` and `Branchsys3` — and a
//! reference's *last* dot-segment is the texture name, its first the package:
//! `BranchIcon.Icon.foo_i00` means texture `foo_i00` in `BranchIcon.utx` (the
//! middle is an internal group Unreal's own name lookup ignores too).

use crate::utx::{self, Rgba};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct Config<'a> {
    /// `dist/game/data/stats/items` — the `*.xml` files naming the icons.
    pub items_dir: &'a Path,
    /// The client root holding `systextures/`.
    pub client_dir: &'a Path,
}

/// Side of one atlas cell. Item icons are authored at 32x32; the odd texture
/// that is not (a few cash-shop ones are 64x64) is box-resampled down to fit.
pub const CELL: u32 = 32;

pub struct Report {
    /// The packed sheet, ready for [`utx::to_png`].
    pub atlas: Rgba,
    /// Lowercased icon reference -> cell index, row-major over `columns`.
    pub cells: BTreeMap<String, u32>,
    pub columns: u32,
    /// References whose texture could not be produced, with the reason.
    pub missing: Vec<(String, String)>,
}

impl Report {
    /// The map the web loads next to the sheet: cell geometry plus every
    /// reference's index.
    pub fn map_json(&self) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"cell\": {CELL},\n"));
        out.push_str(&format!("  \"columns\": {},\n", self.columns));
        out.push_str("  \"icons\": {\n");
        let mut first = true;
        for (name, index) in &self.cells {
            if !first {
                out.push_str(",\n");
            }
            first = false;
            out.push_str(&format!("    \"{name}\": {index}"));
        }
        out.push_str("\n  }\n}\n");
        out
    }
}

/// Every distinct `<set name="icon" val="…"/>` under `items_dir`, lowercased.
fn referenced_icons(items_dir: &Path) -> Result<BTreeSet<String>, String> {
    let pattern = regex::Regex::new(r#"name="icon"\s+val="([^"]+)""#).unwrap();
    let mut icons = BTreeSet::new();
    let mut saw_file = false;
    for path in crate::common::read_dir(items_dir) {
        if path.extension().is_none_or(|e| e != "xml") {
            continue;
        }
        saw_file = true;
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for capture in pattern.captures_iter(&text) {
            icons.insert(capture[1].to_lowercase());
        }
    }
    if !saw_file {
        return Err(format!("{}: no item XML files", items_dir.display()));
    }
    Ok(icons)
}

/// Box-resample to the cell size. Exact for the 2:1 case that actually occurs;
/// for any other ratio each destination pixel averages the source rectangle it
/// covers, which is as right as a sprite this size needs.
fn fit_cell(image: &Rgba) -> Rgba {
    if image.width == CELL && image.height == CELL {
        return Rgba {
            width: CELL,
            height: CELL,
            pixels: image.pixels.clone(),
        };
    }
    let mut pixels = Vec::with_capacity((CELL * CELL * 4) as usize);
    for dy in 0..CELL {
        let y0 = (dy * image.height / CELL) as usize;
        let y1 = (((dy + 1) * image.height).div_ceil(CELL) as usize).max(y0 + 1);
        for dx in 0..CELL {
            let x0 = (dx * image.width / CELL) as usize;
            let x1 = (((dx + 1) * image.width).div_ceil(CELL) as usize).max(x0 + 1);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &image.pixels[(y * image.width as usize + x) * 4..][..4];
                    for (acc, &v) in sum.iter_mut().zip(p) {
                        *acc += v as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            pixels.extend(sum.iter().map(|&v| (v / n) as u8));
        }
    }
    Rgba {
        width: CELL,
        height: CELL,
        pixels,
    }
}

pub fn build(cfg: &Config) -> Result<Report, String> {
    let icons = referenced_icons(cfg.items_dir)?;
    if icons.is_empty() {
        return Err("the item XMLs reference no icons at all".into());
    }

    // Group by package so each .utx is opened and parsed once.
    let mut by_package: BTreeMap<String, Vec<&String>> = BTreeMap::new();
    for reference in &icons {
        let package = reference.split('.').next().unwrap_or_default().to_owned();
        by_package.entry(package).or_default().push(reference);
    }

    let mut decoded: BTreeMap<&String, Rgba> = BTreeMap::new();
    let mut missing = Vec::new();
    for (package_name, references) in &by_package {
        let package = utx::find_package(cfg.client_dir, package_name)
            .and_then(|path| utx::Package::load(&path));
        let package = match package {
            Ok(p) => p,
            Err(e) => {
                // The whole package is absent: every reference into it fails
                // for the same reason, recorded once each so the count is real.
                missing.extend(references.iter().map(|&r| (r.clone(), e.clone())));
                continue;
            }
        };
        for &reference in references {
            let texture = reference.rsplit('.').next().unwrap_or_default();
            match package.texture(texture) {
                Ok(image) => {
                    decoded.insert(reference, fit_cell(&image));
                }
                Err(e) => missing.push((reference.clone(), e)),
            }
        }
    }

    // Near-square sheet, deterministic order (BTreeMap iteration is sorted).
    let count = decoded.len() as u32;
    let columns = (count as f64).sqrt().ceil() as u32;
    let rows = count.div_ceil(columns);
    let (width, height) = (columns * CELL, rows * CELL);
    let mut atlas = Rgba {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
    };
    let mut cells = BTreeMap::new();
    for (index, (reference, image)) in decoded.iter().enumerate() {
        let index = index as u32;
        let (cx, cy) = (index % columns * CELL, index / columns * CELL);
        for row in 0..CELL {
            let src = (row * CELL * 4) as usize;
            let dst = (((cy + row) * width + cx) * 4) as usize;
            atlas.pixels[dst..dst + (CELL * 4) as usize]
                .copy_from_slice(&image.pixels[src..src + (CELL * 4) as usize]);
        }
        cells.insert((*reference).clone(), index);
    }

    Ok(Report {
        atlas,
        cells,
        columns,
        missing,
    })
}
