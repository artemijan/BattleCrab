//! Multisell server packets: `MultiSellList` (the exchange window, one packet
//! per 40-entry page) and `ExMultiSellResult` (the post-exchange ack).
//!
//! The window is written from the **prepared rows** the caller built (Java's
//! `PreparedMultisellListHolder._entries` + `_itemInfos`): a normal window is
//! one row per list entry, an inventory-only (`exc_multisell`) window one row
//! per matching item the player holds, carrying that item's enchant level.
//!
//! Each row also carries Java's `itemEnchantment`: the paired item, or for a
//! `maintainEnchantment` list without one, a display-only stand-in the caller
//! picked. Its enchant, augment, elemental and soul crystal blocks are written
//! for the row and for every product (on a `maintainEnchantment` list) and
//! ingredient of the same item kind. The port tracks no attributes, so the
//! elemental block of an item is always "none".

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::ItemData;
use crate::data::multisell_data::{MultisellList, PAGE_SIZE};
use crate::model::components::commerce::PreparedRow;
use crate::model::inventory::ItemInstance;

/// The elemental block of an item with no attributes, as `MultiSellList`
/// writes it and the client echoes it: attack type `AttributeType.NONE` (-2),
/// then power and the six defences at 0.
pub const NO_ATTRIBUTES: [i16; 8] = [-2, 0, 0, 0, 0, 0, 0, 0];

/// Port of `serverpackets/MultiSellList`. `index` is the first **row** of this
/// page (0, 40, 80, …). Mirrors Java's page math: `size = min(PAGE_SIZE,
/// rows - index)`, `finished` when this page reaches the end.
///
/// `tax_rate` is the castle buy tax of the NPC the window was opened from (0 on
/// the community-board path); it inflates the adena ingredient of a list that
/// declares `applyTaxes`, so the price displayed is the price charged.
///
/// `infos` runs parallel to `rows`: each row's `itemEnchantment`, if any.
pub fn multi_sell_list(
    list: &MultisellList,
    rows: &[PreparedRow],
    infos: &[Option<ItemInstance>],
    index: usize,
    items: &ItemData,
    tax_rate: f64,
) -> Vec<u8> {
    let total = rows.len();
    let remaining = total.saturating_sub(index);
    let size = remaining.min(PAGE_SIZE);
    let finished = remaining <= PAGE_SIZE;

    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MULTI_SELL_LIST);
    w.write_u8(0); // Helios
    w.write_i32(list.list_id);
    w.write_u8(0); // GOD unknown
    w.write_i32(1 + (index / PAGE_SIZE) as i32); // page, from 1
    w.write_i32(finished as i32);
    w.write_i32(PAGE_SIZE as i32);
    w.write_i32(size as i32);
    w.write_u8(0); // Grand Crusade
    w.write_u8(list.is_chance_multisell as u8);
    w.write_i32(32); // Helios — always 32

    for i in 0..size {
        let row_index = index + i;
        let row = &rows[row_index];
        let Some(entry) = list.entries.get(row.entry_index) else {
            continue;
        };
        let info = infos.get(row_index).copied().flatten();
        let info_kind = info.and_then(|i| items.get(i.item_id)).map(|t| t.kind);
        // Java `template.getClass().equals(itemEnchantment.getItem().getClass())`.
        let same_kind = |id: i32| info_kind.is_some() && items.get(id).map(|t| t.kind) == info_kind;
        // The entry id addresses the *row* (Java indexes its prepared list).
        w.write_i32(row_index as i32 + 1); // entry id, from 1
        w.write_u8(entry.stackable as u8);
        // Echoed back by `MultiSellChoose`.
        w.write_i16(info.map_or(0, |i| i.enchant_level) as i16);
        write_item_info(&mut w, info.as_ref());
        w.write_i16(entry.products.len() as i16);
        w.write_i16(entry.ingredients.len() as i16);

        for product in &entry.products {
            match items.get(product.id) {
                Some(t) => {
                    w.write_i32(product.id); // displayId == id on this dist
                    w.write_i64(t.body_part);
                    w.write_i16(t.type2 as i16);
                }
                None => {
                    w.write_i32(product.id);
                    w.write_i64(0);
                    w.write_i16(65535u16 as i16);
                }
            }
            let display = info.filter(|_| list.maintain_enchantment && same_kind(product.id));
            w.write_i64(list.product_count(product));
            w.write_i16(display_enchant(product.enchant_level, display.as_ref()));
            // `(int) Math.ceil(chance)`; NaN/None → 0 (display-only product).
            w.write_i32(product.chance.map(|c| c.ceil() as i32).unwrap_or(0));
            write_item_info(&mut w, display.as_ref());
        }

        for ingredient in &entry.ingredients {
            match items.get(ingredient.id) {
                Some(t) => {
                    w.write_i32(ingredient.id);
                    w.write_i16(t.type2 as i16);
                }
                None => {
                    w.write_i32(ingredient.id);
                    w.write_i16(65535u16 as i16);
                }
            }
            let display = info.filter(|_| same_kind(ingredient.id));
            w.write_i64(list.ingredient_count_taxed(ingredient, tax_rate));
            w.write_i16(display_enchant(ingredient.enchant_level, display.as_ref()));
            write_item_info(&mut w, display.as_ref());
        }
    }
    w.into_bytes()
}

/// A product or ingredient's own enchant level, else the display item's.
fn display_enchant(own: i16, display: Option<&ItemInstance>) -> i16 {
    if own > 0 {
        own
    } else {
        display.map_or(0, |i| i.enchant_level as i16)
    }
}

/// `writeItemAugment`, `writeItemElemental` and `writeItemEnsoulOptions` for
/// one `ItemInfo`, or their null forms.
fn write_item_info(w: &mut PacketWriter, info: Option<&ItemInstance>) {
    let Some(item) = info else {
        w.write_i32(0);
        w.write_i32(0);
        for _ in 0..8 {
            w.write_i16(0);
        }
        w.write_u8(0);
        w.write_u8(0);
        return;
    };
    w.write_i32(item.augment_option1);
    w.write_i32(item.augment_option2);
    for value in NO_ATTRIBUTES {
        w.write_i16(value);
    }
    let regular: Vec<i32> = item
        .ensoul_options
        .into_iter()
        .filter(|&id| id != 0)
        .collect();
    w.write_u8(regular.len() as u8);
    for id in regular {
        w.write_i32(id);
    }
    let special = item.ensoul_special_option;
    w.write_u8(u8::from(special != 0));
    if special != 0 {
        w.write_i32(special);
    }
}

/// Port of `serverpackets/ExMultiSellResult`.
pub fn ex_multi_sell_result(success: bool, ty: i32, count: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_MULTISELL_RESULT);
    w.write_u8(success as u8);
    w.write_i32(ty);
    w.write_i32(count);
    w.into_bytes()
}
