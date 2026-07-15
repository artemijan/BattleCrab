//! Enchant window server packets: `ChooseInventoryItem` (opens the enchant
//! window for a scroll), the two `ExPutEnchant*ItemResult` acks, and
//! `EnchantResult` (the success/fail outcome).

use commons::network::PacketWriter;

use super::opcodes;

/// `EnchantResult` result codes (Java `EnchantResult.*`).
pub mod enchant_result {
    pub const SUCCESS: i32 = 0;
    pub const FAIL: i32 = 1;
    pub const ERROR: i32 = 2;
    pub const BLESSED_FAIL: i32 = 3;
    pub const NO_CRYSTAL: i32 = 4;
    pub const SAFE_FAIL: i32 = 5;
}

/// Port of `serverpackets/ChooseInventoryItem` — tells the client to open the
/// enchant window for scroll `item_id` (sent by the `EnchantScrolls` handler).
pub fn choose_inventory_item(item_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHOOSE_INVENTORY_ITEM);
    w.write_i32(item_id);
    w.into_bytes()
}

/// Port of `serverpackets/ExPutEnchantScrollItemResult` — acks the
/// scroll+target selection (`result` = scroll object id on success, 0 on
/// failure).
pub fn ex_put_enchant_scroll_item_result(result: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PUT_ENCHANT_SCROLL_ITEM_RESULT);
    w.write_i32(result);
    w.into_bytes()
}

/// Port of `serverpackets/ExPutEnchantTargetItemResult` — acks the target-item
/// selection (`result` = target object id on success, 0 on failure).
pub fn ex_put_enchant_target_item_result(result: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PUT_ENCHANT_TARGET_ITEM_RESULT);
    w.write_i32(result);
    w.into_bytes()
}

/// Port of `serverpackets/EnchantResult` — the enchant outcome. `crystal`/
/// `count` carry the crystals returned on a destroying failure; `enchant_level`
/// the item's resulting level (success/safe/blessed). The three trailing shorts
/// are the augment option ids — always 0 for a plain item
/// (`Item.DEFAULT_ENCHANT_OPTIONS`).
pub fn enchant_result(result: i32, crystal: i32, count: i64, enchant_level: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ENCHANT_RESULT);
    w.write_i32(result);
    w.write_i32(crystal);
    w.write_i64(count);
    w.write_i32(enchant_level);
    w.write_i16(0);
    w.write_i16(0);
    w.write_i16(0);
    w.into_bytes()
}
