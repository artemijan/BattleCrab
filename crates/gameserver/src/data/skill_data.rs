//! Port of `data/xml/SkillData` — a generic per-level-value parser for
//! `dist/game/data/stats/skills/*.xml`, scoped to the fields G6's cast
//! pipeline reads (targeting/timing/costs/abnormal info) plus a curated
//! `<effect name>` → `Stat` registry (see module docs on `model::skill`).
//!
//! Every child of `<skill>` follows the same two shapes Java's loader
//! handles generically: a bare scalar (`<castRange>40</castRange>`, applies
//! to every level) or a per-level table (`<mpConsume><value level="1">4</value>
//! <value level="2">8</value></mpConsume>`). `<effects><effect name="X">…`
//! follows the same shape one level deeper for its `<amount>` child, plus a
//! scalar `<mode>DIFF|PER</mode>`. `<conditions>` and any other field this
//! milestone doesn't need are parsed (to keep the reader positioned
//! correctly) and discarded.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

use crate::model::skill::{OperateType, RestorationGroup, RestorationItem, Skill, SkillEffect, StatModifierEffect, TargetType};
use crate::model::stats::{Stat, StatModifierType};

pub const SKILLS_DIR: &str = "data/stats/skills";

/// `<effect name="X">` → the `Stat` it modifies (Java: the concrete effect
/// class name, e.g. `PAtk.java` → `Stat.PHYSICAL_ATTACK`). Only the handful of
/// generic `AbstractStatEffect`-style modifiers G6 needs are registered here;
/// everything else (damage effects, CC, heals, …) is unregistered and simply
/// dropped from the skill's effect list — the skill still loads. TODO(G9+):
/// grow this table (and add non-stat-modifier effect kinds) as combat lands.
const EFFECT_REGISTRY: &[(&str, Stat)] = &[
    ("PAtk", Stat::PhysicalAttack),
    ("PhysicalDefence", Stat::PhysicalDefence),
    ("MAtk", Stat::MagicalAttack),
    ("MagicalDefence", Stat::MagicalDefence),
    ("PhysicalAttackSpeed", Stat::PhysicalAttackSpeed),
    ("MagicalAttackSpeed", Stat::MagicAttackSpeed),
    ("CriticalRate", Stat::CriticalRate),
    ("MagicCriticalRate", Stat::MagicCriticalRate),
    ("PhysicalEvasion", Stat::EvasionRate),
    ("MagicalEvasion", Stat::MagicEvasionRate),
    ("Accuracy", Stat::AccuracyCombat),
    ("MagicAccuracy", Stat::AccuracyMagic),
    ("MaxHp", Stat::MaxHp),
    ("MaxMp", Stat::MaxMp),
    ("MaxCp", Stat::MaxCp),
    ("HpRegen", Stat::RegenerateHpRate),
    ("MpRegen", Stat::RegenerateMpRate),
    ("CpRegen", Stat::RegenerateCpRate),
    // Concentration (1078) and the like: `ReduceCancel` → `Stat.ATTACK_CANCEL`.
    // Without this the effect fell through, produced no stat modifier, and the
    // buff was dropped whole at `apply_skill_effects`' empty-effects guard — so
    // the buff never landed (the community-board "Concentration doesn't work").
    ("ReduceCancel", Stat::AttackCancel),
    // Blessed Shield (1243): `ShieldDefenceRate` → `Stat.SHIELD_DEFENCE_RATE`
    // (single-stat `AbstractStatEffect`). Without this the effect fell through,
    // produced no modifier, and the buff was dropped whole at the empty-effects
    // guard (community-board "Blessed Shield doesn't apply"). `CriticalDamage`
    // is NOT here — it is two-stat (mul/add by mode), handled in a match arm.
    ("ShieldDefenceRate", Stat::ShieldDefenceRate),
];

pub struct SkillData {
    skills: HashMap<(i32, i32), Skill>,
}

impl SkillData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut skills = HashMap::new();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{SKILLS_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_file(&path, &mut skills);
            }
        }
        info!("SkillData: Loaded {} skill levels.", skills.len());
        Self { skills }
    }

    pub fn get(&self, id: i32, level: i32) -> Option<&Skill> {
        self.skills.get(&(id, level))
    }

    /// Java `SkillData.getMaxLevel(id)` — the highest loaded level for a skill
    /// id (0 if the id is unknown). Used by `//cast` when no level is given.
    pub fn max_level(&self, id: i32) -> i32 {
        self.skills.keys().filter(|(sid, _)| *sid == id).map(|(_, lvl)| *lvl).max().unwrap_or(0)
    }

    /// Java `Skill` constructor: `EnableModifySkillDuration` + `SkillDurationList`.
    /// When enabled, override a skill's `abnormalTime` from the config list — for
    /// ordinary levels (`< 100` or `> 140`) the config value replaces the XML
    /// time; for enchanted levels (`100..140`) it is *added* to the base. Toggles
    /// (`operateType=T`) are exempt. Applied once at boot (`main.rs`) so every
    /// downstream reader of `abnormal_time` (buff expiry, DoT ticks) sees it.
    pub fn apply_skill_duration_list(&mut self, list: &HashMap<i32, i32>) {
        if list.is_empty() {
            return;
        }
        for skill in self.skills.values_mut() {
            if skill.operate_type == OperateType::Toggle {
                continue;
            }
            if let Some(&secs) = list.get(&skill.id) {
                if skill.level < 100 || skill.level > 140 {
                    skill.abnormal_time = secs;
                } else {
                    skill.abnormal_time += secs;
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { skills: HashMap::new() }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, skill: Skill) {
        self.skills.insert((skill.id, skill.level), skill);
    }
}

/// A field's per-level values, keyed by level; `0` is the "applies to every
/// level" sentinel used for bare scalars.
type LeveledValues = HashMap<String, HashMap<i32, String>>;

/// Look up `field` at `level`, falling back to the scalar (level 0) entry.
fn value_at<'a>(values: &'a LeveledValues, field: &str, level: i32) -> Option<&'a str> {
    let table = values.get(field)?;
    table.get(&level).or_else(|| table.get(&0)).map(String::as_str)
}

fn parse_file(path: &std::path::Path, out: &mut HashMap<(i32, i32), Skill>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    parse_str(&content, out);
}

