//! Warehouse window packets: `WareHouseDepositList` (the player's inventory
//! items available to deposit) and `WareHouseWithdrawalList` (the warehouse
//! contents). Both reuse the shared item-block writer
//! ([`write_item_entry`](crate::network::enter_world::write_item_entry)). Only
//! the personal warehouse (`whType = PRIVATE = 1`) is wired.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::ItemTemplate;
use crate::model::inventory::ItemInstance;
use crate::network::enter_world::write_item_entry;

const WH_TYPE_PRIVATE: i16 = 1;

/// `serverpackets/WareHouseDepositList` — the deposit window: the inventory
/// items that can go into the warehouse, the player's adena, and the current
/// warehouse fill (`warehouse_size`).
pub fn warehouse_deposit_list(adena: i64, warehouse_size: i32, items: &[(&ItemInstance, &ItemTemplate)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::WAREHOUSE_DEPOSIT_LIST);
    w.write_i16(WH_TYPE_PRIVATE);
    w.write_i64(adena);
    w.write_i32(warehouse_size);
    let stackable: Vec<i32> = items.iter().filter(|(_, t)| t.is_stackable).map(|(it, _)| it.item_id).collect();
    w.write_i16(stackable.len() as i16);
    for id in stackable {
        w.write_i32(id);
    }
    w.write_i16(items.len() as i16);
    for (item, template) in items {
        write_item_entry(&mut w, item, template, false);
        w.write_i32(item.object_id);
    }
    w.into_bytes()
}

/// `serverpackets/WareHouseWithdrawalList` — the withdraw window: the warehouse
/// contents, the player's adena, and the current inventory fill (`inv_size`).
pub fn warehouse_withdrawal_list(adena: i64, inv_size: i32, items: &[(&ItemInstance, &ItemTemplate)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::WAREHOUSE_WITHDRAW_LIST);
    w.write_i16(WH_TYPE_PRIVATE);
    w.write_i64(adena);
    w.write_i16(items.len() as i16);
    let stackable: Vec<i32> = items.iter().filter(|(_, t)| t.is_stackable).map(|(it, _)| it.item_id).collect();
    w.write_i16(stackable.len() as i16);
    for id in stackable {
        w.write_i32(id);
    }
    w.write_i32(inv_size);
    for (item, template) in items {
        write_item_entry(&mut w, item, template, false);
        w.write_i32(item.object_id);
        w.write_i32(0);
        w.write_i32(0);
    }
    w.into_bytes()
}
