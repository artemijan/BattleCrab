//! Movement and location packets, plus the `ActionFailed` terminator.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/ActionFailed.STATIC_PACKET` — the "request consumed"
/// terminator Java sends after (almost) every `Action`/movement request,
/// success or not. `castingType` is always 0 (no `SkillCastingType` bar
/// tracking yet).
pub fn action_failed() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACTION_FAIL);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/ValidateLocation` — the server's "you are actually
/// here" correction to a drifted client (`ValidatePosition` reply).
pub fn validate_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VALIDATE_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.write_u8(0xff); // Java: trailing byte, "TODO: Find me!"
    w.into_bytes()
}

/// Port of `serverpackets/StopMove`.
pub fn stop_move(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STOP_MOVE);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.into_bytes()
}

/// Port of `serverpackets/ChangeMoveType` — walk/run toggle broadcast (Java
/// `Creature.setRunning`/`setWalking`).
pub fn change_move_type(object_id: i32, running: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHANGE_MOVE_TYPE);
    w.write_i32(object_id);
    w.write_i32(if running { 1 } else { 0 });
    w.write_i32(0); // c2
    w.into_bytes()
}

/// Port of `serverpackets/MoveToPawn` — "walk toward that creature, stopping
/// at `distance`" (chasing/follow movement; plain destination moves use
/// `MoveToLocation`).
pub fn move_to_pawn(object_id: i32, target_object_id: i32, distance: i32, x: i32, y: i32, z: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_PAWN);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(distance);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/TeleportToLocation` (fade-style, like Java's
/// constant 0).
pub fn teleport_to_location(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TELEPORT_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0); // fade 0, instant 1
    w.write_i32(heading);
    w.write_i32(0); // unknown
    w.into_bytes()
}

/// Port of `serverpackets/MoveToLocation` with an explicit destination —
/// unlike `enter_world::move_to_location` (which always sends dest==current
/// for the enter-world burst), this is the real move-start packet, broadcast
/// once to the mover *and* other known players (the client only starts
/// walking on the server's confirmation; Java's `Player.broadcastPacket`
/// includes self).
pub fn move_to_location(object_id: i32, dest_x: i32, dest_y: i32, dest_z: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_LOCATION);
    w.write_i32(object_id);
    w.write_i32(dest_x);
    w.write_i32(dest_y);
    w.write_i32(dest_z);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}
