//! `l2r-tools gen-messages` — regenerate `commons`' system-message table.
//!
//! Reads the Java reference and the unpacked client table, writes
//! `generated.rs`. The output is committed, so a run is only ever followed by
//! `cargo fmt` and a reviewed diff. See [`tools::msg_gen`] for what is merged
//! and how custom messages are declared.

use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// The Java reference's `SystemMessageId.java`.
    #[arg(
        long,
        default_value = "../interlude_classic/java/org/l2jmobius/gameserver/network/SystemMessageId.java"
    )]
    java: PathBuf,

    /// The client table in its unpacked text form (`l2r-tools client-dat`
    /// writes it).
    #[arg(
        long,
        default_value = "dist/client/system_decrypted/SystemMsg_Classic-eu.dat"
    )]
    dat: PathBuf,

    /// Where the generated module goes.
    #[arg(
        long,
        default_value = "crates/commons/src/system_messages/generated.rs"
    )]
    out: PathBuf,
}

pub fn run(args: &Args) {
    let read = |p: &PathBuf| {
        std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("cannot read {}: {e}", p.display());
            std::process::exit(2);
        })
    };
    let (text, report) = tools::msg_gen::generate(&read(&args.java), &read(&args.dat))
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        });
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&args.out, &text).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", args.out.display());
        std::process::exit(2);
    });
    println!(
        "{} messages -> {} ({} typed, {} constants, {} custom); run `cargo fmt` before committing",
        report.total,
        args.out.display(),
        report.typed,
        report.constants,
        report.custom,
    );
}
