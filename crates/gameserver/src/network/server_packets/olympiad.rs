//! Grand Olympiad observer-mode packets — `ExOlympiadMode` (enter/leave the
//! spectator camera) and `ExOlympiadMatchList` (the ongoing-match picker).

use commons::network::PacketWriter;

use super::opcodes::EX;

/// `ExOlympiadMode` (`EX_OLYMPIAD_MODE`, 0xFE:0x7D): `3` enters observer mode,
/// `0` leaves it (the client swaps its HUD/camera).
pub fn ex_olympiad_mode(mode: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(0x7D);
    w.write_i32(mode);
    w.into_bytes()
}

/// One ongoing match row for [`ex_olympiad_match_list`].
pub struct OlympiadMatchRow {
    /// Stadium/arena id (arena 1 = 0).
    pub arena: i32,
    /// `true` once the fight is under way (else "standby").
    pub running: bool,
    pub player_a: String,
    pub player_b: String,
}

/// `ExOlympiadMatchList` (`EX_RECEIVE_OLYMPIAD`, 0xFE:0xD5): the list of ongoing
/// matches a spectator can jump between. Interlude only runs the non-classed 1v1
/// type, so every row's game type is `1`.
pub fn ex_olympiad_match_list(games: &[OlympiadMatchRow]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(EX);
    w.write_i16(0xD5);
    w.write_i32(0); // 0 = match list (1 = match result)
    w.write_i32(games.len() as i32);
    w.write_i32(0);
    for g in games {
        w.write_i32(g.arena);
        w.write_i32(1); // game type: 1 = non-classed
        w.write_i32(if g.running { 2 } else { 1 }); // 2 = playing, 1 = standby
        w.write_string(&g.player_a);
        w.write_string(&g.player_b);
    }
    w.into_bytes()
}
