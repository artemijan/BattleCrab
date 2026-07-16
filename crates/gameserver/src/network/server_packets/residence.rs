//! Castle / fortress residence packets — the world map's ownership overlay.
//! Ported from `serverpackets/{ExShowCastleInfo,ExShowFortressInfo}`.
//!
//! Sieges aren't ported yet, so both builders report the static residence
//! lists (all unowned, no sieges scheduled) — the same wire shape a fresh
//! Java DB produces. TODO(sieges): owner clan names, tax, siege dates.

use commons::network::PacketWriter;

use super::opcodes;

/// The nine Interlude castles (`castle` table ids 1..=9).
const CASTLE_IDS: std::ops::RangeInclusive<i32> = 1..=9;

/// The twenty-one fortresses (`fort` table ids 101..=121).
const FORT_IDS: std::ops::RangeInclusive<i32> = 101..=121;

/// Port of `serverpackets/ExShowCastleInfo` — per castle: id, owner clan
/// name, buy-tax percent, siege date (epoch seconds), siege-in-progress and
/// castle-side bytes.
pub fn ex_show_castle_info() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_CASTLE_INFO);
    w.write_i32(CASTLE_IDS.count() as i32);
    for id in CASTLE_IDS {
        w.write_i32(id);
        w.write_string(""); // owner clan name (unowned)
        w.write_i32(0); // tax percent
        w.write_i32(0); // siege date
        w.write_u8(0); // siege in progress
        w.write_u8(0); // castle side (NEUTRAL)
    }
    w.into_bytes()
}

/// Port of `serverpackets/ExShowFortressInfo` — per fort: id, owner clan
/// name, siege-in-progress int, owned-time seconds.
pub fn ex_show_fortress_info() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_FORTRESS_INFO);
    w.write_i32(FORT_IDS.count() as i32);
    for id in FORT_IDS {
        w.write_i32(id);
        w.write_string(""); // owner clan name (unowned)
        w.write_i32(0); // siege in progress
        w.write_i32(0); // owned time
    }
    w.into_bytes()
}
