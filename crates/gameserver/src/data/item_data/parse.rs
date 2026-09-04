//! Reading `data/stats/items/*.xml` into [`super::ItemTemplate`]s — the port
//! of Java's `DocumentItem`.

use super::kinds::{
    ActionType, ArmorType, CrystalType, EtcItemType, ItemHandler, ItemKind, WeaponType,
};
use super::template::{CapsuledItem, ItemStats, ItemTemplate, TradeFlags};
use super::{
    ADENA_ID, ANCIENT_ADENA_ID, SLOT_L_BRACELET, SLOT_L_EAR, SLOT_L_FINGER, SLOT_NECK,
    SLOT_R_BRACELET, TYPE1_ITEM_QUESTITEM_ADENA, TYPE1_SHIELD_ARMOR,
    TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_ACCESSORY, TYPE2_MONEY, TYPE2_OTHER, TYPE2_QUEST,
    TYPE2_SHIELD_ARMOR, TYPE2_WEAPON, slot_mask,
};
use crate::data::item_cond::{self, CondBuilder, ItemCondition};
use crate::data::xml;
use crate::data::xml::{attr_f64, attr_i32, attr_i64, attr_pairs, attr_str};
use crate::model::stats::Stat;
use quick_xml::events::Event;
use std::collections::HashMap;

/// `damage_range` is `a;b;radius;angle`; Java only reads it when all four parts
/// parse, otherwise falling back to 40/0.
fn damage_range_part(raw: Option<&String>, index: usize, fallback: i32) -> i32 {
    let Some(raw) = raw else { return fallback };
    let parts: Vec<&str> = raw.split(';').collect();
    if parts.len() < 4 {
        return fallback;
    }
    parts
        .get(index)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(fallback)
}

