//! `l2r-tools client-dat` — unpack the game client's `system` directory into
//! plaintext and pack it back.
//!
//! See [`tools::client_dat`] for the ciphers and the manifest scheme; this
//! module is only flags and output.

use std::path::PathBuf;
use tools::client_dat;

#[derive(clap::Args)]
pub struct Args {
    /// Which way to convert. Named `Direction` so it does not read as
    /// `client_dat::Mode`, which is the library's own enum.
    mode: Direction,

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

#[derive(Clone, Copy, clap::ValueEnum)]
enum Direction {
    Decrypt,
    Encrypt,
}

pub fn run(args: &Args) {
    let (mode, default_in, default_out) = match args.mode {
        Direction::Decrypt => (client_dat::Mode::Decrypt, "system", "system_decrypted"),
        Direction::Encrypt => (client_dat::Mode::Encrypt, "system_decrypted", "system"),
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
        Direction::Decrypt => "decrypted",
        Direction::Encrypt => "encrypted",
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
