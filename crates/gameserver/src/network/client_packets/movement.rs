//! The two movement packets: the client's move request and its periodic
//! position validation.

use commons::network::PacketReader;

/// Port of `clientpackets/MoveBackwardToLocation` (`cddddddd`). `origin_x/y/z`
/// is only used for the same-origin/target "stop" check — not stored as
/// server-trusted state, per the no-geodata scope (client position is trusted
/// only insofar as it drives where we start interpolating from; the server's
/// own `player.x/y/z` is the authoritative start point).
pub struct MoveBackwardToLocation {
    pub target_x: i32,
    pub target_y: i32,
    pub target_z: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    pub movement_mode: i32,
}

impl MoveBackwardToLocation {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let target_x = r.read_i32()?;
        let target_y = r.read_i32()?;
        let target_z = r.read_i32()?;
        let origin_x = r.read_i32()?;
        let origin_y = r.read_i32()?;
        let origin_z = r.read_i32()?;
        let movement_mode = r.read_i32()?;
        Some(Self {
            target_x,
            target_y,
            target_z,
            origin_x,
            origin_y,
            origin_z,
            movement_mode,
        })
    }
}

/// Port of `clientpackets/ValidatePosition` — the client's periodic position
/// report. The trailing vehicle id is read and discarded (no boats).
pub struct ValidatePosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl ValidatePosition {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        let heading = r.read_i32()?;
        let _vehicle_id = r.read_i32()?;
        Some(Self { x, y, z, heading })
    }
}
