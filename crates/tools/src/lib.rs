//! Offline tools for the datapack and the game client. Where a question is
//! about the datapack, the tool answers it by running the server's *own*
//! engine code over it rather than a reimplementation, so a verdict here is a
//! verdict in game.
//!
//! Each tool is a module exposing a plain `fn(&Config) -> Report` so the same
//! logic can back the `l2r-tools` CLI today and a GUI later — nothing here
//! prints, and nothing here reads the environment. The terminal output and
//! argument parsing live in the binary's `cli` module tree instead.

pub mod client_dat;
pub mod client_files;
pub mod dat_pack;
pub mod dat_roundtrip;
pub mod dat_schema;
pub mod dat_text;
pub mod datapack;
pub mod msg_sync;
pub mod npc_sync;
pub mod npc_xml;
pub mod spawn_pockets;
pub mod system_msg;
