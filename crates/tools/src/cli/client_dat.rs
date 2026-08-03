//! `l2r-tools client-dat` — the whole client data pipeline in one command.
//!
//! `decrypt` takes the client's `system` directory all the way to editable
//! text, and `encrypt` takes it back:
//!
//! ```text
//! decrypt:  system  --cipher-->  system_decrypted  --schema-->  system_text
//! encrypt:  system  <--cipher--  system_decrypted  <--schema--  system_text
//! ```
//!
//! Both stages run by default, because stopping after the cipher leaves a
//! binary record stream that merely *looks* like a broken decrypt — which is
//! exactly how the two-command version misled. `--bytes-only` stops there
//! deliberately, and `dat-text` still runs either stage on its own.
//!
//! See [`tools::client_dat`] for the ciphers and [`tools::dat_schema`] for the
//! layouts; this module is only flags and output.

use std::path::{Path, PathBuf};
use tools::{client_dat, dat_pack, dat_schema::SchemaSet, dat_text};

#[derive(clap::Args)]
pub struct Args {
    /// Which way to convert.
    mode: Direction,

    /// Encrypted client directory. Defaults to `<client-dir>/system`.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Decrypted bytes. Defaults to `<client-dir>/system_decrypted`.
    #[arg(long)]
    bytes_dir: Option<PathBuf>,

    /// Editable text. Defaults to `<client-dir>/system_text`.
    #[arg(long)]
    text_dir: Option<PathBuf>,

    /// Where the client lives, used to build the defaults above.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The vendored schema set.
    #[arg(long, default_value = "dist/client/structure")]
    structure_dir: PathBuf,

    /// Chronicle schema set. The default tries every layout per file and keeps
    /// whichever consumes the file exactly.
    #[arg(long, default_value = "auto")]
    chronicle: String,

    /// Stop after the cipher: do not unpack to text, or pack from it. What you
    /// get is a binary record stream, not something to read.
    #[arg(long)]
    bytes_only: bool,

    /// Also mirror files that carry no `Lineage2Ver` header. Off by default:
    /// the client's ~200 MB of executables and libraries need no conversion
    /// and stay valid where they are.
    #[arg(long)]
    include_plain: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Direction {
    /// `system` -> decrypted bytes -> editable text.
    Decrypt,
    /// Editable text -> decrypted bytes -> `system`.
    Encrypt,
}

pub fn run(args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    let bytes = args
        .bytes_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system_decrypted"));
    let text = args
        .text_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system_text"));

    match args.mode {
        Direction::Decrypt => {
            cipher(args, client_dat::Mode::Decrypt, &system, &bytes);
            if !args.bytes_only {
                unpack(args, &bytes, &text);
            }
        }
        Direction::Encrypt => {
            // Pack first, so the bytes directory holds the edited records
            // before it is re-enciphered.
            if !args.bytes_only {
                pack(args, &text, &bytes);
            }
            cipher(args, client_dat::Mode::Encrypt, &bytes, &system);
        }
    }
}

fn cipher(args: &Args, mode: client_dat::Mode, in_dir: &Path, out_dir: &Path) {
    println!("cipher : {} -> {}", in_dir.display(), out_dir.display());
    let report = client_dat::run(&client_dat::Config {
        mode,
        in_dir,
        out_dir,
        include_plain: args.include_plain,
    })
    .unwrap_or_else(|e| fail(&e));

    let verb = match mode {
        client_dat::Mode::Decrypt => "decrypted",
        client_dat::Mode::Encrypt => "encrypted",
    };
    println!("         {} file(s) {verb}", report.converted());
    if !report.copied.is_empty() {
        println!(
            "         {} unencrypted file(s) copied",
            report.copied.len()
        );
    }
    if let Some(err) = &report.manifest_error {
        eprintln!("manifest: {err}");
    }
    if !report.unresolved.is_empty() {
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
        eprintln!("\n{} cipher failure(s):", failures.len());
        for entry in &failures {
            eprintln!(
                "  {} (Ver{}): {}",
                entry.rel,
                entry.version,
                entry.error.as_deref().unwrap_or("")
            );
        }
        std::process::exit(1);
    }
}

fn schema_set(args: &Args) -> (SchemaSet, Option<String>) {
    let set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| fail(&e));
    let chronicle = (args.chronicle != "auto").then(|| args.chronicle.clone());
    if let Some(c) = &chronicle
        && !set.chronicles.contains_key(c)
    {
        fail(&format!(
            "unknown chronicle `{c}`; known: {}",
            set.chronicle_names().join(", ")
        ));
    }
    (set, chronicle)
}

fn unpack(args: &Args, in_dir: &Path, out_dir: &Path) {
    let (mut set, chronicle) = schema_set(args);
    println!("unpack : {} -> {}", in_dir.display(), out_dir.display());
    let report = dat_text::unpack_dir(
        &mut set,
        &dat_text::Config {
            in_dir,
            out_dir,
            chronicle: chronicle.as_deref(),
            use_enums: false,
        },
    )
    .unwrap_or_else(|e| fail(&e));

    println!(
        "         {}/{} .dat file(s) unpacked",
        report.ok_count(),
        report.entries.len()
    );
    let failures: Vec<_> = report.failures().collect();
    if !failures.is_empty() {
        // Left alone rather than written wrong: text from a drifting walk
        // would repack into a corrupt .dat.
        eprintln!(
            "\n{} file(s) had no layout that consumed them exactly, so were not \
             written as text (their decrypted bytes are still correct):",
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

fn pack(args: &Args, in_dir: &Path, out_dir: &Path) {
    if !in_dir.is_dir() {
        fail(&format!(
            "no text directory at {} — run `client-dat decrypt` first",
            in_dir.display()
        ));
    }
    let (mut set, chronicle) = schema_set(args);
    println!("pack   : {} -> {}", in_dir.display(), out_dir.display());
    let results = dat_pack::pack_dir(&mut set, in_dir, out_dir, chronicle.as_deref())
        .unwrap_or_else(|e| fail(&e));

    let failed: Vec<_> = results.iter().filter(|(_, e)| e.is_some()).collect();
    println!(
        "         {}/{} .dat file(s) packed",
        results.len() - failed.len(),
        results.len()
    );
    if !failed.is_empty() {
        // Their existing bytes stay in place, so the client still gets a valid
        // file — just not one carrying whatever edit was attempted.
        eprintln!(
            "\n{} file(s) could not be packed and were left as they were:",
            failed.len()
        );
        for (name, err) in &failed {
            eprintln!("  {name}: {}", err.as_deref().unwrap_or(""));
        }
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
