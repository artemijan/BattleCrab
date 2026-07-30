//! Private-store packets, both directions: the owner's manage window
//! (`PrivateStoreManageListSell`/`Buy`), a customer's view of the store
//! (`PrivateStoreListSell`/`Buy`), and the title shown above the owner
//! (`PrivateStoreMsgSell`/`Buy`). All reuse the shared item-block writer.
//!
//! The buy side reads the opposite way round: the owner posts *wanted* items
//! and adena, and the customer sells into it.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::ItemTemplate;
use crate::model::inventory::ItemInstance;
use crate::network::enter_world::write_item_entry;

/// An item line as shown in a store window: the instance + its unit price.
pub struct StoreLine<'a> {
    pub item: ItemInstance,
    pub template: &'a ItemTemplate,
    pub price: i64,
}

/// `PrivateStoreManageListSell` (0xA0): the owner's setup window — every
/// inventory item they *could* sell (with the reference-price×2 suggestion) then
/// the items already added to the store (with their set price).
pub fn manage_list_sell(
    owner_object_id: i32,
    owner_adena: i64,
    sellable: &[StoreLine],
    in_store: &[StoreLine],
    packaged: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_MANAGE_LIST);
    w.write_i32(owner_object_id);
    w.write_i32(i32::from(packaged));
    w.write_i64(owner_adena);
    w.write_i32(sellable.len() as i32);
    for line in sellable {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i64(line.template.price * 2); // reference price × 2 suggestion
    }
    w.write_i32(in_store.len() as i32);
    for line in in_store {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i64(line.price);
        w.write_i64(line.template.price * 2);
    }
    w.into_bytes()
}

/// `PrivateStoreListSell` (0xA1): a customer's view of `seller`'s store.
pub fn list_sell(
    seller_object_id: i32,
    buyer_adena: i64,
    items: &[StoreLine],
    packaged: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_LIST);
    w.write_i32(seller_object_id);
    w.write_i32(i32::from(packaged));
    w.write_i64(buyer_adena);
    w.write_i32(0);
    w.write_i32(items.len() as i32);
    for line in items {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i64(line.price);
        w.write_i64(line.template.price * 2);
    }
    w.into_bytes()
}

/// `PrivateStoreMsgSell` (0xA2): the store title shown above the owner.
pub fn msg_sell(owner_object_id: i32, title: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_MSG);
    w.write_i32(owner_object_id);
    w.write_string(title);
    w.into_bytes()
}

/// `PrivateStoreManageListBuy` (0xBD): the owner's buy-store setup window —
/// their whole inventory (as a price reference for what they might want) then
/// the lines already on the wanted list.
///
/// Note the shape difference from the sell window: no package-sell int, and
/// each wanted line carries **price, reference×2 and count** (the sell window's
/// count rides inside the item block, since a sell line is a real item).
pub fn manage_list_buy(
    owner_object_id: i32,
    owner_adena: i64,
    inventory: &[StoreLine],
    wanted: &[StoreLine],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_BUY_MANAGE_LIST);
    w.write_i32(owner_object_id);
    w.write_i64(owner_adena);
    w.write_i32(inventory.len() as i32);
    for line in inventory {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i64(line.template.price * 2);
    }
    w.write_i32(wanted.len() as i32);
    for line in wanted {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i64(line.price);
        w.write_i64(line.template.price * 2);
        w.write_i64(line.item.count);
    }
    w.into_bytes()
}

/// `PrivateStoreListBuy` (0xBE): a customer's view of what `owner` is buying —
/// Java sends only the lines the viewer can actually fill
/// (`getAvailableItems(viewer inventory)`), each with a 1-based slot number.
pub fn list_buy(owner_object_id: i32, viewer_adena: i64, items: &[StoreLine]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_BUY_LIST);
    w.write_i32(owner_object_id);
    w.write_i64(viewer_adena);
    w.write_i32(0); // "viewer's item count?" — Java writes a hard 0
    w.write_i32(items.len() as i32);
    for (idx, line) in items.iter().enumerate() {
        write_item_entry(&mut w, &line.item, line.template, false);
        w.write_i32(idx as i32 + 1); // slot in shop, 1-based
        w.write_i64(line.price);
        w.write_i64(line.template.price * 2);
        w.write_i64(line.item.count);
    }
    w.into_bytes()
}

/// `PrivateStoreMsgBuy` (0xBF): the buy-store title shown above the owner.
pub fn msg_buy(owner_object_id: i32, title: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_BUY_MSG);
    w.write_i32(owner_object_id);
    w.write_string(title);
    w.into_bytes()
}

/// `ExPrivateStoreSetWholeMsg` (0xFE:0x81) — the package store's title, the
/// package-sell counterpart of `PrivateStoreMsgSell`.
pub fn ex_private_store_whole_msg(owner_object_id: i32, title: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PRIVATE_STORE_WHOLE_MSG);
    w.write_i32(owner_object_id);
    w.write_string(title);
    w.into_bytes()
}
