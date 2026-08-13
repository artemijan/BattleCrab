//! The command-line skin over the [`tools`] library.
//!
//! One module per subcommand, each exposing the clap `Args` that describe its
//! flags and a `run` that turns a report into terminal output. Printing,
//! argument parsing and exit codes live here and nowhere else — the library
//! modules stay pure `fn(&Config) -> Report` so the same logic can back a GUI
//! later.
//!
//! This tree belongs to the binary, not the library: `main.rs` declares it, so
//! nothing that prints ever ends up behind `tools::`.

pub mod client_dat;
pub mod dat_text;
pub mod gen_messages;
pub mod msg_color;
pub mod progress;
pub mod spawn_pockets;
pub mod sync_messages;
pub mod sync_npc;

/// Print `message` and exit with status 2 — the CLI's "bad input" bail-out,
/// shared by the subcommands that validate their own arguments.
pub fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
