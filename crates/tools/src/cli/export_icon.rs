//! `l2r-tools export-icon` — pull item icons out of the client as PNGs.
//!
//! See [`utx`] for the package format; this module is only flags,
//! grouping by package so each `.utx` is opened once, and file writing.
//! Output names are lowercased — Unreal treats names case-insensitively, so
//! nothing collides, and the dashboard gets stable paths to link against.

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tools::utx;

#[derive(clap::Args)]
pub struct Args {
    /// Specific icons as the grp files spell them, e.g.
    /// `icon.weapon_arcana_mace_i00`. With none given, every texture in
    /// `--package` is exported.
    icons: Vec<String>,

    /// Package to export wholesale when no icons are named.
    #[arg(long, default_value = "icon")]
    package: String,

    /// Where the client lives; packages are read from `<client-dir>/systextures`.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// Directory the PNGs are written to, named `<texture>.png`.
    #[arg(long, default_value = "web/dashboard/assets/l2icons")]
    out_dir: PathBuf,
}

pub fn run(args: &Args) {
    // package file -> textures wanted from it
    let mut by_package: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    if args.icons.is_empty() {
        match utx::find_package(&args.client_dir, &args.package) {
            Ok(path) => {
                by_package.insert(path, Vec::new());
            }
            Err(e) => super::fail(&e),
        }
    } else {
        for reference in &args.icons {
            match utx::resolve(&args.client_dir, reference) {
                Ok((path, texture)) => by_package.entry(path).or_default().push(texture),
                Err(e) => super::fail(&e),
            }
        }
    }

    if let Err(e) = std::fs::create_dir_all(&args.out_dir) {
        super::fail(&format!("{}: {e}", args.out_dir.display()));
    }

    let mut exported = 0usize;
    let mut failures = 0usize;
    for (path, textures) in &by_package {
        let package = match utx::Package::load(path) {
            Ok(p) => p,
            Err(e) => super::fail(&e),
        };
        let textures: Vec<String> = if textures.is_empty() {
            package.texture_names().map(str::to_owned).collect()
        } else {
            textures.clone()
        };
        let single = args.icons.len() == 1;
        let results: Vec<Result<(String, u32, u32), String>> = textures
            .par_iter()
            .map(|texture| {
                let image = package
                    .texture(texture)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let out = args.out_dir.join(format!("{}.png", texture.to_lowercase()));
                std::fs::write(&out, utx::to_png(&image))
                    .map_err(|e| format!("{}: {e}", out.display()))?;
                Ok((out.display().to_string(), image.width, image.height))
            })
            .collect();
        for result in results {
            match result {
                Ok((out, w, h)) => {
                    exported += 1;
                    if single {
                        println!("{out} ({w}x{h})");
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    failures += 1;
                }
            }
        }
    }

    if exported != 1 || failures > 0 {
        println!(
            "{exported} icon{} -> {}{}",
            if exported == 1 { "" } else { "s" },
            args.out_dir.display(),
            if failures > 0 {
                format!(", {failures} failed")
            } else {
                String::new()
            }
        );
    }
    if failures > 0 {
        std::process::exit(1);
    }
}
