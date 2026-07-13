//! Merchant trade packets (G12's Buy slice): `BuyList` and `ExBuySellList`
//! — Java writes both under the same extended opcode (FE:0xB8), typed by the
//! leading int (0 = the buy tab, 1 = the sell tab).

use commons::network::PacketWriter;

use crate::data::buy_list_data::BuyList;
use crate::data::GameData;
use crate::model::inventory::Inventory;

const EX: u8 = 0xFE;
pub const EX_BUY_SELL_LIST: i16 = 0xB8;

/// Non-quest inventory size (`PlayerInventory.getNonQuestSize`), the
/// "inventory slots" both tabs report.
fn non_quest_size(inventory: &Inventory, data: &GameData) -> i32 {
    inventory
        .items()
        .iter()
        .filter(|i| data.item_data.get(i.item_id).is_none_or(|t| !t.is_quest_item))
        .count() as i32
}

/// Port of `serverpackets/BuyList` — the buy tab. Product entries reuse the
/// `AbstractItemPacket.writeItem` layout with `ItemInfo(Product)`'s fixed
/// fields (object id 0, count 0 = unlimited, nothing enchanted/equipped).
/// Castle tax is 0 (no castles); `baseTax` still applies.
pub fn buy_list(list: &BuyList, inventory: &Inventory, data: &GameData) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(EX_BUY_SELL_LIST);
    w.write_i32(0); // type BUY
    w.write_i64(inventory.adena());
    w.write_i32(list.list_id);
    w.write_i32(non_quest_size(inventory, data));
    let products: Vec<_> = list
        .products
        .iter()
        .filter_map(|p| data.item_data.get(p.item_id).map(|t| (p, t)))
        .collect();
    w.write_i16(products.len() as i16);
    for (p, t) in products {
        w.write_u8(0); // mask
        w.write_i32(0); // object id
        w.write_i32(p.item_id);
        w.write_u8(if t.is_quest_item { 0xFF } else { 0 }); // T1
        w.write_i64(0); // count (unlimited stock)
        w.write_u8(t.type2 as u8);
        w.write_u8(0); // custom type 1
        w.write_i16(0); // equipped
        w.write_i64(t.body_part as i64);
        w.write_u8(0); // enchant
        w.write_u8(0); // custom type 2
        w.write_i32(0); // mana
        w.write_i32(0); // time
        w.write_u8(1); // available
        w.write_i64((p.price as f64 * (1.0 + p.base_tax as f64 / 100.0)) as i64);
    }
    w.into_bytes()
}

/// Port of `serverpackets/ExBuySellList` — the sell tab that accompanies
/// every buy window (and the `done = true` refresh after a purchase).
/// Sellable = unequipped, non-quest, non-adena (the `is_sellable` template
/// flag and pet-control exclusions are not ported). No refund tab.
pub fn ex_buy_sell_list_sell(inventory: &Inventory, data: &GameData, done: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(EX_BUY_SELL_LIST);
    w.write_i32(1); // type SELL
    w.write_i32(non_quest_size(inventory, data));
    let sellable: Vec<_> = inventory
        .items()
        .iter()
        .filter(|i| inventory.paperdoll_slot_of(i.object_id).is_none())
        .filter_map(|i| data.item_data.get(i.item_id).map(|t| (i, t)))
        .filter(|(i, t)| !t.is_quest_item && i.item_id != crate::data::item_data::ADENA_ID)
        .collect();
    w.write_i16(sellable.len() as i16);
    for (item, t) in sellable {
        super::enter_world::write_item_entry(&mut w, item, t, false);
        w.write_i64(t.price / 2);
    }
    w.write_i16(0); // refund list (empty)
    w.write_u8(done as u8);
    w.into_bytes()
}
