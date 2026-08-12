//! Augmentation (variation) server packets: the make/cancel windows, the
//! confirm-refiner result, and the make/cancel results.

use commons::network::PacketWriter;

use super::ex;
use super::opcodes;

/// `ExShowVariationMakeWindow` — open the augment (make) UI. Empty body.
pub fn ex_show_variation_make_window() -> Vec<u8> {
    ex(opcodes::EX_SHOW_VARIATION_MAKE_WINDOW).into_bytes()
}

/// `ExShowVariationCancelWindow` — open the augment-removal UI. Empty body.
pub fn ex_show_variation_cancel_window() -> Vec<u8> {
    ex(opcodes::EX_SHOW_VARIATION_CANCEL_WINDOW).into_bytes()
}

/// `ExPutIntensiveResultForVariationMake` — echo the chosen life stone + the
/// gemstone fee back to the make window (the trailing `1` is Java's `_unk2`).
pub fn ex_put_intensive_result_for_variation_make(
    refiner_obj_id: i32,
    lifestone_item_id: i32,
    gemstone_item_id: i32,
    gemstone_count: i64,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE);
    w.write_i32(refiner_obj_id);
    w.write_i32(lifestone_item_id);
    w.write_i32(gemstone_item_id);
    w.write_i64(gemstone_count);
    w.write_i32(1);
    w.into_bytes()
}

/// `ExVariationResult` — the augment outcome: the two option ids and success.
pub fn ex_variation_result(option1: i32, option2: i32, success: bool) -> Vec<u8> {
    let mut w = ex(opcodes::EX_VARIATION_RESULT);
    w.write_i32(option1);
    w.write_i32(option2);
    w.write_i32(i32::from(success));
    w.into_bytes()
}

/// `ExVariationCancelResult` — augment removal outcome (1 success / 0 failure).
pub fn ex_variation_cancel_result(success: bool) -> Vec<u8> {
    let mut w = ex(opcodes::EX_VARIATION_CANCEL_RESULT);
    w.write_i32(i32::from(success));
    w.into_bytes()
}

/// `ExPutItemResultForVariationMake` — echo the weapon the player dropped into
/// the augment window's first slot. The trailing `1` is Java's success flag.
pub fn ex_put_item_result_for_variation_make(item_obj_id: i32, item_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE);
    w.write_i32(item_obj_id);
    w.write_i32(item_id);
    w.write_i32(1);
    w.into_bytes()
}

/// `ExPutCommissionResultForVariationMake` — echo the gemstone fee the window
/// asked for. Java's two trailing unknowns are `1` and `1`.
pub fn ex_put_commission_result_for_variation_make(
    gemstone_obj_id: i32,
    gemstone_item_id: i32,
    gemstone_count: i64,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PUT_COMMISSION_RESULT_FOR_VARIATION_MAKE);
    w.write_i32(gemstone_obj_id);
    w.write_i32(gemstone_item_id);
    w.write_i64(gemstone_count);
    w.write_i64(1);
    w.write_i32(1);
    w.into_bytes()
}

/// `ExPutItemResultForVariationCancel` — echo the augmented item the player
/// dropped into the *cancel* window, with its two option ids and the adena
/// price. The trailing `1` is Java's success flag.
pub fn ex_put_item_result_for_variation_cancel(
    item_obj_id: i32,
    item_id: i32,
    option1: i32,
    option2: i32,
    price: i64,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL);
    w.write_i32(item_obj_id);
    w.write_i32(item_id);
    w.write_i32(option1);
    w.write_i32(option2);
    w.write_i64(price);
    w.write_i32(1);
    w.into_bytes()
}
