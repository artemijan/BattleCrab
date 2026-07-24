//! Vehicle (boat) packets (G24.5): `VehicleInfo` (the boat's current position)
//! and `VehicleDeparture` (a move order to the next route point, which the
//! client interpolates).

use commons::network::PacketWriter;

use super::opcodes;

/// `VehicleInfo` — the boat's authoritative position + heading (sent on spawn
/// and on each arrival, so a late/idle client sees where it is).
pub fn vehicle_info(object_id: i32, x: i32, y: i32, z: i32, heading: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VEHICLE_INFO);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.into_bytes()
}

/// `VehicleDeparture` — move to `(x, y, z)` at `move_speed`/`rotation_speed`;
/// the client animates the boat there.
pub fn vehicle_departure(
    object_id: i32,
    move_speed: i32,
    rotation_speed: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VEHICLE_DEPARTURE);
    w.write_i32(object_id);
    w.write_i32(move_speed);
    w.write_i32(rotation_speed);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}
