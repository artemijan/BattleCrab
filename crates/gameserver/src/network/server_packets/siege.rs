//! `serverpackets/SiegeInfo` (`CASTLE_SIEGE_INFO`, 0xC9) — the castle-siege
//! registration/roster window shown to a clan leader.

use commons::network::PacketWriter;

use super::opcodes;

/// Build the `SiegeInfo` window. `can_set_time` is Java's
/// `(ownerId == player.getClanId()) && player.isClanLeader()` — whether the
/// viewer may set the siege hour (that owner-only multi-hour list is a separate
/// packet, `RequestSetCastleSiegeTime`; here we always send the fixed date).
#[allow(clippy::too_many_arguments)]
pub fn siege_info(
    castle_id: i32,
    can_set_time: bool,
    owner_id: i32,
    owner_name: &str,
    owner_leader: &str,
    owner_ally_id: i32,
    owner_ally_name: &str,
    now_secs: i32,
    siege_date_secs: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CASTLE_SIEGE_INFO);
    w.write_i32(castle_id);
    w.write_i32(can_set_time as i32);
    w.write_i32(owner_id);
    if owner_id > 0 {
        w.write_string(owner_name);
        w.write_string(owner_leader);
        w.write_i32(owner_ally_id);
        w.write_string(owner_ally_name);
    } else {
        w.write_string("");
        w.write_string("");
        w.write_i32(0);
        w.write_string("");
    }
    w.write_i32(now_secs);
    // The owner-sets-the-hour branch (Config.SIEGE_HOUR_LIST) is deferred with
    // `RequestSetCastleSiegeTime`; always send the scheduled date.
    w.write_i32(siege_date_secs);
    w.write_i32(0);
    w.into_bytes()
}
