//! `l2r-tools icon-atlas` — pack every datapack-referenced item icon into one
//! sprite sheet for the web dashboard.
//!
//! See [`tools::icon_atlas`] for what goes on the sheet and why; this module
//! is only flags and the two output files.

use std::path::PathBuf;
use tools::{icon_atlas, utx};

#[derive(clap::Args)]
pub struct Args {
    /// Item definitions naming the icons.
    #[arg(long, default_value = "dist/game/data/stats/items")]
    items_dir: PathBuf,

    /// Where the client lives; packages are read from `<client-dir>/systextures`.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The sprite sheet. Written as lossless WebP for a `.webp` extension,
    /// PNG otherwise — both pixel-identical, WebP ~20% smaller.
    #[arg(long, default_value = "web/dashboard/assets/l2icons.webp")]
    out_png: PathBuf,

    /// The reference -> cell map the web reads next to the sheet.
    #[arg(long, default_value = "web/dashboard/assets/l2icons.json")]
    out_map: PathBuf,

    /// WebP near-lossless preprocessing level: 100 is pixel-exact, lower
    /// nudges pixel values imperceptibly for a smaller sheet (~15% at 40).
    /// Ignored for PNG output.
    #[arg(long, default_value_t = 40)]
    near_lossless: u8,

    /// List every unresolvable icon reference, not just the count.
    #[arg(long)]
    verbose: bool,
}

pub fn run(args: &Args) {
    let report = match icon_atlas::build(&icon_atlas::Config {
        items_dir: &args.items_dir,
        client_dir: &args.client_dir,
    }) {
        Ok(r) => r,
        Err(e) => super::fail(&e),
    };

    for out in [&args.out_png, &args.out_map] {
        if let Some(parent) = out.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            super::fail(&format!("{}: {e}", parent.display()));
        }
    }
    let sheet = if args.out_png.extension().is_some_and(|e| e == "webp") {
        match utx::to_webp(&report.atlas, args.near_lossless) {
            Ok(bytes) => bytes,
            Err(e) => super::fail(&e),
        }
    } else {
        utx::to_png(&report.atlas)
    };
    if let Err(e) = std::fs::write(&args.out_png, sheet) {
        super::fail(&format!("{}: {e}", args.out_png.display()));
    }
    if let Err(e) = std::fs::write(&args.out_map, report.map_json()) {
        super::fail(&format!("{}: {e}", args.out_map.display()));
    }

    println!(
        "{} icons on a {}x{} sheet -> {} + {}",
        report.cells.len(),
        report.atlas.width,
        report.atlas.height,
        args.out_png.display(),
        args.out_map.display(),
    );
    if !report.missing.is_empty() {
        if args.verbose {
            for (reference, reason) in &report.missing {
                eprintln!("missing {reference}: {reason}");
            }
        }
        eprintln!(
            "{} references have no texture in the client (--verbose lists them)",
            report.missing.len()
        );
    }
}
