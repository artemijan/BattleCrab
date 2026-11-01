//! Player-to-player trade packets: the incoming request (`SendTradeRequest`),
//! the trade window (`TradeStart`), added-item echoes (`TradeOwnAdd`/
//! `TradeOtherAdd`), the confirm echoes (`TradePressOwnOk`/`TradePressOtherOk`),
//! and completion (`TradeDone`). Item blocks reuse the shared writer.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::template::ItemTemplate;
use crate::model::inventory::ItemInstance;
use crate::network::enter_world::write_item_entry;

/// `SendTradeRequest` (0x70): a trade ask shown to the target, naming the sender.
pub fn send_trade_request(sender_object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SEND_TRADE_REQUEST);
    w.write_i32(sender_object_id);
    w.into_bytes()
}

/// `TradeStart` mask bit: the partner is a GM, and their level is not shown.
pub const TRADE_MASK_GM: u8 = 0x10;

/// `TradeStart` (0x14): open the trade window against `partner`, listing this
/// player's tradeable items. Sent as `send_type` 1 (the partner, the relation
/// `mask` and, unless they are a GM, their level) and then 2 (the items).
pub fn trade_start(
    send_type: u8,
    partner_object_id: i32,
    mask: u8,
    partner_level: u8,
    items: &[(ItemInstance, &ItemTemplate)],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TRADE_START);
    w.write_u8(send_type);
    if send_type == 2 {
        w.write_i32(items.len() as i32);
        w.write_i32(items.len() as i32);
        for (item, template) in items {
            write_item_entry(&mut w, item, template, false);
        }
    } else {
        w.write_i32(partner_object_id);
        w.write_u8(mask);
        if mask & TRADE_MASK_GM == 0 {
            w.write_u8(partner_level);
        }
    }
    w.into_bytes()
}

/// `TradeOwnAdd` (0x1A) / `TradeOtherAdd` (0x1B): one item added to the trade,
/// echoed to the adder (`own`) or the partner, as `send_type` 1 and then 2.
pub fn trade_add(
    send_type: u8,
    own: bool,
    item: &ItemInstance,
    template: &ItemTemplate,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(if own {
        opcodes::TRADE_OWN_ADD
    } else {
        opcodes::TRADE_OTHER_ADD
    });
    w.write_u8(send_type);
    if send_type == 2 {
        w.write_i32(1);
    }
    w.write_i32(1); // items added
    write_item_entry(&mut w, item, template, false);
    w.into_bytes()
}

/// `TradeUpdate` (0x81): the adder's own inventory entry for an item just
/// offered, showing what is left in the bag. Phase 1 is the bare header.
/// `remaining` is the inventory count less the offered count; the change type
/// is 3 (modified) while a stackable has some left, otherwise 2 (removed).
/// Java's count overload of `writeItem` writes 1 for `customType2`.
pub fn trade_update(
    send_type: u8,
    item: &ItemInstance,
    template: &ItemTemplate,
    remaining: i64,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TRADE_UPDATE);
    w.write_u8(send_type);
    w.write_i32(1);
    if send_type == 2 {
        w.write_i32(1);
        w.write_i16(if remaining > 0 && template.is_stackable {
            3
        } else {
            2
        });
        let shown = ItemInstance {
            count: remaining,
            custom_type2: 1,
            ..*item
        };
        write_item_entry(&mut w, &shown, template, false);
    }
    w.into_bytes()
}

/// `TradePressOwnOk` (0x53) / `TradePressOtherOk` (0x82): a confirm press echo.
pub fn trade_press_ok(own: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(if own {
        opcodes::TRADE_PRESS_OWN_OK
    } else {
        opcodes::TRADE_PRESS_OTHER_OK
    });
    w.into_bytes()
}

/// `TradeDone` (0x1C): `1` = the trade completed, `0` = it was cancelled.
pub fn trade_done(success: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TRADE_DONE);
    w.write_i32(success as i32);
    w.into_bytes()
}
