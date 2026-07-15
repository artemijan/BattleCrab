//! Outbound (server → client) packets. Ported 1:1 from
//! `gameserver/network/serverpackets`. Each builder returns the serialized body
//! (opcode + payload, unencrypted); the connection task encrypts and frames it.
//!
//! G1 covers only `KeyPacket`; the rest arrive with their milestones.
//!
//! The builders are split into per-domain submodules but flattened back into
//! this module via glob re-exports, so every call site keeps referring to them
//! as `server_packets::<name>` regardless of which file they live in.

pub mod opcodes;

mod char_info;
mod chat;
mod clan;
mod combat;
mod door;
mod enchant;
mod ground_item;
mod private_store;
mod player_trade;
mod warehouse;
mod effect;
mod friend;
mod lobby;
mod manor;
mod movement;
mod npc;
mod party;
mod quest;
mod shortcut;
mod skill;
mod status;
mod system_message;
mod target;
mod variation;

pub use char_info::*;
pub use chat::*;
pub use clan::*;
pub use combat::*;
pub use door::*;
pub use enchant::*;
pub use ground_item::*;
pub use private_store::*;
pub use player_trade::*;
pub use warehouse::*;
pub use effect::*;
pub use friend::*;
pub use lobby::*;
pub use manor::*;
pub use movement::*;
pub use npc::*;
pub use party::*;
pub use quest::*;
pub use shortcut::*;
pub use skill::*;
pub use status::*;
pub use system_message::*;
pub use target::*;
pub use variation::*;
