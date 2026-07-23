//! Private sell-store packets: the owner's manage window
//! (`PrivateStoreManageListSell`), a customer's view of the store
//! (`PrivateStoreListSell`), and the title shown above the owner
//! (`PrivateStoreMsgSell`). All reuse the shared item-block writer.

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
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_MANAGE_LIST);
    w.write_i32(owner_object_id);
    w.write_i32(0); // package sell
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
pub fn list_sell(seller_object_id: i32, buyer_adena: i64, items: &[StoreLine]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PRIVATE_STORE_LIST);
    w.write_i32(seller_object_id);
    w.write_i32(0); // packaged
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
