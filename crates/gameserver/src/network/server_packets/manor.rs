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

/// One seed line for [`ex_show_seed_setting`] — the owner's editable seed setup
/// (`ExShowSeedSetting`). The static half comes from the seed catalogue + item
/// reference prices; `current`/`next` are the `(start_amount, price)` the owner
/// has set for each period (`None` = unset, written as `0, 0`).
pub struct SeedSettingEntry {
    pub seed_id: i32,
    pub level: i32,
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
    /// `Seed.getSeedLimit` — the max sales the owner may set next period.
    pub seed_limit: i32,
    /// `Seed.getSeedReferencePrice` — the castle's per-unit production cost.
    pub seed_reference_price: i32,
    pub seed_min_price: i32,
    pub seed_max_price: i32,
    pub current: Option<(i64, i64)>,
    pub next: Option<(i64, i64)>,
}

/// Port of `serverpackets/ExShowSeedSetting` — the owner's "Edit Seed Setup"
/// window (`manor_menu_select` request 7). One line per seed the castle can
/// farm.
pub fn ex_show_seed_setting(manor_id: i32, seeds: &[SeedSettingEntry]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_SEED_SETTING);
    w.write_i32(manor_id);
    w.write_i32(seeds.len() as i32);
    for s in seeds {
        w.write_i32(s.seed_id);
        w.write_i32(s.level);
        w.write_u8(1);
        w.write_i32(s.reward1_item_id);
        w.write_u8(1);
        w.write_i32(s.reward2_item_id);
        w.write_i32(s.seed_limit);
        w.write_i32(s.seed_reference_price);
        w.write_i32(s.seed_min_price);
        w.write_i32(s.seed_max_price);
        let (cur_sales, cur_price) = s.current.unwrap_or((0, 0));
        w.write_i64(cur_sales);
        w.write_i64(cur_price);
        let (next_sales, next_price) = s.next.unwrap_or((0, 0));
        w.write_i64(next_sales);
        w.write_i64(next_price);
    }
    w.into_bytes()
}

/// One crop line for [`ex_show_crop_setting`] — the owner's editable crop setup
/// (`ExShowCropSetting`). `current`/`next` are `(start_amount, price, reward)`.
pub struct CropSettingEntry {
    pub crop_id: i32,
    pub level: i32,
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
    /// `Seed.getCropLimit` — the max buys the owner may set next period.
    pub crop_limit: i32,
    pub crop_min_price: i32,
    pub crop_max_price: i32,
    pub current: Option<(i64, i64, u8)>,
    pub next: Option<(i64, i64, u8)>,
}

/// Port of `serverpackets/ExShowCropSetting` — the owner's "Edit Crop Setup"
/// window (`manor_menu_select` request 8).
pub fn ex_show_crop_setting(manor_id: i32, crops: &[CropSettingEntry]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_CROP_SETTING);
    w.write_i32(manor_id);
    w.write_i32(crops.len() as i32);
    for c in crops {
        w.write_i32(c.crop_id);
        w.write_i32(c.level);
        w.write_u8(1);
        w.write_i32(c.reward1_item_id);
        w.write_u8(1);
        w.write_i32(c.reward2_item_id);
        w.write_i32(c.crop_limit);
        w.write_i32(0); // Java "???"
        w.write_i32(c.crop_min_price);
        w.write_i32(c.crop_max_price);
        let (cur_buy, cur_price, cur_reward) = c.current.unwrap_or((0, 0, 0));
        w.write_i64(cur_buy);
        w.write_i64(cur_price);
        w.write_u8(cur_reward);
        let (next_buy, next_price, next_reward) = c.next.unwrap_or((0, 0, 0));
        w.write_i64(next_buy);
        w.write_i64(next_price);
        w.write_u8(next_reward);
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

/// One seed-production line for [`ex_show_seed_info`], flattening the fields
/// Java reads from a `SeedProduction` plus its resolved `Seed`. Unknown seed ⇒
/// `seed_level = 0` and both reward item ids `0` (Java's `s == null` branch).
pub struct SeedInfoEntry {
    pub seed_id: i32,
    /// `SeedProduction.getAmount` — quantity left to sell.
    pub amount: i64,
    /// `SeedProduction.getStartAmount` — the quantity originally set up.
    pub start_amount: i64,
    pub price: i64,
    pub seed_level: i32,
    pub reward1_item_id: i32,
    pub reward2_item_id: i32,
}

/// Port of `serverpackets/ExShowSeedInfo` — the "Seed Purchase" manor dialog
/// (`OnNpcManorBypass` request 3), listing what the castle's manor currently
/// sells. `seeds = None` mirrors Java's `_seeds == null` (a next-period view
/// with no setup): the header is written with a `0` count and nothing else.
pub fn ex_show_seed_info(
    manor_id: i32,
    hide_buttons: bool,
    seeds: Option<&[SeedInfoEntry]>,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_SEED_INFO);
    w.write_u8(u8::from(hide_buttons)); // hide "Seed Purchase" button
    w.write_i32(manor_id);
    w.write_i32(0); // unknown
    let Some(seeds) = seeds else {
        w.write_i32(0);
        return w.into_bytes();
    };
    w.write_i32(seeds.len() as i32);
    for seed in seeds {
        w.write_i32(seed.seed_id);
        w.write_i64(seed.amount); // left to buy
        w.write_i64(seed.start_amount); // started amount
        w.write_i64(seed.price); // sell price
        w.write_i32(seed.seed_level);
        w.write_u8(1); // reward 1 present
        w.write_i32(seed.reward1_item_id);
        w.write_u8(1); // reward 2 present
        w.write_i32(seed.reward2_item_id);
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
/// Sent from [`crate::game_loop::manor`] off the castle's `CropProcure` state.
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
pub fn ex_ui_setting(key_mapping: &[u8]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_UI_SETTING);
    w.write_i32(key_mapping.len() as i32);
    for b in key_mapping {
        w.write_u8(*b);
    }
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
    use super::{CropInfoEntry, ex_show_crop_info};
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
