//! Port of `serverpackets/UserInfo` — the masked, multi-block packet that tells
//! the client everything about its own character. We send **all 23
//! `UserInfoType` blocks** (Java `new UserInfo(player)`), so the mask is
//! `[0xFF, 0xFF, 0xFE]` (reversed bit order, see `masks`) and the packet is
//! self-consistent. Clan (id, both crests, pledge type/class) and the
//! inventory limits are real; the one block still written as its empty default
//! is the **elemental** attack/defence attribute, which no Interlude item or
//! skill sets. The armor-set min-enchant byte is likewise 0 — see the note at
//! the ENCHANTLEVEL block, which now carries the worn armor sets' enchant
//! floor (`game_loop::armor_sets`).

use commons::network::PacketWriter;

use crate::data::GameData;
use crate::enums::UserInfoType;
use crate::model::inventory::PaperdollSlot;
use crate::network::masks::build_mask;
use crate::network::server_packets::opcodes;

const OPCODE_USER_INFO: u8 = 0x32;

use crate::model::skill::STEALTH_CLIENT_ID;

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

/// `CommonSkill.CREATE_DWARVEN` — the Dwarven "Create Item" ability.
const CREATE_DWARVEN_SKILL_ID: i32 = 172;
/// Crystallize; Java ORs it in so a non-Dwarf who can crystallize also gets the
/// window.
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
    w.write_i16(UserInfoType::VALUES.len() as i16); // number of mask bits
    w.write_bytes(&build_mask::<3>(
        UserInfoType::VALUES.iter().map(|t| t.mask()),
    ));

    // RELATION — party/clan bitmask (Java `calculateRelation`); the caller
    // computes it via `game_loop::player_info::calculate_relation`. Siege (0x80) is
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

    // ENCHANTLEVEL — weapon enchant (R-hand) then Java's `getArmorMinEnchant()`:
    // the best enchant *floor* across the worn armor sets. The name reads
    // backwards — it is a max over each set's minimum piece — and it is what
    // draws the +6 set glow.
    w.write_i16(UserInfoType::EnchantLevel.block_length() as i16);
    w.write_u8(inventory.paperdoll_enchant_level(PaperdollSlot::RHand) as u8);
    w.write_u8(
        crate::game_loop::armor_sets::max_set_enchant_for(&data.armor_sets, inventory)
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
    // Java: `hasDwarvenCraft() || getSkillLevel(248) > 0` — i.e. Create Item
    // (172) or Crystallize (248). **This byte is what opens the client's
    // create-item window**, so hard-coding it 0 left the whole (ported)
    // crafting subsystem unreachable from the UI.
    let can_craft = [CREATE_DWARVEN_SKILL_ID, CRYSTALLIZE_SKILL_ID]
        .iter()
        .any(|id| skills.0.get(id).is_some_and(|&lvl| lvl > 0));
    w.write_u8(can_craft as u8);
    w.write_u8(0);

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

    // ELEMENTALS — six zeros, which is **Java's own block**: its writer emits
    // `writeShort(0)` six times rather than reading the attribute stats. The
    // port models the attributes themselves (`Stat::FirePower`…), so this is a
    // deliberate match, not a gap.
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

    // ATK_ELEMENTAL
    w.write_i16(UserInfoType::AtkElemental.block_length() as i16);
    w.write_u8(0);
    w.write_i16(0);

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

    // SLOTS. Talisman and brooch-jewel slots are 0 because nothing in this
    // datapack grants `talismanSlots`/`broochJewels` — both are post-Interlude
    // stats, so Java's `getTalismanSlots()` would return 0 here too. Byte 3 is
    // the team aura (`//setteam`); the tail four stay Java's zeros.
    w.write_i16(UserInfoType::Slots.block_length() as i16);
    w.write_u8(0); // talisman slots
    w.write_u8(0); // brooch jewel slots
    w.write_u8(p.team);
    for _ in 0..4 {
        w.write_u8(0);
    }

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
    w.write_i16(crate::model::finalize(
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
