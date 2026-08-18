//! The `RequestGMCommand` (0x7E) answer packets — a GM inspecting a player's
//! stats, clan, skills, quests, inventory or warehouse.
//!
//! Java has seven; two were already ported for their own admin commands
//! (`GMViewItemList` in `enter_world`, `GMViewPledgeInfo` in `clan`), and the
//! five here complete the set. They are read-only dumps: nothing in this file
//! changes state, and the GM's own client renders whichever window it asked
//! for.
//!
//! Java's `EX_GM_VIEW_CHARACTER_INFO` (FE:0x155) is **not** ported — an
//! extended opcode far past anything an Interlude client parses. Only the base
//! `GM_VIEW_CHARACTER_INFO` (0x95) below is reachable here.

use commons::network::PacketWriter;

use super::{PAPERDOLL_ORDER, opcodes};
use crate::data::GameData;
use crate::model::PlayerView;
use crate::model::inventory::ItemInstance;

/// Port of `serverpackets/GMViewCharacterInfo` (0x95) — the GM's read-only
/// twin of `UserInfo`, flat rather than block-masked.
///
/// `exp_percent` is Java's "High Five exp %": how far into the current level
/// the character is, as a fraction. It is written as a **double** even though
/// Java casts the numerator to float first.
pub fn gm_view_character_info(
    v: &PlayerView,
    exp_percent: f64,
    load: (i64, i32),
    is_gm: bool,
) -> Vec<u8> {
    let PlayerView {
        p,
        pos,
        vitals,
        pvitals,
        base,
        speeds,
        collision,
        combat,
        inventory,
        skills,
        ..
    } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_VIEW_CHARACTER_INFO);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(pos.heading);
    w.write_i32(p.object_id);
    w.write_string(&p.name);
    w.write_i32(p.race);
    w.write_i32(i32::from(p.is_female));
    w.write_i32(p.class_id);
    w.write_i32(p.level);
    w.write_i64(p.exp);
    w.write_f64(exp_percent);
    w.write_i32(base.str_);
    w.write_i32(base.dex);
    w.write_i32(base.con);
    w.write_i32(base.int_);
    w.write_i32(base.wit);
    w.write_i32(base.men);
    w.write_i32(0); // LUC
    w.write_i32(0); // CHA
    w.write_i32(vitals.max_hp);
    w.write_i32(vitals.cur_hp as i32);
    w.write_i32(vitals.max_mp);
    w.write_i32(vitals.cur_mp as i32);
    w.write_i64(p.sp);
    w.write_i32(load.0 as i32);
    w.write_i32(load.1);
    w.write_i32(p.pk_kills);
    for slot in PAPERDOLL_ORDER {
        w.write_i32(inventory.paperdoll_object_id(slot));
    }
    for slot in PAPERDOLL_ORDER {
        w.write_i32(inventory.paperdoll_item_id(slot));
    }
    for slot in PAPERDOLL_ORDER {
        // `getPaperdollAugmentation` — both option ids, 0 when unaugmented.
        let (o1, o2) = inventory.paperdoll_augmentation(slot).unwrap_or((0, 0));
        w.write_i32(o1);
        w.write_i32(o2);
    }
    w.write_u8(0); // talisman slots (CT2.3)
    w.write_u8(0); // canEquipCloak (CT2.3)
    w.write_i32(0);
    w.write_i16(0);
    w.write_i32(combat.p_atk as i32);
    w.write_i32(combat.p_atk_spd);
    w.write_i32(combat.p_def as i32);
    w.write_i32(combat.evasion);
    w.write_i32(combat.accuracy);
    w.write_i32(combat.crit_hit as i32);
    w.write_i32(combat.m_atk as i32);
    w.write_i32(combat.m_atk_spd);
    w.write_i32(combat.p_atk_spd); // Java writes the physical speed again here
    w.write_i32(combat.m_def as i32);
    w.write_i32(combat.magic_evasion);
    w.write_i32(combat.magic_accuracy);
    w.write_i32(combat.m_crit_hit as i32);
    w.write_i32(i32::from(v.pvp_flag));
    w.write_i32(p.reputation);
    // `round(getRunSpeed() / _moveMultiplier)` and friends — the same four the
    // `UserInfo` SPEED block sends, just as ints.
    let sp = speeds.client_speed_fields();
    for v in sp {
        w.write_i32(i32::from(v));
    }
    // `_flyRunSpd`/`_flyWalkSpd`, written **twice**: once where the fly pair
    // belongs and again where Java repeats it. Zero unless actually flying.
    let (fly_run, fly_walk) = if p.is_flying() {
        (i32::from(sp[0]), i32::from(sp[1]))
    } else {
        (0, 0)
    };
    for _ in 0..2 {
        w.write_i32(fly_run);
        w.write_i32(fly_walk);
    }
    w.write_f64(speeds.client_move_multiplier());
    w.write_f64(combat.client_atk_speed_multiplier());
    w.write_f64(collision.radius);
    w.write_f64(collision.height);
    w.write_i32(p.hair_style);
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_i32(i32::from(is_gm)); // "builder level"
    w.write_string(&p.title);
    w.write_i32(p.clan_id);
    w.write_i32(p.clan_crest_id);
    w.write_i32(p.ally_id);
    w.write_u8(p.mount_type);
    w.write_u8(p.store_type);
    w.write_u8(u8::from(skills.0.contains_key(&172)));
    w.write_i32(p.pk_kills);
    w.write_i32(p.pvp_kills);
    w.write_i16(p.rec_left as i16);
    w.write_i16(p.rec_have as i16);
    w.write_i32(p.class_id);
    w.write_i32(0); // "special effects? circles around player..."
    w.write_i32(pvitals.max_cp);
    w.write_i32(pvitals.cur_cp as i32);
    w.write_u8(speeds.running as u8);
    w.write_u8(321u32 as u8); // Java's literal 321, truncated to a byte
    w.write_i32(i32::from(p.pledge_class));
    w.write_u8(u8::from(p.is_noble));
    w.write_u8(u8::from(p.is_hero));
    w.write_i32(p.name_color);
    w.write_i32(p.title_color);
    // Attribute attack/defence — no elemental system on this dist, so the
    // attack type is "none" (-2) and every defence is 0, matching `UserInfo`.
    w.write_i16(-2);
    w.write_i16(0);
    for _ in 0..6 {
        w.write_i16(0);
    }
    w.write_i32(p.fame);
    w.write_i32(p.vitality_points);
    w.write_i32(0);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/GMHennaInfo` (0xF0) — the target's dye totals and
/// the three slots.
///
/// `values` is `(INT, STR, CON, MEN, DEX, WIT)` in Java's write order; LUC and
/// CHA are post-Interlude and always 0.
pub fn gm_henna_info(values: (i16, i16, i16, i16, i16, i16), dye_ids: &[i32]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_HENNA_INFO);
    for v in [values.0, values.1, values.2, values.3, values.4, values.5] {
        w.write_i16(v);
    }
    w.write_i16(0); // LUC
    w.write_i16(0); // CHA
    w.write_i32(3); // slots
    w.write_i32(dye_ids.len() as i32);
    for &id in dye_ids {
        w.write_i32(id);
        w.write_i32(1);
    }
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/GMViewSkillInfo` (0x97).
///
/// `disabled` is Java's `clan != null && clan.getReputationScore() < 0`,
/// computed once for the whole list and applied only to clan skills — a clan
/// in reputation debt has its granted skills greyed out.
pub fn gm_view_skill_info(
    name: &str,
    skills: &[(i32, i32, bool, bool)],
    clan_disabled: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_VIEW_SKILL_INFO);
    w.write_string(name);
    w.write_i32(skills.len() as i32);
    for &(id, level, passive, is_clan_skill) in skills {
        w.write_i32(i32::from(passive));
        w.write_i16(level as i16);
        w.write_i16(0); // sub level
        w.write_i32(id);
        w.write_i32(0);
        w.write_u8(u8::from(clan_disabled && is_clan_skill));
        w.write_u8(0); // isEnchantable
    }
    w.into_bytes()
}

