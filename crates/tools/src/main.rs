//! `l2r-tools` — offline datapack/geodata tools.
//!
//! These run the *server's* engine code over the datapack rather than a
//! reimplementation of it, so a verdict here is a verdict in game.

use clap::{Args, Parser, Subcommand, ValueEnum};
use gameserver::geo::GeoEngine;
use std::path::{Path, PathBuf};
use tools::client_dat;
use tools::datapack;
use tools::spawn_pockets::{self, Candidate};

#[derive(Parser)]
#[command(name = "l2r-tools", about, version)]
struct Cli {
    /// Server data root (the directory holding `data/`).
    #[arg(long, default_value = "dist/game", global = true)]
    game_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Find spawn rows buried under the floor by geodata layer snapping.
    SpawnPockets(SpawnPocketsArgs),

    /// Decrypt the game client's `system` files to plaintext, or pack them back.
    ClientDat(ClientDatArgs),
}

#[derive(Args)]
struct ClientDatArgs {
    /// Which way to convert.
    mode: ClientDatMode,

    /// Source directory. Defaults to `<client-dir>/system` when decrypting and
    /// `<client-dir>/system_decrypted` when encrypting.
    in_dir: Option<PathBuf>,

    /// Destination directory. Defaults to the other one of that pair.
    out_dir: Option<PathBuf>,

    /// Where the client lives, used to build the defaults above.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// Also mirror files that carry no `Lineage2Ver` header. Off by default:
    /// the client's ~200 MB of executables and libraries need no conversion
    /// and stay valid where they are.
    #[arg(long)]
    include_plain: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ClientDatMode {
    Decrypt,
    Encrypt,
}

#[derive(Args)]
struct SpawnPocketsArgs {
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

fn parse_ints(s: &str, want: usize) -> Result<Vec<i32>, String> {
    let v: Vec<i32> = s
        .split(',')
        .map(|p| p.trim().parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    if v.len() != want {
        return Err(format!(
            "expected {want} comma-separated integers, got {}",
            v.len()
        ));
    }
    Ok(v)
}

fn parse_near(s: &str) -> Result<(i32, i32, i64), String> {
    let v: Vec<i32> = s
        .split(',')
        .map(|p| p.trim().parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
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

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::SpawnPockets(args) => spawn_pockets_cmd(&cli.game_dir, &args),
        Command::ClientDat(args) => client_dat_cmd(&args),
    }
}

fn client_dat_cmd(args: &ClientDatArgs) {
    let (mode, default_in, default_out) = match args.mode {
        ClientDatMode::Decrypt => (client_dat::Mode::Decrypt, "system", "system_decrypted"),
        ClientDatMode::Encrypt => (client_dat::Mode::Encrypt, "system_decrypted", "system"),
    };
    let in_dir = args
        .in_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join(default_in));
    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join(default_out));

    println!("{} -> {}", in_dir.display(), out_dir.display());
    let report = client_dat::run(&client_dat::Config {
        mode,
        in_dir: &in_dir,
        out_dir: &out_dir,
        include_plain: args.include_plain,
    })
    .unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    let verb = match args.mode {
        ClientDatMode::Decrypt => "decrypted",
        ClientDatMode::Encrypt => "encrypted",
    };
    println!("{} file(s) {verb}", report.converted());
    if !report.copied.is_empty() {
        println!(
            "{} unencrypted file(s) copied verbatim",
            report.copied.len()
        );
    }
    if !report.skipped.is_empty() {
        println!(
            "{} unencrypted file(s) left alone (pass --include-plain to mirror them)",
            report.skipped.len()
        );
    }
    if let Some(err) = &report.manifest_error {
        eprintln!("manifest: {err}");
    }
    if !report.unresolved.is_empty() {
        // Not fatal — a stray .DS_Store lands here too — but never silent: an
        // edited file that quietly missed the client is the worst outcome.
        eprintln!(
            "\n{} file(s) NOT packed, no crypt version known (absent from {} and \
             nothing to compare against in the destination):",
            report.unresolved.len(),
            client_dat::MANIFEST_NAME,
        );
        for rel in &report.unresolved {
            eprintln!("  {rel}");
        }
    }

    let failures: Vec<_> = report.failures().collect();
    if !failures.is_empty() {
        eprintln!("\n{} failure(s):", failures.len());
        for entry in &failures {
            let reason = entry.error.as_deref().unwrap_or("");
            eprintln!("  {} (Ver{}): {reason}", entry.rel, entry.version);
        }
        std::process::exit(1);
    }
}

fn spawn_pockets_cmd(game_dir: &Path, args: &SpawnPocketsArgs) {
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
