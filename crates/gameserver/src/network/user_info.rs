//! Port of `serverpackets/UserInfo` — the masked, multi-block packet that tells
//! the client everything about its own character. We send **all 23
//! `UserInfoType` blocks** (Java `new UserInfo(player)`), so the mask is
//! `[0xFF, 0xFF, 0xFE]` (reversed bit order, see `masks`) and the packet is
//! self-consistent. Values not yet modeled (clan, elementals, inventory
//! limits) are written as their empty defaults — see per-block TODOs.

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::enums::UserInfoType;
use crate::model::inventory::PaperdollSlot;
use crate::network::masks::build_mask;
use crate::network::server_packets::opcodes;

const OPCODE_USER_INFO: u8 = 0x32;

/// `AbnormalVisualEffect.STEALTH.getClientId()` — the translucent GM-invisible
/// glow the client renders on the character.
const STEALTH_CLIENT_ID: i16 = 21;

/// Port of `serverpackets/ExUserInfoAbnormalVisualEffect`. The GM-invisibility
/// STEALTH effect is the only abnormal visual we model, so the effect list is
/// `[STEALTH]` when invisible and empty otherwise (Java appends STEALTH to the
/// real effect set whenever `isInvisible()`). Sent to the GM's own client so
/// the invisible state is actually shown on the character.
pub fn ex_user_info_abnormal_visual_effect(object_id: i32, invisible: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT);
    w.write_i32(object_id);
    w.write_i32(0); // transformation id
    w.write_i32(if invisible { 1 } else { 0 }); // effect count
    if invisible {
        w.write_i16(STEALTH_CLIENT_ID);
    }
    w.into_bytes()
}

