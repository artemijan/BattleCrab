//! Manor / crop packets plus the compass-zone, UI-setting, auto-soulshot and
//! world-map UI packets.

use commons::network::PacketWriter;

use super::opcodes;

/// `ExSetCompassZoneCode` values this slice can produce (Java declares
/// seven; the siege/PvP/altered codes wait for their zone types).
pub mod compass_zone {
    pub const PEACE: i32 = 0x0C;
    pub const GENERAL: i32 = 0x0F;
}

/// Port of `serverpackets/ExSetCompassZoneCode` — the client's compass zone
/// indicator (peace icon vs general), sent by `Player.revalidateZone` when
/// the code changes.
pub fn ex_set_compass_zone_code(code: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SET_COMPASS_ZONE_CODE);
    w.write_i32(code);
    w.into_bytes()
}

/// Port of `serverpackets/ExSendManorList` — the castle ids that have a manor
/// (Java writes the count then each `getResidenceId()`). Empty when manor is
/// disabled (`AllowManor = False`).
pub fn ex_send_manor_list(castle_ids: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SEND_MANOR_LIST);
    w.write_i32(castle_ids.len() as i32);
    for &id in castle_ids {
        w.write_i32(id);
    }
    w.into_bytes()
}

/// One reference-crop line for [`ex_show_manor_default_info`] — the static seed
/// catalogue view (Java resolves the two reference prices from item data).
pub struct ManorDefaultEntry {
    pub crop_id: i32,
    pub level: i32,
    /// `Seed.getSeedReferencePrice()` = the seed item's reference price.
    pub seed_reference_price: i32,
    /// `Seed.getCropReferencePrice()` = the crop item's reference price.
    pub crop_reference_price: i32,
    /// `Seed.getReward(1)` / `getReward(2)` item ids.
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
}

/// Port of `serverpackets/ExShowManorDefaultInfo` — the "Seed/Crop reference"
/// table the manor menu opens (`manor_menu_select` request 5). Java builds it
/// from `CastleManorManager.getCrops()` (one seed per distinct crop id). The
/// two per-line prices are the seed/crop items' reference prices, resolved from
/// item data by the caller.
pub fn ex_show_manor_default_info(crops: &[ManorDefaultEntry], hide_buttons: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_MANOR_DEFAULT_INFO);
    w.write_u8(u8::from(hide_buttons)); // hide "Seed Purchase" / "Crop Sales"
    w.write_i32(crops.len() as i32);
    for crop in crops {
        w.write_i32(crop.crop_id);
        w.write_i32(crop.level);
        w.write_i32(crop.seed_reference_price);
        w.write_i32(crop.crop_reference_price);
        w.write_u8(1); // reward 1 type
        w.write_i32(crop.reward1_item_id);
        w.write_u8(1); // reward 2 type
        w.write_i32(crop.reward2_item_id);
    }
    w.into_bytes()
}

/// One crop-procurement line for [`ex_show_crop_info`], flattening the fields
/// Java reads from a `CropProcure` plus its resolved `Seed`. When the seed
/// can't be resolved Java writes `seed_level = 0` and both reward item ids as
/// `0`, so a caller with no seed data supplies those defaults directly.
pub struct CropInfoEntry {
    pub crop_id: i32,
    /// `CropProcure.getAmount` — remaining quantity the manor will still buy.
    pub amount: i64,
    /// `CropProcure.getStartAmount` — the quantity originally offered.
    pub start_amount: i64,
    pub price: i64,
    /// `CropProcure.getReward` — the reward-type id (0/1/2).
    pub reward: u8,
    /// `Seed.getLevel`, or 0 when the seed is unknown.
    pub seed_level: i32,
    /// `Seed.getReward(1)` / `getReward(2)` item ids, or 0 when unknown.
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
}

/// Port of `serverpackets/ExShowCropInfo` — the "Crop Sales" manor dialog a
/// castle owner opens through the chamberlain (`OnNpcManorBypass` request 4).
/// `crops = None` mirrors Java's `_crops == null` (next-period view while the
/// manor isn't yet approved): the header is written without a crop count.
///
/// TODO(manor): nothing sends this yet — the trigger (CastleChamberlain
/// `onNpcManorBypass` + castle ownership) and the crop data source
/// (`CastleManorManager.getCropProcure` / `Seed`) are unported, so callers
/// currently have no `CropInfoEntry` list to pass. Wire it once the manor
/// system lands; the serializer itself matches `writeImpl` byte-for-byte.
pub fn ex_show_crop_info(
    manor_id: i32,
    hide_buttons: bool,
    crops: Option<&[CropInfoEntry]>,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_CROP_INFO);
    w.write_u8(u8::from(hide_buttons)); // hide "Crop Sales" button
    w.write_i32(manor_id);
    w.write_i32(0);
    if let Some(crops) = crops {
        w.write_i32(crops.len() as i32);
        for crop in crops {
            w.write_i32(crop.crop_id);
            w.write_i64(crop.amount); // buy residual
            w.write_i64(crop.start_amount); // buy
            w.write_i64(crop.price); // buy price
            w.write_u8(crop.reward);
            w.write_i32(crop.seed_level);
            w.write_u8(1); // reward 1 present
            w.write_i32(crop.reward1_item_id);
            w.write_u8(1); // reward 2 present
            w.write_i32(crop.reward2_item_id);
        }
    }
    w.into_bytes()
}

