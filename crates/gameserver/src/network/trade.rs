//! Merchant trade packets (G12's Buy slice): `BuyList` and `ExBuySellList`
//! — Java writes both under the same extended opcode (FE:0xB8), typed by the
//! leading int (0 = the buy tab, 1 = the sell tab).

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::data::buy_list_data::BuyList;
use crate::model::inventory::{Inventory, ItemInstance};

const EX: u8 = 0xFE;
pub const EX_BUY_SELL_LIST: i16 = 0xB8;

/// Port of `serverpackets/BuyList` — the buy tab. Product entries reuse the
/// `AbstractItemPacket.writeItem` layout with `ItemInfo(Product)`'s fixed
/// fields (object id 0, count 0 = unlimited, nothing enchanted/equipped).
///
/// `castle_tax_rate` is the merchant's castle buy-tax fraction (Java
/// `Merchant.showBuyWindow` passes `getCastleTaxRate(BUY)`, or 0 when the caller
/// asks for an untaxed window — the mercenary manager's lists do). It is added
/// to the product's own `baseTax` before the price is written.
pub fn buy_list(
    list: &BuyList,
    inventory: &Inventory,
    data: &GameData,
    castle_tax_rate: f64,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(EX_BUY_SELL_LIST);
    w.write_i32(0); // type BUY
    w.write_i64(inventory.adena());
    w.write_i32(list.list_id);
    w.write_i32(inventory.non_quest_size(&data.item_data) as i32);
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
        w.write_i64(
            (p.price as f64 * (1.0 + castle_tax_rate + f64::from(p.base_tax) / 100.0)) as i64,
        );
    }
    w.into_bytes()
}

/// Port of `serverpackets/ExBuySellList` — the sell tab that accompanies
/// every buy window (and the `done = true` refresh after a purchase).
/// Sellable = unequipped + `is_sellable` + unaugmented (Java `Item.isSellable`
/// is `!isAugmented() && template.isSellable()`; the template flag alone
/// already excludes adena and quest items on this dist — the extra quest gate
/// stays as a belt-and-braces guard). Pet-control exclusion is TODO(G29).
/// `refund` is the buy-back tab: the player's `Refund` container, addressed
/// by list position in `RequestRefundItem`.
pub fn ex_buy_sell_list_sell(
    inventory: &Inventory,
    refund: &[ItemInstance],
    data: &GameData,
    done: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(EX_BUY_SELL_LIST);
    w.write_i32(1); // type SELL
    w.write_i32(inventory.non_quest_size(&data.item_data) as i32);
    let sellable: Vec<_> = inventory
        .items()
        .iter()
        .filter(|i| inventory.paperdoll_slot_of(i.object_id).is_none() && !i.is_augmented())
        .filter_map(|i| data.item_data.get(i.item_id).map(|t| (i, t)))
        .filter(|(_, t)| t.is_sellable && !t.is_quest_item)
        .collect();
    w.write_i16(sellable.len() as i16);
    for (item, t) in sellable {
        super::enter_world::write_item_entry(&mut w, item, t, false);
        w.write_i64(t.price / 2);
    }
    // The written index is the container position — `RequestRefundItem`
    // addresses the `Refund` vec with it, so it must survive the template
    // filter un-shifted.
    let refundable: Vec<_> = refund
        .iter()
        .enumerate()
        .filter_map(|(idx, i)| data.item_data.get(i.item_id).map(|t| (idx, i, t)))
        .collect();
    w.write_i16(refundable.len() as i16);
    for (idx, item, t) in refundable {
        super::enter_world::write_item_entry(&mut w, item, t, false);
        w.write_i32(idx as i32);
        w.write_i64((t.price / 2) * item.count);
    }
    w.write_u8(done as u8);
    w.into_bytes()
}