pub fn user_info(v: &crate::model::PlayerView, data: &GameData, cfg: &crate::config::CharacterConfig, relation: i32) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, pvitals, base, speeds, collision, combat, inventory, .. } = v;
    let name_units = p.name.encode_utf16().count() as i32;
    let title_units = p.title.encode_utf16().count() as i32;

    // Java `calcBlockSize`: 5 header bytes + every block's length, where
    // BASIC_INFO and CLAN additionally carry the name/title UTF-16 bytes.
    let init_size: i32 = 5
        + UserInfoType::VALUES
            .iter()
            .map(|t| {
                t.block_length()
                    + match t {
                        UserInfoType::BasicInfo => name_units * 2,
                        UserInfoType::Clan => title_units * 2,
                        _ => 0,
                    }
            })
            .sum::<i32>();

    let mut w = PacketWriter::new();
    w.write_u8(OPCODE_USER_INFO);
    w.write_i32(p.object_id);
    w.write_i32(init_size);
    w.write_i16(UserInfoType::VALUES.len() as i16); // number of mask bits
    w.write_bytes(&build_mask::<3>(UserInfoType::VALUES.iter().map(|t| t.mask())));

    // RELATION — party/clan bitmask (Java `calculateRelation`); the caller
    // computes it via `game_loop::party::calculate_relation`. Siege (0x80) is
    // unported and stays clear.
    w.write_i32(relation);

    // BASIC_INFO (+ name*2)
    w.write_i16((UserInfoType::BasicInfo.block_length() + name_units * 2) as i16);
    w.write_sized_string(&p.name);
    w.write_u8(p.is_gm(data) as u8); // isGM — enables the client's `//command` bar
    w.write_u8(p.race as u8);
    w.write_u8(p.is_female as u8);
    w.write_i32(p.base_class_id); // root class id
    w.write_i32(p.class_id);
    w.write_u8(p.level as u8);

    // BASE_STATS
    w.write_i16(UserInfoType::BaseStats.block_length() as i16);
    w.write_i16(base.str_ as i16);
    w.write_i16(base.dex as i16);
    w.write_i16(base.con as i16);
    w.write_i16(base.int_ as i16);
    w.write_i16(base.wit as i16);
    w.write_i16(base.men as i16);
    w.write_i16(0);
    w.write_i16(0);

    // MAX_HPCPMP
    w.write_i16(UserInfoType::MaxHpCpMp.block_length() as i16);
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.max_mp);
    w.write_i32(pvitals.max_cp);

    // CURRENT_HPMPCP_EXP_SP
    w.write_i16(UserInfoType::CurrentHpMpCpExpSp.block_length() as i16);
    w.write_i32(vitals.cur_hp.round() as i32);
    w.write_i32(vitals.cur_mp.round() as i32);
    w.write_i32(pvitals.cur_cp.round() as i32);
    w.write_i64(p.sp);
    w.write_i64(p.exp);
    w.write_f64(p.exp_percent(data));

    // ENCHANTLEVEL — weapon enchant (R-hand). The armor min-enchant byte stays
    // 0 until ArmorSetData is ported: Java's `getArmorMinEnchant` is the
    // paperdoll cache's max *set* enchant, which is 0 with no recognized set.
    w.write_i16(UserInfoType::EnchantLevel.block_length() as i16);
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8);
    w.write_u8(0);

    // APPAREANCE (sic)
    w.write_i16(UserInfoType::Appearance.block_length() as i16);
    w.write_i32(p.hair_style);
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_u8(1); // hair accessory enabled

    // STATUS
    w.write_i16(UserInfoType::Status.block_length() as i16);
    w.write_u8(p.mount_type); // mount type (0 none, 1 strider, 2 wyvern, 3 wolf)
    w.write_u8(0); // private store type
    w.write_u8(0); // dwarven craft / crafting
    w.write_u8(0);

    // STATS — base values (TODO(G7): full combat-stat calc).
    w.write_i16(UserInfoType::Stats.block_length() as i16);
    w.write_i16(20); // no weapon equipped (40 with weapon)
    w.write_i32(combat.p_atk as i32);
    w.write_i32(combat.p_atk_spd);
    w.write_i32(combat.p_def as i32);
    w.write_i32(combat.evasion);
    w.write_i32(combat.accuracy);
    w.write_i32(combat.crit_hit as i32);
    w.write_i32(combat.m_atk as i32);
    w.write_i32(combat.m_atk_spd);
    w.write_i32(combat.p_atk_spd); // atk speed - 1 (client quirk)
    w.write_i32(combat.magic_evasion);
    w.write_i32(combat.m_def as i32);
    w.write_i32(combat.magic_accuracy);
    w.write_i32(combat.m_crit_hit as i32);

    // ELEMENTALS — TODO(G6): attribute attack/defense.
    w.write_i16(UserInfoType::Elementals.block_length() as i16);
    for _ in 0..6 {
        w.write_i16(0);
    }

    // POSITION
    w.write_i16(UserInfoType::Position.block_length() as i16);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(0); // vehicle object id

    // SPEED
    w.write_i16(UserInfoType::Speed.block_length() as i16);
    w.write_i16(speeds.run_spd as i16);
    w.write_i16(speeds.walk_spd as i16);
    w.write_i16(speeds.swim_run_spd as i16);
    w.write_i16(speeds.swim_walk_spd as i16);
    w.write_i16(0); // fly run
    w.write_i16(0); // fly walk
    w.write_i16(0); // fly run (mount)
    w.write_i16(0); // fly walk (mount)

    // MULTIPLIER
    w.write_i16(UserInfoType::Multiplier.block_length() as i16);
    w.write_f64(speeds.move_multiplier);
    w.write_f64(1.0); // attack speed multiplier

    // COL_RADIUS_HEIGHT
    w.write_i16(UserInfoType::ColRadiusHeight.block_length() as i16);
    w.write_f64(collision.radius);
    w.write_f64(collision.height);

    // ATK_ELEMENTAL
    w.write_i16(UserInfoType::AtkElemental.block_length() as i16);
    w.write_u8(0);
    w.write_i16(0);

    // CLAN (+ title*2) — id/privileges/leader real as of G11; crests and
    // ally wait for their systems.
    w.write_i16((UserInfoType::Clan.block_length() + title_units * 2) as i16);
    w.write_sized_string(&p.title);
    w.write_i16(0); // pledge type (main clan)
    w.write_i32(p.clan_id);
    w.write_i32(0); // clan crest large
    w.write_i32(0); // clan crest
    w.write_i32(p.clan_privs);
    w.write_u8(p.clan_leader as u8);
    w.write_i32(0); // ally id
    w.write_i32(0); // ally crest
    w.write_u8(0); // in matching room

    // SOCIAL
    w.write_i16(UserInfoType::Social.block_length() as i16);
    w.write_u8(v.pvp_flag); // pvp flag
    w.write_i32(p.reputation);
    w.write_u8(0); // noble
    w.write_u8(p.hero_aura as u8); // hero (isHero || (isGM && GMHeroAura))
    w.write_u8(0); // pledge class
    w.write_i32(p.pk_kills);
    w.write_i32(p.pvp_kills);
    w.write_i16(0); // recom left
    w.write_i16(0); // recom have

    // VITA_FAME
    w.write_i16(UserInfoType::VitaFame.block_length() as i16);
    w.write_i32(p.vitality_points);
    w.write_u8(0); // vita bonus
    w.write_i32(p.fame);
    w.write_i32(0); // raidboss points

    // SLOTS — TODO(G6): talisman/brooch slots from inventory.
    w.write_i16(UserInfoType::Slots.block_length() as i16);
    for _ in 0..7 {
        w.write_u8(0);
    }

    // MOVEMENTS
    w.write_i16(UserInfoType::Movements.block_length() as i16);
    w.write_u8(0); // 1 water, 2 flying, else 0
    w.write_u8(speeds.running as u8);

    // COLOR
    w.write_i16(UserInfoType::Color.block_length() as i16);
    w.write_i32(v.p.name_color); // name color (from access level)
    w.write_i32(v.p.title_color); // title color (from access level)

    // INVENTORY_LIMIT
    w.write_i16(UserInfoType::InventoryLimit.block_length() as i16);
    w.write_i16(0);
    w.write_i16(0);
    w.write_i16(cfg.inventory_limit(p.race) as i16);
    w.write_u8(0);

    // TRUE_HERO
    w.write_i16(UserInfoType::TrueHero.block_length() as i16);
    w.write_i32(0);
    w.write_i16(0);
    w.write_u8(0);

    w.into_bytes()
}