/// Port of `serverpackets/ExPCCafePointInfo` — updates the client's PC-cafe
/// point display. Mirrors the 3-arg Java ctor `(points, pointsToAdd, time)`
/// used by `//pccafepoints`: `periodType = 1` (acquisition window),
/// `remainTime = 0`, `pointType` red when spending (`add < 0`) else cyan, and
/// the trailing `time * 3` seconds value.
pub fn ex_pccafe_point_info(points: i32, add_point: i32, time: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_PCCAFE_POINT_INFO);
    w.write_i32(points); // num points
    w.write_i32(add_point); // points inc display
    w.write_u8(1); // period type (1 = acquisition)
    w.write_i32(0); // remain time
    w.write_u8(if add_point < 0 { 2 } else { 1 }); // color (2 = red, 1 = cyan)
    w.write_i32(time * 3); // seconds * 3
    w.into_bytes()
}

/// Port of `serverpackets/settings/ExUISetting` — the player's stored UI key
/// mapping. TODO(G-later): load the stored mapping; null → length 0 for now.
pub fn ex_ui_setting() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_UI_SETTING);
    w.write_i32(0); // no stored key-mapping
    w.into_bytes()
}

/// Port of `serverpackets/ShowMiniMap` — opens the world-map window.
/// `map_id` 0 = the base world map (1665 = the Seven Signs variant, unused).
/// The trailing byte is the Seven Signs period state (always 0 here).
pub fn show_mini_map(map_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHOW_MINI_MAP);
    w.write_i32(map_id);
    w.write_u8(0); // Seven Signs state
    w.into_bytes()
}

/// Port of `serverpackets/ExAutoSoulShot`: acks a `RequestAutoSoulShot`
/// toggle so the client updates the shortcut's auto-use glow (`itemId`,
/// `enable` 1/0, `type`).
pub fn ex_auto_soul_shot(item_id: i32, enable: bool, shot_type: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_AUTO_SOUL_SHOT);
    w.write_i32(item_id);
    w.write_i32(if enable { 1 } else { 0 });
    w.write_i32(shot_type);
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{ex_show_crop_info, CropInfoEntry};
    use crate::network::server_packets::opcodes;
    use commons::network::PacketWriter;

    #[test]
    fn ex_show_crop_info_layout_matches_java() {
        // One crop line, buttons hidden. Byte layout hand-computed against
        // `ExShowCropInfo.writeImpl`.
        let crops = [CropInfoEntry {
            crop_id: 5000,
            amount: 10,
            start_amount: 40,
            price: 999,
            reward: 2,
            seed_level: 3,
            reward1_item_id: 6000,
            reward2_item_id: 6001,
        }];
        let bytes = ex_show_crop_info(1, true, Some(&crops));

        let mut exp = PacketWriter::new();
        exp.write_u8(opcodes::EX);
        exp.write_i16(opcodes::EX_SHOW_CROP_INFO);
        exp.write_u8(1); // hide buttons
        exp.write_i32(1); // manor id
        exp.write_i32(0);
        exp.write_i32(1); // crop count
        exp.write_i32(5000);
        exp.write_i64(10);
        exp.write_i64(40);
        exp.write_i64(999);
        exp.write_u8(2); // reward type
        exp.write_i32(3); // seed level
        exp.write_u8(1);
        exp.write_i32(6000);
        exp.write_u8(1);
        exp.write_i32(6001);
        assert_eq!(bytes, exp.into_bytes());

        // `crops = None` (Java `_crops == null`): header only, no crop count.
        let none = ex_show_crop_info(7, false, None);
        let mut exp_none = PacketWriter::new();
        exp_none.write_u8(opcodes::EX);
        exp_none.write_i16(opcodes::EX_SHOW_CROP_INFO);
        exp_none.write_u8(0);
        exp_none.write_i32(7);
        exp_none.write_i32(0);
        assert_eq!(none, exp_none.into_bytes());
    }
}
