//! Henna / dye symbol packets (G16). Ports of `serverpackets/HennaInfo`,
//! `HennaEquipList`, `HennaRemoveList`, `HennaItemDrawInfo`,
//! `HennaItemRemoveInfo`. The runtime flow lives in `game_loop/character/henna.rs`.
//!
//! LUC and CHA are written as zero: the player has neither stat until S5, and
//! no dye's LUC/CHA bonus is read yet.

use commons::network::PacketWriter;

use super::opcodes;

/// One dye line for the equip window: `(dye_id, dye_item_id, wear count, wear
/// fee, allowed, duration)`.
pub type HennaLine = (i32, i32, i64, i64, bool, i32);

/// One worn dye in the remove window: `(dye_id, dye_item_id, cancel count,
/// cancel fee)`.
pub type HennaRemoveLine = (i32, i32, i64, i64);

/// The player's six worn-henna stat sums, in the packet's INT/STR/CON/MEN/DEX/WIT
/// wire order.
#[derive(Debug, Clone, Copy, Default)]
pub struct HennaStatWire {
    pub int_: i16,
    pub str_: i16,
    pub con: i16,
    pub men: i16,
    pub dex: i16,
    pub wit: i16,
}

/// Port of `serverpackets/HennaInfo` — the worn-dye panel: per-stat totals, the
/// worn slot count, and the worn dye ids. `dyes` = `(dye_id, allowed_for_class)`.
pub fn henna_info(sums: HennaStatWire, worn_slots: i32, dyes: &[(i32, bool)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::HENNA_INFO);
    w.write_i16(sums.int_);
    w.write_i16(sums.str_);
    w.write_i16(sums.con);
    w.write_i16(sums.men);
    w.write_i16(sums.dex);
    w.write_i16(sums.wit);
    w.write_i16(0); // LUC
    w.write_i16(0); // CHA
    w.write_i32(worn_slots); // 3 - empty slots
    w.write_i32(dyes.len() as i32);
    for &(dye_id, allowed) in dyes {
        w.write_i32(dye_id);
        w.write_i32(allowed as i32);
    }
    w.write_i32(0); // Premium Slot Dye ID
    w.write_i32(0); // Premium Slot Dye Time Left
    w.write_i32(0); // Premium Slot Dye isValid
    w.into_bytes()
}
/// Port of `serverpackets/HennaEquipList` — the draw window: dyes the player's
/// class may wear and currently holds the item for. Java counts every dye the
/// class may wear but writes only the held ones; the count here is of the
/// lines written, so the client does not misread the list.
pub fn henna_equip_list(adena: i64, lines: &[HennaLine]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::HENNA_EQUIP_LIST);
    w.write_i64(adena);
    w.write_i32(3); // available equip slots
    w.write_i32(lines.len() as i32);
    for &(dye_id, item_id, count, fee, allowed, duration) in lines {
        w.write_i32(dye_id);
        w.write_i32(item_id);
        w.write_i64(count);
        w.write_i64(fee);
        w.write_i32(allowed as i32);
        w.write_i32(duration);
    }
    w.into_bytes()
}

/// Port of `serverpackets/HennaRemoveList` — the remove window: the worn dyes.
/// The premium fourth slot, which would raise the 3s to 4, is not ported.
pub fn henna_remove_list(adena: i64, worn_slots: i32, lines: &[HennaRemoveLine]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::HENNA_UNEQUIP_LIST);
    w.write_i64(adena);
    w.write_i32(3); // max size
    w.write_i32(worn_slots);
    for &(dye_id, item_id, count, fee) in lines {
        w.write_i32(dye_id);
        w.write_i32(item_id);
        w.write_i64(count);
        w.write_i64(fee);
        w.write_i32(0);
        w.write_i32(0);
    }
    w.into_bytes()
}

/// Six `(current: i32, preview: i16)` stat pairs, INT/STR/CON/MEN/DEX/WIT order,
/// for the item-info previews.
pub type StatPreview = [(i32, i16); 6];

/// Port of `serverpackets/HennaItemDrawInfo` — the "draw this dye" preview:
/// costs + the current vs. after-adding stat columns.
pub fn henna_item_draw_info(
    dye: &crate::data::henna_data::Henna,
    allowed: bool,
    adena: i64,
    stats: &StatPreview,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::HENNA_ITEM_INFO);
    w.write_i32(dye.dye_id);
    w.write_i32(dye.dye_item_id);
    w.write_i64(dye.wear_count);
    w.write_i64(dye.wear_fee);
    w.write_i32(allowed as i32);
    w.write_i64(adena);
    for &(current, preview) in stats {
        w.write_i32(current);
        w.write_i16(preview);
    }
    w.write_i32(0); // LUC current
    w.write_i16(0); // LUC equip
    w.write_i32(0); // CHA current
    w.write_i16(0); // CHA equip
    w.write_i32(dye.duration);
    w.into_bytes()
}

/// Port of `serverpackets/HennaItemRemoveInfo` — the "remove this dye" preview.
pub fn henna_item_remove_info(
    dye_id: i32,
    item_id: i32,
    count: i64,
    fee: i64,
    allowed: bool,
    adena: i64,
    stats: &StatPreview,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::HENNA_UNEQUIP_INFO);
    w.write_i32(dye_id);
    w.write_i32(item_id);
    w.write_i64(count);
    w.write_i64(fee);
    w.write_i32(allowed as i32);
    w.write_i64(adena);
    for &(current, preview) in stats {
        w.write_i32(current);
        w.write_i16(preview);
    }
    w.write_i32(0); // LUC current
    w.write_i16(0); // LUC equip
    w.write_i32(0); // CHA current
    w.write_i16(0); // CHA equip
    w.write_i32(0);
    w.into_bytes()
}
