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
    // `getDamage()` — the 0..6 crack grade the client draws on the gate mesh.
    // Java returns 0 outright for a door belonging to no castle/fort; here a
    // non-siege door never loses HP, so the same formula yields 0 for it.
    w.write_i32(damage_grade(door.current_hp, t.hp_max));
    w.write_i32(0); // isEnemy
    w.write_i32(door.door_id);
    w.write_i32(door.current_hp);
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

/// Java `Door.getDamage()`: `6 - ceil(cur / max * 6)`, clamped to `0..=6` — the
/// visible damage grade, not a percentage. A full-HP gate is 0, a breached one 6.
fn damage_grade(current_hp: i32, max_hp: i32) -> i32 {
    if max_hp <= 0 {
        return 0;
    }
    let ratio = current_hp as f64 / max_hp as f64;
    ((6.0 - (ratio * 6.0).ceil()) as i32).clamp(0, 6)
}

#[cfg(test)]
mod tests {
    use super::damage_grade;

    /// Java `Door.getDamage()` — `6 - ceil(cur/max * 6)`. The `ceil` means the
    /// grade is a *sixth*, not a percentage: the first crack only appears once
    /// the gate is below 5/6 HP, and a gate on its last hit point still shows 5.
    #[test]
    fn the_damage_grade_walks_zero_to_six() {
        assert_eq!(damage_grade(1000, 1000), 0, "untouched");
        assert_eq!(damage_grade(900, 1000), 0, "still within the top sixth");
        assert_eq!(damage_grade(800, 1000), 1, "the first crack");
        assert_eq!(damage_grade(500, 1000), 3, "half");
        assert_eq!(damage_grade(1, 1000), 5, "one HP is not yet breached");
        assert_eq!(damage_grade(0, 1000), 6, "breached");
        // Defensive: a template with no HP can't divide.
        assert_eq!(damage_grade(0, 0), 0);
    }
}
