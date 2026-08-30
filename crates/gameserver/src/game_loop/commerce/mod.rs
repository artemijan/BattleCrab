//! Goods and adena changing hands — every path an item takes between two
//! owners: merchant shops and multisells, player-to-player trade, the private
//! sell/buy/manufacture stores and their offline continuation, the warehouses,
//! the dwarven recipe book and the buff shop.
//!
//! Inventory mechanics themselves (add, drop, equip, enchant) belong to
//! [`crate::game_loop::items`]; this module is about the exchange around them.

pub(in crate::game_loop) mod crafting;
pub(crate) mod multisell;
pub(crate) mod offline_trade;
pub(in crate::game_loop) mod private_store;
pub(crate) mod sell_buffs;
pub(crate) mod shop;
pub(in crate::game_loop) mod trade;
pub(in crate::game_loop) mod warehouse;
