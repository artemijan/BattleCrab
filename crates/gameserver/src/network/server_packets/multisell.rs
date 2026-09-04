//! Multisell server packets: `MultiSellList` (the exchange window, one packet
//! per 40-entry page) and `ExMultiSellResult` (the post-exchange ack).
//!
//! The window is written from the **prepared rows** the caller built (Java's
//! `PreparedMultisellListHolder._entries` + `_itemInfos`): a normal window is
//! one row per list entry, an inventory-only (`exc_multisell`) window one row
//! per matching item the player holds, carrying that item's enchant level.
//! Augment/elemental blocks are still the null form (0 ints / 0 shorts) — the
//! port doesn't track item attributes, and no dist multisell keys on them.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::item_data::ItemData;
use crate::data::multisell_data::{MultisellList, PAGE_SIZE};
use crate::model::components::commerce::PreparedRow;

/// Port of `serverpackets/MultiSellList`. `index` is the first **row** of this
/// page (0, 40, 80, …). Mirrors Java's page math: `size = min(PAGE_SIZE,
/// rows - index)`, `finished` when this page reaches the end.
///
/// `tax_rate` is the castle buy tax of the NPC the window was opened from (0 on
/// the community-board path); it inflates the adena ingredient of a list that
/// declares `applyTaxes`, so the price displayed is the price charged.
pub fn multi_sell_list(
    list: &MultisellList,
    rows: &[PreparedRow],
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
        // The entry id addresses the *row* (Java indexes its prepared list).
        w.write_i32(row_index as i32 + 1); // entry id, from 1
        w.write_u8(entry.stackable as u8);
        w.write_i16(row.enchant_level as i16); // the paired item's enchant
        write_null_augment(&mut w);
        write_null_elemental(&mut w);
        w.write_u8(0);
        w.write_u8(0);
        w.write_i16(entry.products.len() as i16);
        w.write_i16(entry.ingredients.len() as i16);

        for product in &entry.products {
            match items.get(product.id) {
                Some(t) => {
                    w.write_i32(product.id); // displayId == id on this dist
                    w.write_i64(t.body_part as i64);
                    w.write_i16(t.type2 as i16);
                }
                None => {
                    w.write_i32(product.id);
                    w.write_i64(0);
                    w.write_i16(65535u16 as i16);
                }
            }
            w.write_i64(list.product_count(product));
            w.write_i16(product.enchant_level);
            // `(int) Math.ceil(chance)`; NaN/None → 0 (display-only product).
            w.write_i32(product.chance.map(|c| c.ceil() as i32).unwrap_or(0));
            write_null_augment(&mut w);
            write_null_elemental(&mut w);
            w.write_u8(0);
            w.write_u8(0);
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
            w.write_i64(list.ingredient_count_taxed(ingredient, tax_rate));
            w.write_i16(ingredient.enchant_level);
            write_null_augment(&mut w);
            write_null_elemental(&mut w);
            w.write_u8(0);
            w.write_u8(0);
        }
    }
    w.into_bytes()
}

/// `AbstractItemPacket.writeItemAugment(null)`.
fn write_null_augment(w: &mut PacketWriter) {
    w.write_i32(0);
    w.write_i32(0);
}

/// `AbstractItemPacket.writeItemElemental(null)` — attack element type/power +
/// six defence shorts.
fn write_null_elemental(w: &mut PacketWriter) {
    for _ in 0..8 {
        w.write_i16(0);
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
