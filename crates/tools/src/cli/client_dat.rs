//! `l2r-tools client-dat` — the client's editable data files, both ways.
//!
//! ```text
//! decrypt:  system/*.{ini,int,dat}  ->  system_decrypted/  (text)
//! encrypt:  system_decrypted/       ->  system/
//! ```
//!
//! One directory each side, original filenames, no binary halfway stage. See
//! [`tools::client_files`] for how each type is handled; this module is only
//! flags and output.

use std::path::PathBuf;
use tools::{client_files, dat_schema::SchemaSet};

#[derive(clap::Args)]
pub struct Args {
    /// Which way to convert.
    mode: Direction,

    /// The client's `system` directory. Defaults to `<client-dir>/system`.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Editable files. Defaults to `<client-dir>/system_decrypted`.
    #[arg(long)]
    decrypted_dir: Option<PathBuf>,

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

    /// `sync-messages`: the tables to write. Repeatable.
    #[arg(long = "message-file", default_values_t = [
        "SystemMsg_Classic-eu.dat".to_string(),
        "SystemMsg-eu.dat".to_string(),
    ])]
    message_files: Vec<String>,

    /// `sync-messages`: report what would change without writing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Direction {
    /// `system` -> editable text.
    Decrypt,
    /// Editable text -> `system`.
    Encrypt,
    /// Push `commons::system_messages` into the client's SystemMsg tables:
    /// overwrite text and colour, append this server's own messages.
    SyncMessages,
}

pub fn run(args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    let decrypted = args
        .decrypted_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system_decrypted"));

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

    if matches!(args.mode, Direction::SyncMessages) {
        sync_messages(args, &mut set, &system);
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
            (client_files::decrypt(&mut set, &cfg), "decrypted")
        }
        Direction::Encrypt => {
            println!("{} -> {}", decrypted.display(), system.display());
            (client_files::encrypt(&mut set, &cfg), "encrypted")
        }
        // Handled above; it never reaches the directory converter.
        Direction::SyncMessages => unreachable!(),
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

fn sync_messages(args: &Args, set: &mut SchemaSet, system: &std::path::Path) {
    let mut failed = false;
    for name in &args.message_files {
        if !system.join(name).is_file() {
            println!("{name}: not in {} — skipped", system.display());
            continue;
        }
        match tools::msg_sync::sync(set, system, name, args.dry_run) {
            Ok(report) => {
                println!(
                    "{}: {} row(s) retext/recoloured, {} appended, {} of {} rows{}",
                    report.file,
                    report.updated,
                    report.appended.len(),
                    report.total_rows,
                    report.total_rows,
                    if args.dry_run { " (dry run)" } else { "" },
                );
                if !report.appended.is_empty() {
                    println!("  appended ids: {:?}", report.appended);
                }
                if report.missing_not_custom > 0 {
                    // Not an error: the Java reference is newer than this
                    // client. Adding rows it was never built to use is not
                    // something to do silently.
                    println!(
                        "  {} table message(s) this client has no row for, left alone",
                        report.missing_not_custom
                    );
                }
            }
            Err(e) => {
                eprintln!("{name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
