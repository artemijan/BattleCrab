//! Fishing packets (G32): `ExFishingStart` / `ExFishingEnd` /
//! `ExUserInfoFishing`. The bob appearing, the reel result, and the
//! self-view fishing flag.

use commons::network::PacketWriter;

use super::opcodes;

/// `ExFishingStart` — the bob lands at `bait`. Broadcast so onlookers see it.
pub fn ex_fishing_start(player_object_id: i32, bait: (i32, i32, i32)) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_FISHING_START);
    w.write_i32(player_object_id);
    w.write_u8(0); // fish type
    w.write_i32(bait.0);
    w.write_i32(bait.1);
    w.write_i32(bait.2);
    w.write_u8(1); // 0 = newbie, 1 = normal, 2 = night
    w.into_bytes()
}

/// `ExFishingEnd` — the line reeled in; `reason` is the `FishingEndReason`
/// (0 = WIN, 1 = LOSE, 2 = STOP).
pub fn ex_fishing_end(player_object_id: i32, reason: u8) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_FISHING_END);
    w.write_i32(player_object_id);
    w.write_u8(reason);
    w.into_bytes()
}

/// `ExUserInfoFishing` — the self-view fishing flag + bob location (zeroed when
/// not fishing).
pub fn ex_user_info_fishing(
    player_object_id: i32,
    is_fishing: bool,
    bait: (i32, i32, i32),
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_USER_INFO_FISHING);
    w.write_i32(player_object_id);
    w.write_u8(is_fishing as u8);
    let (x, y, z) = if is_fishing { bait } else { (0, 0, 0) };
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}