pub(super) fn parse_file(
    path: &std::path::Path,
    out: &mut HashMap<i32, ItemTemplate>,
    stats_out: &mut HashMap<i32, ItemStats>,
    armor_out: &mut HashMap<i32, ArmorType>,
    weapon_out: &mut HashMap<i32, WeaponType>,
    weapon_shots_out: &mut HashMap<i32, (i32, i32)>,
    icons_out: &mut HashMap<i32, String>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    let mut cur_id: Option<i32> = None;
    let mut cur_name = String::new();
    let mut cur_kind = ItemKind::Etc;
    let mut attrs: HashMap<String, String> = HashMap::new();
    let mut in_capsules = false;
    let mut cur_capsules: Vec<CapsuledItem> = Vec::new();
    let mut in_skills = false;
    let mut cur_item_skills: Vec<(i32, i32)> = Vec::new();
    let mut in_stats = false;
    let mut cur_stat_type: Option<String> = None;
    let mut cur_stats = ItemStats::default();
    let mut cur_conditions: Vec<ItemCondition> = Vec::new();
    let mut conds = CondBuilder::default();

    for event in xml::events(&content) {
        match event {
            Event::Start(e) if e.name().as_ref() == b"item" => {
                cur_id = attr_i32(&e, b"id");
                cur_name = attr_str(&e, b"name").unwrap_or_default();
                cur_kind = match attr_str(&e, b"type").as_deref() {
                    Some("Weapon") => ItemKind::Weapon,
                    Some("Armor") => ItemKind::Armor,
                    _ => ItemKind::Etc,
                };
                attrs.clear();
                cur_capsules.clear();
                cur_item_skills.clear();
                cur_conditions.clear();
                cur_stats = ItemStats::default();
            }
            Event::Start(e) if e.name().as_ref() == b"stats" => {
                in_stats = true;
            }
            Event::End(e) if e.name().as_ref() == b"stats" => {
                in_stats = false;
            }
            Event::Start(e) if in_stats && e.name().as_ref() == b"stat" => {
                cur_stat_type = attr_str(&e, b"type");
            }
            Event::End(e) if in_stats && e.name().as_ref() == b"stat" => {
                cur_stat_type = None;
            }
            Event::Text(t) if in_stats && cur_stat_type.is_some() => {
                let ty = cur_stat_type.as_deref().unwrap();
                if let Ok(text) = t.unescape()
                    && let Ok(val) = text.trim().parse::<f64>()
                {
                    match ty {
                        "pAtkRange" => cur_stats.atk_range = Some(val as i32),
                        "randomDamage" => cur_stats.random_damage = Some(val as i32),
                        "sDef" => cur_stats.shield_def = Some(val as i32),
                        "rShld" => cur_stats.shield_rate = Some(val as i32),
                        _ => {
                            if let Some(stat) = stat_from_xml(ty) {
                                cur_stats.bonuses.push((stat, val));
                            }
                        }
                    }
                }
            }
            Event::Empty(e) if e.name().as_ref() == b"set" => {
                if cur_id.is_none() {
                    continue;
                }
                if let (Some(name), Some(val)) = (attr_str(&e, b"name"), attr_str(&e, b"val")) {
                    attrs.insert(name, val);
                }
            }
            Event::Start(e) if e.name().as_ref() == b"capsuled_items" => {
                in_capsules = true;
            }
            Event::End(e) if e.name().as_ref() == b"capsuled_items" => {
                in_capsules = false;
            }
            Event::Empty(e) if in_capsules && e.name().as_ref() == b"item" => {
                if let (Some(item_id), Some(min), Some(max), Some(chance)) = (
                    attr_i32(&e, b"id"),
                    attr_i64(&e, b"min"),
                    attr_i64(&e, b"max"),
                    attr_f64(&e, b"chance"),
                ) {
                    cur_capsules.push(CapsuledItem {
                        item_id,
                        min,
                        max,
                        chance: (chance * 1000.0) as i32,
                    });
                }
            }
            Event::Start(e) if e.name().as_ref() == b"skills" => {
                in_skills = true;
            }
            Event::End(e) if e.name().as_ref() == b"skills" => {
                in_skills = false;
            }
            Event::Empty(e) if in_skills && e.name().as_ref() == b"skill" => {
                if let (Some(id), Some(level)) = (attr_i32(&e, b"id"), attr_i32(&e, b"level")) {
                    cur_item_skills.push((id, level));
                }
            }
            // `<cond msgId="113" addName="1"><and><player …/></and></cond>` —
            // Java `DocumentItem`'s `cond` arm. The block's message lives on
            // the `<cond>` element; the tree is assembled by [`CondBuilder`].
            Event::Start(e) if e.name().as_ref() == b"cond" => {
                conds.begin(item_cond::message_from(
                    attr_str(&e, b"msg"),
                    attr_str(&e, b"msgId").as_deref(),
                    attr_str(&e, b"addName").as_deref(),
                ));
            }
            Event::End(e) if e.name().as_ref() == b"cond" => {
                if let Some(condition) = conds.finish() {
                    cur_conditions.push(condition);
                }
            }
            Event::Start(e) if conds.is_open() => {
                match e.name().as_ref() {
                    // A `<player>`/`<target>` written with an explicit end tag
                    // rather than self-closed. None on this dist, but the two
                    // spellings are the same element.
                    b"player" => conds.push_leaf(item_cond::player_condition(&attr_pairs(&e))),
                    b"target" => conds.push_leaf(item_cond::target_condition(&attr_pairs(&e))),
                    name => conds.open_group(name),
                }
            }
            Event::Empty(e) if conds.is_open() => match e.name().as_ref() {
                b"player" => conds.push_leaf(item_cond::player_condition(&attr_pairs(&e))),
                b"target" => conds.push_leaf(item_cond::target_condition(&attr_pairs(&e))),
                _ => {}
            },
            Event::End(e) if conds.is_open() => {
                if !matches!(e.name().as_ref(), b"player" | b"target") {
                    conds.close_group(e.name().as_ref());
                }
            }
            Event::End(e) if e.name().as_ref() == b"item" => {
                if let Some(item_id) = cur_id.take() {
                    out.insert(
                        item_id,
                        make_template(
                            item_id,
                            std::mem::take(&mut cur_name),
                            cur_kind,
                            &attrs,
                            std::mem::take(&mut cur_capsules),
                            std::mem::take(&mut cur_item_skills),
                            std::mem::take(&mut cur_conditions),
                        ),
                    );
                    let stats = std::mem::take(&mut cur_stats);
                    if !stats.bonuses.is_empty()
                        || stats.atk_range.is_some()
                        || stats.random_damage.is_some()
                    {
                        stats_out.insert(item_id, stats);
                    }
                    if let Some(at) = attrs.get("armor_type").map(|s| ArmorType::from_name(s))
                        && at != ArmorType::None
                    {
                        armor_out.insert(item_id, at);
                    }
                    if let Some(wt) = attrs.get("weapon_type").map(|s| WeaponType::from_name(s))
                        && wt != WeaponType::None
                    {
                        weapon_out.insert(item_id, wt);
                    }
                    // `Weapon._soulShotCount`/`_spiritShotCount` — only weapons
                    // declaring a non-zero count can charge that shot kind.
                    let ss = attrs
                        .get("soulshots")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let sps = attrs
                        .get("spiritshots")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    if ss != 0 || sps != 0 {
                        weapon_shots_out.insert(item_id, (ss, sps));
                    }
                    if let Some(icon) = attrs.get("icon") {
                        icons_out.insert(item_id, icon.clone());
                    }
                }
            }
            _ => {}
        }
    }
}

