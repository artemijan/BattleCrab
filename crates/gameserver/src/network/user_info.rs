//! Port of `serverpackets/UserInfo` — the masked, multi-block packet that tells
//! the client everything about its own character. We send **all 23 blocks**
//! (Java `new UserInfo(player)`), so the mask is `[0xFF, 0xFF, 0x7F]` and the
//! packet is self-consistent. Values not yet modeled (clan, elementals,
//! inventory limits) are written as their empty defaults — see per-block TODOs.

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::model::Player;

const OPCODE_USER_INFO: u8 = 0x32;

/// Sum of every `UserInfoType` block length (the fixed part, before name/title).
const FIXED_BLOCKS_SIZE: i32 = 372;

pub fn user_info(p: &Player, data: &GameData) -> Vec<u8> {
    let name_units = p.name.encode_utf16().count() as i32;
    let title_units = p.title.encode_utf16().count() as i32;
    let init_size = 5 + FIXED_BLOCKS_SIZE + name_units * 2 + title_units * 2;

    let mut w = PacketWriter::new();
    w.write_u8(OPCODE_USER_INFO);
    w.write_i32(p.object_id);
    w.write_i32(init_size);
    w.write_i16(23); // number of mask bits
    w.write_bytes(&[0xFF, 0xFF, 0x7F]); // all 23 blocks present

    // RELATION (4) — TODO(G9): party/clan relation.
    w.write_i32(0);

    // BASIC_INFO (16 + name*2)
    w.write_i16((16 + name_units * 2) as i16);
    w.write_sized_string(&p.name);
    w.write_u8(0); // isGM
    w.write_u8(p.race as u8);
    w.write_u8(p.is_female as u8);
    w.write_i32(p.base_class_id); // root class id
    w.write_i32(p.class_id);
    w.write_u8(p.level as u8);

    // BASE_STATS (18)
    w.write_i16(18);
    w.write_i16(p.str_ as i16);
    w.write_i16(p.dex as i16);
    w.write_i16(p.con as i16);
    w.write_i16(p.int_ as i16);
    w.write_i16(p.wit as i16);
    w.write_i16(p.men as i16);
    w.write_i16(0);
    w.write_i16(0);

    // MAX_HPCPMP (14)
    w.write_i16(14);
    w.write_i32(p.max_hp);
    w.write_i32(p.max_mp);
    w.write_i32(p.max_cp);

    // CURRENT_HPMPCP_EXP_SP (38)
    w.write_i16(38);
    w.write_i32(p.cur_hp.round() as i32);
    w.write_i32(p.cur_mp.round() as i32);
    w.write_i32(p.cur_cp.round() as i32);
    w.write_i64(p.sp);
    w.write_i64(p.exp);
    w.write_f64(p.exp_percent(data));

    // ENCHANTLEVEL (4) — TODO(G6): weapon/armor enchant from inventory.
    w.write_i16(4);
    w.write_u8(0);
    w.write_u8(0);

    // APPAREANCE (15)
    w.write_i16(15);
    w.write_i32(p.hair_style);
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_u8(1); // hair accessory enabled

    // STATUS (6)
    w.write_i16(6);
    w.write_u8(0); // mount type
    w.write_u8(0); // private store type
    w.write_u8(0); // dwarven craft / crafting
    w.write_u8(0);

    // STATS (56) — base values (TODO(G7): full combat-stat calc).
    w.write_i16(56);
    w.write_i16(20); // no weapon equipped (40 with weapon)
    w.write_i32(p.p_atk);
    w.write_i32(p.p_atk_spd);
    w.write_i32(p.p_def);
    w.write_i32(p.evasion);
    w.write_i32(p.accuracy);
    w.write_i32(p.crit_hit);
    w.write_i32(p.m_atk);
    w.write_i32(p.m_atk_spd);
    w.write_i32(p.p_atk_spd); // atk speed - 1 (client quirk)
    w.write_i32(p.magic_evasion);
    w.write_i32(p.m_def);
    w.write_i32(p.magic_accuracy);
    w.write_i32(p.m_crit_hit);

    // ELEMENTALS (14) — TODO(G6): attribute attack/defense.
    w.write_i16(14);
    for _ in 0..6 {
        w.write_i16(0);
    }

    // POSITION (18)
    w.write_i16(18);
    w.write_i32(p.x);
    w.write_i32(p.y);
    w.write_i32(p.z);
    w.write_i32(0); // vehicle object id

    // SPEED (18)
    w.write_i16(18);
    w.write_i16(p.run_spd as i16);
    w.write_i16(p.walk_spd as i16);
    w.write_i16(p.swim_run_spd as i16);
    w.write_i16(p.swim_walk_spd as i16);
    w.write_i16(0); // fly run
    w.write_i16(0); // fly walk
    w.write_i16(0); // fly run (mount)
    w.write_i16(0); // fly walk (mount)

    // MULTIPLIER (18)
    w.write_i16(18);
    w.write_f64(p.move_multiplier);
    w.write_f64(1.0); // attack speed multiplier

    // COL_RADIUS_HEIGHT (18)
    w.write_i16(18);
    w.write_f64(p.collision_radius);
    w.write_f64(p.collision_height);

    // ATK_ELEMENTAL (5)
    w.write_i16(5);
    w.write_u8(0);
    w.write_i16(0);

    // CLAN (32 + title*2) — TODO(G9): clan/ally.
    w.write_i16((32 + title_units * 2) as i16);
    w.write_sized_string(&p.title);
    w.write_i16(0); // pledge type
    w.write_i32(0); // clan id
    w.write_i32(0); // clan crest large
    w.write_i32(0); // clan crest
    w.write_i32(0); // clan privileges
    w.write_u8(0); // is clan leader
    w.write_i32(0); // ally id
    w.write_i32(0); // ally crest
    w.write_u8(0); // in matching room

    // SOCIAL (22)
    w.write_i16(22);
    w.write_u8(0); // pvp flag
    w.write_i32(p.reputation);
    w.write_u8(0); // noble
    w.write_u8(0); // hero
    w.write_u8(0); // pledge class
    w.write_i32(p.pk_kills);
    w.write_i32(p.pvp_kills);
    w.write_i16(0); // recom left
    w.write_i16(0); // recom have

    // VITA_FAME (15)
    w.write_i16(15);
    w.write_i32(p.vitality_points);
    w.write_u8(0); // vita bonus
    w.write_i32(p.fame);
    w.write_i32(0); // raidboss points

    // SLOTS (9) — TODO(G6): talisman/brooch slots from inventory.
    w.write_i16(9);
    for _ in 0..7 {
        w.write_u8(0);
    }

    // MOVEMENTS (4)
    w.write_i16(4);
    w.write_u8(0); // 1 water, 2 flying, else 0
    w.write_u8(p.running as u8);

    // COLOR (10)
    w.write_i16(10);
    w.write_i32(0xFFFFFF); // name color
    w.write_i32(0xFFFF77); // title color

    // INVENTORY_LIMIT (9) — TODO(G6): real inventory limit.
    w.write_i16(9);
    w.write_i16(0);
    w.write_i16(0);
    w.write_i16(80); // inventory slot limit
    w.write_u8(0);

    // TRUE_HERO (9)
    w.write_i16(9);
    w.write_i32(0);
    w.write_i16(0);
    w.write_u8(0);

    w.into_bytes()
}
