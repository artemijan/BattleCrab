//! Door and static-object packets.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/DoorStatusUpdate`. `enemy` (siege-active doors)
/// and the HP-damage grade are always their idle values — no sieges, and
/// nothing damages doors yet.
pub fn door_status_update(
    door: &crate::model::door::Door,
    t: &crate::data::door_data::DoorTemplate,
    open: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DOOR_STATUS_UPDATE);
    w.write_i32(door.object_id);
    w.write_i32(!open as i32); // "isClosed"
    w.write_i32(0); // damage grade (getDamage)
    w.write_i32(0); // isEnemy
    w.write_i32(door.door_id);
    w.write_i32(t.hp_max); // current HP (always full)
    w.write_i32(t.hp_max);
    w.into_bytes()
}

/// Port of `serverpackets/StaticObjectInfo`'s door constructor
/// (`type = 1`, mesh index 1 — `Door._meshindex` default; the GM
/// forced-targetable variant is not ported).
pub fn static_object_info_door(
    door: &crate::model::door::Door,
    t: &crate::data::door_data::DoorTemplate,
    open: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATIC_OBJECT);
    w.write_i32(door.door_id); // staticObjectId
    w.write_i32(door.object_id);
    w.write_i32(1); // type: door
    w.write_i32(t.targetable as i32);
    w.write_i32(1); // mesh index
    w.write_i32(!open as i32); // isClosed
    w.write_i32(0); // isEnemy
    w.write_i32(t.hp_max); // current HP
    w.write_i32(t.hp_max); // max HP
    w.write_i32(t.show_hp as i32);
    w.write_i32(0); // damage grade
    w.into_bytes()
}

/// Port of `serverpackets/StaticObjectInfo`'s StaticObject constructor —
/// Java hardcodes type 0/targetable/mesh 0/no HP for the decoration kind
/// regardless of the template's `type` attribute.
pub fn static_object_info(static_id: i32, object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATIC_OBJECT);
    w.write_i32(static_id);
    w.write_i32(object_id);
    w.write_i32(0); // type
    w.write_i32(1); // targetable
    w.write_i32(0); // mesh index
    w.write_i32(0); // isClosed
    w.write_i32(0); // isEnemy
    w.write_i32(0); // current HP
    w.write_i32(0); // max HP
    w.write_i32(0); // showHp
    w.write_i32(0); // damage grade
    w.into_bytes()
}
