//! Status-bar and relation packets.

use commons::network::PacketWriter;

use super::opcodes;

/// Java `StatusUpdateType` client ids used so far (grow as more stats need to
/// be pushed — regen, level-up, gear/buff changes, …).
pub mod status_update_type {
    /// `StatusUpdateType.PVP_FLAG` — the purple-name flag value (0/1/2).
    pub const PVP_FLAG: u8 = 0x1A;
    pub const CUR_HP: u8 = 0x09;
    pub const MAX_HP: u8 = 0x0A;
    pub const CUR_MP: u8 = 0x0B;
    pub const MAX_MP: u8 = 0x0C;
    pub const CUR_CP: u8 = 0x21;
    pub const MAX_CP: u8 = 0x22;
}

/// Port of `serverpackets/StatusUpdate`. `updates` is a list of
/// `(StatusUpdateType client id, value)` pairs, in the order Java's
/// `LinkedHashMap` would iterate (insertion order). `isVisible`/caster id
/// (used to tell nearby players who's responsible for the change) are scoped
/// to "not visible" — self-only updates, no known-list broadcast yet (G7).
pub fn status_update(object_id: i32, updates: &[(u8, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::STATUS_UPDATE);
    w.write_i32(object_id);
    w.write_i32(0); // caster id
    w.write_u8(0); // isVisible
    w.write_u8(updates.len() as u8);
    for &(kind, value) in updates {
        w.write_u8(kind);
        w.write_i32(value);
    }
    w.into_bytes()
}

/// Port of `serverpackets/RelationChanged` in its single-relation `SEND_ONE`
/// form (`new RelationChanged(playable, relation, autoAttackable)`): tells one
/// viewer how `object_id` relates to it now — the purple-flag/attackable state
/// under the target's name. `relation` is the party/clan/war bitmask (0 in a
/// world with no clans/parties yet); `reputation` is the karma value and
/// `pvp_flag` the 0/1/2 flag byte.
pub fn relation_changed(
    object_id: i32,
    relation: i32,
    auto_attackable: bool,
    reputation: i32,
    pvp_flag: u8,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RELATION_CHANGED);
    w.write_u8(0x02); // SEND_ONE
    w.write_i32(object_id);
    w.write_i32(relation);
    w.write_u8(auto_attackable as u8);
    w.write_i32(reputation);
    w.write_u8(pvp_flag);
    w.into_bytes()
}
