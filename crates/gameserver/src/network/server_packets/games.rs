//! Packets for the "games" managers — the Monster Race board/animation
//! (`MonRaceInfo`, G26.5).

use super::opcodes;
use commons::network::PacketWriter;

/// `MonRaceInfo(code1, code2, monsters, speeds)` — the Derby Track board and
/// race animation. `code1/code2` pick the phase (`(-1,0)` set up / `(0,15322)`
/// they're off / `(13765,-1)` mid-race). `monsters[i]` is
/// `(object_id, display_id, collision_height, collision_radius)` for lane index
/// `i`; the 20 per-step speeds are written only on the setup packet
/// (`code1 == 0`), else zeroed.
pub fn mon_race_info(
    code1: i32,
    code2: i32,
    monsters: &[(i32, i32, f64, f64); 8],
    speeds: &[[i32; 20]; 8],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MON_RACE_INFO);
    w.write_i32(code1);
    w.write_i32(code2);
    w.write_i32(8);
    for (i, &(object_id, display_id, coll_h, coll_r)) in monsters.iter().enumerate() {
        let y = 181875 + 58 * (7 - i as i32);
        w.write_i32(object_id);
        w.write_i32(display_id + 1_000_000);
        w.write_i32(14107); // origin X
        w.write_i32(y); // origin Y
        w.write_i32(-3566); // origin Z
        w.write_i32(12080); // end X
        w.write_i32(y); // end Y
        w.write_i32(-3566); // end Z
        w.write_f64(coll_h);
        w.write_f64(coll_r);
        w.write_i32(120);
        for &step in &speeds[i] {
            w.write_u8(if code1 == 0 { step as u8 } else { 0 });
        }
    }
    w.into_bytes()
}

/// `ExCursedWeaponList` — every cursed-weapon item id the server knows (the
/// client's window uses it to label the entries).
pub fn ex_cursed_weapon_list(item_ids: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_CURSED_WEAPON_LIST);
    w.write_i32(item_ids.len() as i32);
    for id in item_ids {
        w.write_i32(*id);
    }
    w.into_bytes()
}

/// `ExCursedWeaponLocation` — one entry per *live* cursed weapon: its item id,
/// whether it is currently wielded (`1`) or lying on the ground (`0`), and
/// where. Java sends nothing at all when the list is empty.
pub fn ex_cursed_weapon_location(entries: &[(i32, i32, i32, i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_CURSED_WEAPON_LOCATION);
    w.write_i32(entries.len() as i32);
    for (item_id, activated, x, y, z) in entries {
        w.write_i32(*item_id);
        w.write_i32(*activated);
        w.write_i32(*x);
        w.write_i32(*y);
        w.write_i32(*z);
    }
    w.into_bytes()
}
