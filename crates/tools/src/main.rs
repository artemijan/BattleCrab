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

    /// Decrypt the game client's `system` files to plaintext, or pack them back.
    ClientDat(cli::client_dat::Args),

    /// Render decrypted `.dat` files as editable text using their schema.
    DatText(cli::dat_text::Args),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::SpawnPockets(args) => cli::spawn_pockets::run(&cli.game_dir, &args),
        Command::ClientDat(args) => cli::client_dat::run(&args),
        Command::DatText(args) => cli::dat_text::run(&args),
    }
}
