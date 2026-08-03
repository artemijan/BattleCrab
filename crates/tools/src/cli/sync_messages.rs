//! `l2r-tools sync-messages` — push `commons::system_messages` into the client.
//!
//! The server sends a message id; the client supplies the wording and colour
//! from its own table. So a message this server invents shows as nothing, and
//! a reworded one keeps the old text, until this has run. See
//! [`tools::msg_sync`] for what it writes and how the appended rows are filled.

use std::path::PathBuf;
use tools::dat_schema::SchemaSet;

#[derive(clap::Args)]
pub struct Args {
    /// The client's `system` directory. Defaults to `<client-dir>/system`.
    #[arg(long)]
    system_dir: Option<PathBuf>,

    /// Where the client lives, used to build the default above.
    #[arg(long, default_value = "dist/client")]
    client_dir: PathBuf,

    /// The vendored schema set.
    #[arg(long, default_value = "dist/client/structure")]
    structure_dir: PathBuf,

    /// Tables to write. Repeatable.
    #[arg(long = "message-file", default_values_t = [
        "SystemMsg_Classic-eu.dat".to_string(),
        "SystemMsg-eu.dat".to_string(),
    ])]
    message_files: Vec<String>,

    /// Report what would change without writing anything.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(args: &Args) {
    let system = args
        .system_dir
        .clone()
        .unwrap_or_else(|| args.client_dir.join("system"));
    let mut set = SchemaSet::load(&args.structure_dir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    let mut failed = false;
    for name in &args.message_files {
        if !system.join(name).is_file() {
            println!("{name}: not in {} — skipped", system.display());
            continue;
        }
        match tools::msg_sync::sync(&mut set, &system, name, args.dry_run) {
            Ok(report) => {
                println!(
                    "{}: {} row(s) retext/recoloured, {} appended, {} rows total{}",
                    report.file,
                    report.updated,
                    report.appended.len(),
                    report.total_rows,
                    if args.dry_run { " (dry run)" } else { "" },
                );
                if !report.appended.is_empty() {
                    println!("  appended ids: {:?}", report.appended);
                }
                if report.missing_not_custom > 0 {
                    // Not an error: the Java reference is newer than this
                    // client. Adding rows it was never built to use is not
                    // something to do behind the operator's back.
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
