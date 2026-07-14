//! `CharInfo` (how a player appears to others) and `DeleteObject`.

use commons::network::PacketWriter;

use crate::model::inventory::PaperdollSlot;
use super::opcodes;

/// Port of `serverpackets/DeleteObject` — removes an object from the client's
/// screen when it leaves the observer's known area.
pub fn delete_object(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DELETE_OBJECT);
    w.write_i32(object_id);
    w.write_u8(0); // c2
    w.into_bytes()
}

/// `CharInfo.PAPERDOLL_ORDER` — the 12-slot equipment view other clients get
/// (a subset of the 33-slot `UserInfo` order; `RHand` repeats for LRHAND).
const CHAR_INFO_PAPERDOLL_ORDER: [PaperdollSlot; 12] = [
    PaperdollSlot::Under,
    PaperdollSlot::Head,
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Cloak,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// `ServerPacket.PAPERDOLL_ORDER_AUGMENT`.
const CHAR_INFO_PAPERDOLL_ORDER_AUGMENT: [PaperdollSlot; 3] =
    [PaperdollSlot::RHand, PaperdollSlot::LHand, PaperdollSlot::RHand];

/// `ServerPacket.PAPERDOLL_ORDER_VISUAL_ID`.
const CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::RHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// Port of `serverpackets/CharInfo` — how this player appears on *other*
/// players' clients (the counterpart of `UserInfo` for the owner). Values for
/// systems not yet modeled (clan, mounts, stores, cubics, fishing, abnormal
/// visual effects…) are their empty Java defaults; the vehicle branch and the
/// GM-sees-invisible variant are skipped (no boats/GM model).
pub fn char_info(v: &crate::model::PlayerView) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, pvitals, speeds, collision, combat, inventory, .. } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_INFO);
    w.write_u8(0); // Grand Crusade
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(0); // vehicle object id
    w.write_i32(p.object_id);
    w.write_string(&p.name);
    w.write_i16(p.race as i16);
    w.write_u8(p.is_female as u8);
    w.write_i32(p.base_class_id); // root class id

    for slot in CHAR_INFO_PAPERDOLL_ORDER {
        let item_id = inventory.paperdoll_item_id(slot);
        w.write_i32(item_id);
    }
    for slot in CHAR_INFO_PAPERDOLL_ORDER_AUGMENT {
        let augment = inventory.paperdoll_augmentation(slot);
        w.write_i32(augment.map_or(0, |a| a.0));
        w.write_i32(augment.map_or(0, |a| a.1));
    }
    w.write_u8(0); // armor min enchant
    for slot in CHAR_INFO_PAPERDOLL_ORDER_VISUAL_ID {
        let visual_id = inventory.paperdoll_visual_id(slot);
        w.write_i32(visual_id);
    }

    w.write_u8(0); // pvp flag
    w.write_i32(p.reputation);
    w.write_i32(combat.m_atk_spd);
    w.write_i32(combat.p_atk_spd);
    w.write_i16(speeds.run_spd as i16);
    w.write_i16(speeds.walk_spd as i16);
    w.write_i16(speeds.swim_run_spd as i16);
    w.write_i16(speeds.swim_walk_spd as i16);
    w.write_i16(0); // fly run
    w.write_i16(0); // fly walk
    w.write_i16(0); // fly run (repeat)
    w.write_i16(0); // fly walk (repeat)
    w.write_f64(speeds.move_multiplier);
    w.write_f64(1.0); // attack speed multiplier
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(p.hair_style); // visual hair
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_string(&p.title);
    w.write_i32(p.clan_id);
    w.write_i32(0); // clan crest id
    w.write_i32(0); // ally id
    w.write_i32(0); // ally crest id
    w.write_u8(1); // !isSitting — standing
    w.write_u8(speeds.running as u8);
    w.write_u8(0); // in combat
    w.write_u8(0); // alike dead
    w.write_u8(0); // invisible
    w.write_u8(0); // mount type
    w.write_u8(0); // private store type
    w.write_i16(0); // cubic count (+ cubic ids)
    w.write_u8(0); // in matching room
    w.write_u8(0); // 1 water, 2 flying mount
    w.write_i16(0); // recom have
    w.write_i32(0); // mount npc id
    w.write_i32(p.class_id);
    w.write_i32(0); // TODO: Find me! (Java unknown)
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8); // weapon enchant
    w.write_u8(0); // team id
    w.write_i32(0); // clan crest large id
    w.write_u8(0); // noble
    w.write_u8(p.hero_aura as u8); // hero (isHero || (isGM && GMHeroAura))
    w.write_u8(0); // fishing
    w.write_i32(0); // bait x
    w.write_i32(0); // bait y
    w.write_i32(0); // bait z
    w.write_i32(p.name_color); // name color (from access level)
    w.write_i32(pos.heading);
    w.write_u8(0); // pledge class
    w.write_i16(0); // pledge type
    w.write_i32(p.title_color); // title color (from access level)
    w.write_u8(0); // cursed weapon level
    w.write_i32(0); // clan reputation score
    w.write_i32(0); // transformation display id
    w.write_i32(0); // agathion id
    w.write_u8(0); // nPvPRestrainStatus
    w.write_i32(pvitals.cur_cp.round() as i32);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_hp.round() as i32);
    w.write_i32(vitals.max_mp);
    w.write_i32(vitals.cur_mp.round() as i32);
    w.write_u8(0); // cBRLectureMark
    w.write_i32(0); // abnormal visual effect count (+ short ids)
    w.write_u8(0); // true hero (100 when true)
    w.write_u8(1); // hair accessory enabled
    w.write_u8(0); // used ability points
    w.into_bytes()
}
