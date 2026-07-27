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
pub fn move_to_pawn(
    object_id: i32,
    target_object_id: i32,
    distance: i32,
    x: i32,
    y: i32,
    z: i32,
    tx: i32,
    ty: i32,
    tz: i32,
) -> Vec<u8> {
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

/// Port of `serverpackets/ExTeleportToLocationActivate` — the "teleport
/// finished" packet `Creature.teleToLocation` sends to the player right
/// after `setXYZ`. Without it the client never leaves the black loading
/// screen (it does not act on `TeleportToLocation` alone).
pub fn ex_teleport_to_location_activate(
    object_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_TELEPORT_TO_LOCATION_ACTIVATE);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0); // unknown (not instance id)
    w.write_i32(heading);
    w.write_i32(0); // unknown
    w.into_bytes()
}

/// Port of `serverpackets/Ride` — mount / dismount broadcast. `ride_type` is the
/// `MountType` ordinal (0 none, 1 strider, 2 wyvern, 3 wolf); `mount_npc_id` is
/// sent as `+ 1_000_000` (0 stays 0), matching Java.
pub fn ride(
    object_id: i32,
    mounted: bool,
    ride_type: u8,
    mount_npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RIDE);
    w.write_i32(object_id);
    w.write_i32(mounted as i32);
    w.write_i32(ride_type as i32);
    w.write_i32(if mount_npc_id == 0 {
        0
    } else {
        mount_npc_id + 1_000_000
    });
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Port of `serverpackets/MoveToLocation` with an explicit destination —
/// unlike `enter_world::move_to_location` (which always sends dest==current
/// for the enter-world burst), this is the real move-start packet, broadcast
/// once to the mover *and* other known players (the client only starts
/// walking on the server's confirmation; Java's `Player.broadcastPacket`
/// includes self).
pub fn move_to_location(
    object_id: i32,
    dest_x: i32,
    dest_y: i32,
    dest_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Vec<u8> {
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

/// Port of `serverpackets/ExServerPrimitive` (FE:11) — the debug drawing
/// packet (`//debug` doors/geodata/movement visualizers). `name` keys the
/// drawing client-side: re-sending the same name replaces the previous
/// geometry, which is how a drawing is moved or cleared (a single
/// zero-length black line at z −16000 is Java's "erase" idiom).
pub fn ex_server_primitive(
    name: &str,
    x: i32,
    y: i32,
    z: i32,
    lines: &[(u32, (i32, i32, i32), (i32, i32, i32))],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SERVER_PRIMITIVE);
    w.write_string(name);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(65535); // display range/angle (Java constant)
    w.write_i32(65535);
    w.write_i32(lines.len() as i32);
    for &(rgb, (x1, y1, z1), (x2, y2, z2)) in lines {
        w.write_u8(2); // type: line
        w.write_string(""); // per-line name (unused)
        w.write_i32(((rgb >> 16) & 0xFF) as i32);
        w.write_i32(((rgb >> 8) & 0xFF) as i32);
        w.write_i32((rgb & 0xFF) as i32);
        w.write_i32(0); // name colored
        w.write_i32(x1);
        w.write_i32(y1);
        w.write_i32(z1);
        w.write_i32(x2);
        w.write_i32(y2);
        w.write_i32(z2);
    }
    w.into_bytes()
}
