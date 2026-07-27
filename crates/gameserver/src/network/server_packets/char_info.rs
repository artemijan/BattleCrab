//! `CharInfo` (how a player appears to others) and `DeleteObject`.

use commons::network::PacketWriter;

use super::opcodes;
use crate::model::inventory::PaperdollSlot;

/// Port of `serverpackets/ExVoteSystemInfo` — the recommendation panel state.
/// The bonus fields (`bonusTime`/`bonusVal`/`bonusType`) are always 0 in
/// Interlude Classic, matching Java's hardcoded ctor.
pub fn ex_vote_system_info(rec_left: i32, rec_have: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_VOTE_SYSTEM_INFO);
    w.write_i32(rec_left);
    w.write_i32(rec_have);
    w.write_i32(0); // bonus time
    w.write_i32(0); // bonus value
    w.write_i32(0); // bonus type
    w.into_bytes()
}

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
const CHAR_INFO_PAPERDOLL_ORDER_AUGMENT: [PaperdollSlot; 3] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::RHand,
];

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
/// `visuals` is the creature's live abnormal-visual client-id list
/// (`game_loop::abnormal::visual_effects`), passed in rather than read off the
/// view because `PlayerView` carries no `Buffs`.
pub fn char_info(v: &crate::model::PlayerView, visuals: &[i16], cubics: &[i32]) -> Vec<u8> {
    let crate::model::PlayerView {
        p,
        pos,
        vitals,
        pvitals,
        speeds,
        collision,
        combat,
        inventory,
        ..
    } = v;
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
    // Sent divided by the move multiplier (Java `_runSpd = round(getRunSpeed()
    // / _moveMultiplier)`); the client multiplies it back.
    let client_speeds = speeds.client_speed_fields();
    for spd in client_speeds {
        w.write_i16(spd);
    }
    // `_flyRunSpd/_flyWalkSpd = isFlying() ? run/walk : 0`, written twice
    // (Java sends the same pair for both fly slots) — onlookers' clients use
    // these to animate a wyvern rider's flight.
    let (fly_run, fly_walk) = if p.is_flying() {
        (client_speeds[0], client_speeds[1])
    } else {
        (0, 0)
    };
    w.write_i16(fly_run);
    w.write_i16(fly_walk);
    w.write_i16(fly_run);
    w.write_i16(fly_walk);
    w.write_f64(speeds.client_move_multiplier()); // Java getMovementSpeedMultiplier (leg-anim rate)
    w.write_f64(combat.client_atk_speed_multiplier()); // Java getAttackSpeedMultiplier (swing-anim rate)
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(p.hair_style); // visual hair
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_string(&p.title);
    w.write_i32(p.clan_id);
    w.write_i32(p.clan_crest_id);
    w.write_i32(p.ally_id);
    w.write_i32(p.ally_crest_id);
    w.write_u8(1); // !isSitting — standing
    w.write_u8(speeds.running as u8);
    w.write_u8(0); // in combat
    w.write_u8(0); // alike dead
    w.write_u8(0); // invisible
    w.write_u8(p.mount_type); // mount type (1 strider, 2 wyvern, 3 wolf, 0 none)
    w.write_u8(p.store_type); // private store type
                              // Cubic count then one short per cubic id. This was hard-coded to 0, so a
                              // summoned cubic was invisible to every other player — the same shape as
                              // the abnormal-visual-effect count before G19 fixed it.
    w.write_i16(cubics.len() as i16);
    for id in cubics {
        w.write_i16(*id as i16);
    }
    w.write_u8(0); // in matching room
                   // Java: `insideZone(WATER) ? 1 : isFlyingMounted() ? 2 : 0`, where
                   // `isFlyingMounted()` is *transform*-based (Gracia sky mounts) — a wyvern
                   // rider stays 0 even in Java (its flight renders via the mount npc id +
                   // the fly speeds above). Water: TODO with water zones.
    w.write_u8(0);
    w.write_i16(p.rec_have as i16); // recom have
    w.write_i32(if p.mount_npc_id == 0 {
        0
    } else {
        p.mount_npc_id + 1_000_000
    }); // mount npc id
    w.write_i32(p.class_id);
    w.write_i32(0); // TODO: Find me! (Java unknown)
                    // Java: `isMounted() ? 0 : _enchantLevel` — no weapon glow on a mount.
    w.write_u8(if p.is_mounted() {
        0
    } else {
        inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8
    });
    w.write_u8(v.p.team); // team aura (`//setteam`)
    w.write_i32(0); // clan crest large id
    w.write_u8(0); // noble
    w.write_u8(p.hero_aura as u8); // hero (isHero || (isGM && GMHeroAura))
    w.write_u8(0); // fishing
    w.write_i32(0); // bait x
    w.write_i32(0); // bait y
    w.write_i32(0); // bait z
    w.write_i32(p.name_color); // name color (from access level)
    w.write_i32(pos.heading);
    w.write_u8(p.pledge_class); // clan rank → on-head crown (calculatePledgeClass)
    w.write_i16(0); // pledge type
    w.write_i32(p.title_color); // title color (from access level)
    w.write_u8(0); // cursed weapon level
    w.write_i32(0); // clan reputation score
    w.write_i32(p.transform_display_id); // transformation display id
    w.write_i32(0); // agathion id
    w.write_u8(0); // nPvPRestrainStatus
    w.write_i32(pvitals.cur_cp.round() as i32);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_hp.round() as i32);
    w.write_i32(vitals.max_mp);
    w.write_i32(vitals.cur_mp.round() as i32);
    w.write_u8(0); // cBRLectureMark
                   // `CharInfo`: the abnormal-visual list everyone nearby sees on this
                   // character — the stun swirl, poison tint, silence mark. Java also appends
                   // STEALTH here when a GM sees through invisibility (`_gmSeeInvis`), which
                   // this port handles on the self-only Ex packet instead.
    w.write_i32(visuals.len() as i32);
    for &id in visuals {
        w.write_i16(id);
    }
    w.write_u8(0); // true hero (100 when true)
    w.write_u8(1); // hair accessory enabled
    w.write_u8(0); // used ability points
    w.into_bytes()
}
