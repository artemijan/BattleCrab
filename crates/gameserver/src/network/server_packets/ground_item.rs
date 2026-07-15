//! Ground-item packets (Classic layouts): `SpawnItem` (an item already lying in
//! view when you enter its region), `DropItem` (a fresh drop, with the toss
//! animation from a dropper), and `GetItem` (the pickup animation). Augmentation
//! isn't modelled, so its flag byte is always 0.

use commons::network::PacketWriter;

use super::opcodes;

/// A ground item's wire fields.
pub struct GroundItemView {
    pub object_id: i32,
    pub display_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub stackable: bool,
    pub count: i64,
    pub enchant: i32,
}

/// `serverpackets/SpawnItem` — the item is already on the ground; no toss
/// animation.
pub fn spawn_item(it: &GroundItemView) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SPAWN_ITEM);
    w.write_i32(it.object_id);
    w.write_i32(it.display_id);
    w.write_i32(it.x);
    w.write_i32(it.y);
    w.write_i32(it.z);
    w.write_i32(it.stackable as i32);
    w.write_i64(it.count);
    w.write_i32(0); // c2
    w.write_u8(it.enchant as u8);
    w.write_u8(0); // augmentation present
    w.write_u8(0);
    w.into_bytes()
}

/// `serverpackets/DropItem` — a fresh drop tossed from `dropper_object_id`.
pub fn drop_item(dropper_object_id: i32, it: &GroundItemView) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DROP_ITEM);
    w.write_i32(dropper_object_id);
    w.write_i32(it.object_id);
    w.write_i32(it.display_id);
    w.write_i32(it.x);
    w.write_i32(it.y);
    w.write_i32(it.z);
    w.write_u8(it.stackable as u8);
    w.write_i64(it.count);
    w.write_u8(0);
    w.write_u8(it.enchant as u8);
    w.write_u8(0); // augmentation present
    w.write_u8(0);
    w.into_bytes()
}

/// `serverpackets/GetItem` — `player_object_id` picks up the item at (x,y,z).
pub fn get_item(player_object_id: i32, item_object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GET_ITEM);
    w.write_i32(player_object_id);
    w.write_i32(item_object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}
