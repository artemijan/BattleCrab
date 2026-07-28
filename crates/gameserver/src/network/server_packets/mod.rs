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
mod command_channel;
mod community_board;
mod door;
mod effect;
mod enchant;
mod fishing;
mod friend;
mod games;
mod ground_item;
mod henna;
mod lobby;
mod mail;
mod manor;
mod movement;
mod multisell;
mod npc;
mod olympiad;
mod party;
mod party_room;
mod player_trade;
mod private_store;
mod quest;
mod recipe;
mod residence;
mod shortcut;
mod siege;
mod skill;
mod status;
mod system_message;
mod target;
mod variation;
mod vehicle;
mod warehouse;

pub use char_info::*;
pub use chat::*;
pub use clan::*;
pub use combat::*;
pub use command_channel::*;
pub use community_board::*;
pub use door::*;
pub use effect::*;
pub use enchant::*;
pub use fishing::*;
pub use friend::*;
pub use games::*;
pub use ground_item::*;
pub use henna::*;
pub use lobby::*;
pub use mail::*;
pub use manor::*;
pub use movement::*;
pub use multisell::*;
pub use npc::*;
pub use olympiad::*;
pub use party::*;
pub use party_room::*;
pub use player_trade::*;
pub use private_store::*;
pub use quest::*;
pub use recipe::*;
pub use residence::*;
pub use shortcut::*;
pub use siege::*;
pub use skill::*;
pub use status::*;
pub use system_message::*;
pub use target::*;
pub use variation::*;
pub use vehicle::*;
pub use warehouse::*;
