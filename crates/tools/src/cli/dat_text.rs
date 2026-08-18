//! `l2r-tools dat-text` — render decrypted `.dat` files as editable text.
//!
//! See [`dat_text`] for the walk and [`tools::dat_schema`] for where the
//! layouts come from; this module is only flags and output.

use std::path::PathBuf;
use tools::{dat_schema::SchemaSet, dat_text};

#[derive(clap::Args)]
pub struct Args {
    /// Which way to convert.
    mode: Direction,

    /// Source directory. Defaults to `<client-dir>/system_decrypted` for
    /// `unpack` — run `client-dat decrypt` first to produce it.
    in_dir: Option<PathBuf>,

    /// Destination directory. Defaults to `<client-dir>/system_text`.
    out_dir: Option<PathBuf>,

    /// Where the client lives, used to build the defaults above.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The vendored schema set.
    #[arg(long, default_value = "dist/client/structure")]
    structure_dir: PathBuf,

    /// Chronicle schema set to use. The default tries every layout per file and
    /// keeps whichever consumes the file exactly, which is what a client that
    /// mixes revisions needs.
    #[arg(long, default_value = "auto")]
    chronicle: String,

    /// Print enum-valued integers as their labels. Off by default: the labels
    /// do not round-trip back to bytes yet.
    #[arg(long)]
    enums: bool,

    /// List every file, not just the failures.
    #[arg(long)]
    verbose: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Direction {
    /// Decrypted `.dat` -> `.dat.txt`.
    Unpack,
    /// `.dat.txt` -> decrypted `.dat`.
    Pack,
}

pub fn run(args: &Args) {
    let (default_in, default_out) = match args.mode {
        Direction::Unpack => ("system_decrypted", "system_text"),
        Direction::Pack => ("system_text", "system_decrypted"),
    };
    let in_dir = args
        .in_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join(default_in));
    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join(default_out));

    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let chronicle = (args.chronicle != "auto").then(|| args.chronicle.clone());
    if let Some(c) = &chronicle
        && !set.chronicles.contains_key(c)
    {
        eprintln!(
            "unknown chronicle `{c}`; known: {}",
            set.chronicle_names().join(", ")
        );
        std::process::exit(2);
    }

    println!("{} -> {}", in_dir.display(), out_dir.display());
    if matches!(args.mode, Direction::Pack) {
        let results = tools::dat_pack::pack_dir(&mut set, &in_dir, &out_dir, chronicle.as_deref())
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(2);
            });
        let failed: Vec<_> = results.iter().filter(|(_, e)| e.is_some()).collect();
        println!(
            "{}/{} file(s) packed",
            results.len() - failed.len(),
            results.len()
        );
        if !failed.is_empty() {
            eprintln!("\n{} file(s) NOT written:", failed.len());
            for (name, err) in &failed {
                eprintln!("  {name}: {}", err.as_deref().unwrap_or(""));
            }
            std::process::exit(1);
        }
        return;
    }

    let report = dat_text::unpack_dir(
        &mut set,
        &dat_text::Config {
            in_dir: &in_dir,
            out_dir: &out_dir,
            chronicle: chronicle.as_deref(),
            use_enums: args.enums,
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    if args.verbose {
        for e in &report.entries {
            let layout = e.layout.as_deref().unwrap_or("-");
            println!("  {:<44} {layout:<28} {}", e.file, e.detail);
        }
    }
    println!(
        "{}/{} file(s) unpacked",
        report.ok_count(),
        report.entries.len()
    );

    let failures: Vec<_> = report.failures().collect();
    if !failures.is_empty() {
        // Not written rather than written wrong: a drifting walk would produce
        // text that repacks into a corrupt .dat.
        eprintln!(
            "\n{} file(s) had no layout that consumed them exactly, so were not written:",
            failures.len()
        );
        for e in &failures {
            eprintln!(
                "  {} [{}] {}",
                e.file,
                e.layout.as_deref().unwrap_or("-"),
                e.detail
            );
        }
    }
}