fn make_template(
    item_id: i32,
    name: String,
    kind: ItemKind,
    attrs: &HashMap<String, String>,
    capsuled_items: Vec<CapsuledItem>,
    item_skills: Vec<(i32, i32)>,
    pre_conditions: Vec<ItemCondition>,
) -> ItemTemplate {
    let weight = attrs
        .get("weight")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let is_stackable = attrs
        .get("is_stackable")
        .map(|v| v == "true")
        .unwrap_or(false);
    let is_quest_item = attrs
        .get("is_questitem")
        .map(|v| v == "true")
        .unwrap_or(false);
    let is_infinite = attrs
        .get("is_infinite")
        .map(|v| v == "true")
        .unwrap_or(false);
    let part = slot_mask(attrs.get("bodypart").map(|s| s.as_str()).unwrap_or("none"));

    let (type1, type2) = match kind {
        ItemKind::Weapon => (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_WEAPON),
        ItemKind::Armor => {
            if part == SLOT_NECK
                || (part & SLOT_L_EAR) != 0
                || (part & SLOT_L_FINGER) != 0
                || (part & SLOT_R_BRACELET) != 0
                || (part & SLOT_L_BRACELET) != 0
            {
                (TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_ACCESSORY)
            } else {
                (TYPE1_SHIELD_ARMOR, TYPE2_SHIELD_ARMOR)
            }
        }
        ItemKind::Etc => {
            let type2 = if is_quest_item {
                TYPE2_QUEST
            } else if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
                TYPE2_MONEY
            } else {
                TYPE2_OTHER
            };
            (TYPE1_ITEM_QUESTITEM_ADENA, type2)
        }
    };

    let handler = match attrs.get("handler").map(|s| s.as_str()) {
        Some("ExtractableItems") => ItemHandler::ExtractableItems,
        Some("ItemSkills") | Some("ItemSkillsTemplate") => ItemHandler::ItemSkills,
        Some("Seed") => ItemHandler::Seed,
        Some("SoulShots") => ItemHandler::SoulShots,
        Some("SpiritShot") => ItemHandler::SpiritShot,
        Some("BlessedSpiritShot") => ItemHandler::BlessedSpiritShot,
        Some("EnchantScrolls") => ItemHandler::EnchantScrolls,
        Some("Recipes") => ItemHandler::Recipes,
        Some("BeastSoulShot") => ItemHandler::BeastSoulShot,
        Some("BeastSpiritShot") => ItemHandler::BeastSpiritShot,
        Some("FishShots") => ItemHandler::FishShots,
        Some("SummonItems") => ItemHandler::SummonItems,
        Some("Book") => ItemHandler::Book,
        Some("RollingDice") => ItemHandler::RollingDice,
        Some("PetFood") => ItemHandler::PetFood,
        Some("MercTicket") => ItemHandler::MercTicket,
        // `Elixir extends ItemSkills` and adds one guard — "not a pet" — which
        // the port gets for free: an etc-item use always arrives from a player's
        // own inventory. So it collapses onto `ItemSkills`, the same way
        // `ItemSkillsTemplate` does above.
        Some("Elixir") => ItemHandler::ItemSkills,
        _ => ItemHandler::None,
    };

    ItemTemplate {
        item_id,
        name,
        kind,
        crystal_type: CrystalType::from_name(attrs.get("crystal_type").map(|s| s.as_str())),
        attack_radius: damage_range_part(attrs.get("damage_range"), 2, 40),
        attack_angle: damage_range_part(attrs.get("damage_range"), 3, 0),
        mp_consume: attrs
            .get("mp_consume")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        reduced_mp_consume: attrs
            .get("reduced_mp_consume")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        reduced_mp_consume_chance: attrs
            .get("reduced_mp_consume_chance")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        crystal_count: attrs
            .get("crystal_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        body_part: part,
        weight,
        is_stackable,
        is_infinite,
        type1,
        type2,
        is_quest_item,
        is_sellable: attrs
            .get("is_sellable")
            .map(|v| v == "true")
            .unwrap_or(true),
        is_freightable: attrs.get("is_freightable").map(|v| v == "true") == Some(true),
        trade_flags: TradeFlags {
            dropable: attrs
                .get("is_dropable")
                .map(|v| v == "true")
                .unwrap_or(true),
            tradable: attrs
                .get("is_tradable")
                .map(|v| v == "true")
                .unwrap_or(true),
            destroyable: attrs
                .get("is_destroyable")
                .map(|v| v == "true")
                .unwrap_or(true),
            // Java: quest items are never depositable (barring the
            // `CustomDepositableQuestItems` config, which this dist leaves off).
            depositable: !is_quest_item
                && attrs
                    .get("is_depositable")
                    .map(|v| v == "true")
                    .unwrap_or(true),
        },
        time: attrs.get("time").and_then(|v| v.parse().ok()).unwrap_or(-1),
        duration: attrs
            .get("duration")
            .and_then(|v| v.parse().ok())
            .unwrap_or(-1),
        price: attrs.get("price").and_then(|v| v.parse().ok()).unwrap_or(0),
        handler,
        capsuled_items,
        extractable_count_min: attrs
            .get("extractableCountMin")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        extractable_count_max: attrs
            .get("extractableCountMax")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        item_skills,
        etc_item_type: EtcItemType::from_name(attrs.get("etcitem_type").map(|s| s.as_str())),
        enchant_enabled: attrs
            .get("enchant_enabled")
            .map(|v| v == "true")
            .unwrap_or(false),
        enchant_limit: attrs
            .get("enchant_limit")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        is_magic_weapon: kind == ItemKind::Weapon
            && attrs
                .get("is_magic_weapon")
                .map(|v| v == "true")
                .unwrap_or(false),
        immediate_effect: attrs
            .get("immediate_effect")
            .map(|v| v == "true")
            .unwrap_or(false),
        ex_immediate_effect: attrs
            .get("ex_immediate_effect")
            .map(|v| v == "true")
            .unwrap_or(false),
        default_action: ActionType::from_name(attrs.get("default_action").map(|s| s.as_str())),
        pre_conditions,
        is_oly_restricted: attrs
            .get("is_oly_restricted")
            .map(|v| v == "true")
            .unwrap_or(false),
        is_event_restricted: attrs
            .get("is_event_restricted")
            .map(|v| v == "true")
            .unwrap_or(false),
        for_npc: attrs.get("for_npc").map(|v| v == "true").unwrap_or(false),
    }
}

/// Map an item `<stat type="..">` name to the engine [`Stat`] it feeds.
/// Returns `None` for stat kinds the finalizers don't compute yet (elemental
/// power/resistance, shield defence, `sDef`, `moveSpeed`, …); those are dropped
/// rather than silently miscredited to a related stat. `pAtkRange`/
/// `randomDamage` are handled by the caller (they aren't `Stat`s).
fn stat_from_xml(name: &str) -> Option<Stat> {
    Some(match name {
        "pAtk" => Stat::PhysicalAttack,
        "mAtk" => Stat::MagicalAttack,
        "pDef" => Stat::PhysicalDefence,
        "mDef" => Stat::MagicalDefence,
        "pAtkSpd" => Stat::PhysicalAttackSpeed,
        "mAtkSpd" => Stat::MagicAttackSpeed,
        "rCrit" => Stat::CriticalRate,
        "mCritRate" => Stat::MagicCriticalRate,
        "accCombat" => Stat::AccuracyCombat,
        "accMagic" => Stat::AccuracyMagic,
        "rEvas" => Stat::EvasionRate,
        "mEvas" => Stat::MagicEvasionRate,
        "maxHp" => Stat::MaxHp,
        "maxMp" => Stat::MaxMp,
        _ => return None,
    })
}