fn parse_str(content: &str, out: &mut HashMap<(i32, i32), Skill>) {
    let mut reader = Reader::from_str(content);

    // Current `<skill>` being built (id/name/toLevel + the generic field map).
    let mut skill_id = -1;
    let mut skill_name = String::new();
    let mut to_level = 1;
    let mut values: LeveledValues = HashMap::new();
    let mut cur_field = String::new();
    let mut pending_level: i32 = 0;

    // Effects collected for the current skill: (xml name, per-level params
    // keyed by param name — `amount` for stat modifiers, `power` for the
    // instant damage/heal handlers —, mode, RestorationRandom groups).
    let mut effects: Vec<(String, LeveledValues, String, Vec<RestorationGroup>, u8, u32)> = Vec::new();
    let mut in_effects = false;
    let mut in_conditions = false;
    let mut cur_effect_name: Option<String> = None;
    let mut cur_effect_params: LeveledValues = HashMap::new();
    let mut cur_effect_mode = String::from("DIFF");
    let mut cur_effect_field = String::new();
    // OR of `ArmorType::mask_bit`s from the current effect's `<armorType>`
    // list (`ConditionUsingItemType`); 0 = no armor condition. Reset per effect.
    let mut cur_effect_armor: u8 = 0;
    // OR of `WeaponType::mask_bit`s from the current effect's `<weaponType>`
    // list; 0 = no weapon condition. Reset per effect.
    let mut cur_effect_weapon: u32 = 0;

    // `RestorationRandom`'s `<items><item chance="30"><item id=".." count=".."
    // /></item></items>` shape doesn't fit the scalar/leveled-value model
    // above (a list of chance-weighted item groups), so it's tracked
    // separately: `cur_restoration_groups` accumulates finished groups for
    // the current `<effect>`, `cur_group_chance`/`cur_group_items` build the
    // group currently open.
    let mut cur_restoration_groups: Vec<RestorationGroup> = Vec::new();
    let mut cur_group_chance: f64 = 0.0;
    let mut cur_group_items: Vec<RestorationItem> = Vec::new();

    // Tag-name stack relative to `<skill>` (path[0] == "skill" once inside one).
    let mut path: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) => {
                // Self-closing leaf (e.g. an attribute-only tag with no text).
                // Not pushed onto `path` since no matching `End` event follows
                // — the one shape this loader reads here is `RestorationRandom`'s
                // inner `<item id=".." count=".."/>`, sitting right inside an
                // open group (`path` still at the group's depth, 5).
                if in_effects && cur_effect_field == "items" && path.len() == 5 && e.name().as_ref() == b"item" {
                    if let (Some(item_id), Some(count)) = (attr_i32(&e, b"id"), attr_i64(&e, b"count")) {
                        cur_group_items.push(RestorationItem {
                            item_id,
                            count,
                            min_enchant: attr_i32(&e, b"minEnchant").unwrap_or(0),
                            max_enchant: attr_i32(&e, b"maxEnchant").unwrap_or(0),
                        });
                    }
                } else if in_effects && path.len() == 2 && e.name().as_ref() == b"effect" {
                    // A param-less self-closing `<effect name="X" />` (Spoil,
                    // Sweeper, ConsumeBody, …). No Start/End pair fires for an
                    // `Empty` element, so capture it here with empty params —
                    // otherwise the effect is silently dropped and the skill
                    // becomes a no-op.
                    if let Some(effect_name) = attr_str(&e, b"name") {
                        effects.push((effect_name, HashMap::new(), String::from("DIFF"), Vec::new(), 0, 0));
                    }
                }
            }
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();

                if path.is_empty() {
                    if name != "skill" {
                        // The `<list>` document root (or anything else outside
                        // a `<skill>`) is not tracked — the stack is relative
                        // to `<skill>`, see its matching End guard below.
                        continue;
                    }
                    skill_id = attr_i32(&e, b"id").unwrap_or(-1);
                    skill_name = attr_str(&e, b"name").unwrap_or_default();
                    to_level = attr_i32(&e, b"toLevel").unwrap_or(1).max(1);
                    values.clear();
                    effects.clear();
                    in_effects = false;
                    in_conditions = false;
                } else if path.len() == 1 {
                    cur_field = name.clone();
                    if name == "effects" {
                        in_effects = true;
                    } else if name == "conditions" {
                        in_conditions = true;
                    }
                } else if path.len() == 2 && name == "value" && !in_effects && !in_conditions {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                } else if path.len() == 2 && in_effects && name == "effect" {
                    cur_effect_name = attr_str(&e, b"name");
                    cur_effect_params = HashMap::new();
                    cur_effect_mode = String::from("DIFF");
                    cur_effect_armor = 0;
                    cur_effect_weapon = 0;
                    cur_restoration_groups = Vec::new();
                } else if path.len() == 3 && in_effects {
                    cur_effect_field = name.clone();
                } else if path.len() == 4 && in_effects && name == "value" {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                } else if path.len() == 4 && in_effects && cur_effect_field == "items" && name == "item" {
                    // `RestorationRandom`'s outer `<item chance="30">` group tag.
                    cur_group_chance = attr_f64(&e, b"chance").unwrap_or(0.0);
                    cur_group_items = Vec::new();
                }
                path.push(name);
            }
            Ok(Event::Text(txt)) => {
                let text = txt.unescape().unwrap_or_default();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                if in_conditions {
                    // Not parsed (see module docs) — nothing to record.
                } else if in_effects && cur_effect_field == "armorType" && path.len() == 5 {
                    // `<effect><armorType><item>MAGIC</item>...` — OR each armor
                    // kind's bit into the effect's condition mask.
                    cur_effect_armor |= crate::data::item_data::ArmorType::from_name(text).mask_bit();
                } else if in_effects && cur_effect_field == "weaponType" && path.len() == 5 {
                    // `<effect><weaponType><item>BOW</item>...` — OR each weapon
                    // kind's bit into the effect's weapon-condition mask.
                    cur_effect_weapon |= crate::data::item_data::WeaponType::from_name(text).mask_bit();
                } else if in_effects {
                    match path.len() {
                        4 if cur_effect_field == "mode" => {
                            cur_effect_mode = text.to_string();
                        }
                        // Directly under `<effect><param>SCALAR</param>`.
                        4 => {
                            cur_effect_params.entry(cur_effect_field.clone()).or_default().insert(0, text.to_string());
                        }
                        // `<effect><param><value level="N">...`
                        5 => {
                            cur_effect_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                } else {
                    match path.len() {
                        // `<field>SCALAR</field>` directly under `<skill>`.
                        2 => {
                            values.entry(cur_field.clone()).or_default().insert(0, text.to_string());
                        }
                        // `<field><value level="N">...`
                        3 => {
                            values.entry(cur_field.clone()).or_default().insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                let closed = path.pop().unwrap_or_default();
                if closed == "skill" {
                    finalize_skill(skill_id, &skill_name, to_level, &values, &effects, out);
                    skill_id = -1;
                } else if closed == "effects" {
                    in_effects = false;
                } else if closed == "conditions" {
                    in_conditions = false;
                } else if closed == "item" && in_effects && cur_effect_field == "items" {
                    // Closes a `RestorationRandom` group (the inner
                    // `<item id=".." count=".."/>` is self-closing, so this
                    // `End` only ever fires for the outer group tag).
                    cur_restoration_groups
                        .push(RestorationGroup { chance: cur_group_chance, items: std::mem::take(&mut cur_group_items) });
                } else if closed == "effect" && in_effects {
                    if let Some(name) = cur_effect_name.take() {
                        effects.push((name, cur_effect_params.clone(), cur_effect_mode.clone(), std::mem::take(&mut cur_restoration_groups), cur_effect_armor, cur_effect_weapon));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn finalize_skill(
    id: i32,
    name: &str,
    to_level: i32,
    values: &LeveledValues,
    effects: &[(String, LeveledValues, String, Vec<RestorationGroup>, u8, u32)],
    out: &mut HashMap<(i32, i32), Skill>,
) {
    if id < 0 {
        return;
    }
    for level in 1..=to_level {
        let get_i = |field: &str, default: i32| value_at(values, field, level).and_then(|v| v.parse().ok()).unwrap_or(default);
        let get_f = |field: &str, default: f64| value_at(values, field, level).and_then(|v| v.parse().ok()).unwrap_or(default);
        let operate_type = match value_at(values, "operateType", level) {
            Some("A1") | Some("A2") => OperateType::Active,
            Some("P") => OperateType::Passive,
            Some("T") => OperateType::Toggle,
            _ => OperateType::Other,
        };
        let target_type = match value_at(values, "targetType", level) {
            Some("SELF") => TargetType::Self_,
            Some("TARGET") => TargetType::Target,
            Some("ENEMY") => TargetType::Enemy,
            Some("ENEMY_ONLY") => TargetType::EnemyOnly,
            Some("NPC_BODY") => TargetType::NpcBody,
            _ => TargetType::Other,
        };

        let skill_effects = effects
            .iter()
            .flat_map(|(xml_name, params, mode, groups, armor_condition, weapon_condition)| {
                let param = |key: &str| -> Option<f64> { value_at(params, key, level).and_then(|v| v.parse().ok()) };
                let modifier_mode = if mode == "PER" { StatModifierType::Per } else { StatModifierType::Diff };
                let stat_mod = |stat: Stat, amount: f64| {
                    SkillEffect::StatModifier(StatModifierEffect {
                        stat,
                        mode: modifier_mode,
                        amount,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                    })
                };
                match xml_name.as_str() {
                    // Java instantiates these handlers whenever the `<effect>` is
                    // present and reads `params.getDouble("power", 0)` — the
                    // effect is always created, `power` defaulting to 0 when the
                    // param is absent (e.g. skills 1011/4717/4718, whose
                    // `<item>power</item>` parses to the param key `item`, not
                    // `power`). Mirror that default here; do NOT drop the effect,
                    // or the skill becomes a silent no-op.
                    "MagicalAttack" => vec![SkillEffect::MagicalAttack { power: param("power").unwrap_or(0.0) }],
                    // Ranged magical nuke (e.g. Prominence 1230). Java's
                    // `MagicalAttackRange` computes the same
                    // `calcMagicDam(mAtk, power, mDef, sps, bss, mcrit)` core as
                    // `MagicalAttack`; the only extra is a `shieldDefPercent`
                    // shield-block term, which the `MagicalAttack` damage path
                    // doesn't model yet either, so route it to the same effect.
                    // Without this the effect fell through to `EFFECT_REGISTRY`,
                    // wasn't found, and got dropped — the skill cast but dealt
                    // no damage.
                    // TODO(G7.5): honor `shieldDefPercent` (adds
                    // `shldDef * pct/100` to mDef on shield-block) once shield
                    // defense is modeled in the magic-damage formula.
                    "MagicalAttackRange" => vec![SkillEffect::MagicalAttack { power: param("power").unwrap_or(0.0) }],
                    // Soul-charge magic nuke (e.g. some Kamael/dagger-mage
                    // skills). Java's `MagicalSoulAttack` runs the identical
                    // `calcMagicDam(mAtk, power, mDef, sps, bss, mcrit)` core as
                    // `MagicalAttack`; its only difference is scaling mAtk by
                    // `1.3 + souls*0.05` when the caster has charged souls.
                    // Souls/charges aren't modeled yet, so that multiplier is
                    // exactly 1.0 here and the damage is identical to
                    // `MagicalAttack` — same silent-drop trap as
                    // `MagicalAttackRange` if left unhandled.
                    // TODO(G7.5): scale mAtk by charged souls once charges land.
                    "MagicalSoulAttack" => vec![SkillEffect::MagicalAttack { power: param("power").unwrap_or(0.0) }],
                    // Vampiric Touch/Claw: magic damage + self-heal of
                    // `percentage`% of the drained HP.
                    "HpDrain" => vec![SkillEffect::HpDrain {
                        power: param("power").unwrap_or(0.0),
                        percentage: param("percentage").unwrap_or(0.0),
                    }],
                    // Poison/bleed damage-over-time (e.g. Curse Poison 1168).
                    // Java always creates the effect and reads `power`/`ticks`
                    // (`ticks` is a scalar child → level-0 fallback); `canKill`
                    // defaults false. Without this arm the effect fell through
                    // to `EFFECT_REGISTRY`, wasn't found, and got dropped — the
                    // debuff landed but never dealt damage.
                    "DamOverTime" => vec![SkillEffect::DamOverTime {
                        power: param("power").unwrap_or(0.0),
                        ticks: param("ticks").unwrap_or(0.0) as i32,
                        can_kill: value_at(params, "canKill", level) == Some("true"),
                    }],
                    // Dagger blows (calcBlowDamage). FatalBlow/Backstab roll
                    // `criticalChance` (default 0) to double; SoulBlow doesn't
                    // (its charged-soul boost is unmodeled → ×1). Backstab also
                    // requires flanking. Their `Lethal` sibling effect drops.
                    "FatalBlow" => vec![SkillEffect::Blow {
                        power: param("power").unwrap_or(0.0),
                        chance_boost: param("chanceBoost").unwrap_or(0.0),
                        critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
                        backstab: false,
                    }],
                    "Backstab" => vec![SkillEffect::Blow {
                        power: param("power").unwrap_or(0.0),
                        chance_boost: param("chanceBoost").unwrap_or(0.0),
                        critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
                        backstab: true,
                    }],
                    "SoulBlow" => vec![SkillEffect::Blow {
                        power: param("power").unwrap_or(0.0),
                        chance_boost: param("chanceBoost").unwrap_or(0.0),
                        critical_chance: None,
                        backstab: false,
                    }],
                    // Physical skill damage. `PhysicalSoulAttack` runs the
                    // identical `77·((pAtk·pAtkMod)·levelMod + power)/(pDef·pDefMod)`
                    // core; its only extra is a charged-soul boost that is ×1
                    // until charges are modeled, so it routes here too (like
                    // MagicalSoulAttack→MagicalAttack). The `FatalBlow`/
                    // `Backstab`/`SoulBlow` blow skills use a different
                    // `calcBlowDamage` formula and are intentionally left to fall
                    // through until that formula is ported.
                    // TODO(G20): honor charged souls on PhysicalSoulAttack.
                    "PhysicalAttack" | "PhysicalSoulAttack" => vec![SkillEffect::PhysicalAttack {
                        power: param("power").unwrap_or(0.0),
                        p_atk_mod: param("pAtkMod").unwrap_or(1.0),
                        p_def_mod: param("pDefMod").unwrap_or(1.0),
                        critical_chance: param("criticalChance").unwrap_or(10.0),
                    }],
                    "Heal" => vec![SkillEffect::Heal { power: param("power").unwrap_or(0.0) }],
                    "Restoration" => match (param("itemId"), param("itemCount")) {
                        (Some(item_id), Some(item_count)) => vec![SkillEffect::GiveItem {
                            item_id: item_id as i32,
                            item_count: item_count as i64,
                            item_enchant_level: param("itemEnchantmentLevel").unwrap_or(0.0) as i32,
                        }],
                        _ => Vec::new(),
                    },
                    "RestorationRandom" => vec![SkillEffect::GiveItemRandom { groups: groups.clone() }],
                    // Spoil (254/…): mark the mob spoiled. No params — the
                    // landing roll and target checks live in the effect handler.
                    "Spoil" => vec![SkillEffect::Spoil],
                    // Sweeper (42/474): claim the dead mob's spoil loot.
                    "Sweeper" => vec![SkillEffect::Sweeper],
                    // ConsumeBody (paired with Sweeper on 42): decay the corpse.
                    "ConsumeBody" => vec![SkillEffect::ConsumeBody],
                    // Cure Poison/Bleeding etc.: the `<dispel>` string is a
                    // per-level `"TYPE,level"` list (Java splits on ';' then ',').
                    // Falls through to an empty effect (silent no-op, like other
                    // unhandled bodies) if the string is missing/unparseable,
                    // rather than dropping the whole cast. Without this arm the
                    // effect fell through to `EFFECT_REGISTRY`, wasn't found, and
                    // got dropped — the skill cast but cured nothing.
                    "DispelBySlot" => match value_at(params, "dispel", level) {
                        Some(spec) if !spec.is_empty() => {
                            let dispel = spec
                                .split(';')
                                .filter_map(|pair| {
                                    let mut it = pair.split(',');
                                    let ty = it.next()?.trim().to_string();
                                    let lvl = it.next()?.trim().parse::<i32>().ok()?;
                                    Some((ty, lvl))
                                })
                                .collect::<Vec<_>>();
                            if dispel.is_empty() {
                                Vec::new()
                            } else {
                                vec![SkillEffect::DispelBySlot { dispel }]
                            }
                        }
                        _ => Vec::new(),
                    },
                    // Both the basic (247) and advanced HQ skills carry this;
                    // isAdvanced is not yet behaviorally distinct (see the effect).
                    "HeadquarterCreate" => vec![SkillEffect::CreateHeadquarter],
                    // Java throws if amount is 0/missing; we drop the effect
                    // (silent no-op) to match how other bad effect bodies fall
                    // through, rather than panicking at data-load.
                    "GiveRecommendation" => match param("amount") {
                        Some(amount) if amount != 0.0 => {
                            vec![SkillEffect::GiveRecommendation { amount: amount as i32 }]
                        }
                        _ => Vec::new(),
                    },
                    // Only the TOWN escape is portable (see `SkillEffect::EscapeToTown`);
                    // CASTLE/CLANHALL/FORTRESS variants drop like unregistered names.
                    "Escape" if value_at(params, "escapeType", level) == Some("TOWN") => {
                        vec![SkillEffect::EscapeToTown]
                    }
                    // `Speed` pumps four move-speed stats at once (Java
                    // `Speed.pump`); the 1-name→1-stat `EFFECT_REGISTRY` can't
                    // express that, so expand it here. Without this, movement
                    // buffs (Wind Walk, Agility) loaded with an empty effect
                    // list and did nothing — server or client.
                    "Speed" => match param("amount") {
                        Some(amount) => [Stat::RunSpeed, Stat::WalkSpeed, Stat::SwimRunSpeed, Stat::SwimWalkSpeed]
                            .into_iter()
                            .map(|stat| stat_mod(stat, amount))
                            .collect(),
                        None => Vec::new(),
                    },
                    // Blessing of Protection (5182): PK-damage immunity. No stat
                    // modifier, so it would otherwise fall through to an empty
                    // effect list and never land as a buff — carry a marker so
                    // `apply_skill_effects` still creates the icon-only timed buff.
                    // TODO(G-pvp): honor the actual damage immunity.
                    "ProtectionBlessing" => vec![SkillEffect::ProtectionBlessing],
                    // Death Whisper (1242) & co.: Java `CriticalDamage extends
                    // AbstractStatEffect(params, CRITICAL_DAMAGE, CRITICAL_DAMAGE_ADD)`
                    // — a two-stat effect that pumps the multiplicative
                    // `CRITICAL_DAMAGE` in `PER` mode and the additive
                    // `CRITICAL_DAMAGE_ADD` in `DIFF` mode. The 1-name→1-stat
                    // `EFFECT_REGISTRY` can't express that, so pick the stat by
                    // mode here (like `Speed`). Without this the effect fell
                    // through, produced no modifier, and the buff was dropped
                    // whole (community-board "Death Whisper doesn't apply").
                    "CriticalDamage" => param("amount")
                        .map(|amount| {
                            let stat = if modifier_mode == StatModifierType::Per {
                                Stat::CriticalDamage
                            } else {
                                Stat::CriticalDamageAdd
                            };
                            stat_mod(stat, amount)
                        })
                        .into_iter()
                        .collect(),
                    // Mental Shield (1035) / Stun Resistance ("Resist Shock",
                    // 1259): Java `DefenceTrait` raises per-`TraitType` resistance
                    // (HOLD/SLEEP/SHOCK…) — not a single `Stat`, and its params
                    // are the trait names, not `amount`. The trait-defense math
                    // isn't modeled yet, so carry a marker (like
                    // `ProtectionBlessing`) so the buff still lands icon-only
                    // instead of being dropped whole.
                    "DefenceTrait" => vec![SkillEffect::DefenceTrait],
                    // Vampiric Rage (1268): Java `VampiricAttack` grants a chance
                    // to absorb a % of melee damage as HP. The melee-absorb path
                    // isn't modeled, so carry an icon-only marker rather than
                    // dropping the buff.
                    "VampiricAttack" => vec![SkillEffect::VampiricAttack],
                    // Community-board dance/song buffs whose combat/cost math
                    // isn't modeled yet — Dance of Light (277, `AttackAttribute`),
                    // Song of Champion/Renewal (`MagicMpCost`/`Reuse`), Gift of
                    // Seraphim (4703, `Reuse`), Song of Vengeance (305,
                    // `DamageShield`). Each maps to a per-magic-type or element
                    // stat the port doesn't have, so carry an icon-only marker
                    // (like `DefenceTrait`) rather than dropping the buff whole at
                    // the empty-effects guard — the buff must show and expire.
                    "AttackAttribute" => vec![SkillEffect::AttackAttribute],
                    "MagicMpCost" => vec![SkillEffect::MagicMpCost],
                    "Reuse" => vec![SkillEffect::Reuse],
                    "DamageShield" => vec![SkillEffect::DamageShield],
                    _ => match EFFECT_REGISTRY.iter().find(|(n, _)| n == xml_name).map(|(_, s)| *s) {
                        Some(stat) => param("amount").map(|amount| stat_mod(stat, amount)).into_iter().collect(),
                        None => Vec::new(),
                    },
                }
            })
            .collect::<Vec<_>>();

        // Effect names present in the XML but not in `EFFECT_REGISTRY` are
        // silently dropped (see module docs) — expected for the vast majority
        // of skills, which are outside G6's scope.
        out.insert(
            (id, level),
            Skill {
                id,
                level,
                name: name.to_string(),
                operate_type,
                target_type,
                magic_type: get_i("isMagic", 0),
                magic_level: get_i("magicLevel", 0),
                activate_rate: get_i("activateRate", -1),
                lvl_bonus_rate: get_i("lvlBonusRate", 0),
                effect_point: get_i("effectPoint", 0),
                cast_range: get_i("castRange", 0),
                effect_range: get_i("effectRange", 0),
                hit_time: get_i("hitTime", 0),
                hit_cancel_time: get_f("hitCancelTime", 0.0),
                cool_time: get_i("coolTime", 0),
                reuse_delay: get_i("reuseDelay", 0),
                reuse_delay_group: get_i("reuseDelayGroup", -1),
                mp_consume: get_i("mpConsume", 0),
                mp_initial_consume: get_i("mpInitialConsume", 0),
                hp_consume: get_i("hpConsume", 0),
                abnormal_time: get_i("abnormalTime", 0),
                abnormal_level: get_i("abnormalLevel", 0),
                abnormal_type: value_at(values, "abnormalType", level).unwrap_or("NONE").to_string(),
                // Java `AffectScope` defaults to SINGLE when the tag is absent.
                single_target: value_at(values, "affectScope", level).map_or(true, |s| s == "SINGLE"),
                // Java `set.getBoolean("canBeDispelled", true)` / `("isDebuff", false)`.
                can_be_dispelled: value_at(values, "canBeDispelled", level).map_or(true, |v| v == "true"),
                is_debuff: value_at(values, "isDebuff", level).map_or(false, |v| v == "true"),
                effects: skill_effects,
            },
        );
    }
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

fn attr_i64(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i64> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

fn attr_f64(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<f64> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard: the real dist XMLs are `<list>`-rooted, which the
    /// original parser mis-indexed (it tracked the root on the tag stack and
    /// loaded 0 skills). Wind Strike 1177 is the canonical probe.
    #[test]
    fn loads_real_dist_files() {
        let sd = SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(sd.skills.len() > 10_000, "expected thousands of skill levels, got {}", sd.skills.len());
        let ws = sd.get(1177, 1).expect("Wind Strike lvl 1");
        assert_eq!(ws.target_type, TargetType::EnemyOnly);
        assert_eq!(ws.cast_range, 600);
        assert!(matches!(ws.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0));
        assert_eq!(ws.reuse_delay_group, -1, "no <reuseDelayGroup> must stay -1, never 0");
        assert_eq!(ws.reuse_key(), 1177);

        // Prominence 1230: a ranged nuke backed by the `MagicalAttackRange`
        // effect. It must parse to a `MagicalAttack` (power 108 at lvl 28) —
        // before the handler existed the effect fell through and was dropped,
        // so the skill cast but dealt zero damage.
        let prominence = sd.get(1230, 28).expect("Prominence lvl 28");
        assert!(matches!(prominence.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 108.0));

        // Power Strike 3: the canonical `PhysicalAttack` skill. Before the
        // handler existed every physical attack skill (1164 XML entries) cast
        // but dealt zero damage. Power 30 at lvl 1, default mods/crit chance.
        let power_strike = sd.get(3, 1).expect("Power Strike lvl 1");
        assert!(matches!(
            power_strike.effects.as_slice(),
            [SkillEffect::PhysicalAttack { power, p_atk_mod, p_def_mod, critical_chance }]
                if *power == 30.0 && *p_atk_mod == 1.0 && *p_def_mod == 1.0 && *critical_chance == 10.0
        ));

        // Vampiric Touch 1147: an `HpDrain` skill — magic damage + 40% self-heal.
        // Before the handler existed it fell through and dealt no damage.
        let vampiric = sd.get(1147, 1).expect("Vampiric Touch lvl 1");
        assert!(matches!(
            vampiric.effects.as_slice(),
            [SkillEffect::HpDrain { power, percentage }] if *power == 18.0 && *percentage == 40.0
        ));

        // Dagger blows: Mortal Blow 16 (FatalBlow, crit-double, no flank),
        // Backstab 30 (flank-required), Shining Edge 505 (SoulBlow, no crit).
        let mortal_blow = sd.get(16, 1).expect("Mortal Blow lvl 1");
        assert!(matches!(
            mortal_blow.effects.as_slice(),
            [SkillEffect::Blow { power, chance_boost, critical_chance: Some(_), backstab: false }]
                if *power == 73.0 && *chance_boost == 200.0
        ));
        let backstab = sd.get(30, 1).expect("Backstab lvl 1");
        assert!(matches!(
            backstab.effects.first(),
            Some(SkillEffect::Blow { power, chance_boost, critical_chance: Some(cc), backstab: true })
                if *power == 1107.0 && *chance_boost == 400.0 && *cc == 5.0
        ));
        let shining_edge = sd.get(505, 1).expect("Shining Edge lvl 1");
        assert!(matches!(
            shining_edge.effects.first(),
            Some(SkillEffect::Blow { power, critical_chance: None, backstab: false, .. }) if *power == 1853.0
        ));

        // Decrease Speed 1160: single-target (`affectScope SINGLE`) bad skill
        // with a `Speed` PER -20% debuff, and the landing-rate inputs the
        // caster-feedback + resist roll read (`activateRate` 80, `lvlBonusRate` 30).
        let decrease_speed = sd.get(1160, 1).expect("Decrease Speed lvl 1");
        assert!(decrease_speed.single_target && decrease_speed.is_bad());
        assert_eq!(decrease_speed.activate_rate, 80);
        assert_eq!(decrease_speed.lvl_bonus_rate, 30);
        // An area skill (`affectScope RANGE`) is not single-target.
        let sonic_storm = sd.get(7, 1).expect("Sonic Storm lvl 1");
        assert!(!sonic_storm.single_target);
        // A skill with no `<activateRate>` defaults to -1 (always lands): the
        // buff Might 1068.
        let might = sd.get(1068, 1).expect("Might lvl 1");
        assert_eq!(might.activate_rate, -1);

        // Skill 1011 "Heal": the reference datapack's effect body is
        // `<item>power</item>`, which parses to the param key `item` — so the
        // `power` param is absent. Java still creates the Heal effect with
        // `getDouble("power", 0)` = 0 (healing via the mAtk term); the effect
        // must NOT be dropped. Guard that the effect exists with power 0.
        let heal = sd.get(1011, 3).expect("Heal lvl 3");
        assert!(matches!(heal.effects.as_slice(), [SkillEffect::Heal { power }] if *power == 0.0));

        // "Knight - Individual" shares reuse group 10008 with its siblings.
        let ki = sd.get(10248, 1).expect("Knight - Individual lvl 1");
        assert_eq!(ki.reuse_delay_group, 10008);
        assert_eq!(ki.reuse_key(), 10008);

        // The `/unstuck` escape skills (G15.5): static 5-minute (2099) and
        // GM 1-second (2100) casts whose `Escape TOWN` effect must parse to
        // `EscapeToTown` — an empty effect list would cast and go nowhere.
        let escape = sd.get(2099, 1).expect("Escape (5-minute) lvl 1");
        assert_eq!(escape.magic_type, 2, "static skill");
        assert_eq!(escape.hit_time, 300_000);
        assert_eq!(escape.target_type, TargetType::Self_);
        assert!(matches!(escape.effects.as_slice(), [SkillEffect::EscapeToTown]));
        let gm_escape = sd.get(2100, 1).expect("Escape: 1 Second lvl 1");
        assert!(matches!(gm_escape.effects.as_slice(), [SkillEffect::EscapeToTown]));

        // Blessing of Protection 5182 (Newbie Helper): its `ProtectionBlessing`
        // effect carries no stat modifier — before this arm it fell through to
        // an empty effect list and never landed as a buff. It must parse to the
        // marker so `apply_skill_effects` still creates the icon-only PK_PROTECT
        // buff (7200 s).
        let blessing = sd.get(5182, 1).expect("Blessing of Protection lvl 1");
        assert!(matches!(blessing.effects.as_slice(), [SkillEffect::ProtectionBlessing]));
        assert_eq!(blessing.abnormal_time, 7200);

        // The Newbie Helper support buffs must all load with their stat effects
        // (empty-effect skills would silently drop and show no icon): Wind Walk
        // 4322 pumps all four move speeds; Shield 4323 is PhysicalDefence;
        // Empower 4331 is MAtk.
        let wind_walk = sd.get(4322, 1).expect("Adventurer's Wind Walk lvl 1");
        assert_eq!(wind_walk.stat_modifier_effects().len(), 4, "Speed pumps 4 move stats");
        assert!(!sd.get(4323, 1).expect("Shield").stat_modifier_effects().is_empty());
        assert!(!sd.get(4331, 1).expect("Empower").stat_modifier_effects().is_empty());

        // Skill 22490 "Mysterious Spiritshot d 5000" — the `Restoration`
        // effect backing the "Mysterious Blessed Spiritshot Pack (5000)
        // (D-grade)" item (22599). Previously parsed with an empty effect
        // list, so using the pack consumed it and granted nothing.
        let spiritshot_pack = sd.get(22490, 5).expect("Mysterious Spiritshot d 5000 lvl 5");
        assert!(matches!(
            spiritshot_pack.effects.as_slice(),
            [SkillEffect::GiveItem { item_id: 21852, item_count: 5000, item_enchant_level: 0 }]
        ));

        // Skill 323 "Quiver of Arrow" — a real `RestorationRandom` skill
        // (three weighted groups of Mithril Arrow).
        let quiver = sd.get(323, 1).expect("Quiver of Arrow lvl 1");
        match quiver.effects.as_slice() {
            [SkillEffect::GiveItemRandom { groups }] => {
                assert_eq!(groups.len(), 3);
                assert_eq!(groups[0].chance, 30.0);
                assert_eq!(groups[0].items, vec![RestorationItem { item_id: 1344, count: 700, min_enchant: 0, max_enchant: 0 }]);
                assert_eq!(groups[1].chance, 50.0);
                assert_eq!(groups[1].items[0].count, 1400);
                assert_eq!(groups[2].chance, 20.0);
                assert_eq!(groups[2].items[0].count, 2800);
            }
            other => panic!("expected one GiveItemRandom effect, got {other:?}"),
        }

        // Grade-penalty skills (6209 weapon / 6213 armor) back the expertise
        // penalty — each level must carry the registry-known stat maluses so
        // `refresh_expertise_penalty` actually debuffs the over-grade wearer.
        let weapon_pen = sd.get(6209, 1).expect("Weapon Grade Penalty lvl 1");
        assert!(!weapon_pen.stat_modifier_effects().is_empty(), "6209 must have stat effects");
        assert!(weapon_pen.stat_modifier_effects().iter().any(|e| e.stat == Stat::PhysicalAttack));
        let armor_pen = sd.get(6213, 4).expect("Armor Grade Penalty lvl 4");
        assert!(!armor_pen.stat_modifier_effects().is_empty(), "6213 must have stat effects");

        // Clan Advent (19009) — the clan-leader-online aura applied via the clan
        // login/logout hooks. Permanent (`abnormalTime=-1`) with all six stat
        // effects: PAtk/PDef/MDef/MAtk percent buffs + flat HP/MP regen.
        let advent = sd.get(19009, 1).expect("Clan Advent lvl 1");
        assert_eq!(advent.abnormal_time, -1, "Clan Advent is permanent");
        let stats: Vec<Stat> = advent.stat_modifier_effects().iter().map(|e| e.stat).collect();
        for want in [
            Stat::PhysicalAttack,
            Stat::PhysicalDefence,
            Stat::MagicalDefence,
            Stat::MagicalAttack,
            Stat::RegenerateHpRate,
            Stat::RegenerateMpRate,
        ] {
            assert!(stats.contains(&want), "Clan Advent must modify {want:?}, got {stats:?}");
        }

        // Curse Poison 1168: a `DamOverTime` debuff (power 11, ticks 5, no
        // `canKill`) at lvl 1. Before the handler existed the effect fell
        // through `EFFECT_REGISTRY` and was dropped, so the poison landed as a
        // buff icon but never dealt damage.
        let curse_poison = sd.get(1168, 1).expect("Curse Poison lvl 1");
        assert!(matches!(
            curse_poison.effects.as_slice(),
            [SkillEffect::DamOverTime { power, ticks, can_kill: false }] if *power == 11.0 && *ticks == 5
        ));
        assert_eq!(curse_poison.abnormal_time, 30, "poison lasts 30s");

        // Cure Poison 1012: a `DispelBySlot` cleanse whose per-level `<dispel>`
        // string parses to `(POISON, level)` pairs (3/7/9 across levels 1-3).
        // Before the handler existed the effect fell through `EFFECT_REGISTRY`
        // and was dropped, so the cure cast but removed nothing.
        for (lvl, want) in [(1, 3), (2, 7), (3, 9)] {
            let cure = sd.get(1012, lvl).unwrap_or_else(|| panic!("Cure Poison lvl {lvl}"));
            assert!(
                matches!(cure.effects.as_slice(), [SkillEffect::DispelBySlot { dispel }] if dispel.as_slice() == [("POISON".to_string(), want)]),
                "Cure Poison lvl {lvl} dispels POISON,{want}, got {:?}",
                cure.effects,
            );
        }

        // Spoil 254: an `ENEMY_ONLY` debuff carrying the `Spoil` effect and a
        // per-level `magicLevel` (10 at lvl 1) the `calcMagicSuccess` roll reads.
        let spoil = sd.get(254, 1).expect("Spoil lvl 1");
        assert_eq!(spoil.target_type, TargetType::EnemyOnly);
        assert_eq!(spoil.magic_level, 10);
        assert!(spoil.is_bad(), "Spoil has negative effectPoint");
        assert!(matches!(spoil.effects.as_slice(), [SkillEffect::Spoil]));

        // Sweeper 42: an `NPC_BODY` (corpse) skill whose effects are
        // `Sweeper` then `ConsumeBody` (order matters — claim loot, then decay).
        let sweeper = sd.get(42, 1).expect("Sweeper lvl 1");
        assert_eq!(sweeper.target_type, TargetType::NpcBody);
        assert!(matches!(sweeper.effects.as_slice(), [SkillEffect::Sweeper, SkillEffect::ConsumeBody]));

        // Community-board buffer skills that previously loaded with an empty
        // effect list (every effect unregistered → dropped whole at the
        // empty-`buff_effects` bail) and so never landed / showed no icon.
        //
        // Blessed Shield 1243 (`ShieldDefenceRate`, PER +5% at lvl 1) and Death
        // Whisper 1242 (`CriticalDamage`, PER +25% at lvl 1) carry real stat
        // modifiers now. Death Whisper's PER mode must pick `CRITICAL_DAMAGE`
        // (not the `CRITICAL_DAMAGE_ADD` diff-mode sibling).
        let blessed_shield = sd.get(1243, 1).expect("Blessed Shield lvl 1");
        assert!(matches!(
            blessed_shield.effects.as_slice(),
            [SkillEffect::StatModifier(m)]
                if m.stat == Stat::ShieldDefenceRate && m.mode == StatModifierType::Per && m.amount == 5.0
        ));
        let death_whisper = sd.get(1242, 1).expect("Death Whisper lvl 1");
        assert!(matches!(
            death_whisper.effects.as_slice(),
            [SkillEffect::StatModifier(m)]
                if m.stat == Stat::CriticalDamage && m.mode == StatModifierType::Per && m.amount == 25.0
        ));

        // Mental Shield 1035 and Stun Resistance ("Resist Shock") 1259 carry a
        // `DefenceTrait` marker; Vampiric Rage 1268 carries a `VampiricAttack`
        // marker. No stat modifier, but the marker keeps the buff off the
        // empty-effects bail so it lands icon-only for its 1200 s.
        let mental_shield = sd.get(1035, 1).expect("Mental Shield lvl 1");
        assert!(matches!(mental_shield.effects.as_slice(), [SkillEffect::DefenceTrait]));
        assert_eq!(mental_shield.abnormal_time, 1200);
        let resist_shock = sd.get(1259, 1).expect("Stun Resistance lvl 1");
        assert!(matches!(resist_shock.effects.as_slice(), [SkillEffect::DefenceTrait]));
        let vampiric_rage = sd.get(1268, 1).expect("Vampiric Rage lvl 1");
        assert!(matches!(vampiric_rage.effects.as_slice(), [SkillEffect::VampiricAttack]));
    }

    /// A trimmed Wind Strike (1177): per-level `targetType` and
    /// `MagicalAttack` power, scalar `isMagic`/`castRange`, per-level
    /// `effectPoint` — the exact shapes in `01100-01199.xml`.
    #[test]
    fn parses_wind_strike_shaped_skill() {
        let xml = r#"
        <list>
            <skill id="1177" toLevel="2" name="Wind Strike">
                <castRange>600</castRange>
                <effectPoint>
                    <value level="1">-92</value>
                    <value level="2">-106</value>
                </effectPoint>
                <effectRange>1100</effectRange>
                <hitTime>4000</hitTime>
                <isMagic>1</isMagic>
                <mpConsume>
                    <value level="1">7</value>
                    <value level="2">7</value>
                </mpConsume>
                <mpInitialConsume>
                    <value level="1">2</value>
                    <value level="2">2</value>
                </mpInitialConsume>
                <operateType>A1</operateType>
                <reuseDelay>1200</reuseDelay>
                <targetType>
                    <value level="1">ENEMY_ONLY</value>
                    <value level="2">ENEMY</value>
                </targetType>
                <effects>
                    <effect name="MagicalAttack">
                        <power>
                            <value level="1">12</value>
                            <value level="2">13</value>
                        </power>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = HashMap::new();
        parse_str(xml, &mut out);

        let l1 = out.get(&(1177, 1)).expect("level 1 parsed");
        assert_eq!(l1.target_type, TargetType::EnemyOnly);
        assert_eq!(l1.magic_type, 1);
        assert_eq!(l1.effect_point, -92);
        assert!(l1.is_bad());
        assert_eq!(l1.cast_range, 600);
        assert_eq!(l1.effect_range, 1100);
        assert_eq!(l1.hit_time, 4000);
        assert_eq!(l1.reuse_delay, 1200);
        assert_eq!(l1.mp_consume, 7);
        assert_eq!(l1.mp_initial_consume, 2);
        assert!(matches!(l1.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0));

        let l2 = out.get(&(1177, 2)).expect("level 2 parsed");
        assert_eq!(l2.target_type, TargetType::Enemy);
        assert!(matches!(l2.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 13.0));
    }

    /// A Heal-shaped effect parses to `SkillEffect::Heal`; a stat-modifier
    /// effect still lands in `StatModifier` with `<amount>`; an unregistered
    /// effect name is dropped without dropping the skill.
    #[test]
    fn parses_heal_stat_and_unknown_effects() {
        let xml = r#"
        <list>
            <skill id="1015" toLevel="1" name="Battle Heal">
                <operateType>A1</operateType>
                <targetType>TARGET</targetType>
                <effects>
                    <effect name="Heal">
                        <power>83</power>
                    </effect>
                    <effect name="PAtk">
                        <amount>10</amount>
                        <mode>PER</mode>
                    </effect>
                    <effect name="SomeUnportedEffect">
                        <power>5</power>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = HashMap::new();
        parse_str(xml, &mut out);

        let s = out.get(&(1015, 1)).expect("skill parsed");
        assert_eq!(s.target_type, TargetType::Target);
        assert_eq!(s.effects.len(), 2, "unknown effect dropped");
        assert!(matches!(s.effects[0], SkillEffect::Heal { power } if power == 83.0));
        assert!(matches!(
            s.effects[1],
            SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttack, mode: StatModifierType::Per, amount, .. }) if amount == 10.0
        ));
    }

    /// Concentration-shaped skill: a lone `ReduceCancel` effect must parse to a
    /// `StatModifier(ATTACK_CANCEL)`. Before it was registered, the effect fell
    /// through, the effect list was empty, and `apply_skill_effects` dropped the
    /// whole buff — so Concentration never landed from the community board.
    #[test]
    fn reduce_cancel_parses_to_attack_cancel_stat() {
        let xml = r#"
        <list>
            <skill id="1078" toLevel="1" name="Concentration">
                <operateType>A2</operateType>
                <abnormalType>CANCEL_PROB_DOWN</abnormalType>
                <abnormalTime>1200</abnormalTime>
                <targetType>TARGET</targetType>
                <effects>
                    <effect name="ReduceCancel">
                        <amount>-18</amount>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = HashMap::new();
        parse_str(xml, &mut out);

        let s = out.get(&(1078, 1)).expect("skill parsed");
        assert_eq!(s.effects.len(), 1, "the ReduceCancel effect is not dropped");
        assert!(matches!(
            s.effects[0],
            SkillEffect::StatModifier(StatModifierEffect { stat: Stat::AttackCancel, mode: StatModifierType::Diff, amount, .. }) if amount == -18.0
        ));
    }

    /// Community-board dance/song buffs whose only effects are `AttackAttribute`
    /// (Dance of Light), `MagicMpCost`/`Reuse` (Song of Champion/Renewal),
    /// `Reuse` (Gift of Seraphim) or `DamageShield` (Song of Vengeance) must parse
    /// to their icon-only marker rather than being dropped. Before these arms
    /// existed, every effect fell through `EFFECT_REGISTRY`, the effect list was
    /// empty, and `apply_skill_effects` dropped the whole buff — so none of these
    /// landed from the community board.
    #[test]
    fn dance_song_buffs_parse_to_iconless_markers() {
        let xml = r#"
        <list>
            <skill id="277" toLevel="1" name="Dance of Light">
                <operateType>A2</operateType>
                <abnormalTime>120</abnormalTime>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="AttackAttribute">
                        <amount>20</amount>
                        <attribute>HOLY</attribute>
                    </effect>
                </effects>
            </skill>
            <skill id="8547" toLevel="1" name="Song of Champion">
                <operateType>A2</operateType>
                <abnormalTime>120</abnormalTime>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="MagicMpCost">
                        <amount>-20</amount>
                        <mode>PER</mode>
                        <magicType>0</magicType>
                    </effect>
                    <effect name="Reuse">
                        <amount>-10</amount>
                        <mode>PER</mode>
                        <magicType>0</magicType>
                    </effect>
                </effects>
            </skill>
            <skill id="305" toLevel="1" name="Song of Vengeance">
                <operateType>A2</operateType>
                <abnormalTime>120</abnormalTime>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="DamageShield">
                        <amount>20</amount>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = HashMap::new();
        parse_str(xml, &mut out);

        let dol = out.get(&(277, 1)).expect("Dance of Light parsed");
        assert!(
            matches!(dol.effects.as_slice(), [SkillEffect::AttackAttribute]),
            "AttackAttribute is not dropped"
        );
        let soc = out.get(&(8547, 1)).expect("Song of Champion parsed");
        assert!(
            matches!(soc.effects.as_slice(), [SkillEffect::MagicMpCost, SkillEffect::Reuse]),
            "MagicMpCost/Reuse are not dropped"
        );
        let sov = out.get(&(305, 1)).expect("Song of Vengeance parsed");
        assert!(
            matches!(sov.effects.as_slice(), [SkillEffect::DamageShield]),
            "DamageShield is not dropped"
        );
    }

    /// `EnableModifySkillDuration`/`SkillDurationList`: an ordinary-level buff in
    /// the list has its `abnormalTime` replaced, a toggle in the list is exempt,
    /// and a skill absent from the list is untouched (Java `Skill` constructor).
    #[test]
    fn skill_duration_list_overrides_abnormal_time() {
        let xml = r#"
        <list>
            <skill id="1078" toLevel="1" name="Concentration">
                <operateType>A2</operateType>
                <abnormalTime>1200</abnormalTime>
                <targetType>TARGET</targetType>
            </skill>
            <skill id="9999" toLevel="1" name="A Toggle">
                <operateType>T</operateType>
                <abnormalTime>1200</abnormalTime>
                <targetType>SELF</targetType>
            </skill>
            <skill id="5555" toLevel="1" name="Not Listed">
                <operateType>A2</operateType>
                <abnormalTime>1200</abnormalTime>
                <targetType>TARGET</targetType>
            </skill>
        </list>"#;
        let mut out = HashMap::new();
        parse_str(xml, &mut out);
        let mut sd = SkillData { skills: out };

        let list = HashMap::from([(1078, 7200), (9999, 7200)]);
        sd.apply_skill_duration_list(&list);

        assert_eq!(sd.get(1078, 1).unwrap().abnormal_time, 7200, "active buff time replaced");
        assert_eq!(sd.get(9999, 1).unwrap().abnormal_time, 1200, "toggle is exempt");
        assert_eq!(sd.get(5555, 1).unwrap().abnormal_time, 1200, "skill not in list is untouched");

        // Enchanted levels (100..=140) add rather than replace.
        let enchanted = Skill { level: 101, ..sd.get(1078, 1).unwrap().clone() };
        sd.insert_for_test(enchanted);
        sd.apply_skill_duration_list(&HashMap::from([(1078, 100)]));
        assert_eq!(sd.get(1078, 101).unwrap().abnormal_time, 7300, "enchanted level adds to base");
    }
}
