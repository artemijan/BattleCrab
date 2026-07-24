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

/// `GetOnVehicle` — `player` boarded `boat` at seat `(x,y,z)` (relative).
pub fn get_on_vehicle(
    player_object_id: i32,
    boat_object_id: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GET_ON_VEHICLE);
    w.write_i32(player_object_id);
    w.write_i32(boat_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// `GetOffVehicle` — `player` stepped off `boat` at world point `(x,y,z)`.
pub fn get_off_vehicle(
    player_object_id: i32,
    boat_object_id: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GET_OFF_VEHICLE);
    w.write_i32(player_object_id);
    w.write_i32(boat_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// `MoveToLocationInVehicle` — `player` walks on `boat`'s deck from `origin` to
/// `dest` (both relative to the boat); the client interpolates the on-deck walk.
pub fn move_to_location_in_vehicle(
    player_object_id: i32,
    boat_object_id: i32,
    dest: (i32, i32, i32),
    origin: (i32, i32, i32),
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MOVE_TO_LOCATION_IN_VEHICLE);
    w.write_i32(player_object_id);
    w.write_i32(boat_object_id);
    w.write_i32(dest.0);
    w.write_i32(dest.1);
    w.write_i32(dest.2);
    w.write_i32(origin.0);
    w.write_i32(origin.1);
    w.write_i32(origin.2);
    w.into_bytes()
}

/// `StopMoveInVehicle` — `player` stopped on `boat`'s deck at `(x,y,z)`
/// (relative) facing `heading`.
pub fn stop_move_in_vehicle(
    player_object_id: i32,
    boat_object_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STOP_MOVE_IN_VEHICLE);
    w.write_i32(player_object_id);
    w.write_i32(boat_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(heading);
    w.into_bytes()
}
