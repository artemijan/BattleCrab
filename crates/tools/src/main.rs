//! `l2r-tools` — offline tools for the datapack and the game client.
//!
//! This file is only the command table. Each subcommand's flags and output
//! live in its own module under [`cli`], and the work itself lives in the
//! `tools` library, so adding a tool means adding a module and one arm here.

mod cli;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    SpawnPockets(cli::spawn_pockets::Args),

    /// Client data, end to end: decrypt `system` to editable text, or pack and
    /// re-encrypt it back.
    ClientDat(cli::client_dat::Args),

    /// One stage of that pipeline on its own: decrypted `.dat` <-> text.
    DatText(cli::dat_text::Args),

    /// Edit system-message colours in a terminal UI.
    MsgColor(cli::msg_color::Args),

    /// Push this server's system-message table into the client's SystemMsg
    /// files: overwrite text and colour, append messages the client lacks.
    SyncMessages(cli::sync_messages::Args),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::SpawnPockets(args) => cli::spawn_pockets::run(&cli.game_dir, &args),
        Command::ClientDat(args) => cli::client_dat::run(&args),
        Command::DatText(args) => cli::dat_text::run(&args),
        Command::MsgColor(args) => cli::msg_color::run(&args),
        Command::SyncMessages(args) => cli::sync_messages::run(&args),
    }
}
