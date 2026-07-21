//! `SpecialCamera` (0xD6) — the cinematic camera packet.

use super::*;

/// The wire is **eleven** ints after the opcode, not twelve: Java's canonical
/// constructor accepts `range` and never writes it. A port that helpfully
/// serialised it would shift every following field by four bytes and desync
/// the whole cinematic.
#[test]
fn the_wire_carries_eleven_ints_and_drops_range() {
    let pkt = crate::network::server_packets::special_camera(
        1_000, // object id
        1_800, // force
        180,   // angle1
        -1,    // angle2
        1_500, // time
        15_000, // range — dropped
        10_000, // duration
        0, 0, 1, 0, 0,
    );
    assert_eq!(pkt.len(), 1 + 11 * 4, "opcode + eleven ints");
    assert_eq!(pkt[0], 0xD6);

    let field = |i: usize| i32::from_le_bytes(pkt[1 + i * 4..5 + i * 4].try_into().unwrap());
    assert_eq!(field(0), 1_000, "object id");
    assert_eq!(field(4), 1_500, "time");
    // The field after `time` is **duration**, not `range` — this is the
    // assertion that catches a helpfully-serialised range.
    assert_eq!(field(5), 10_000, "duration follows time directly");
    assert_ne!(field(5), 15_000, "range is not on the wire");
}

/// Valakas's opening shot, transcribed from the script, round-trips field for
/// field — a literal check that the argument order matches Java's.
#[test]
fn valakas_opening_shot_serialises_as_written() {
    // `new SpecialCamera(npc, 1800, 180, -1, 1500, 15000, 10000, 0, 0, 1, 0, 0)`
    let pkt = crate::network::server_packets::special_camera(
        7, 1_800, 180, -1, 1_500, 15_000, 10_000, 0, 0, 1, 0, 0,
    );
    let field = |i: usize| i32::from_le_bytes(pkt[1 + i * 4..5 + i * 4].try_into().unwrap());
    assert_eq!(
        (field(1), field(2), field(3), field(4), field(5), field(8)),
        (1_800, 180, -1, 1_500, 10_000, 1),
        "force, angle1, angle2, time, duration, isWide"
    );
}