/// Port of `serverpackets/GmViewQuestInfo` (0x99) — every started quest and
/// its condition. The trailing short is Java's own unexplained `0`.
pub fn gm_view_quest_info(name: &str, quests: &[(i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_VIEW_QUEST_INFO);
    w.write_string(name);
    w.write_i16(quests.len() as i16);
    for &(id, cond) in quests {
        w.write_i32(id);
        w.write_i32(cond);
    }
    w.write_i16(0); // "some size"
    w.into_bytes()
}

/// Port of `serverpackets/GMViewWarehouseWithdrawList` (0x9B) — the target's
/// warehouse, with each entry's object id repeated after the item block (the
/// withdraw window addresses items by it).
pub fn gm_view_warehouse_withdraw_list(
    name: &str,
    adena: i64,
    items: &[ItemInstance],
    data: &GameData,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::GM_VIEW_WAREHOUSE_WITHDRAW_LIST);
    w.write_string(name);
    w.write_i64(adena);
    let listed: Vec<_> = items
        .iter()
        .filter_map(|i| data.item_data.get(i.item_id).map(|t| (i, t)))
        .collect();
    w.write_i16(listed.len() as i16);
    for (item, template) in listed {
        super::super::enter_world::write_item_entry(&mut w, item, template, false);
        w.write_i32(item.object_id);
    }
    w.into_bytes()
}
