//! Target selection packets.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/MyTargetSelected`, sent only to the selecting
/// player. `color` is `player.level - target.level` for auto-attackable
/// targets (tints the target bar by level gap), 0 otherwise.
pub fn my_target_selected(target_object_id: i32, color: i16) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MY_TARGET_SELECTED);
    w.write_i32(1); // Grand Crusade
    w.write_i32(target_object_id);
    w.write_i16(color);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetSelected` — broadcast to other known players,
/// never to the selecting player themselves (they get `MyTargetSelected`).
pub fn target_selected(object_id: i32, target_object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_SELECTED);
    w.write_i32(object_id);
    w.write_i32(target_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TargetUnselected` — unlike `target_selected`,
/// Java broadcasts this with includeSelf=true, so the deselecting player
/// receives it too (the client needs it to drop its target UI).
pub fn target_unselected(object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TARGET_UNSELECTED);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}
