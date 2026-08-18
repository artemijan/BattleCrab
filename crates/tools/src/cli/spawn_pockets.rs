//! `l2r-tools spawn-pockets` — report spawn rows buried under the floor by
//! geodata layer snapping.
//!
//! See [`spawn_pockets`] for how a burial is detected and why the
//! obvious detectors do not work; this module is only flags and output.

use gameserver::geo::GeoEngine;
use std::path::{Path, PathBuf};
use tools::datapack;
use tools::spawn_pockets::{self, Candidate};

#[derive(clap::Args)]
pub struct Args {
    /// Area to sweep, `minx,miny,maxx,maxy`. The fill is area-bounded.
    #[arg(long, value_parser = parse_bbox, conflicts_with_all = ["region", "all_regions"])]
    bbox: Option<datapack::Bbox>,

    /// Sweep one geodata region tile, `tx_ty` (e.g. `20_21` for Cruma Tower).
    #[arg(long, conflicts_with_all = ["bbox", "all_regions"])]
    region: Option<String>,

    /// Sweep every region tile that contains spawn rows, one at a time.
    #[arg(long, conflicts_with_all = ["bbox", "region"])]
    all_regions: bool,

    /// Spawn XMLs to judge. Defaults to `<game-dir>/data/spawns`; point it at
    /// another checkout to sweep a different datapack copy.
    #[arg(long)]
    spawns_dir: Option<PathBuf>,

    /// Extra known-good standing position to seed the fill from, `x,y,z`
    /// (repeatable) — e.g. where a player reported the problem from.
    #[arg(long = "seed", value_parser = parse_point)]
    seeds: Vec<(i32, i32, i32)>,

    /// Print the raw metrics of every candidate row, not just buried ones.
    /// This is what the burial thresholds were calibrated against.
    #[arg(long)]
    csv: bool,

    /// Restrict the metric dump to rows near `x,y[,radius]` (default radius
    /// 400) — for investigating one mob a player reported. Implies `--csv`.
    #[arg(long, value_parser = parse_near)]
    near: Option<(i32, i32, i64)>,
}

fn parse_bbox(s: &str) -> Result<datapack::Bbox, String> {
    let v = parse_ints(s, 4)?;
    Ok((v[0], v[1], v[2], v[3]))
}

fn parse_point(s: &str) -> Result<(i32, i32, i32), String> {
    let v = parse_ints(s, 3)?;
    Ok((v[0], v[1], v[2]))
}

fn split_ints(s: &str) -> Result<Vec<i32>, String> {
    s.split(',')
        .map(|p| p.trim().parse::<i32>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_ints(s: &str, want: usize) -> Result<Vec<i32>, String> {
    let v = split_ints(s)?;
    if v.len() != want {
        return Err(format!(
            "expected {want} comma-separated integers, got {}",
            v.len()
        ));
    }
    Ok(v)
}

fn parse_near(s: &str) -> Result<(i32, i32, i64), String> {
    let v = split_ints(s)?;
    match v.len() {
        2 => Ok((v[0], v[1], 400)),
        3 => Ok((v[0], v[1], v[2] as i64)),
        n => Err(format!("expected `x,y` or `x,y,radius`, got {n} values")),
    }
}

fn parse_region(s: &str) -> Result<(i32, i32), String> {
    let (tx, ty) = s
        .split_once('_')
        .ok_or_else(|| format!("expected a region tile like `20_21`, got `{s}`"))?;
    Ok((
        tx.parse().map_err(|_| format!("bad tile x in `{s}`"))?,
        ty.parse().map_err(|_| format!("bad tile y in `{s}`"))?,
    ))
}

pub fn run(game_dir: &Path, args: &Args) {
    let spawns_dir = args
        .spawns_dir
        .clone()
        .unwrap_or_else(|| game_dir.join("data/spawns"));
    let rows = datapack::spawn_rows(&spawns_dir);
    if rows.is_empty() {
        eprintln!("no spawn rows found under {}", spawns_dir.display());
        std::process::exit(2);
    }
    let mut seeds = datapack::teleport_locations(&game_dir.join("data/teleporters"));
    seeds.extend(args.seeds.iter().copied());

    let areas: Vec<(String, datapack::Bbox)> = if let Some(bbox) = args.bbox {
        vec![("bbox".to_string(), bbox)]
    } else if let Some(region) = &args.region {
        let (tx, ty) = parse_region(region).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2);
        });
        vec![(region.clone(), datapack::region_bbox(tx, ty))]
    } else if args.all_regions {
        datapack::regions_with_spawns(&rows)
            .into_iter()
            .map(|(tx, ty)| (format!("{tx}_{ty}"), datapack::region_bbox(tx, ty)))
            .collect()
    } else {
        eprintln!("pass one of --bbox, --region or --all-regions");
        std::process::exit(2);
    };

    println!(
        "loading geodata from {}",
        game_dir.join("data/geodata").display()
    );
    let geo = GeoEngine::load(&game_dir.join("data/geodata"));
    println!("{} spawn rows, {} seed locations", rows.len(), seeds.len());

    let (mut buried, mut judged, mut uncovered) = (0usize, 0usize, 0usize);
    for (name, bbox) in &areas {
        let report = spawn_pockets::sweep(
            &geo,
            &spawn_pockets::Config {
                rows: &rows,
                seeds: &seeds,
                bbox: *bbox,
            },
        );
        if areas.len() > 1 && report.rows_judged == 0 {
            continue;
        }
        for c in &report.candidates {
            let near = args.near.is_none_or(|(nx, ny, r)| {
                ((c.row.x - nx) as i64).pow(2) + ((c.row.y - ny) as i64).pow(2) <= r * r
            });
            if (args.csv || args.near.is_some()) && near {
                println!("{}", csv_line(name, c));
            }
            if c.buried {
                println!("{}", buried_line(name, c, areas.len() > 1));
            }
        }
        buried += report.buried().count();
        judged += report.rows_judged;
        uncovered += report.uncovered;
        if areas.len() > 1 {
            println!(
                "[{name}] {} judged, {} buried, {} uncovered ({} walkable cells)",
                report.rows_judged,
                report.buried().count(),
                report.uncovered,
                report.walkable_cells
            );
        }
    }
    println!(
        "{buried} buried rows out of {judged} judged; {uncovered} rows on cells the fill never \
         reached (no verdict either way)"
    );
}

fn buried_line(area: &str, c: &Candidate, prefix: bool) -> String {
    let head = if prefix {
        format!("[{area}] ")
    } else {
        String::new()
    };
    format!(
        "{head}BURIED {}:{} id={} at ({},{}) z=\"{}\" -> snapped {}, walkable floor {} | \
         suggest z=\"{}\" | visible {}/{} before, {}/{} after",
        c.row.file,
        c.row.line,
        c.row.id,
        c.row.x,
        c.row.y,
        c.row.z,
        c.snapped_z,
        c.floor_z,
        c.suggested_z,
        c.visible_before,
        c.vantage_points,
        c.visible_after,
        c.vantage_points,
    )
}

fn csv_line(area: &str, c: &Candidate) -> String {
    format!(
        "CSV\t{area}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}/{}",
        c.row.file,
        c.row.line,
        c.row.id,
        c.row.x,
        c.row.y,
        c.row.z,
        c.snapped_z,
        c.floor_z,
        c.gap(),
        c.vantage_points,
        c.walk_to_snapped,
        c.walk_to_floor,
        c.visible_before,
        c.visible_after,
    )
}
