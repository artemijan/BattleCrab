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
mod gm_view;
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
pub use gm_view::*;
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

/// Java `ServerPacket.PAPERDOLL_ORDER` — the 33-slot equipment write order the
/// client expects, mapped from the `InventorySlot` wire order.
///
/// It is the base-class default, so it lives here rather than in either packet
/// that inherits it: `CharSelectionInfo` (`lobby`) and `GMViewCharacterInfo`
/// (`gm_view`) both write it because neither overrides `getPaperdollOrder()`
/// the way `CharInfo` does — those overrides stay private to their own files.
///
/// `RHand` appears twice (the slot the LRHAND display component reads), and
/// everything past `Brooch` is post-Interlude and always empty here.
pub const PAPERDOLL_ORDER: [crate::model::inventory::PaperdollSlot; 33] = {
    use crate::model::inventory::PaperdollSlot;
    [
        PaperdollSlot::Under,
        PaperdollSlot::REar,
        PaperdollSlot::LEar,
        PaperdollSlot::Neck,
        PaperdollSlot::RFinger,
        PaperdollSlot::LFinger,
        PaperdollSlot::Head,
        PaperdollSlot::RHand,
        PaperdollSlot::LHand,
        PaperdollSlot::Gloves,
        PaperdollSlot::Chest,
        PaperdollSlot::Legs,
        PaperdollSlot::Feet,
        PaperdollSlot::Cloak,
        PaperdollSlot::RHand,
        PaperdollSlot::Hair,
        PaperdollSlot::Hair2,
        PaperdollSlot::RBracelet,
        PaperdollSlot::LBracelet,
        PaperdollSlot::Deco1,
        PaperdollSlot::Deco2,
        PaperdollSlot::Deco3,
        PaperdollSlot::Deco4,
        PaperdollSlot::Deco5,
        PaperdollSlot::Deco6,
        PaperdollSlot::Belt,
        PaperdollSlot::Brooch,
        PaperdollSlot::BroochJewel1,
        PaperdollSlot::BroochJewel2,
        PaperdollSlot::BroochJewel3,
        PaperdollSlot::BroochJewel4,
        PaperdollSlot::BroochJewel5,
        PaperdollSlot::BroochJewel6,
    ]
};

/// An extended packet's header: the `0xFE` opcode plus its sub-opcode, ready
/// for the builder to append its own body. Every `Ex…` builder starts here, so
/// it lives beside the submodules rather than being redefined in each of them.
fn ex(sub: i16) -> commons::network::PacketWriter {
    let mut w = commons::network::PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(sub);
    w
}
