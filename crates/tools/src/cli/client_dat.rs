//! `l2r-tools client-dat` — the client's editable data files, both ways.
//!
//! ```text
//! decrypt:    system/*.{ini,int,dat}  ->  system_decrypted/  (text)
//! encrypt:    system_decrypted/       ->  system/
//! roundtrip:  system/  ->  scratch text  ->  system_encrypted/  -> compare
//! ```
//!
//! One directory each side, original filenames, no binary halfway stage. See
//! [`client_files`] for how each type is handled and
//! [`dat_roundtrip`] for what `roundtrip` proves; this module is only
//! flags and output.

use std::path::PathBuf;
use tools::{client_files, dat_roundtrip, dat_schema::SchemaSet};

use super::fail;
use super::progress::Bar;

#[derive(clap::Args)]
pub struct Args {
    /// Which way to convert.
    mode: Direction,

    /// The client's `system` directory. Defaults to `<client-dir>/system`.
    /// `roundtrip` only ever reads it.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Editable files. Defaults to `<client-dir>/system_decrypted`, except for
    /// `roundtrip`, which defaults to a scratch directory of its own.
    #[arg(long)]
    decrypted_dir: Option<PathBuf>,

    /// Where `roundtrip` rebuilds the client's `system`, to be compared against
    /// the real one. Defaults to `<client-dir>/system_encrypted`.
    #[arg(long)]
    encrypted_dir: Option<PathBuf>,

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

    /// List every file, not just the failures.
    #[arg(long)]
    verbose: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Direction {
    /// `system` -> editable text.
    Decrypt,
    /// Editable text -> `system`.
    Encrypt,
    /// Decrypt and re-encrypt untouched, then check the client got its own
    /// files back. Never writes to `system` or `system_decrypted`.
    Roundtrip,
}

pub fn run(args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    // A round trip decrypts into scratch space rather than the editable tree:
    // it would otherwise overwrite hand edits that are not in `system` yet, and
    // losing them to a *verification* command would be a poor trade.
    let decrypted = args.decrypted_dir.clone().unwrap_or_else(|| {
        args.client_dir.join(match args.mode {
            Direction::Roundtrip => "system_roundtrip",
            _ => "system_decrypted",
        })
    });

    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| fail(&e));
    let chronicle = (args.chronicle != "auto").then(|| args.chronicle.clone());
    if let Some(c) = &chronicle
        && !set.chronicles.contains_key(c)
    {
        fail(&format!(
            "unknown chronicle `{c}`; known: {}",
            set.chronicle_names().join(", ")
        ));
    }

    if args.mode == Direction::Roundtrip {
        let encrypted = args
            .encrypted_dir
            .clone()
            .unwrap_or_else(|| args.client_dir.join("system_encrypted"));
        roundtrip(args, &mut set, &system, &decrypted, &encrypted, chronicle);
        return;
    }

    let cfg = client_files::Config {
        system_dir: &system,
        decrypted_dir: &decrypted,
        chronicle: chronicle.as_deref(),
    };
    let (report, verb) = match args.mode {
        Direction::Decrypt => {
            println!("{} -> {}", system.display(), decrypted.display());
            let mut bar = Bar::new("decrypting");
            (client_files::decrypt(&mut set, &cfg, &mut bar), "decrypted")
        }
        Direction::Encrypt => {
            println!("{} -> {}", decrypted.display(), system.display());
            let mut bar = Bar::new("encrypting");
            (client_files::encrypt(&mut set, &cfg, &mut bar), "encrypted")
        }
        Direction::Roundtrip => unreachable!("handled above"),
    };
    let report = report.unwrap_or_else(|e| fail(&e));

    if args.verbose {
        for e in &report.entries {
            let status = e.error.as_deref().unwrap_or("ok");
            println!("  {:<44} {:<34} {status}", e.file, e.detail);
        }
    }
    println!(
        "{}/{} file(s) {verb}",
        report.ok_count(),
        report.entries.len()
    );
    if !report.skipped.is_empty() {
        println!(
            "{} file(s) skipped (no Lineage2Ver header, or not produced by decrypt)",
            report.skipped.len()
        );
    }

