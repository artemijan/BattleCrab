//! Player-to-player trade packets: the incoming request (`SendTradeRequest`),
//! the trade window (`TradeStart`), added-item echoes (`TradeOwnAdd`/
//! `TradeOtherAdd`), the confirm echoes (`TradePressOwnOk`/`TradePressOtherOk`),
//! and completion (`TradeDone`). Item blocks reuse the shared writer.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::ItemTemplate;
use crate::model::inventory::ItemInstance;
use crate::network::enter_world::write_item_entry;

/// `SendTradeRequest` (0x70): a trade ask shown to the target, naming the sender.
pub fn send_trade_request(sender_object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SEND_TRADE_REQUEST);
    w.write_i32(sender_object_id);
    w.into_bytes()
}

/// `TradeStart` (0x14): open the trade window against `partner`, listing this
/// player's tradeable items.
pub fn trade_start(partner_object_id: i32, partner_level: u8, items: &[(ItemInstance, &ItemTemplate)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TRADE_START);
    w.write_i32(partner_object_id);
    w.write_u8(0); // mask (normal partner)
    w.write_u8(partner_level);
    w.write_i16(items.len() as i16);
    for (item, template) in items {
        write_item_entry(&mut w, item, template, false);
    }
    w.into_bytes()
}

/// `TradeOwnAdd` (0x1A) / `TradeOtherAdd` (0x1B): one item added to the trade,
/// echoed to the adder (`own`) or the partner.
pub fn trade_add(own: bool, item: &ItemInstance, template: &ItemTemplate) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(if own { opcodes::TRADE_OWN_ADD } else { opcodes::TRADE_OTHER_ADD });
    w.write_i16(1); // items added
    write_item_entry(&mut w, item, template, false);
    w.into_bytes()
}

/// `TradePressOwnOk` (0x53) / `TradePressOtherOk` (0x82): a confirm press echo.
pub fn trade_press_ok(own: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(if own { opcodes::TRADE_PRESS_OWN_OK } else { opcodes::TRADE_PRESS_OTHER_OK });
    w.into_bytes()
}

/// `TradeDone` (0x1C): `1` = the trade completed, `0` = it was cancelled.
pub fn trade_done(success: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TRADE_DONE);
    w.write_i32(success as i32);
    w.into_bytes()
}
