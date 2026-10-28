//! Port of `serverpackets/UserInfo` (protocol 140) — the masked, multi-block
//! packet that tells the client everything about its own character. We send
//! **all 23 `UserInfoType` blocks** (Java `new UserInfo(player)`), so the mask
//! is `[0xFF, 0xFF, 0xFE]` (reversed bit order, see `masks`). The values Rust
//! cannot fill yet are the ones whose systems are unported: LUC/CHA (class
//! templates, S5), the talisman/brooch/agathion slot counts, and item
//! attributes, which leave the element blocks to the stat modifiers alone.

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::enums::UserInfoType;
use crate::model::inventory::PaperdollSlot;
use crate::model::stat_finalize::finalize;
use crate::model::stats::Element;
use crate::network::masks::build_mask;
use crate::network::server_packets::opcodes;

pub(crate) const OPCODE_USER_INFO: u8 = 0x32;

use crate::model::skill::abnormal::STEALTH_CLIENT_ID;

/// Port of `serverpackets/ExUserInfoAbnormalVisualEffect`. Carries the
/// transformation id (so the GM sees their own transformed model — UserInfo has
/// no transform field) and the abnormal-effect list, of which STEALTH
/// (GM invisibility) is the only one we model (Java appends STEALTH whenever
/// `isInvisible()`). Sent to the player's own client.
pub fn ex_user_info_abnormal_visual_effect(
    object_id: i32,
    invisible: bool,
    transform_display_id: i32,
    visuals: &[i16],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT);
    w.write_i32(object_id);
    w.write_i32(transform_display_id); // transformation id
    // The buff-driven visuals, plus STEALTH when GM-invisible (Java appends it
    // to the set rather than sending it alone).
    let extra = usize::from(invisible && !visuals.contains(&STEALTH_CLIENT_ID));
    w.write_i32((visuals.len() + extra) as i32);
    for &id in visuals {
        w.write_i16(id);
    }
    if extra == 1 {
        w.write_i16(STEALTH_CLIENT_ID);
    }
    w.into_bytes()
}

/// `CommonSkill.CRYSTALLIZE`.
const CRYSTALLIZE_SKILL_ID: i32 = 248;