    let failures: Vec<_> = report.failures().collect();
    if !failures.is_empty() {
        eprintln!("\n{} failure(s):", failures.len());
        for e in &failures {
            eprintln!(
                "  {} [{}] {}",
                e.file,
                e.detail,
                e.error.as_deref().unwrap_or("")
            );
        }
        std::process::exit(1);
    }
}

fn roundtrip(
    args: &Args,
    set: &mut SchemaSet,
    system: &std::path::Path,
    decrypted: &std::path::Path,
    encrypted: &std::path::Path,
    chronicle: Option<String>,
) {
    println!(
        "{} -> {} -> {}",
        system.display(),
        decrypted.display(),
        encrypted.display()
    );
    let report = dat_roundtrip::verify(
        set,
        &dat_roundtrip::Config {
            system_dir: system,
            decrypted_dir: decrypted,
            encrypted_dir: encrypted,
            chronicle: chronicle.as_deref(),
        },
        &mut Bar::new("decrypting"),
        &mut Bar::new("encrypting"),
    )
    .unwrap_or_else(|e| fail(&e));

    if args.verbose {
        for f in &report.files {
            println!(
                "  {:<44} {:<11} {:>9} -> {:<9} {}",
                f.file,
                verdict_word(&f.verdict),
                f.original_len,
                f.rebuilt_len,
                &f.original[..16]
            );
        }
    }

    println!(
        "\n{} file(s) checked: {} byte-identical, {} equivalent, {} changed, {} missing",
        report.files.len(),
        report.identical(),
        report.equivalent(),
        report.count(|v| matches!(v, dat_roundtrip::Verdict::Broken(_))),
        report.count(|v| *v == dat_roundtrip::Verdict::Missing),
    );

    // "Equivalent" needs saying out loud, not burying: those files really do
    // have a different hash, and a reader who is only told the count will
    // assume the worst. Naming all of them is `--verbose`, though — on a
    // pristine client it is most of the tree.
    let framed = report.equivalent();
    if framed > 0 {
        println!(
            "\n{framed} file(s) re-encrypt to different bytes carrying identical data.\n\
             Their zlib stream is framed differently — NCsoft's deflate encoder is not\n\
             `flate2` at any level — but every decrypted byte matches, so the client\n\
             reads exactly what it read before. Re-run with --verbose to list them."
        );
    }

    if !report.conversion_errors.is_empty() {
        eprintln!(
            "\n{} file(s) failed to convert:",
            report.conversion_errors.len()
        );
        for (file, why) in &report.conversion_errors {
            eprintln!("  {file}: {why}");
        }
    }

    let broken: Vec<_> = report.failures().collect();
    if !broken.is_empty() {
        eprintln!("\n{} file(s) did NOT survive the round trip:", broken.len());
        for f in &broken {
            let detail = match &f.verdict {
                dat_roundtrip::Verdict::Broken(why) => why.clone(),
                dat_roundtrip::Verdict::Missing => "never written".to_string(),
                _ => String::new(),
            };
            eprintln!("  {:<44} {detail}", f.file);
            eprintln!("      was {}", f.original);
            if let Some(now) = &f.rebuilt {
                eprintln!("      now {now}");
            }
        }
    }

    if report.passed() {
        println!("\nPASS — every file came back with the contents it went in with.");
    } else {
        eprintln!("\nFAIL");
        std::process::exit(1);
    }
}

fn verdict_word(v: &dat_roundtrip::Verdict) -> &'static str {
    match v {
        dat_roundtrip::Verdict::Identical => "identical",
        dat_roundtrip::Verdict::Equivalent => "equivalent",
        dat_roundtrip::Verdict::Broken(_) => "CHANGED",
        dat_roundtrip::Verdict::Missing => "MISSING",
    }
}
