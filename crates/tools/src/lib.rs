//! Offline tools that answer questions about the datapack by running the
//! server's own engine code over it.
//!
//! Each tool is a module exposing a plain `fn(&Config) -> Report` so the same
//! logic can back the `l2r-tools` CLI today and a GUI later — nothing here
//! prints, and nothing here reads the environment.

pub mod datapack;
pub mod spawn_pockets;