pub fn user_info(
    v: &crate::model::PlayerView,
    data: &GameData,
    cfg: &crate::config::CharacterConfig,
    relation: i32,
) -> Vec<u8> {
    let crate::model::PlayerView {
        skills,
        p,
        pos,
        vitals,
        pvitals,
        base,
        speeds,
        collision,
        combat,
        inventory,
        ..
    } = v;
    let name_units = p.name.encode_utf16().count() as i32;
    let title_units = p.title.encode_utf16().count() as i32;

    // Java `calcBlockSize`: 5 header bytes + every block's length, where
    // BASIC_INFO and CLAN additionally carry the name/title UTF-16 bytes.
    let init_size: i32 = 5 + UserInfoType::VALUES
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

    // UserInfo carries the full masked stat block — sized up front so the
    // build never reallocates.
    let mut w = PacketWriter::with_capacity(512);
    w.write_u8(OPCODE_USER_INFO);
    w.write_i32(p.object_id);
    w.write_i32(init_size);
    // The mask-bit count: Salvation hardcodes 24 though `UserInfoType` still
    // has 23 entries in three mask bytes.
    w.write_i16(24);
    w.write_bytes(&build_mask::<3>(
        UserInfoType::VALUES.iter().map(|t| t.mask()),
    ));

    // RELATION — party/clan bitmask (Java `calculateRelation`); the caller
    // computes it via `game_loop::character::player_info::calculate_relation`. Siege (0x80) is
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
    w.write_i16(0); // LUC: the class templates' `baseLUC` arrives in S5
    w.write_i16(0); // CHA: likewise `baseCHA`

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

    // ENCHANTLEVEL — weapon enchant (R-hand) then Java's `getArmorMinEnchant()`:
    // the best enchant *floor* across the worn armor sets. The name reads
    // backwards — it is a max over each set's minimum piece — and it is what
    // draws the +6 set glow.
    w.write_i16(UserInfoType::EnchantLevel.block_length() as i16);
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8);
    w.write_u8(
        crate::game_loop::items::armor_sets::max_set_enchant_for(&data.armor_sets, inventory)
            .clamp(0, u8::MAX as i32) as u8,
    );

    // APPAREANCE (sic)
    w.write_i16(UserInfoType::Appearance.block_length() as i16);
    w.write_i32(p.hair_style);
    w.write_i32(p.hair_color);
    w.write_i32(p.face);
    w.write_u8(1); // hair accessory enabled

    // STATUS
    w.write_i16(UserInfoType::Status.block_length() as i16);
    w.write_u8(p.mount_type); // mount type (0 none, 1 strider, 2 wyvern, 3 wolf)
    w.write_u8(p.store_type); // private store type
    // Java: `getCrystallizeGrade() != ItemGrade.NONE`. The grade is set by the
    // `Crystallize` effect of the passive skill 248, every level of which
    // grants one, so it is "knows Crystallize".
    let can_crystallize = skills
        .0
        .get(&CRYSTALLIZE_SKILL_ID)
        .is_some_and(|&lvl| lvl > 0);
    w.write_u8(can_crystallize as u8);
    // `getAbilityPoints() - getAbilityPointsUsed()`: one point per level past
    // 84. Nothing spends them yet, so the used count is Java's default 0.
    w.write_u8((p.level - 84).max(0) as u8);

    // STATS — the finalized combat stats.
    //
    // The leading short is Java's `getActiveWeaponItem() != null ? 40 : 20`:
    // the character's physical attack range, which the client uses to decide
    // how close to walk before swinging. It was hard-coded to the **unarmed**
    // 20, so an armed player reported the shorter reach.
    w.write_i16(UserInfoType::Stats.block_length() as i16);
    let armed = inventory.paperdoll_item_id(PaperdollSlot::RHand) != 0;
    w.write_i16(if armed { 40 } else { 20 });
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

    // ELEMENTALS — `getDefenseElementValue` for the six attributes in client
    // order. Players have no template base, and no item attributes are ported,
    // so the value is the stat modifiers alone.
    w.write_i16(UserInfoType::Elementals.block_length() as i16);
    for element in Element::ALL {
        w.write_i16(finalize(v.mods, element.attribute_stat(true), 0.0) as i32 as i16);
    }

    // POSITION
    w.write_i16(UserInfoType::Position.block_length() as i16);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_i32(0); // vehicle object id

    // SPEED — sent divided by the move multiplier (Java `_runSpd = round(
    // getRunSpeed() / _moveMultiplier)`); the client multiplies it back.
    w.write_i16(UserInfoType::Speed.block_length() as i16);
    let client_speeds = speeds.client_speed_fields();
    for spd in client_speeds {
        w.write_i16(spd);
    }
    w.write_i16(0); // _flRunSpd (never set in Java)
    w.write_i16(0); // _flWalkSpd (never set in Java)
    // `_flyRunSpd/_flyWalkSpd = isFlying() ? run/walk : 0` — a wyvern rider's
    // client uses these for the flight animation; zeros froze it mid-air.
    let (fly_run, fly_walk) = if p.is_flying() {
        (client_speeds[0], client_speeds[1])
    } else {
        (0, 0)
    };
    w.write_i16(fly_run);
    w.write_i16(fly_walk);

    // MULTIPLIER
    w.write_i16(UserInfoType::Multiplier.block_length() as i16);
    w.write_f64(speeds.client_move_multiplier()); // Java getMovementSpeedMultiplier (leg-anim rate)
    w.write_f64(combat.client_atk_speed_multiplier()); // Java getAttackSpeedMultiplier (swing-anim rate)

    // COL_RADIUS_HEIGHT
    w.write_i16(UserInfoType::ColRadiusHeight.block_length() as i16);
    w.write_f64(collision.radius);
    w.write_f64(collision.height);

    // ATK_ELEMENTAL — `CreatureStat.getAttackElement`: the weapon's own
    // attribute (none are ported), else the first attribute whose power is the
    // strictly highest above 0; NONE (client id -2) when there is none.
    w.write_i16(UserInfoType::AtkElemental.block_length() as i16);
    let (attack_element, attack_value) = Element::ALL
        .iter()
        .map(|&el| (el, finalize(v.mods, el.attribute_stat(false), 0.0) as i32))
        .fold((None, 0), |(best, top), (el, value)| {
            if value > top {
                (Some(el), value)
            } else {
                (best, top)
            }
        });
    w.write_u8(attack_element.map_or(-2i8, |el| el.index() as i8) as u8);
    w.write_i16(attack_value as i16);

    // CLAN (+ title*2). The large crest is mirrored onto the player at
    // enter-world (and on every crest change), because this builder has no
    // access to `World.clans` — the same reason `clan_crest_id` lives there.
    w.write_i16((UserInfoType::Clan.block_length() + title_units * 2) as i16);
    w.write_sized_string(&p.title);
    w.write_i16(p.pledge_type as i16); // Java getPledgeType (0 = main clan)
    w.write_i32(p.visible_clan_id());
    w.write_i32(p.visible_clan_crest_large_id());
    w.write_i32(p.visible_clan_crest_id()); // Java getClanCrestId
    w.write_i32(p.clan_privs);
    w.write_u8(p.clan_leader as u8);
    w.write_i32(p.visible_ally_id());
    w.write_i32(p.visible_ally_crest_id());
    w.write_u8(v.in_matching_room as u8); // isInMatchingRoom (G30)

    // SOCIAL
    w.write_i16(UserInfoType::Social.block_length() as i16);
    w.write_u8(v.pvp_flag); // pvp flag
    w.write_i32(p.reputation);
    w.write_u8(p.is_noble as u8); // noble (Java isNoble) — the nobless sparkle
    w.write_u8(p.hero_aura as u8); // hero (isHero || (isGM && GMHeroAura))
    w.write_u8(p.pledge_class); // clan rank → on-head crown (calculatePledgeClass)
    w.write_i32(p.pk_kills);
    w.write_i32(p.pvp_kills);
    w.write_i16(p.rec_left as i16); // recom left
    w.write_i16(p.rec_have as i16); // recom have

    // VITA_FAME
    w.write_i16(UserInfoType::VitaFame.block_length() as i16);
    w.write_i32(p.vitality_points);
    w.write_u8(0); // vita bonus
    w.write_i32(p.fame);
    w.write_i32(p.raidboss_points);

    // SLOTS, 11 bytes in protocol 140 (the init size above still counts
    // `UserInfoType`'s 9, as Java's does). The talisman, brooch-jewel and
    // agathion slot counts are stats Rust does not model yet (S5/S10), so
    // they read 0; byte 3 is the team aura (`//setteam`).
    w.write_i16(11);
    w.write_u8(0); // talisman slots
    w.write_u8(0); // brooch jewel slots
    w.write_u8(p.team);
    w.write_i32(0);
    w.write_u8(0); // charm slots: 1 when the agathion slot count is above 0
    w.write_u8(0); // agathion slots - 1

    // MOVEMENTS
    w.write_i16(UserInfoType::Movements.block_length() as i16);
    // Java: `insideZone(WATER) ? 1 : isFlyingMounted() ? 2 : 0`.
    //
    // The `2` arm stays unreachable on purpose: `isFlyingMounted()` is
    // *transform*-based (Gracia sky mounts), and a wyvern rider is
    // `isFlying()` but **not** `isFlyingMounted()`, so Java writes 0 for them
    // here too — their flight renders from the mount npc id and the fly speeds.
    w.write_u8(u8::from(v.in_water));
    w.write_u8(speeds.running as u8);

    // COLOR
    w.write_i16(UserInfoType::Color.block_length() as i16);
    w.write_i32(v.p.name_color); // name color (from access level)
    w.write_i32(v.p.title_color); // title color (from access level)

    // INVENTORY_LIMIT
    w.write_i16(UserInfoType::InventoryLimit.block_length() as i16);
    w.write_i16(0);
    w.write_i16(0);
    w.write_i16(crate::model::stat_finalize::finalize(
        v.mods,
        crate::model::stats::Stat::InventoryNormal,
        cfg.inventory_limit_for(p.race, p.is_gm(data)) as f64,
    ) as i16);
    // Java: `isCursedWeaponEquipped() ? getLevel(cursedWeaponEquippedId) : 0` —
    // the wielder's stage, which is what colours the name in the client.
    w.write_u8(v.cursed_weapon_level);

    // TRUE_HERO — Java `isTrueHero() ? 100 : 0`, a hero flag of its own
    // (`//settruehero`), independent of the SOCIAL glow byte above.
    w.write_i16(UserInfoType::TrueHero.block_length() as i16);
    w.write_i32(0);
    w.write_i16(0);
    w.write_u8(if p.true_hero { 100 } else { 0 });

    w.into_bytes()
}
