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

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::model::skill::{
    AffectObject, AffectScope, DispelSlot, OperateType, RestorationGroup, RestorationItem, Skill,
    SkillEffect, StatModifierEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

pub const SKILLS_DIR: &str = "data/stats/skills";

/// `<effect name="X">` → the `Stat` it modifies (Java: the concrete effect
/// class name, e.g. `PAtk.java` → `Stat.PHYSICAL_ATTACK`). Only the handful of
/// generic `AbstractStatEffect`-style modifiers G6 needs are registered here;
/// everything else (damage effects, CC, heals, …) is unregistered and simply
/// dropped from the skill's effect list — the skill still loads. TODO(G9+):
/// grow this table (and add non-stat-modifier effect kinds) as combat lands.
/// Java `Fear.getTicks()` — hard-coded, not a datapack param.
const FEAR_TICKS: i32 = 5;

/// The `<effect name>` → [`Stat`] lookup behind [`EFFECT_REGISTRY`], shared with
/// the augment-option loader (`data/stats/augmentation/options/*` uses the same
/// effect names as skills).
pub fn stat_for_effect_name(name: &str) -> Option<Stat> {
    EFFECT_REGISTRY
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
}

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
    // Shield Mastery (153, a widely-learned shield-user passive), Shield
    // Fortress (322), Knighthood (429), …: `ShieldDefence` →
    // `Stat.SHIELD_DEFENCE` (single-stat `AbstractStatEffect`), folded into
    // `shield_stats` alongside `ShieldDefenceRate` above.
    ("ShieldDefence", Stat::ShieldDefence),
    // Archery 431/Long Shot 113/Rapid Fire 413/Snipe 972: `PhysicalAttackRange`
    // → `Stat.PHYSICAL_ATTACK_RANGE` (single-stat `AbstractStatEffect`, all
    // four `<weaponType>BOW</weaponType>`-conditioned — the condition mask is
    // already generic, read off `armor_condition`/`weapon_condition` like
    // every other registry entry). Folded into `recalculate_stats`'
    // `combat.atk_range` line, which previously read the equipped weapon's
    // raw range directly with no stat modifier applied at all.
    ("PhysicalAttackRange", Stat::PhysicalAttackRange),
    // Focus Death 355/Critical Blow 409/Mortal Strike 410/Assassination 432
    // (all `PER`): `FatalBlowRate` → `Stat.BLOW_RATE` (single-stat
    // `AbstractStatEffect`, multiplicative). Folded into
    // `formulas::calc_blow_success`, which previously had no term for it at
    // all — `BLOW_RATE` was identity 1.0 no matter what a caster had learned.
    ("FatalBlowRate", Stat::BlowRate),
    // Higher Mana Gain (285), a learnable passive: `ManaCharge` →
    // `Stat.MANA_CHARGE`, a flat bonus the recharge skills read off their
    // *recipient*. A plain single-stat `AbstractStatEffect`, so the generic
    // registry wiring is all it needs.
    ("ManaCharge", Stat::ManaCharge),
    // Anti Magic (146), M. Def. (147): `ResistDDMagic` →
    // `Stat.MAGIC_SUCCESS_RES`. An `AbstractStatPercentEffect`, so it is always
    // `PER` and the generic registry wiring (which honours the effect's own
    // `<mode>`, `PER` here) is all it needs.
    ("ResistDDMagic", Stat::MagicSuccessRes),
];

pub struct SkillData {
    skills: HashMap<(i32, i32), Skill>,
    /// The enchanted variants, keyed `(id, level, subLevel)` — Java pre-builds
    /// one `Skill` per declared sub-level (routes 1001–1020 / 2001–2020 /
    /// 3001–3020) exactly like this (PLAN_G19_SKILL_ENCHANT.md).
    enchanted: HashMap<(i32, i32, i32), Skill>,
    /// `EnchantSkillGroupsData`'s route map: which sub-level ranges each
    /// `(id, level)` can enchant into. Non-empty = `Skill.isEnchantable()`.
    routes: HashMap<(i32, i32), Vec<(i32, i32)>>,
}

/// The three maps one parse pass fills (skills + enchanted variants + routes).
#[derive(Default)]
pub(crate) struct ParsedSkills {
    pub(crate) skills: HashMap<(i32, i32), Skill>,
    pub(crate) enchanted: HashMap<(i32, i32, i32), Skill>,
    pub(crate) routes: HashMap<(i32, i32), Vec<(i32, i32)>>,
}

impl SkillData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut out = ParsedSkills::default();
        if let Ok(dir) = std::fs::read_dir(format!("{file_path}{SKILLS_DIR}")) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_file(&path, &mut out);
            }
        }
        info!(
            "SkillData: Loaded {} skill levels (+{} enchanted variants, {} enchantable).",
            out.skills.len(),
            out.enchanted.len(),
            out.routes.len()
        );
        Self {
            skills: out.skills,
            enchanted: out.enchanted,
            routes: out.routes,
        }
    }

    pub fn get(&self, id: i32, level: i32) -> Option<&Skill> {
        self.skills.get(&(id, level))
    }

    /// The skill at an enchant sub-level (`sub <= 0` = the plain skill).
    pub fn get_enchanted(&self, id: i32, level: i32, sub: i32) -> Option<&Skill> {
        if sub <= 0 {
            self.get(id, level)
        } else {
            self.enchanted.get(&(id, level, sub))
        }
    }

    /// The enchant routes available to `(id, level)` as `(first, last)`
    /// sub-level bounds — empty for a non-enchantable skill.
    pub fn enchant_routes(&self, id: i32, level: i32) -> &[(i32, i32)] {
        self.routes
            .get(&(id, level))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Java `SkillData.getMaxLevel(id)` — the highest loaded level for a skill
    /// id (0 if the id is unknown). Used by `//cast` when no level is given.
    pub fn max_level(&self, id: i32) -> i32 {
        self.skills
            .keys()
            .filter(|(sid, _)| *sid == id)
            .map(|(_, lvl)| *lvl)
            .max()
            .unwrap_or(0)
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
        Self {
            skills: HashMap::new(),
            enchanted: HashMap::new(),
            routes: HashMap::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, skill: Skill) {
        self.skills.insert((skill.id, skill.level), skill);
    }

    #[doc(hidden)]
    pub fn insert_enchanted_for_test(&mut self, skill: Skill) {
        self.enchanted
            .insert((skill.id, skill.level, skill.sub_level), skill);
    }

    #[doc(hidden)]
    pub fn insert_route_for_test(&mut self, id: i32, level: i32, range: (i32, i32)) {
        self.routes.entry((id, level)).or_default().push(range);
    }
}

/// A field's per-level values, keyed by level; `0` is the "applies to every
/// level" sentinel used for bare scalars.
type LeveledValues = HashMap<String, HashMap<i32, String>>;

/// Look up `field` at `level`, falling back to the scalar (level 0) entry.
fn value_at<'a>(values: &'a LeveledValues, field: &str, level: i32) -> Option<&'a str> {
    let table = values.get(field)?;
    table
        .get(&level)
        .or_else(|| table.get(&0))
        .map(String::as_str)
}

fn parse_file(path: &std::path::Path, out: &mut ParsedSkills) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    parse_str(&content, out);
}

/// The level-range attributes on an `<effect>` element, with Java's defaulting:
/// `level` supplies the default for both `fromLevel` and `toLevel`, and
/// `subLevel` for both sub-level bounds.
fn effect_level_attrs(
    e: &quick_xml::events::BytesStart,
) -> (Option<i32>, Option<i32>, Option<i32>, Option<i32>) {
    let level = attr_i32(e, b"level");
    let sub_level = attr_i32(e, b"subLevel");
    (
        attr_i32(e, b"fromLevel").or(level),
        attr_i32(e, b"toLevel").or(level),
        attr_i32(e, b"fromSubLevel").or(sub_level),
        attr_i32(e, b"toSubLevel").or(sub_level),
    )
}

/// One `<value fromLevel toLevel [fromSubLevel toSubLevel]>` ranged row —
/// held raw and resolved per (level, sub) at finalize, where the `{…}`
/// expression variables (`base`/`index`/`subIndex`) are known.
#[derive(Debug, Clone)]
struct RangedRow {
    from_level: i32,
    to_level: i32,
    /// 0 for a plain per-level row; ≥ 1001 for an enchant-route row.
    from_sub: i32,
    to_sub: i32,
    text: String,
}

/// The ranged bounds off a `<value>` tag, `None` for a plain `level=`-keyed
/// (or bare) row. `fromLevel` defaults to 1 and `toLevel` to "all levels",
/// matching Java's `parseValues` defaults.
fn ranged_bounds(e: &quick_xml::events::BytesStart) -> Option<RangedRow> {
    let from_level = attr_i32(e, b"fromLevel");
    let from_sub = attr_i32(e, b"fromSubLevel");
    if from_level.is_none() && from_sub.is_none() {
        return None;
    }
    Some(RangedRow {
        from_level: from_level.unwrap_or(1),
        to_level: attr_i32(e, b"toLevel").unwrap_or(i32::MAX),
        from_sub: from_sub.unwrap_or(0),
        to_sub: attr_i32(e, b"toSubLevel").or(from_sub).unwrap_or(0),
        text: String::new(),
    })
}

fn parse_str(content: &str, out: &mut ParsedSkills) {
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
    let mut effects: Vec<ParsedEffect> = Vec::new();
    // The current `<effect>`'s level-range attributes (Java `NamedParamInfo`).
    let mut cur_effect_levels: (Option<i32>, Option<i32>, Option<i32>, Option<i32>) =
        (None, None, None, None);
    let mut in_effects = false;
    let mut cur_scope = EffectScope::General;
    let mut in_conditions = false;
    // The one parsed skill condition: `OpExistNpc` (see `Skill::op_exist_npc`).
    // Everything else under `<conditions>` is still skipped.
    let mut cur_cond_name: Option<String> = None;
    let mut cur_cond_field = String::new();
    let mut cur_cond_npc_ids: Vec<i32> = Vec::new();
    let mut cur_cond_range: i32 = 0;
    let mut cur_cond_is_around = false;
    let mut op_exist_npc: Option<crate::model::skill::OpExistNpcCondition> = None;
    // Ranged `<value>` rows (fromLevel/fromSubLevel bounds) — collected raw
    // per skill field / effect param, resolved at finalize.
    let mut pending_range: Option<RangedRow> = None;
    let mut field_rows: HashMap<String, Vec<RangedRow>> = HashMap::new();
    let mut cur_effect_sub_params: HashMap<String, Vec<RangedRow>> = HashMap::new();
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
                if in_effects
                    && cur_effect_field == "items"
                    && path.len() == 5
                    && e.name().as_ref() == b"item"
                {
                    if let (Some(item_id), Some(count)) =
                        (attr_i32(&e, b"id"), attr_i64(&e, b"count"))
                    {
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
                        let (from_level, to_level, from_sub_level, to_sub_level) =
                            effect_level_attrs(&e);
                        effects.push(ParsedEffect {
                            scope: cur_scope,
                            name: effect_name,
                            params: HashMap::new(),
                            sub_params: HashMap::new(),
                            mode: String::from("DIFF"),
                            groups: Vec::new(),
                            armor_condition: 0,
                            weapon_condition: 0,
                            from_level,
                            to_level,
                            from_sub_level,
                            to_sub_level,
                        });
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
                    op_exist_npc = None;
                    field_rows.clear();
                    pending_range = None;
                } else if path.len() == 1 {
                    cur_field = name.clone();
                    // Any `<*Effects>` block opens the effect section; which one
                    // it is decides the scope every effect inside gets.
                    if name.ends_with("ffects") {
                        in_effects = true;
                        cur_scope = EffectScope::from_xml(&name);
                    } else if name == "conditions" {
                        in_conditions = true;
                    }
                } else if path.len() == 2 && name == "value" && !in_effects && !in_conditions {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                    pending_range = ranged_bounds(&e);
                } else if path.len() == 2 && in_conditions && name == "condition" {
                    cur_cond_name = attr_str(&e, b"name");
                    cur_cond_npc_ids = Vec::new();
                    cur_cond_range = 0;
                    cur_cond_is_around = false;
                } else if path.len() == 3 && in_conditions {
                    cur_cond_field = name.clone();
                } else if path.len() == 2 && in_effects && name == "effect" {
                    cur_effect_name = attr_str(&e, b"name");
                    cur_effect_levels = effect_level_attrs(&e);
                    cur_effect_params = HashMap::new();
                    cur_effect_mode = String::from("DIFF");
                    cur_effect_armor = 0;
                    cur_effect_weapon = 0;
                    cur_restoration_groups = Vec::new();
                    cur_effect_sub_params = HashMap::new();
                } else if path.len() == 3 && in_effects {
                    cur_effect_field = name.clone();
                } else if path.len() == 4 && in_effects && name == "value" {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
                    pending_range = ranged_bounds(&e);
                } else if path.len() == 4
                    && in_effects
                    && cur_effect_field == "items"
                    && name == "item"
                {
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
                    // Only `OpExistNpc`'s fields are read; every other
                    // condition is still skipped (see module docs).
                    match (path.len(), cur_cond_field.as_str()) {
                        (4, "range") => cur_cond_range = text.parse().unwrap_or(0),
                        (4, "isAround") => cur_cond_is_around = text.eq_ignore_ascii_case("true"),
                        // `<npcIds><item>13018</item>…`
                        (5, "npcIds") => {
                            if let Ok(v) = text.parse() {
                                cur_cond_npc_ids.push(v);
                            }
                        }
                        _ => {}
                    }
                } else if in_effects && cur_effect_field == "armorType" && path.len() == 5 {
                    // `<effect><armorType><item>MAGIC</item>...` — OR each armor
                    // kind's bit into the effect's condition mask.
                    cur_effect_armor |=
                        crate::data::item_data::ArmorType::from_name(text).mask_bit();
                } else if in_effects && cur_effect_field == "weaponType" && path.len() == 5 {
                    // `<effect><weaponType><item>BOW</item>...` — OR each weapon
                    // kind's bit into the effect's weapon-condition mask.
                    cur_effect_weapon |=
                        crate::data::item_data::WeaponType::from_name(text).mask_bit();
                } else if in_effects {
                    match path.len() {
                        4 if cur_effect_field == "mode" => {
                            cur_effect_mode = text.to_string();
                        }
                        // Directly under `<effect><param>SCALAR</param>`.
                        4 => {
                            cur_effect_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .insert(0, text.to_string());
                        }
                        // `<effect><param><value fromLevel=… [fromSubLevel=…]>`
                        // — a ranged (possibly computed) row.
                        5 if pending_range.is_some() => {
                            let mut row = pending_range.take().expect("checked");
                            row.text = text.to_string();
                            cur_effect_sub_params
                                .entry(cur_effect_field.clone())
                                .or_default()
                                .push(row);
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
                            values
                                .entry(cur_field.clone())
                                .or_default()
                                .insert(0, text.to_string());
                        }
                        // `<field><value fromLevel=… [fromSubLevel=…]>` — a
                        // ranged (possibly computed) row. Before this branch
                        // these rows fell into the level-0 slot below, where
                        // the last row's `{…}` text clobbered the field's
                        // scalar fallback.
                        3 if pending_range.is_some() => {
                            let mut row = pending_range.take().expect("checked");
                            row.text = text.to_string();
                            field_rows.entry(cur_field.clone()).or_default().push(row);
                        }
                        // `<field><value level="N">...`
                        3 => {
                            values
                                .entry(cur_field.clone())
                                .or_default()
                                .insert(pending_level, text.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                let closed = path.pop().unwrap_or_default();
                if closed == "skill" {
                    finalize_skill(
                        skill_id,
                        &skill_name,
                        to_level,
                        &values,
                        &effects,
                        &field_rows,
                        &op_exist_npc,
                        out,
                    );
                    skill_id = -1;
                } else if closed.ends_with("ffects") {
                    in_effects = false;
                } else if closed == "conditions" {
                    in_conditions = false;
                } else if closed == "condition" && in_conditions {
                    if cur_cond_name.take().as_deref() == Some("OpExistNpc") {
                        op_exist_npc = Some(crate::model::skill::OpExistNpcCondition {
                            npc_ids: std::mem::take(&mut cur_cond_npc_ids),
                            range: cur_cond_range,
                            is_around: cur_cond_is_around,
                        });
                    }
                } else if closed == "item" && in_effects && cur_effect_field == "items" {
                    // Closes a `RestorationRandom` group (the inner
                    // `<item id=".." count=".."/>` is self-closing, so this
                    // `End` only ever fires for the outer group tag).
                    cur_restoration_groups.push(RestorationGroup {
                        chance: cur_group_chance,
                        items: std::mem::take(&mut cur_group_items),
                    });
                } else if closed == "effect"
                    && in_effects
                    && let Some(name) = cur_effect_name.take()
                {
                    effects.push(ParsedEffect {
                        scope: cur_scope,
                        name,
                        params: cur_effect_params.clone(),
                        sub_params: std::mem::take(&mut cur_effect_sub_params),
                        mode: cur_effect_mode.clone(),
                        groups: std::mem::take(&mut cur_restoration_groups),
                        armor_condition: cur_effect_armor,
                        weapon_condition: cur_effect_weapon,
                        from_level: cur_effect_levels.0,
                        to_level: cur_effect_levels.1,
                        from_sub_level: cur_effect_levels.2,
                        to_sub_level: cur_effect_levels.3,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

/// Which `<*Effects>` block an effect was declared in — Java `EffectScope`.
///
/// `START` and `END` are parsed as [`Self::Other`] and dropped: they hang off
/// lifecycle hooks this port doesn't have (cast start, buff end). `CHANNELING`
/// feeds `Skill.channeling_effects`, applied per `ChannelingTick`
/// (PLAN_G19_GROUND_CHANNELING.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectScope {
    General,
    SelfScope,
    Pve,
    Pvp,
    Channeling,
    Other,
}

impl EffectScope {
    fn from_xml(node: &str) -> Self {
        match node {
            "effects" => Self::General,
            "selfEffects" => Self::SelfScope,
            "pveEffects" => Self::Pve,
            "pvpEffects" => Self::Pvp,
            "channelingEffects" => Self::Channeling,
            _ => Self::Other,
        }
    }
}

/// One `<effect>` element as parsed, before it is resolved into a per-level
/// [`Skill`]. Java's `SkillData.NamedParamInfo`.
#[derive(Clone)]
struct ParsedEffect {
    scope: EffectScope,
    name: String,
    params: LeveledValues,
    /// Ranged `<value>` rows inside this effect's params, resolved per
    /// (level, sub) at finalize.
    sub_params: HashMap<String, Vec<RangedRow>>,
    mode: String,
    groups: Vec<RestorationGroup>,
    armor_condition: u8,
    weapon_condition: u32,
    /// `fromLevel`/`toLevel`, with the `level` attribute as the default for
    /// **both** (Java: `parseInteger(attributes, "fromLevel", level)`), so
    /// `level="3"` means exactly level 3.
    from_level: Option<i32>,
    to_level: Option<i32>,
    /// `fromSubLevel`/`toSubLevel`, likewise defaulting from `subLevel`.
    /// Sub-levels are the **skill-enchant** routes (1001+/2001+); this port has
    /// no enchanted skills, so a sub-level always reads as 0 and any effect
    /// gated on a positive range never applies — see [`Self::applies_at`].
    from_sub_level: Option<i32>,
    to_sub_level: Option<i32>,
}

impl ParsedEffect {
    /// Java `SkillData.forEachNamedParamInfoParam`'s gate:
    ///
    /// ```java
    /// ((fromLevel == null && toLevel == null) || (fromLevel <= level && toLevel >= level))
    ///   && ((fromSubLevel == null && toSubLevel == null) || (fromSubLevel <= subLevel && toSubLevel >= subLevel))
    /// ```
    ///
    /// An effect with no level attributes applies at every level — which is
    /// what the parser previously assumed for *all* effects, so the 775
    /// level-gated elements in this datapack were applying outside their
    /// range. Frenzy 176's `PAtk`/`CriticalRate` (`fromLevel="6" toLevel="9"`)
    /// were live at levels 1-5, for instance.
    ///
    /// `sub` is 0 for the unenchanted skill — the sub-level clause then
    /// rejects every effect that names a range (all of which start at 1001+),
    /// matching Java; the enchanted variants pass their real sub-level.
    fn applies_at(&self, level: i32, sub: i32) -> bool {
        let sub_level: i32 = sub;
        let level_ok = match (self.from_level, self.to_level) {
            (None, None) => true,
            (from, to) => from.is_none_or(|f| f <= level) && to.is_none_or(|t| t >= level),
        };
        let sub_ok = match (self.from_sub_level, self.to_sub_level) {
            (None, None) => true,
            (from, to) => from.is_none_or(|f| f <= sub_level) && to.is_none_or(|t| t >= sub_level),
        };
        level_ok && sub_ok
    }
}

fn finalize_skill(
    id: i32,
    name: &str,
    to_level: i32,
    values: &LeveledValues,
    effects: &[ParsedEffect],
    field_rows: &HashMap<String, Vec<RangedRow>>,
    op_exist_npc: &Option<crate::model::skill::OpExistNpcCondition>,
    out: &mut ParsedSkills,
) {
    if id < 0 {
        return;
    }
    for level in 1..=to_level {
        // The plain (sub 0) skill: ranged level rows resolved, sub rows inert.
        let vals = patched_values(values, field_rows, level, 0);
        let effs = patched_effects(effects, level, 0);
        out.skills.insert(
            (id, level),
            build_skill(id, name, level, 0, &vals, &effs, op_exist_npc),
        );

        // The enchanted variants — one instance per declared sub-level, like
        // Java's parse loop, plus the route registration `addRouteForSkill`
        // does per instance.
        for (from_sub, to_sub) in declared_sub_ranges(field_rows, effects, level) {
            out.routes
                .entry((id, level))
                .or_default()
                .push((from_sub, to_sub));
            for sub in from_sub..=to_sub {
                let vals = patched_values(values, field_rows, level, sub);
                let effs = patched_effects(effects, level, sub);
                out.enchanted.insert(
                    (id, level, sub),
                    build_skill(id, name, level, sub, &vals, &effs, op_exist_npc),
                );
            }
        }
    }
}

/// Resolve `values` for one (level, sub): ranged level rows first (they form
/// the level's base — and `{N+index}` magic-level tables now actually parse),
/// then, for `sub > 0`, the matching enchant-route rows on top. `base` in an
/// expression is the field's value *before* the row applies, `index`/`subIndex`
/// are 1-based offsets into the row's ranges — Java `SkillData.parseValues`.
fn patched_values(
    values: &LeveledValues,
    field_rows: &HashMap<String, Vec<RangedRow>>,
    level: i32,
    sub: i32,
) -> LeveledValues {
    if field_rows.is_empty() {
        return values.clone();
    }
    let mut out = values.clone();
    for pass_sub in [false, true] {
        if pass_sub && sub == 0 {
            break;
        }
        for (field, rows) in field_rows {
            for r in rows {
                let is_sub_row = r.from_sub > 0;
                if is_sub_row != pass_sub || !(r.from_level <= level && level <= r.to_level) {
                    continue;
                }
                if is_sub_row && !(r.from_sub <= sub && sub <= r.to_sub) {
                    continue;
                }
                if let Some(resolved) = resolve_row(&out, field, r, level, sub) {
                    out.entry(field.clone())
                        .or_default()
                        .insert(level, resolved);
                }
            }
        }
    }
    out
}

/// Resolve one ranged row's text: `{…}` through the expression evaluator
/// (`None` drops the row, like Java's exception path), plain text verbatim.
fn resolve_row(
    current: &LeveledValues,
    field: &str,
    r: &RangedRow,
    level: i32,
    sub: i32,
) -> Option<String> {
    let text = r.text.trim();
    if !text.starts_with('{') {
        return Some(text.to_string());
    }
    let vars = crate::data::skill_expr::ExprVars {
        base: value_at(current, field, level).and_then(|v| v.parse().ok()),
        index: (level - r.from_level + 1) as f64,
        sub_index: if r.from_sub > 0 {
            (sub - r.from_sub + 1) as f64
        } else {
            0.0
        },
    };
    crate::data::skill_expr::eval_braced(text, vars).map(fmt_num)
}

/// Format an evaluated number so the downstream `parse::<i32>()`/`::<f64>()`
/// readers both work: whole values print bare (`214`, not `214.0`).
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}

/// The same two-pass resolution over each effect's ranged param rows.
fn patched_effects(effects: &[ParsedEffect], level: i32, sub: i32) -> Vec<ParsedEffect> {
    effects
        .iter()
        .map(|e| {
            if e.sub_params.is_empty() {
                return e.clone();
            }
            let mut out = e.clone();
            for pass_sub in [false, true] {
                if pass_sub && sub == 0 {
                    break;
                }
                for (field, rows) in &e.sub_params {
                    for r in rows {
                        let is_sub_row = r.from_sub > 0;
                        if is_sub_row != pass_sub || !(r.from_level <= level && level <= r.to_level)
                        {
                            continue;
                        }
                        if is_sub_row && !(r.from_sub <= sub && sub <= r.to_sub) {
                            continue;
                        }
                        if let Some(resolved) = resolve_row(&out.params, field, r, level, sub) {
                            out.params
                                .entry(field.clone())
                                .or_default()
                                .insert(level, resolved);
                        }
                    }
                }
            }
            out
        })
        .collect()
}

/// The distinct enchant-route sub-level ranges declared for this level —
/// from field rows, effect param rows and effect-level sub gates.
fn declared_sub_ranges(
    field_rows: &HashMap<String, Vec<RangedRow>>,
    effects: &[ParsedEffect],
    level: i32,
) -> Vec<(i32, i32)> {
    // Rows within one route are often fragmented (`1001–1005`, `1006–1006`,
    // …), so merge by the route bucket (`sub / 1000`) — the registry's unit
    // is the route, and instances are built over the merged span.
    let mut buckets: std::collections::BTreeMap<i32, (i32, i32)> =
        std::collections::BTreeMap::new();
    let mut add = |from_sub: i32, to_sub: i32| {
        let e = buckets.entry(from_sub / 1000).or_insert((from_sub, to_sub));
        e.0 = e.0.min(from_sub);
        e.1 = e.1.max(to_sub);
    };
    let covers = |from: i32, to: i32| from <= level && level <= to;
    for rows in field_rows.values() {
        for r in rows {
            if r.from_sub > 0 && covers(r.from_level, r.to_level) {
                add(r.from_sub, r.to_sub);
            }
        }
    }
    for e in effects {
        for rows in e.sub_params.values() {
            for r in rows {
                if r.from_sub > 0 && covers(r.from_level, r.to_level) {
                    add(r.from_sub, r.to_sub);
                }
            }
        }
        if let Some(from_sub) = e.from_sub_level.filter(|&f| f > 0) {
            let level_ok = match (e.from_level, e.to_level) {
                (None, None) => true,
                (f, t) => f.is_none_or(|f| f <= level) && t.is_none_or(|t| t >= level),
            };
            if level_ok {
                add(from_sub, e.to_sub_level.unwrap_or(from_sub));
            }
        }
    }
    buckets.into_values().collect()
}

fn build_skill(
    id: i32,
    name: &str,
    level: i32,
    sub: i32,
    values: &LeveledValues,
    effects: &[ParsedEffect],
    op_exist_npc: &Option<crate::model::skill::OpExistNpcCondition>,
) -> Skill {
    {
        // Integer reads fall back through f64 truncation — an enchant-route
        // expression can evaluate fractionally (`Curse Gloom +1` abnormalTime
        // = 10.5) and Java's `StatSet.getInt` truncates via `Number.intValue`.
        let get_i = |field: &str, default: i32| {
            value_at(values, field, level)
                .and_then(|v| {
                    v.parse::<i32>()
                        .ok()
                        .or_else(|| v.parse::<f64>().ok().map(|f| f as i32))
                })
                .unwrap_or(default)
        };
        let get_f = |field: &str, default: f64| {
            value_at(values, field, level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let operate_type = match value_at(values, "operateType", level) {
            Some("A1") | Some("A2") => OperateType::Active,
            Some("P") => OperateType::Passive,
            Some("T") => OperateType::Toggle,
            // `SkillOperateType.isChanneling()`: CA1. (CA5 doesn't occur on
            // this dist's reachable content — it stays `Other`.)
            Some("CA1") => OperateType::Channeling,
            _ => OperateType::Other,
        };
        // Java `SkillOperateType.isContinuous()` — the A2..A6/DA2..DA5 family.
        // `OperateType` above collapses A1 and A2 into `Active` (the cast
        // pipeline treats them alike), so continuity is read from the raw
        // string instead of derived from it. The NPC AI needs it to tell a
        // buff/debuff apart from an instant nuke when bucketing skills.
        let is_continuous = matches!(
            value_at(values, "operateType", level),
            Some("A2" | "A3" | "A4" | "A5" | "A6" | "DA2" | "DA4" | "DA5")
        );
        let target_type = match value_at(values, "targetType", level) {
            Some("SELF") => TargetType::Self_,
            Some("TARGET") => TargetType::Target,
            Some("ENEMY") => TargetType::Enemy,
            Some("ENEMY_ONLY") => TargetType::EnemyOnly,
            Some("ENEMY_NOT") => TargetType::EnemyNot,
            Some("NPC_BODY") => TargetType::NpcBody,
            Some("SUMMON") => TargetType::Summon,
            Some("PC_BODY") => TargetType::PcBody,
            Some("GROUND") => TargetType::Ground,
            Some("NONE") => TargetType::None_,
            _ => TargetType::Other,
        };
        // `<abnormalVisualEffect>` is a `;`-separated list of enum names.
        let abnormal_visuals: Vec<i16> = value_at(values, "abnormalVisualEffect", level)
            .unwrap_or("")
            .split(';')
            .filter_map(|n| crate::model::skill::abnormal_visual_client_id(n.trim()))
            .collect();
        // `overHit` is an **effect** parameter, not a skill field — the damage
        // handlers (Backstab, EnergyAttack, PhysicalAttack, …) each read
        // `params.getBoolean("overHit", false)`. A skill carries at most one
        // damage effect in practice, so hoisting "any effect declares it" to the
        // skill is behaviourally identical and avoids threading the flag
        // through every `SkillEffect` variant.
        let over_hit = effects
            .iter()
            .filter(|e| e.applies_at(level, sub))
            .any(|e| {
                value_at(&e.params, "overHit", level)
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
            });
        let toggle_group_id = get_i("toggleGroupId", 0);
        // `<trait>` — the debuff's own trait, matched against the target's
        // `DefenceTrait` resistances when it lands.
        let trait_type = value_at(values, "trait", level)
            .map(crate::model::skill::TraitType::from_xml)
            .unwrap_or_default();
        // `affectScope` defaults to SINGLE when absent (Java's Skill ctor).
        let affect_scope = match value_at(values, "affectScope", level) {
            Some("RANGE") => AffectScope::Range,
            Some("POINT_BLANK") => AffectScope::PointBlank,
            Some("PARTY") => AffectScope::Party,
            Some("PLEDGE") => AffectScope::Pledge,
            Some("DEAD_PLEDGE") => AffectScope::DeadPledge,
            Some("DEAD_PARTY") => AffectScope::DeadParty,
            Some("DEAD_UNION") => AffectScope::DeadUnion,
            Some("FAN") => AffectScope::Fan,
            Some("FAN_PB") => AffectScope::FanPointBlank,
            Some("SQUARE") => AffectScope::Square,
            Some("SQUARE_PB") => AffectScope::SquarePointBlank,
            Some("RING_RANGE") => AffectScope::RingRange,
            Some("SINGLE") | Some("NONE") | None => AffectScope::Single,
            _ => AffectScope::Other,
        };
        // `affectObject` defaults to ALL. `*_PC` narrows Java's check to
        // players only; with no non-player creature able to be a "friend" in
        // the ported world they collapse onto the same filter.
        let affect_object = match value_at(values, "affectObject", level) {
            Some("NOT_FRIEND") | Some("NOT_FRIEND_PC") => AffectObject::NotFriend,
            Some("FRIEND") | Some("FRIEND_PC") => AffectObject::Friend,
            Some("CLAN") => AffectObject::Clan,
            Some("ALL") | None => AffectObject::All,
            _ => AffectObject::Other,
        };
        let affect_range = get_i("affectRange", 0);
        // `<affectLimit>min-max</affectLimit>`; a bare value sets min only.
        let affect_limit = value_at(values, "affectLimit", level)
            .map(|v| {
                let mut parts = v.split('-').map(|p| p.trim().parse::<i32>().unwrap_or(0));
                (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
            })
            .unwrap_or((0, 0));
        // `<fanRange>unk;startDegree;fanAffectRange;fanAffectAngle</fanRange>`
        // (Java splits on ';' into `_fanRange[4]`); level-valued in the XML.
        let fan_range = value_at(values, "fanRange", level)
            .map(|v| {
                let mut out = [0i32; 4];
                for (slot, part) in out.iter_mut().zip(v.split(';')) {
                    *slot = part.trim().parse().unwrap_or(0);
                }
                out
            })
            .unwrap_or([0; 4]);

        let build_scope = |want: EffectScope| {
            effects
                .iter()
                // Java `forEachNamedParamInfoParam`: an effect whose declared level
                // range excludes this level is simply not part of the skill here.
                .filter(|e| e.applies_at(level, sub) && e.scope == want)
                .flat_map(|e| {
                    let (xml_name, params, mode, groups, armor_condition, weapon_condition) = (
                        &e.name,
                        &e.params,
                        &e.mode,
                        &e.groups,
                        &e.armor_condition,
                        &e.weapon_condition,
                    );
                    let param = |key: &str| -> Option<f64> {
                        value_at(params, key, level).and_then(|v| v.parse().ok())
                    };
                    let modifier_mode = if mode == "PER" {
                        StatModifierType::Per
                    } else {
                        StatModifierType::Diff
                    };
                    let stat_mod = |stat: Stat, amount: f64| {
                        SkillEffect::StatModifier(StatModifierEffect {
                            stat,
                            mode: modifier_mode,
                            amount,
                            armor_condition: *armor_condition,
                            weapon_condition: *weapon_condition,
                            qualifier: None,
                            two_handed: false,
                        })
                    };
                    match xml_name.as_str() {
                        // Vital Force (148), Esprit (171), Acrobatic Move (225),
                        // Clear Mind (1297): a flat stat bonus that only counts
                        // while the creature is in the named locomotion state.
                        // Java names its own `<stat>`/`<type>`/`<value>` rather
                        // than using the generic `amount`/`mode` pair, and merges
                        // into `_moveTypeStats` — always additive, never percent —
                        // so `modifier_mode` is deliberately not consulted.
                        //
                        // Before this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and was dropped: Vital
                        // Force and Clear Mind carry *only* `StatByMoveType`, so
                        // both were passives that did precisely nothing.
                        "StatByMoveType" => {
                            let stat = value_at(params, "stat", level).and_then(Stat::from_xml);
                            let move_type = value_at(params, "type", level)
                                .and_then(crate::model::stats::MoveType::from_xml);
                            match (stat, move_type, param("value")) {
                                (Some(stat), Some(move_type), Some(amount)) => {
                                    vec![SkillEffect::StatModifier(StatModifierEffect {
                                        stat,
                                        mode: StatModifierType::Diff,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: Some(
                                            crate::model::stats::StatQualifier::MoveType(move_type),
                                        ),
                                        two_handed: false,
                                    })]
                                }
                                _ => Vec::new(),
                            }
                        }
                        // Guts (139) / Touch of Life (341) / Touch of Death (342):
                        // a multiplier on how likely an incoming *debuff* is to
                        // land. Java `mergeMul(RESIST_ABNORMAL_DEBUFF,
                        // 1 + amount/100)` — which is exactly what `Per` mode does
                        // here — so the mode is forced rather than read from the
                        // XML (these effects carry no `<mode>`, which would default
                        // to DIFF and silently mean something else entirely).
                        //
                        // Java's handler switches on `<slot>` and only implements
                        // DEBUFF ("only this one is in use it seems"); a different
                        // slot pumps nothing, so it is skipped here too.
                        "ResistAbnormalByCategory" => {
                            let slot = value_at(params, "slot", level).unwrap_or("DEBUFF");
                            param("amount")
                                .filter(|_| slot == "DEBUFF")
                                .map(|amount| {
                                    SkillEffect::StatModifier(StatModifierEffect {
                                        stat: Stat::ResistAbnormalDebuff,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        // Ultimate Defense (110) / Ultimate Evasion (111): the same
                        // shape for resisting *dispel*. Java only implements the
                        // BUFF slot.
                        "ResistDispelByCategory" => {
                            let slot = value_at(params, "slot", level).unwrap_or("BUFF");
                            param("amount")
                                .filter(|_| slot == "BUFF")
                                .map(|amount| {
                                    SkillEffect::StatModifier(StatModifierEffect {
                                        stat: Stat::ResistDispelBuff,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        // Prophecy family / Heroic Miracle: block a set of abnormal
                        // types from landing while this buff is up.
                        "BlockAbnormalSlot" => {
                            let slots: Vec<String> = value_at(params, "slot", level)
                                .unwrap_or("")
                                .split(';')
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect();
                            if slots.is_empty() {
                                return Vec::new();
                            }
                            vec![SkillEffect::BlockAbnormalSlot { slots }]
                        }
                        // Stun / sleep / paralyze (540 uses) and Root (79): no stat
                        // modifier at all — the whole mechanic is the abnormal-state
                        // flag they contribute (`Skill::effect_flags`).
                        "BlockActions" => {
                            // Java: a non-empty `allowedSkills` whitelist makes this
                            // CONDITIONAL_BLOCK_ACTIONS instead. Both gate the same
                            // way in `hasBlockActions()`.
                            let conditional = value_at(params, "allowedSkills", level)
                                .is_some_and(|v| !v.trim().is_empty());
                            vec![SkillEffect::BlockActions { conditional }]
                        }
                        "Root" => vec![SkillEffect::Root],
                        // The elemental attribute pair (PLAN_G19_ATTRIBUTES.md):
                        // one flat StatModifier per element named in the
                        // (comma-separable) `attribute` param, default FIRE —
                        // Java's `Stat.valueOf(attribute + "_POWER"/"_RES")`.
                        "AttackAttribute" | "DefenceAttribute" => {
                            let Some(amount) = param("amount") else {
                                return Vec::new();
                            };
                            let defence = xml_name.as_str() == "DefenceAttribute";
                            value_at(params, "attribute", level)
                                .unwrap_or("FIRE")
                                .split(',')
                                .filter_map(|n| crate::model::stats::Element::from_xml(n.trim()))
                                .map(|el| {
                                    stat_mod(
                                        if defence {
                                            el.res_stat()
                                        } else {
                                            el.power_stat()
                                        },
                                        amount,
                                    )
                                })
                                .collect()
                        }
                        // Polearm Mastery 216: `HitNumber` is a plain
                        // AbstractStatEffect over ATTACK_COUNT_MAX (amount 5).
                        "HitNumber" => param("amount")
                            .map(|amount| stat_mod(Stat::AttackCountMax, amount))
                            .into_iter()
                            .collect(),
                        // The rest of the state-flag CC family (Seal of Silence,
                        // Shield Slam, Mystic Immunity, Horror): no parameters, the
                        // mechanic is entirely the flag.
                        "Mute" => vec![SkillEffect::Mute],
                        "PhysicalMute" => vec![SkillEffect::PhysicalMute],
                        "DebuffBlock" => vec![SkillEffect::DebuffBlock],
                        "BlockControl" => vec![SkillEffect::BlockControl],
                        "TargetCancel" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![SkillEffect::TargetCancel { chance }]
                        }
                        // Aggression 28/18, Judgment 401, Tribunal 400: no params.
                        "GetAgro" => vec![SkillEffect::GetAgro],
                        // Charm 15, Lure 51: `power` (default 0, Java always
                        // instantiates the handler even with no param).
                        "AddHate" => {
                            vec![SkillEffect::AddHate {
                                power: param("power").unwrap_or(0.0),
                            }]
                        }
                        "DeleteHate" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![SkillEffect::DeleteHate { chance }]
                        }
                        "DeleteHateOfMe" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![SkillEffect::DeleteHateOfMe { chance }]
                        }
                        // TODO(G19+): `TargetMe` (paired with `GetAgro` on
                        // Aggression 28/Aggression Aura 18) and `RandomizeHate`
                        // (Confusion 2, Switch 12) fall through here unregistered
                        // and are dropped — see PLAN_G19_HATE_EFFECTS.md's
                        // "Deferred" section for why (a locked-target UI concept
                        // and a general nearby-visible-creatures query,
                        // respectively, neither of which exists on this port yet).
                        // Java instantiates these handlers whenever the `<effect>` is
                        // present and reads `params.getDouble("power", 0)` — the
                        // effect is always created, `power` defaulting to 0 when the
                        // param is absent (e.g. skills 1011/4717/4718, whose
                        // `<item>power</item>` parses to the param key `item`, not
                        // `power`). Mirror that default here; do NOT drop the effect,
                        // or the skill becomes a silent no-op.
                        "MagicalAttack" => vec![SkillEffect::MagicalAttack {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // The EffectPoint totem spawner (Symbol of Noise 455, Day
                        // of Doom 1422, Anti-summoning Field 1424; PLAN_G19_SYMBOLS.md).
                        "SummonNpc" => vec![SkillEffect::SummonNpc {
                            npc_id: param("npcId").unwrap_or(0.0) as i32,
                            npc_count: param("npcCount").unwrap_or(1.0) as i32,
                            despawn_delay: param("despawnDelay").unwrap_or(0.0) as i32,
                        }],
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
                        "MagicalAttackRange" => vec![SkillEffect::MagicalAttack {
                            power: param("power").unwrap_or(0.0),
                        }],
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
                        "MagicalSoulAttack" => vec![SkillEffect::MagicalAttack {
                            power: param("power").unwrap_or(0.0),
                        }],
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
                        // Periodic HP / MP effects riding the same tick chain as DamOverTime.
                        // `HealEffect` scales the healing its bearer *receives* — a two-stat
                        // AbstractStatEffect like CriticalDamage: PER feeds the multiplier,
                        // DIFF the flat addend.
                        "HealEffect" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::HealEffect
                                } else {
                                    Stat::HealEffectAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        // Instant CP change (Braveheart, Wrath, Touch of Death).
                        "Cp" => param("amount")
                            .map(|amount| SkillEffect::Cp {
                                amount,
                                percent: modifier_mode == StatModifierType::Per,
                            })
                            .into_iter()
                            .collect(),
                        "HealOverTime" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![SkillEffect::HealOverTime { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "ManaDamOverTime" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![SkillEffect::ManaDamOverTime { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "DamOverTime" => vec![SkillEffect::DamOverTime {
                            power: param("power").unwrap_or(0.0),
                            ticks: param("ticks").unwrap_or(0.0) as i32,
                            can_kill: value_at(params, "canKill", level) == Some("true"),
                        }],
                        // Dagger blows (calcBlowDamage). FatalBlow/Backstab roll
                        // `criticalChance` (default 0) to double; SoulBlow doesn't
                        // (its charged-soul boost is unmodeled → ×1). Backstab also
                        // requires flanking. Their `Lethal` sibling effect is a
                        // separate `<effect>` block, parsed in its own arm below.
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
                        // Backstab (30), Lethal Blow (344), Deadly Blow (263),
                        // Critical Blow (409), Lethal Shot (343), Turn/Banish
                        // Undead/Seraph (1400/405/450): without this arm the
                        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
                        // and the bonus instant-kill/half-kill chance never
                        // rolled — only these skills' other (already-ported)
                        // effect landed.
                        "Lethal" => vec![SkillEffect::Lethal {
                            full_lethal: param("fullLethal").unwrap_or(0.0),
                            half_lethal: param("halfLethal").unwrap_or(0.0),
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
                        "PhysicalAttack" | "PhysicalSoulAttack" => {
                            vec![SkillEffect::PhysicalAttack {
                                power: param("power").unwrap_or(0.0),
                                p_atk_mod: param("pAtkMod").unwrap_or(1.0),
                                p_def_mod: param("pDefMod").unwrap_or(1.0),
                                critical_chance: param("criticalChance").unwrap_or(10.0),
                            }]
                        }
                        "Heal" => vec![SkillEffect::Heal {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Miracle (1426), Benediction (1271), Restore Life (1258),
                        // Revival (181), Touch of Life (341): without this arm the
                        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
                        // and the heal amount was silently 0.
                        "HealPercent" => vec![SkillEffect::HealPercent {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Sonic Focus (8), Focus Force (50), Sonic Rage (345), …:
                        // without this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and the "build Force"
                        // toggle/skill did nothing.
                        "FocusMomentum" => vec![SkillEffect::FocusMomentum {
                            amount: param("amount").unwrap_or(1.0) as i32,
                            max_charges: param("maxCharges").unwrap_or(0.0) as i32,
                        }],
                        // Double Sonic Slash (5), Sonic Blaster (6), Force Burst
                        // (17), …: `chargeConsume` is a *skill-level* tag (a
                        // sibling of `<targetType>`), not a child of the
                        // `<effect name="EnergyAttack">` element itself — Java's
                        // effect constructors read the skill's whole merged param
                        // set, so it reaches `_chargeConsume` the same way. Without
                        // this arm the effect fell through to `EFFECT_REGISTRY`,
                        // wasn't found, and every Force-spend attack did nothing.
                        "EnergyAttack" => vec![SkillEffect::EnergyAttack {
                            power: param("power").unwrap_or(0.0),
                            critical_chance: param("criticalChance").unwrap_or(10.0),
                            p_def_mod: param("pDefMod").unwrap_or(1.0),
                            charge_consume: value_at(values, "chargeConsume", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        }],
                        // Pet food (Wolf Food 2048, etc.). Without this arm the
                        // food item was consumed and restored nothing.
                        "Feed" => vec![SkillEffect::Feed {
                            normal: param("normal").unwrap_or(0.0) as i32,
                        }],
                        "SummonCubic" => vec![SkillEffect::SummonCubic {
                            cubic_id: param("cubicId").unwrap_or(-1.0) as i32,
                            cubic_level: param("cubicLvl").unwrap_or(0.0) as i32,
                        }],
                        "Restoration" => match (param("itemId"), param("itemCount")) {
                            (Some(item_id), Some(item_count)) => vec![SkillEffect::GiveItem {
                                item_id: item_id as i32,
                                item_count: item_count as i64,
                                item_enchant_level: param("itemEnchantmentLevel").unwrap_or(0.0)
                                    as i32,
                            }],
                            _ => Vec::new(),
                        },
                        "RestorationRandom" => vec![SkillEffect::GiveItemRandom {
                            groups: groups.clone(),
                        }],
                        // Spoil (254/…): mark the mob spoiled. No params — the
                        // landing roll and target checks live in the effect handler.
                        "Spoil" => vec![SkillEffect::Spoil],
                        // Sweeper (42/474): claim the dead mob's spoil loot.
                        "Sweeper" => vec![SkillEffect::Sweeper],
                        // ConsumeBody (paired with Sweeper on 42): decay the corpse.
                        "ConsumeBody" => vec![SkillEffect::ConsumeBody],
                        // Sow (2097): the manor sow, cast via a Seed item.
                        "Sow" => vec![SkillEffect::Sow],
                        // Harvesting (2098): claim a sown corpse's crop.
                        "Harvesting" => vec![SkillEffect::Harvesting],
                        // Cure Poison/Bleeding etc.: the `<dispel>` string is a
                        // per-level `"TYPE,level"` list (Java splits on ';' then ',').
                        // Falls through to an empty effect (silent no-op, like other
                        // unhandled bodies) if the string is missing/unparseable,
                        // rather than dropping the whole cast. Without this arm the
                        // effect fell through to `EFFECT_REGISTRY`, wasn't found, and
                        // got dropped — the skill cast but cured nothing.
                        // The Bane family: `<dispel>` is a plain `;` list of
                        // abnormal types (no `,level` suffix) plus a `<rate>`.
                        "DispelBySlotProbability" => {
                            let dispel: Vec<String> = value_at(params, "dispel", level)
                                .unwrap_or("")
                                .split(';')
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect();
                            if dispel.is_empty() {
                                return Vec::new();
                            }
                            let rate = value_at(params, "rate", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![SkillEffect::DispelBySlotProbability { dispel, rate }]
                        }
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
                        // The "Cancel" family: Cancellation 1056/Touch of Death
                        // 342 (BUFF, rate 25, max 5), Cleanse 1409/Purification
                        // Field 1425 (DEBUFF, rate 100, max 10). `slot` defaults
                        // to BUFF (Java's `DispelSlotType` default) when absent.
                        "DispelByCategory" => {
                            let slot = match value_at(params, "slot", level) {
                                Some("DEBUFF") => DispelSlot::Debuff,
                                Some("ALL") => DispelSlot::All,
                                _ => DispelSlot::Buff,
                            };
                            let rate = value_at(params, "rate", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(0);
                            let max = value_at(params, "max", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(0);
                            vec![SkillEffect::DispelByCategory { slot, rate, max }]
                        }
                        // Both the basic (247) and advanced HQ skills carry this;
                        // isAdvanced is not yet behaviorally distinct (see the effect).
                        "HeadquarterCreate" => vec![SkillEffect::CreateHeadquarter],
                        // "Common Craft" (1322) / "Dwarven Craft" (1321): param-less
                        // self-closing effects whose whole job is to open the recipe
                        // window. Without these arms both skills parsed to zero
                        // effects and the cast did nothing.
                        "OpenCommonRecipeBook" => {
                            vec![SkillEffect::OpenRecipeBook { dwarven: false }]
                        }
                        "OpenDwarfRecipeBook" => {
                            vec![SkillEffect::OpenRecipeBook { dwarven: true }]
                        }
                        // Java throws if amount is 0/missing; we drop the effect
                        // (silent no-op) to match how other bad effect bodies fall
                        // through, rather than panicking at data-load.
                        "GiveRecommendation" => match param("amount") {
                            Some(amount) if amount != 0.0 => {
                                vec![SkillEffect::GiveRecommendation {
                                    amount: amount as i32,
                                }]
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
                            Some(amount) => [
                                Stat::RunSpeed,
                                Stat::WalkSpeed,
                                Stat::SwimRunSpeed,
                                Stat::SwimWalkSpeed,
                            ]
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
                        // Noblesse Blessing (1323): no params, no stat modifier —
                        // the whole mechanic is the `NOBLESS_BLESSING` flag the
                        // death path reads. Without this arm the effect fell through
                        // to `EFFECT_REGISTRY`, wasn't found, and the buff was
                        // dropped whole (the skill cast but nothing landed).
                        "NoblesseBless" => vec![SkillEffect::NoblesseBless],
                        // Fear (65/405/450/1092/1169/1272/1381/1400): forced flight.
                        // The `<effect name="Fear"/>` element carries no params in
                        // this dist — Java's `Fear` constructor ignores its `StatSet`
                        // outright and `getTicks()` returns a hard-coded 5 — so the
                        // cadence is a literal, not a parsed value. Every one of
                        // these skills also carries `BlockControl`, so the *buff*
                        // already landed before this arm existed (icon, duration and
                        // the `BLOCK_CONTROL` flag); what was missing was the flight
                        // itself, so the debuff simply never moved anyone.
                        "Fear" => vec![SkillEffect::Fear { ticks: FEAR_TICKS }],
                        // Silent Move 221, Stealth 411, Dance of Shadows 366, and
                        // the stealth half of Fake Death 60. Java's handler is an
                        // empty constructor plus `getEffectFlags` — a pure state
                        // flag, no params at all.
                        // Mana Burn 1398, Mana Storm 1399, Aura Sink 1102, Seal of
                        // Gloom 1210 — MP drain. `critical`/`criticalLimit` are the
                        // effect's own params (all four declare `critical=true`);
                        // the crit *rate* comes from the skill's
                        // `<magicCriticalRate>`, not from here.
                        //
                        // Mana Burn and Mana Storm carry only this effect, so before
                        // this arm both parsed to an empty effect list and were
                        // dropped whole — the nukes cast and drained nothing.
                        "MagicalAttackMp" => vec![SkillEffect::MagicalAttackMp {
                            power: param("power").unwrap_or(0.0),
                            critical: value_at(params, "critical", level) == Some("true"),
                            critical_limit: param("criticalLimit").unwrap_or(0.0),
                        }],
                        // The MP-restore family. All four are instant effects that
                        // differ only in how the amount is computed; the shared
                        // apply path lives in `restore_mp`.
                        "ManaHeal" => vec![SkillEffect::ManaHeal {
                            power: param("power").unwrap_or(0.0),
                        }],
                        "ManaHealByLevel" => vec![SkillEffect::ManaHealByLevel {
                            power: param("power").unwrap_or(0.0),
                        }],
                        "ManaHealPercent" => vec![SkillEffect::ManaHealPercent {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Java's `Mp` handler reads `amount`/`mode`, not `power`.
                        "Mp" => vec![SkillEffect::MpRestore {
                            amount: param("amount").unwrap_or(0.0),
                            percent: modifier_mode == StatModifierType::Per,
                        }],
                        // Java defaults `chance` to 100 when the tag is absent —
                        // which is every Confuse skill on this dist (only the two
                        // `RandomizeHate` ones declare 80).
                        "Confuse" => vec![SkillEffect::Confuse {
                            chance: value_at(params, "chance", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(100),
                        }],
                        "RandomizeHate" => vec![SkillEffect::RandomizeHate {
                            chance: value_at(params, "chance", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(100),
                        }],
                        // Sword/Blunt Weapon Mastery 205, Dagger Mastery 209,
                        // Dance of Shadows 366. Only the params the reachable
                        // content sets are read; the rest keep Java's defaults.
                        "TriggerSkillByAttack" => {
                            let int_param = |key: &str, default: i32| {
                                value_at(params, key, level)
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(default)
                            };
                            let allow_weapons = value_at(params, "allowWeapons", level)
                                .filter(|v| !v.eq_ignore_ascii_case("ALL"))
                                .map(|v| {
                                    v.split(',')
                                        .map(|w| {
                                            crate::data::item_data::WeaponType::from_name(w.trim())
                                                .mask_bit()
                                        })
                                        .fold(0u32, |acc, b| acc | b)
                                })
                                .unwrap_or(0);
                            let skill_id = int_param("skillId", 0);
                            // Java bails when the skill id or level is 0.
                            if skill_id == 0 {
                                Vec::new()
                            } else {
                                vec![SkillEffect::TriggerSkillByAttack {
                                    min_damage: int_param("minDamage", 1),
                                    chance: int_param("chance", 100),
                                    skill_id,
                                    skill_level: int_param("skillLevel", 1),
                                    on_party: value_at(params, "targetType", level)
                                        == Some("MY_PARTY"),
                                    is_critical: value_at(params, "isCritical", level)
                                        == Some("true"),
                                    allow_weapons,
                                }]
                            }
                        }
                        // Rage 94, Frenzy 176, Two-handed Weapon Mastery 293.
                        // Java's handler carries eleven stat/mode pairs; the only
                        // ones any reachable skill sets are `pAtk` and
                        // `pAccuracy`, so those two are read and the rest keep
                        // their zero default (the same
                        // scope-to-what-the-dist-reaches call `TriggerSkillByAttack`
                        // made).
                        //
                        // Two conditions, both from Java's static fields:
                        // `ConditionUsingItemType(BLUNT|SWORD)` — expressed through
                        // the existing `weapon_condition` mask — and
                        // `ConditionUsingSlotType(SLOT_LR_HAND)`, the new
                        // `two_handed` axis.
                        "TwoHandedBluntBonus" | "TwoHandedSwordBonus" => {
                            let weapon = if xml_name == "TwoHandedBluntBonus" {
                                crate::data::item_data::WeaponType::Blunt.mask_bit()
                            } else {
                                crate::data::item_data::WeaponType::Sword.mask_bit()
                            };
                            let pair = |amount_key: &str, mode_key: &str, stat: Stat| {
                                let amount = value_at(params, amount_key, level)
                                    .and_then(|v| v.parse::<f64>().ok())?;
                                if amount == 0.0 {
                                    return None;
                                }
                                let mode = if value_at(params, mode_key, level) == Some("PER") {
                                    StatModifierType::Per
                                } else {
                                    StatModifierType::Diff
                                };
                                Some(SkillEffect::StatModifier(StatModifierEffect {
                                    stat,
                                    mode,
                                    amount,
                                    weapon_condition: weapon,
                                    two_handed: true,
                                    ..Default::default()
                                }))
                            };
                            [
                                pair("pAtkAmount", "pAtkMode", Stat::PhysicalAttack),
                                pair("pAccuracyAmount", "pAccuracyMode", Stat::AccuracyCombat),
                            ]
                            .into_iter()
                            .flatten()
                            .collect()
                        }
                        "Resurrection" => {
                            let int_param = |key: &str, d: i32| {
                                value_at(params, key, level)
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(d)
                            };
                            vec![SkillEffect::Resurrection {
                                power: int_param("power", 0),
                                hp_percent: int_param("hpPercent", 0),
                                mp_percent: int_param("mpPercent", 0),
                                cp_percent: int_param("cpPercent", 0),
                            }]
                        }
                        // Java throws on an empty `Summon` param set; here a
                        // missing/zero `npcId` simply yields no effect, matching how
                        // every other arm handles unusable params.
                        "Summon" => {
                            let int_param = |key: &str, d: i32| {
                                value_at(params, key, level)
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(d)
                            };
                            let npc_id = int_param("npcId", 0);
                            if npc_id == 0 {
                                Vec::new()
                            } else {
                                vec![SkillEffect::Summon {
                                    npc_id,
                                    life_time: int_param("lifeTime", 0),
                                    consume_item_id: int_param("consumeItemId", 0),
                                    consume_item_count: int_param("consumeItemCount", 1) as i64,
                                }]
                            }
                        }
                        "SummonPet" => vec![SkillEffect::SummonPet],
                        "BlockMove" => vec![SkillEffect::BlockMove],
                        // `type` picks the Java stat: PHYSICAL (the default) or
                        // MAGICAL. Physical Mirror 350 and Magical Mirror 351 carry
                        // *only* this effect, so both were dropped whole before it.
                        // `type` is a `BasicProperty`: `NONE`, `PHYSICAL` (the
                        // default) or **`MAGIC`** — not "MAGICAL", which is the
                        // spelling this port first guessed and which would have
                        // silently routed every magic reflect into the physical
                        // stat. Both Mirrors carry one effect of each kind.
                        //
                        // Their `<armorTYpe>SHIELD</armorTYpe>` gate is a datapack
                        // typo (10 occurrences against 220 correct `<armorType>`).
                        // Java matches element names exactly too, so the condition
                        // is inert on both sides and is faithfully reproduced by
                        // not special-casing it.
                        "ReflectSkill" => vec![SkillEffect::ReflectSkill {
                            magic: value_at(params, "type", level) == Some("MAGIC"),
                            amount: param("amount").unwrap_or(0.0),
                        }],
                        "SilentMove" => vec![SkillEffect::SilentMove],
                        // Fake Death 60. Two halves: the `FAKE_DEATH` flag and an
                        // MP upkeep with the same `power * getTicksMultiplier()`
                        // shape as `ManaDamOverTime`, which it shares the tick
                        // chain with. Skill 60 carries *only* this and
                        // `SilentMove`, so with both unported the effect list came
                        // out empty and the whole skill was dropped — it cast and
                        // did nothing at all.
                        "FakeDeath" => vec![SkillEffect::FakeDeath {
                            power: param("power").unwrap_or(0.0),
                            ticks: value_at(params, "ticks", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        }],
                        // "Transform <Monster>" scroll family (541-558, 617-674):
                        // polymorph the caster into `transformationId`. No stat
                        // modifier of its own — the transform template's own
                        // stat/speed/skill overrides apply via
                        // `admin::transforms::apply_transform_state` — so without
                        // this arm the effect fell through to `EFFECT_REGISTRY`,
                        // wasn't found, and the buff was dropped whole.
                        "Transformation" => match param("transformationId") {
                            Some(id) if id != 0.0 => vec![SkillEffect::Transform {
                                transformation_id: id as i32,
                            }],
                            _ => Vec::new(),
                        },
                        // Fighter-class toggle upkeep (Accuracy 256, Guard Stance
                        // 288, War Frenzy 424, Super Haste 7029, …): without this
                        // arm the effect fell through to `EFFECT_REGISTRY`, wasn't
                        // found, and the toggle's *stat* half (parsed separately,
                        // below) landed as a free buff with no MP cost at all.
                        "MpConsumePerLevel" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![SkillEffect::MpConsumePerLevel { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        // Death Whisper (1242) & co.: Java `CriticalDamage extends
                        // AbstractStatEffect(params, CRITICAL_DAMAGE, CRITICAL_DAMAGE_ADD)`
                        // — a two-stat effect that pumps the multiplicative
                        // `CRITICAL_DAMAGE` in `PER` mode and the additive
                        // `CRITICAL_DAMAGE_ADD` in `DIFF` mode. The 1-name→1-stat
                        // `EFFECT_REGISTRY` can't express that, so pick the stat by
                        // mode here (like `Speed`). Without this the effect fell
                        // through, produced no modifier, and the buff was dropped
                        // whole (community-board "Death Whisper doesn't apply").
                        // The `AbstractStatEffect` crit-damage family: one handler,
                        // two stats, picked by mode (PER → the multiplier, DIFF →
                        // the flat add). Every one of these was parsed *before* this
                        // slice and pumped a stat that nothing read — see
                        // `formulas::crit_damage_multiplier`.
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
                        "DefenceCriticalRate" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::DefenceCriticalRate
                                } else {
                                    Stat::DefenceCriticalRateAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        "DefenceCriticalDamage" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::DefenceCriticalDamage
                                } else {
                                    Stat::DefenceCriticalDamageAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        // Prophecy of Wind (1357), Victories of Pa'agrio (1414).
                        // Java's `MAGIC_CRITICAL_DAMAGE_ADD` half is dropped: the
                        // magic branch of `calcCritDamage` reads only the
                        // multiplier, and `calcCritDamageAdd`'s magic result is
                        // never applied (see `Formulas.calcMagicDam`'s own TODO).
                        "MagicCriticalDamage" => param("amount")
                            .filter(|_| modifier_mode == StatModifierType::Per)
                            .map(|amount| stat_mod(Stat::MagicCriticalDamage, amount))
                            .into_iter()
                            .collect(),
                        "DefenceMagicCriticalDamage" => param("amount")
                            .filter(|_| modifier_mode == StatModifierType::Per)
                            .map(|amount| stat_mod(Stat::DefenceMagicCriticalDamage, amount))
                            .into_iter()
                            .collect(),
                        // Focus Death (355), Focus Power (357): a crit-damage
                        // multiplier that applies only from a given attack
                        // position. Java merges `(amount/100)+1` multiplicatively
                        // into `_positionTypeStats` — a different map, merge and
                        // identity from the move-type one, so the qualifier routes
                        // it accordingly. Read only by the *autoattack* branch of
                        // `calcCritDamage`, matching Java.
                        "CriticalDamagePosition" => {
                            let position = match value_at(params, "position", level) {
                                // Java `params.getEnum("position", Position.class, Position.FRONT)`.
                                Some("BACK") => crate::model::movement::Position::Back,
                                Some("SIDE") => crate::model::movement::Position::Side,
                                _ => crate::model::movement::Position::Front,
                            };
                            param("amount")
                                .map(|amount| {
                                    SkillEffect::StatModifier(StatModifierEffect {
                                        stat: Stat::CriticalDamage,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: Some(
                                            crate::model::stats::StatQualifier::Position(position),
                                        ),
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        // Mental Shield (1035) / Stun Resistance ("Resist Shock",
                        // 1259): Java `DefenceTrait` raises per-`TraitType`
                        // resistance (HOLD/SLEEP/SHOCK…). Its params are the trait
                        // *names*, not `amount`, so they are read straight off the
                        // param map rather than through the usual `amount` lookup.
                        "DefenceTrait" => {
                            // Every param is a trait name → percent; Java
                            // divides by 100 and treats >= 1.0 as invulnerable.
                            let traits: Vec<(crate::model::skill::TraitType, f64)> = params
                                .keys()
                                .filter_map(|key| {
                                    let raw = value_at(params, key, level)?;
                                    let pct: f64 = raw.parse().ok()?;
                                    Some((
                                        crate::model::skill::TraitType::from_xml(key),
                                        pct / 100.0,
                                    ))
                                })
                                .collect();
                            vec![SkillEffect::DefenceTrait { traits }]
                        }
                        // Vampiric Rage (1268): Java `VampiricAttack` grants a chance
                        // to absorb a % of melee damage as HP. The melee-absorb path
                        // isn't modeled, so carry an icon-only marker rather than
                        // dropping the buff.
                        "VampiricAttack" => vec![SkillEffect::VampiricAttack],
                        // "Detect <Category> Weakness" (75/80/87/88/104, 359/360):
                        // Java `AttackTrait` merges a `*_WEAKNESS` bonus onto the
                        // caster — genuinely inert in the reference server too (see
                        // the doc comment on `SkillEffect::AttackTrait`), so this
                        // carries an icon-only marker like `DefenceTrait`/
                        // `VampiricAttack` rather than the per-trait param map.
                        "AttackTrait" => vec![SkillEffect::AttackTrait],
                        // Celestial Shield (1418), Flames of Invincibility (1427),
                        // Dance of Medusa (367), Sonic/Force Barrier (442/443): a
                        // skill carries two of these, one `BLOCK_HP` and one
                        // `BLOCK_MP` (`<effect name="DamageBlock"><type>BLOCK_HP
                        // </type></effect>`, a plain string param, not `param()`'s
                        // f64). Without this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and these short
                        // invulnerability shields did nothing.
                        "DamageBlock" => {
                            let ty = value_at(params, "type", level);
                            vec![SkillEffect::DamageBlock {
                                block_hp: ty == Some("BLOCK_HP"),
                                block_mp: ty == Some("BLOCK_MP"),
                            }]
                        }
                        // Community-board dance/song buffs whose combat/cost math
                        // isn't modeled yet — Song of Champion/Renewal
                        // (`MagicMpCost`/`Reuse`), Gift of Seraphim (4703, `Reuse`),
                        // Song of Vengeance (305, `DamageShield`). Each maps to a
                        // per-magic-type stat the port doesn't have, so carry an
                        // icon-only marker (like `DefenceTrait`) rather than
                        // dropping the buff whole at the empty-effects guard — the
                        // buff must show and expire. (`AttackAttribute` graduated
                        // to a real element-POWER modifier in the G19 attributes
                        // slice; its arm above wins.)
                        "MagicMpCost" => vec![SkillEffect::MagicMpCost],
                        "Reuse" => vec![SkillEffect::Reuse],
                        "DamageShield" => vec![SkillEffect::DamageShield],
                        // Expand Inventory/Warehouse/Trade/Common Craft/Dwarven
                        // Craft (1368-1372, the craftsman-guild storage passives):
                        // Java `EnlargeSlot extends AbstractStatEffect` reads
                        // `amount` + a `type` string picking one of 6 `Stat`s; an
                        // absent `type` (Expand Inventory) defaults to
                        // INVENTORY_NORMAL. Expand Trade carries two effect blocks
                        // per level, one TRADE_BUY one TRADE_SELL. The 1-name-1-stat
                        // `EFFECT_REGISTRY` can't express the type-selected stat, so
                        // without this arm the effect fell through and these
                        // passives did nothing.
                        "EnlargeSlot" => {
                            let stat = match value_at(params, "type", level) {
                                Some("STORAGE_PRIVATE") => Stat::StoragePrivate,
                                Some("TRADE_SELL") => Stat::TradeSell,
                                Some("TRADE_BUY") => Stat::TradeBuy,
                                Some("RECIPE_DWARVEN") => Stat::RecipeDwarven,
                                Some("RECIPE_COMMON") => Stat::RecipeCommon,
                                _ => Stat::InventoryNormal,
                            };
                            param("amount")
                                .map(|amount| stat_mod(stat, amount))
                                .into_iter()
                                .collect()
                        }
                        _ => match EFFECT_REGISTRY
                            .iter()
                            .find(|(n, _)| n == xml_name)
                            .map(|(_, s)| *s)
                        {
                            Some(stat) => param("amount")
                                .map(|amount| stat_mod(stat, amount))
                                .into_iter()
                                .collect(),
                            None => Vec::new(),
                        },
                    }
                })
                .collect::<Vec<_>>()
        };
        // Java keeps one effect list per `EffectScope`; the port carries the
        // ones it can act on. `START`/`END` parse as `Other` and are dropped —
        // they hang off lifecycle hooks this port doesn't have.
        let skill_effects = build_scope(EffectScope::General);
        let self_effects = build_scope(EffectScope::SelfScope);
        let pve_effects = build_scope(EffectScope::Pve);
        let pvp_effects = build_scope(EffectScope::Pvp);
        let channeling_effects = build_scope(EffectScope::Channeling);

        // Effect names present in the XML but not in `EFFECT_REGISTRY` are
        // silently dropped (see module docs) — expected for the vast majority
        // of skills, which are outside G6's scope.
        Skill {
            id,
            level,
            sub_level: sub,
            name: name.to_string(),
            operate_type,
            is_continuous,
            target_type,
            over_hit,
            abnormal_visuals,
            toggle_group_id,
            affect_scope,
            trait_type,
            affect_object,
            affect_range,
            affect_limit,
            fan_range,
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
            without_action: value_at(values, "withoutAction", level) == Some("true"),
            item_consume_id: get_i("itemConsumeId", 0),
            item_consume_count: get_i("itemConsumeCount", 0),
            abnormal_time: get_i("abnormalTime", 0),
            abnormal_level: get_i("abnormalLevel", 0),
            abnormal_type: value_at(values, "abnormalType", level)
                .unwrap_or("NONE")
                .to_string(),
            // Java `set.getBoolean("canBeDispelled", true)` / `("isDebuff", false)`.
            can_be_dispelled: value_at(values, "canBeDispelled", level).is_none_or(|v| v == "true"),
            is_debuff: value_at(values, "isDebuff", level) == Some("true"),
            // Java `set.getBoolean("stayAfterDeath", false)`. The dist writes
            // both `true` and `True` for this tag and `Boolean.parseBoolean`
            // is case-insensitive, so compare loosely.
            stay_after_death: value_at(values, "stayAfterDeath", level)
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
            effects: skill_effects,
            self_effects,
            pve_effects,
            pvp_effects,
            channeling_effects,
            op_exist_npc: op_exist_npc.clone(),
            // Java `set.getInt("mpPerChanneling", _mpConsume)` — the
            // default is the skill's own mpConsume, not 0.
            mp_per_channeling: get_i("mpPerChanneling", get_i("mpConsume", 0)),
            // XML values are seconds; Java stores ms (`getFloat × 1000`).
            channeling_tick_ms: (get_f("channelingTickInterval", 0.0) * 1000.0) as i32,
            channeling_start_ms: (get_f("channelingStart", 0.0) * 1000.0) as i32,
            // `<attributeType>FIRE</attributeType>` + `<attributeValue>20`
            // — the skill's element for `calcAttributeBonus`. `NONE` and
            // unknown names read as no element, like Java's enum default.
            attribute_type: value_at(values, "attributeType", level)
                .and_then(crate::model::stats::Element::from_xml),
            attribute_value: get_i("attributeValue", 0),
        }
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

    /// Skill-enchant sub-levels against the real dist (PLAN_G19_SKILL_ENCHANT.md).
    /// Sonic Storm 7 at level 40 declares all three routes: route 1 enchants
    /// the `EnergyAttack` power (`{base + base/100*subIndex}` off base 20732),
    /// route 2 the crit chance (base 15 — itself a *ranged* `fromLevel 1–44`
    /// row, the shape the parser used to mis-key), route 3 the pDefMod
    /// (`{0.99 − 0.006·(subIndex−1)}`).
    #[test]
    fn skill_enchant_sublevels_resolve() {
        let sd = SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));

        assert_eq!(
            sd.enchant_routes(7, 40),
            &[(1001, 1020), (2001, 2020), (3001, 3020)],
            "Sonic Storm 40's three routes"
        );
        assert!(
            sd.enchant_routes(7, 39).is_empty(),
            "the routes open at level 40"
        );
        assert!(
            sd.enchant_routes(1177, 1).is_empty(),
            "Wind Strike is not enchantable"
        );

        let base = sd.get(7, 40).expect("Sonic Storm 40");
        let (p0, c0, d0) = match base.effects.as_slice() {
            [
                SkillEffect::EnergyAttack {
                    power,
                    critical_chance,
                    p_def_mod,
                    ..
                },
            ] => (*power, *critical_chance, *p_def_mod),
            other => panic!("EnergyAttack expected: {other:?}"),
        };
        assert_eq!((p0, c0, d0), (20732.0, 15.0, 1.0));
        assert_eq!(base.sub_level, 0);

        // Route 1, +1 and +10: power scales, the other params hold their base.
        let e1 = sd.get_enchanted(7, 40, 1001).expect("+1 power route");
        assert_eq!(e1.sub_level, 1001);
        match e1.effects.as_slice() {
            [
                SkillEffect::EnergyAttack {
                    power,
                    critical_chance,
                    p_def_mod,
                    ..
                },
            ] => {
                assert!(
                    (power - (20732.0 + 20732.0 / 100.0)).abs() < 1e-6,
                    "+1: {power}"
                );
                assert_eq!((*critical_chance, *p_def_mod), (15.0, 1.0));
            }
            other => panic!("{other:?}"),
        }
        let e10 = sd.get_enchanted(7, 40, 1010).expect("+10 power route");
        match e10.effects.as_slice() {
            [SkillEffect::EnergyAttack { power, .. }] => {
                assert!((power - (20732.0 * 1.10)).abs() < 1e-6, "+10: {power}");
            }
            other => panic!("{other:?}"),
        }

        // Route 2 enchants the crit chance; route 3 the pDefMod.
        match sd
            .get_enchanted(7, 40, 2001)
            .expect("+1 crit route")
            .effects
            .as_slice()
        {
            [
                SkillEffect::EnergyAttack {
                    power,
                    critical_chance,
                    ..
                },
            ] => {
                assert!((critical_chance - 15.15).abs() < 1e-6, "{critical_chance}");
                assert_eq!(*power, 20732.0, "power keeps its base on route 2");
            }
            other => panic!("{other:?}"),
        }
        match sd
            .get_enchanted(7, 40, 3005)
            .expect("+5 pdef route")
            .effects
            .as_slice()
        {
            [SkillEffect::EnergyAttack { p_def_mod, .. }] => {
                assert!(
                    (p_def_mod - (0.99 - 0.006 * 4.0)).abs() < 1e-6,
                    "{p_def_mod}"
                );
            }
            other => panic!("{other:?}"),
        }

        // A skill-FIELD route (not an effect param): Curse Gloom 1263's
        // duration route — `abnormalTime` base 10 (itself a ranged 1–24 row),
        // `{base + 0.5 * subIndex}` on 2001–2020. Java's `StatSet.getInt`
        // truncates the fractional +1 (10.5 → 10); +2 is a clean 11. The
        // fragmented power-route rows (1001–1005, 1006–1006, …) bucket-merge
        // into one (1001, 1020) route.
        assert_eq!(sd.enchant_routes(1263, 20), &[(1001, 1020), (2001, 2020)]);
        let cg = sd.get(1263, 20).expect("Curse Gloom 20");
        assert_eq!(cg.abnormal_time, 10);
        assert_eq!(
            sd.get_enchanted(1263, 20, 2001)
                .expect("+1 duration")
                .abnormal_time,
            10
        );
        assert_eq!(
            sd.get_enchanted(1263, 20, 2002)
                .expect("+2 duration")
                .abnormal_time,
            11
        );
        assert_eq!(
            sd.get_enchanted(1263, 20, 2020)
                .expect("+20 duration")
                .abnormal_time,
            20
        );

        // The cost table (data/EnchantSkillGroups.xml): 30 levels; +1 costs
        // 90% NORMAL with a Superior Giant's Codex 30297 and adena.
        let groups = crate::data::enchant_skill_groups::EnchantSkillGroups::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        assert_eq!(groups.len(), 30);
        let one = groups.cost_for(1).expect("level 1");
        assert_eq!(one.chance.get("NORMAL"), Some(&90));
        assert_eq!(one.sp.get("NORMAL"), Some(&4_250_000));
        let items = one.items.get("NORMAL").expect("NORMAL items");
        assert!(
            items.contains(&(30297, 1)) && items.contains(&(57, 2_380_000)),
            "{items:?}"
        );
    }

    /// Regression guard: the real dist XMLs are `<list>`-rooted, which the
    /// original parser mis-indexed (it tracked the root on the tag stack and
    /// loaded 0 skills). Wind Strike 1177 is the canonical probe.
    #[test]
    fn loads_real_dist_files() {
        let sd = SkillData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        assert!(
            sd.skills.len() > 10_000,
            "expected thousands of skill levels, got {}",
            sd.skills.len()
        );
        let ws = sd.get(1177, 1).expect("Wind Strike lvl 1");
        assert_eq!(ws.target_type, TargetType::EnemyOnly);
        assert_eq!(ws.cast_range, 600);
        assert!(
            matches!(ws.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0)
        );
        assert_eq!(
            ws.reuse_delay_group, -1,
            "no <reuseDelayGroup> must stay -1, never 0"
        );
        assert_eq!(ws.reuse_key(), 1177);

        // Prominence 1230: a ranged nuke backed by the `MagicalAttackRange`
        // effect. It must parse to a `MagicalAttack` (power 108 at lvl 28) —
        // before the handler existed the effect fell through and was dropped,
        // so the skill cast but dealt zero damage.
        let prominence = sd.get(1230, 28).expect("Prominence lvl 28");
        assert!(
            matches!(prominence.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 108.0)
        );

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
        assert!(decrease_speed.affect_scope == AffectScope::Single && decrease_speed.is_bad());
        assert_eq!(decrease_speed.activate_rate, 80);
        assert_eq!(decrease_speed.lvl_bonus_rate, 30);
        // An area skill (`affectScope RANGE`) is not single-target.
        let sonic_storm = sd.get(7, 1).expect("Sonic Storm lvl 1");
        assert!(sonic_storm.affect_scope != AffectScope::Single);

        // Tempest 1176 — the canonical AoE nuke, and the reference case for the
        // whole affect-scope block: RANGE scope, NOT_FRIEND filter, a 200-unit
        // sweep around the target, and a 5-12 target cap.
        let tempest = sd.get(1176, 1).expect("Tempest lvl 1");
        assert_eq!(tempest.affect_scope, AffectScope::Range);
        assert_eq!(tempest.affect_object, AffectObject::NotFriend);
        assert_eq!(tempest.affect_range, 200);
        assert_eq!(tempest.affect_limit, (5, 12));
        // `getAffectLimit()` is `min + Rnd.get(max)`, so the "5-12" above can
        // actually yield up to 16 targets — verified at both roll extremes.
        assert_eq!(tempest.affect_limit(|_| 0), 5);
        assert_eq!(tempest.affect_limit(|bound| bound - 1), 16);
        // Sonic Storm carries the same 5-12 cap over a tighter 150 sweep.
        assert_eq!(sonic_storm.affect_range, 150);
        assert_eq!(sonic_storm.affect_limit, (5, 12));

        // Geometric scopes (PLAN_G19_GEOMETRIC_SCOPES.md). Sonic Buster 9 is
        // the reference FAN: a 180° half-circle of radius 200 —
        // `<fanRange>0;0;200;180</fanRange>` as `unk;startDegree;radius;angle`.
        let sonic_buster = sd.get(9, 1).expect("Sonic Buster lvl 1");
        assert_eq!(sonic_buster.affect_scope, AffectScope::Fan);
        assert_eq!(sonic_buster.fan_range, [0, 0, 200, 180]);
        // Divine Judgment 6314 — RING_RANGE: an annulus of 100..270 around
        // the target; the inner radius rides in `fan_range[2]`.
        let judgment = sd.get(6314, 1).expect("Divine Judgment lvl 1");
        assert_eq!(judgment.affect_scope, AffectScope::RingRange);
        assert_eq!(judgment.affect_range, 270);
        assert_eq!(judgment.fan_range, [0, 0, 100, 0]);
        // Frintezza Charge 5015 — SQUARE with a **level-valued** fanRange
        // (the only leveled tuple in the dist): 400×150 at level 1, 700×200
        // at level 3. A skill with no `<fanRange>` parses to all zeroes.
        assert_eq!(
            sd.get(5015, 1).expect("Frintezza Charge lvl 1").fan_range,
            [0, 0, 400, 150]
        );
        assert_eq!(
            sd.get(5015, 3).expect("Frintezza Charge lvl 3").fan_range,
            [0, 0, 700, 200]
        );
        assert_eq!(tempest.fan_range, [0; 4]);

        // Over-hit (G20): 59 learnable skills carry `<overHit>true</overHit>` —
        // a killing blow with one pays bonus XP for the excess damage.
        assert!(
            sd.get(1, 1).expect("Triple Slash").over_hit,
            "Triple Slash over-hits"
        );
        assert!(sd.get(7, 1).expect("Sonic Storm").over_hit);
        assert!(!sd.get(1068, 1).expect("Might").over_hit, "a buff does not");

        // Polearm Mastery 216 raises ATTACK_COUNT_MAX to 5 (`HitNumber`) —
        // this is what turns a polearm into a sweep weapon; the weapon type
        // alone does not.
        let mastery = sd.get(216, 1).expect("Polearm Mastery lvl 1");
        assert!(
            mastery
                .stat_modifier_effects()
                .iter()
                .any(|m| m.stat == Stat::AttackCountMax && m.amount == 5.0),
            "got {:?}",
            mastery.effects
        );

        // Abnormal *visual* effects — the cosmetic half of everything above.
        // Shield Stun 92 draws STUN(7), Bleed 96 draws DOT_BLEEDING(1), Horror
        // 65 draws TURN_FLEE(32); Might 1068 draws nothing.
        assert_eq!(
            sd.get(92, 1).expect("Shield Stun").abnormal_visuals,
            vec![7]
        );
        assert_eq!(sd.get(96, 1).expect("Bleed").abnormal_visuals, vec![1]);
        assert_eq!(sd.get(65, 1).expect("Horror").abnormal_visuals, vec![32]);
        assert!(sd.get(1068, 1).expect("Might").abnormal_visuals.is_empty());
        // An unknown enum name resolves to nothing rather than panicking.
        assert_eq!(
            crate::model::skill::abnormal_visual_client_id("NOT_A_REAL_AVE"),
            None
        );
        assert_eq!(
            crate::model::skill::abnormal_visual_client_id("STUN"),
            Some(7)
        );

        // The rest of the CC family, against the real Interlude skills.
        // Seal of Silence 1246 silences (magic only); Shield Slam 353 is the
        // physical twin; Mystic Immunity 1411 blocks incoming debuffs; Horror
        // 65 blocks control; Trick 11 cancels the target.
        use crate::model::skill::effect_flag;
        assert_eq!(
            sd.get(1246, 1).expect("Seal of Silence").effect_flags(),
            effect_flag::MUTED
        );
        assert_eq!(
            sd.get(353, 1).expect("Shield Slam").effect_flags() & effect_flag::PHYSICAL_MUTED,
            effect_flag::PHYSICAL_MUTED
        );
        assert_eq!(
            sd.get(1411, 1).expect("Mystic Immunity").effect_flags() & effect_flag::DEBUFF_BLOCK,
            effect_flag::DEBUFF_BLOCK
        );
        assert_eq!(
            sd.get(65, 1).expect("Horror").effect_flags() & effect_flag::BLOCK_CONTROL,
            effect_flag::BLOCK_CONTROL
        );
        assert!(
            matches!(
                sd.get(11, 1)
                    .expect("Trick")
                    .effects
                    .iter()
                    .find(|e| matches!(e, SkillEffect::TargetCancel { .. })),
                Some(SkillEffect::TargetCancel { .. })
            ),
            "Trick cancels its target"
        );
        // A silence must not also block physical skills, and vice versa.
        assert_eq!(
            sd.get(1246, 1).unwrap().effect_flags() & effect_flag::PHYSICAL_MUTED,
            0
        );

        // Noblesse Blessing 1323 — its only effect is the flag the death path
        // reads; without the parse arm the buff would be dropped whole.
        let bless = sd.get(1323, 1).expect("Noblesse Blessing");
        assert!(matches!(
            bless.effects.as_slice(),
            [SkillEffect::NoblesseBless]
        ));
        assert_eq!(bless.effect_flags(), effect_flag::NOBLESS_BLESSING);
        assert!(
            !bless.stay_after_death,
            "the blessing itself is what death consumes"
        );
        // `<stayAfterDeath>` is parsed case-insensitively — the dist writes both
        // spellings: Final Flying Form 840 `true`, Report Status 6038 `True`.
        // Might 1068 is untagged.
        assert!(sd.get(840, 1).expect("Final Flying Form").stay_after_death);
        assert!(
            sd.get(6038, 1).expect("Report Status").stay_after_death,
            "`True` parses too"
        );
        assert!(!sd.get(1068, 1).expect("Might").stay_after_death);

        // Fury Fists 222 — an upkeep toggle: `HealOverTime` with a *negative*
        // power, i.e. an HP cost per tick, not a heal. Silent Move 221 is the
        // MP-cost twin. Both are toggles, so their upkeep also drives the
        // toggle-off-on-exhaustion path.
        let fury_fists = sd.get(222, 1).expect("Fury Fists lvl 1");
        assert_eq!(fury_fists.operate_type, OperateType::Toggle);
        assert!(
            matches!(
                fury_fists.effects.iter().find(|e| matches!(e, SkillEffect::HealOverTime { .. })),
                Some(SkillEffect::HealOverTime { power, ticks }) if *power == -12.0 && *ticks == 2
            ),
            "got {:?}",
            fury_fists.effects
        );
        let silent_move = sd.get(221, 1).expect("Silent Move lvl 1");
        assert!(
            matches!(
                silent_move.effects.iter().find(|e| matches!(e, SkillEffect::ManaDamOverTime { .. })),
                Some(SkillEffect::ManaDamOverTime { power, ticks }) if *power == 9.0 && *ticks == 5
            ),
            "got {:?}",
            silent_move.effects
        );

        // Braveheart 440 grants a flat +1000 CP; Touch of Death 342 takes CP as
        // a percentage.
        let braveheart = sd.get(440, 1).expect("Braveheart lvl 1");
        assert!(
            matches!(
                braveheart.effects.iter().find(|e| matches!(e, SkillEffect::Cp { .. })),
                Some(SkillEffect::Cp { amount, percent: false }) if *amount == 1000.0
            ),
            "got {:?}",
            braveheart.effects
        );
        assert!(matches!(
            sd.get(342, 1).expect("Touch of Death").effects.iter().find(|e| matches!(e, SkillEffect::Cp { .. })),
            Some(SkillEffect::Cp { amount, percent: true }) if *amount == -90.0
        ));
        // Touch of Life 341 raises the healing its target receives (PER → the
        // multiplicative stat); Touch of Death 342 lowers it.
        assert!(
            sd.get(341, 1)
                .expect("Touch of Life")
                .stat_modifier_effects()
                .iter()
                .any(|m| m.stat == Stat::HealEffect && m.amount == 30.0)
        );

        // Guts 139 — the debuff-resistance buff: a negative `amount` on
        // `ResistAbnormalByCategory` means *more* resistant, and it must parse
        // as a PER modifier (the XML carries no <mode>, so a naive read would
        // make it DIFF and mean something entirely different).
        let guts = sd.get(139, 1).expect("Guts lvl 1");
        let resist = guts
            .stat_modifier_effects()
            .into_iter()
            .find(|m| m.stat == Stat::ResistAbnormalDebuff)
            .expect("Guts pumps ResistAbnormalDebuff");
        assert_eq!(resist.mode, StatModifierType::Per);
        assert_eq!(
            resist.amount, -50.0,
            "Guts lvl 1 is -50 → x0.5 debuff chance"
        );
        // Touch of Death 342 is the same effect with the sign flipped.
        let touch_of_death = sd.get(342, 1).expect("Touch of Death lvl 1");
        assert_eq!(
            touch_of_death
                .stat_modifier_effects()
                .into_iter()
                .find(|m| m.stat == Stat::ResistAbnormalDebuff)
                .map(|m| m.amount),
            Some(30.0)
        );
        // Ultimate Defense 110 resists *dispel* rather than debuffs.
        let ultimate_defense = sd.get(110, 1).expect("Ultimate Defense lvl 1");
        assert!(
            ultimate_defense
                .stat_modifier_effects()
                .iter()
                .any(|m| m.stat == Stat::ResistDispelBuff && m.amount == -80.0)
        );

        // Prophecy of Water 1355 blocks the BUFF_SPECIAL_* slots, which is how
        // the Prophecies stay mutually exclusive.
        let prophecy = sd.get(1355, 1).expect("Prophecy of Water lvl 1");
        let blocked = prophecy.blocked_abnormals();
        assert!(
            blocked.contains(&"BUFF_SPECIAL_ATTACK".to_string()),
            "got {blocked:?}"
        );
        assert_eq!(blocked.len(), 5, "all five BUFF_SPECIAL slots: {blocked:?}");
        // An ordinary buff blocks nothing.
        assert!(
            sd.get(1068, 1)
                .expect("Might")
                .blocked_abnormals()
                .is_empty()
        );

        // Warrior Bane 1350 / Mass Warrior Bane 1344 — probabilistic dispel.
        let bane = sd.get(1350, 1).expect("Warrior Bane lvl 1");
        match bane
            .effects
            .iter()
            .find(|e| matches!(e, SkillEffect::DispelBySlotProbability { .. }))
        {
            Some(SkillEffect::DispelBySlotProbability { dispel, rate }) => {
                assert_eq!(*rate, 80, "single-target Bane is 80%");
                assert!(dispel.contains(&"SPEED_UP".to_string()), "got {dispel:?}");
            }
            other => panic!("expected DispelBySlotProbability, got {other:?}"),
        }
        let mass_bane = sd.get(1344, 1).expect("Mass Warrior Bane lvl 1");
        assert!(
            mass_bane.effects.iter().any(|e| matches!(
                e,
                SkillEffect::DispelBySlotProbability { rate, .. } if *rate == 40
            )),
            "the mass version trades rate for reach"
        );

        // Shield Stun 92 / Arrest 402 — the crowd-control pair. Neither carries
        // a stat modifier: the whole mechanic is the abnormal-state flag.
        let shield_stun = sd.get(92, 1).expect("Shield Stun lvl 1");
        assert_eq!(
            shield_stun.effect_flags(),
            crate::model::skill::effect_flag::BLOCK_ACTIONS
        );
        assert_eq!(shield_stun.abnormal_type, "STUN");
        assert!(shield_stun.stat_modifier_effects().is_empty());
        let arrest = sd.get(402, 1).expect("Arrest lvl 1");
        assert_eq!(
            arrest.effect_flags(),
            crate::model::skill::effect_flag::ROOTED
        );
        assert_eq!(arrest.abnormal_type, "ROOT_PHYSICALLY");
        // A root does NOT block actions — only movement.
        assert_eq!(
            arrest.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS,
            0
        );
        // An ordinary buff contributes no state flags at all.
        assert_eq!(sd.get(1068, 1).expect("Might").effect_flags(), 0);

        // Thunder Storm 48 casts from SELF with a POINT_BLANK sweep — the
        // scope that centres on the *caster* rather than the target, which is
        // why its targetType is SELF even though it is an offensive skill.
        let thunder_storm = sd.get(48, 1).expect("Thunder Storm lvl 1");
        assert_eq!(thunder_storm.affect_scope, AffectScope::PointBlank);
        assert_eq!(thunder_storm.target_type, TargetType::Self_);
        assert_eq!(thunder_storm.affect_object, AffectObject::NotFriend);
        assert_eq!(thunder_storm.affect_range, 150);
        // ...and it is *also* a stun, so it exercises both G19 slices at once:
        // a caster-centred sweep that block-actions everything it catches.
        assert_eq!(
            thunder_storm.effect_flags(),
            crate::model::skill::effect_flag::BLOCK_ACTIONS
        );
        // A skill with no `<activateRate>` defaults to -1 (always lands): the
        // buff Might 1068.
        let might = sd.get(1068, 1).expect("Might lvl 1");
        assert_eq!(might.activate_rate, -1);
        // ...and, carrying no <affectLimit>, is uncapped.
        assert_eq!(might.affect_limit(|_| 0), 0);

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
        assert!(matches!(
            escape.effects.as_slice(),
            [SkillEffect::EscapeToTown]
        ));
        let gm_escape = sd.get(2100, 1).expect("Escape: 1 Second lvl 1");
        assert!(matches!(
            gm_escape.effects.as_slice(),
            [SkillEffect::EscapeToTown]
        ));

        // G15 item-cast slice: `ItemSkillsTemplate` picks the instant vs cast
        // branch from `withoutAction` + the item's `immediate_effect`, and
        // `checkConsume` reads the skill's `itemConsumeId`. Scroll of Escape
        // (2013) declares neither `withoutAction` nor a short hit time, so it
        // must cast for its full 20 s and name its reagent.
        let soe = sd.get(2013, 1).expect("Scroll of Escape lvl 1");
        assert_eq!(soe.hit_time, 20_000);
        assert!(!soe.without_action, "no <withoutAction> -> cast branch");
        assert_eq!(soe.item_consume_id, 736, "the scroll itself");
        assert_eq!(soe.item_consume_count, 1);
        // Scroll: Might (2057) is the 4 s buff-scroll shape.
        let might = sd.get(2057, 1).expect("Scroll: Might lvl 1");
        assert_eq!(might.hit_time, 4000);
        assert!(!might.without_action);
        assert_eq!(might.item_consume_id, 3933);
        // A potion skill carries no reagent — the item handler consumes it via
        // the item's own `immediate_effect`.
        assert_eq!(
            sd.get(2031, 1)
                .expect("Healing Potion lvl 1")
                .item_consume_id,
            0
        );

        // Blessing of Protection 5182 (Newbie Helper): its `ProtectionBlessing`
        // effect carries no stat modifier — before this arm it fell through to
        // an empty effect list and never landed as a buff. It must parse to the
        // marker so `apply_skill_effects` still creates the icon-only PK_PROTECT
        // buff (7200 s).
        let blessing = sd.get(5182, 1).expect("Blessing of Protection lvl 1");
        assert!(matches!(
            blessing.effects.as_slice(),
            [SkillEffect::ProtectionBlessing]
        ));
        assert_eq!(blessing.abnormal_time, 7200);

        // The Newbie Helper support buffs must all load with their stat effects
        // (empty-effect skills would silently drop and show no icon): Wind Walk
        // 4322 pumps all four move speeds; Shield 4323 is PhysicalDefence;
        // Empower 4331 is MAtk.
        let wind_walk = sd.get(4322, 1).expect("Adventurer's Wind Walk lvl 1");
        assert_eq!(
            wind_walk.stat_modifier_effects().len(),
            4,
            "Speed pumps 4 move stats"
        );
        assert!(
            !sd.get(4323, 1)
                .expect("Shield")
                .stat_modifier_effects()
                .is_empty()
        );
        assert!(
            !sd.get(4331, 1)
                .expect("Empower")
                .stat_modifier_effects()
                .is_empty()
        );

        // Skill 22490 "Mysterious Spiritshot d 5000" — the `Restoration`
        // effect backing the "Mysterious Blessed Spiritshot Pack (5000)
        // (D-grade)" item (22599). Previously parsed with an empty effect
        // list, so using the pack consumed it and granted nothing.
        let spiritshot_pack = sd
            .get(22490, 5)
            .expect("Mysterious Spiritshot d 5000 lvl 5");
        assert!(matches!(
            spiritshot_pack.effects.as_slice(),
            [SkillEffect::GiveItem {
                item_id: 21852,
                item_count: 5000,
                item_enchant_level: 0
            }]
        ));

        // Skill 323 "Quiver of Arrow" — a real `RestorationRandom` skill
        // (three weighted groups of Mithril Arrow).
        let quiver = sd.get(323, 1).expect("Quiver of Arrow lvl 1");
        match quiver.effects.as_slice() {
            [SkillEffect::GiveItemRandom { groups }] => {
                assert_eq!(groups.len(), 3);
                assert_eq!(groups[0].chance, 30.0);
                assert_eq!(
                    groups[0].items,
                    vec![RestorationItem {
                        item_id: 1344,
                        count: 700,
                        min_enchant: 0,
                        max_enchant: 0
                    }]
                );
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
        assert!(
            !weapon_pen.stat_modifier_effects().is_empty(),
            "6209 must have stat effects"
        );
        assert!(
            weapon_pen
                .stat_modifier_effects()
                .iter()
                .any(|e| e.stat == Stat::PhysicalAttack)
        );
        let armor_pen = sd.get(6213, 4).expect("Armor Grade Penalty lvl 4");
        assert!(
            !armor_pen.stat_modifier_effects().is_empty(),
            "6213 must have stat effects"
        );

        // Clan Advent (19009) — the clan-leader-online aura applied via the clan
        // login/logout hooks. Permanent (`abnormalTime=-1`) with all six stat
        // effects: PAtk/PDef/MDef/MAtk percent buffs + flat HP/MP regen.
        let advent = sd.get(19009, 1).expect("Clan Advent lvl 1");
        assert_eq!(advent.abnormal_time, -1, "Clan Advent is permanent");
        let stats: Vec<Stat> = advent
            .stat_modifier_effects()
            .iter()
            .map(|e| e.stat)
            .collect();
        for want in [
            Stat::PhysicalAttack,
            Stat::PhysicalDefence,
            Stat::MagicalDefence,
            Stat::MagicalAttack,
            Stat::RegenerateHpRate,
            Stat::RegenerateMpRate,
        ] {
            assert!(
                stats.contains(&want),
                "Clan Advent must modify {want:?}, got {stats:?}"
            );
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
            let cure = sd
                .get(1012, lvl)
                .unwrap_or_else(|| panic!("Cure Poison lvl {lvl}"));
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
        assert!(matches!(
            sweeper.effects.as_slice(),
            [SkillEffect::Sweeper, SkillEffect::ConsumeBody]
        ));

        // Common Craft 1322 / Dwarven Craft 1321: self-target ability skills
        // whose only effect opens the matching recipe window. Both parsed to an
        // empty effect list before `OpenCommonRecipeBook`/`OpenDwarfRecipeBook`
        // were registered, so casting them did nothing at all.
        let common_craft = sd.get(1322, 1).expect("Common Craft lvl 1");
        assert_eq!(common_craft.target_type, TargetType::Self_);
        assert!(matches!(
            common_craft.effects.as_slice(),
            [SkillEffect::OpenRecipeBook { dwarven: false }]
        ));
        let dwarven_craft = sd.get(1321, 1).expect("Dwarven Craft lvl 1");
        assert!(matches!(
            dwarven_craft.effects.as_slice(),
            [SkillEffect::OpenRecipeBook { dwarven: true }]
        ));

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
        assert!(matches!(
            mental_shield.effects.as_slice(),
            [SkillEffect::DefenceTrait { .. }]
        ));
        assert_eq!(mental_shield.abnormal_time, 1200);
        let resist_shock = sd.get(1259, 1).expect("Stun Resistance lvl 1");
        assert!(matches!(
            resist_shock.effects.as_slice(),
            [SkillEffect::DefenceTrait { .. }]
        ));
        let vampiric_rage = sd.get(1268, 1).expect("Vampiric Rage lvl 1");
        assert!(matches!(
            vampiric_rage.effects.as_slice(),
            [SkillEffect::VampiricAttack]
        ));
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
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let l1 = out.skills.get(&(1177, 1)).expect("level 1 parsed");
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
        assert!(
            matches!(l1.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 12.0)
        );

        let l2 = out.skills.get(&(1177, 2)).expect("level 2 parsed");
        assert_eq!(l2.target_type, TargetType::Enemy);
        assert!(
            matches!(l2.effects.as_slice(), [SkillEffect::MagicalAttack { power }] if *power == 13.0)
        );
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
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let s = out.skills.get(&(1015, 1)).expect("skill parsed");
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
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let s = out.skills.get(&(1078, 1)).expect("skill parsed");
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
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let dol = out.skills.get(&(277, 1)).expect("Dance of Light parsed");
        // `AttackAttribute` graduated from icon-only marker to a real element
        // POWER modifier in the G19 attributes slice.
        assert!(
            matches!(
                dol.effects.as_slice(),
                [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::HolyPower, amount, .. })] if *amount == 20.0
            ),
            "Dance of Light grants HolyPower +20: {:?}",
            dol.effects
        );
        let soc = out.skills.get(&(8547, 1)).expect("Song of Champion parsed");
        assert!(
            matches!(
                soc.effects.as_slice(),
                [SkillEffect::MagicMpCost, SkillEffect::Reuse]
            ),
            "MagicMpCost/Reuse are not dropped"
        );
        let sov = out.skills.get(&(305, 1)).expect("Song of Vengeance parsed");
        assert!(
            matches!(sov.effects.as_slice(), [SkillEffect::DamageShield]),
            "DamageShield is not dropped"
        );
    }

    /// G19 `EnlargeSlot`: the craftsman-guild storage passives (real dist
    /// shapes — Expand Inventory has no `<type>`, defaulting to
    /// `INVENTORY_NORMAL`; Expand Dwarven Craft picks `RECIPE_DWARVEN`; Expand
    /// Trade carries two effect blocks, one `TRADE_BUY` one `TRADE_SELL`).
    /// Before this arm the effect fell through to `EFFECT_REGISTRY` (a
    /// 1-name-1-stat table that can't express the type-selected stat) and
    /// these skills did nothing.
    #[test]
    fn enlarge_slot_picks_stat_by_type_param() {
        let xml = r#"
        <list>
            <skill id="1372" toLevel="1" name="Expand Inventory">
                <operateType>P</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="EnlargeSlot">
                        <amount>6</amount>
                        <mode>DIFF</mode>
                    </effect>
                </effects>
            </skill>
            <skill id="1368" toLevel="1" name="Expand Dwarven Craft">
                <operateType>P</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="EnlargeSlot">
                        <amount>6</amount>
                        <mode>DIFF</mode>
                        <type>RECIPE_DWARVEN</type>
                    </effect>
                </effects>
            </skill>
            <skill id="1370" toLevel="1" name="Expand Trade">
                <operateType>P</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="EnlargeSlot">
                        <amount>1</amount>
                        <mode>DIFF</mode>
                        <type>TRADE_BUY</type>
                    </effect>
                    <effect name="EnlargeSlot">
                        <amount>1</amount>
                        <mode>DIFF</mode>
                        <type>TRADE_SELL</type>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let inv = out.skills.get(&(1372, 1)).expect("Expand Inventory parsed");
        assert!(
            matches!(inv.effects.as_slice(), [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::InventoryNormal, amount, .. })] if *amount == 6.0),
            "no <type> defaults to INVENTORY_NORMAL: {:?}",
            inv.effects
        );
        let dwc = out
            .skills
            .get(&(1368, 1))
            .expect("Expand Dwarven Craft parsed");
        assert!(
            matches!(dwc.effects.as_slice(), [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::RecipeDwarven, amount, .. })] if *amount == 6.0),
            "type=RECIPE_DWARVEN picked: {:?}",
            dwc.effects
        );
        let trade = out.skills.get(&(1370, 1)).expect("Expand Trade parsed");
        assert!(
            matches!(
                trade.effects.as_slice(),
                [
                    SkillEffect::StatModifier(StatModifierEffect {
                        stat: Stat::TradeBuy,
                        ..
                    }),
                    SkillEffect::StatModifier(StatModifierEffect {
                        stat: Stat::TradeSell,
                        ..
                    }),
                ]
            ),
            "both TRADE_BUY and TRADE_SELL land: {:?}",
            trade.effects
        );
    }

    /// G19 hate-manipulation effects — real dist shapes: `GetAgro` is a
    /// self-closing no-param tag (Aggression 28, paired with `TargetMe`,
    /// which stays unported — no locked-target UI concept on this port);
    /// `AddHate` reads `power`; `DeleteHate`/`DeleteHateOfMe` read `chance`.
    /// Before this arm all four fell through to `EFFECT_REGISTRY`, weren't
    /// found, and were silently dropped.
    #[test]
    fn hate_effects_parse_getagro_addhate_deletehate() {
        let xml = r#"
        <list>
            <skill id="28" toLevel="1" name="Aggression">
                <operateType>A1</operateType>
                <targetType>ENEMY_ONLY</targetType>
                <effects>
                    <effect name="TargetMe" />
                    <effect name="GetAgro" />
                </effects>
            </skill>
            <skill id="15" toLevel="1" name="Charm">
                <operateType>A1</operateType>
                <targetType>ENEMY_ONLY</targetType>
                <effects>
                    <effect name="AddHate">
                        <power>500</power>
                    </effect>
                </effects>
            </skill>
            <skill id="1273" toLevel="1" name="Eva's Serenade">
                <operateType>A2</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="DeleteHate">
                        <chance>80</chance>
                    </effect>
                </effects>
            </skill>
            <skill id="1156" toLevel="1" name="Forget">
                <operateType>A2</operateType>
                <targetType>ENEMY_ONLY</targetType>
                <effects>
                    <effect name="DeleteHateOfMe">
                        <chance>80</chance>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let aggression = out.skills.get(&(28, 1)).expect("Aggression parsed");
        assert!(
            matches!(aggression.effects.as_slice(), [SkillEffect::GetAgro]),
            "GetAgro lands (TargetMe stays unported, dropped): {:?}",
            aggression.effects
        );
        let charm = out.skills.get(&(15, 1)).expect("Charm parsed");
        assert!(
            matches!(charm.effects.as_slice(), [SkillEffect::AddHate { power }] if *power == 500.0),
            "AddHate power=500: {:?}",
            charm.effects
        );
        let eva = out.skills.get(&(1273, 1)).expect("Eva's Serenade parsed");
        assert!(
            matches!(
                eva.effects.as_slice(),
                [SkillEffect::DeleteHate { chance: 80 }]
            ),
            "DeleteHate chance=80: {:?}",
            eva.effects
        );
        let forget = out.skills.get(&(1156, 1)).expect("Forget parsed");
        assert!(
            matches!(
                forget.effects.as_slice(),
                [SkillEffect::DeleteHateOfMe { chance: 80 }]
            ),
            "DeleteHateOfMe chance=80: {:?}",
            forget.effects
        );
    }

    /// G19 `DispelByCategory` — the "Cancel" family, real dist shapes:
    /// Cancellation (`BUFF`/25/5) and Cleanse (`DEBUFF`/100/10, no `<slot>`
    /// exercised here since Cancellation already covers the explicit-BUFF
    /// path — Cleanse's own tag is DEBUFF). Before this arm the effect fell
    /// through to `EFFECT_REGISTRY`, wasn't found, and these skills stripped
    /// nothing.
    #[test]
    fn dispel_by_category_parses_slot_rate_max() {
        let xml = r#"
        <list>
            <skill id="1056" toLevel="1" name="Cancellation">
                <operateType>A1</operateType>
                <targetType>TARGET</targetType>
                <effects>
                    <effect name="DispelByCategory">
                        <slot>BUFF</slot>
                        <rate>25</rate>
                        <max>5</max>
                    </effect>
                </effects>
            </skill>
            <skill id="1409" toLevel="1" name="Cleanse">
                <operateType>A1</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="DispelByCategory">
                        <slot>DEBUFF</slot>
                        <rate>100</rate>
                        <max>10</max>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let cancellation = out.skills.get(&(1056, 1)).expect("Cancellation parsed");
        assert!(
            matches!(
                cancellation.effects.as_slice(),
                [SkillEffect::DispelByCategory {
                    slot: DispelSlot::Buff,
                    rate: 25,
                    max: 5
                }]
            ),
            "BUFF/25/5: {:?}",
            cancellation.effects
        );
        let cleanse = out.skills.get(&(1409, 1)).expect("Cleanse parsed");
        assert!(
            matches!(
                cleanse.effects.as_slice(),
                [SkillEffect::DispelByCategory {
                    slot: DispelSlot::Debuff,
                    rate: 100,
                    max: 10
                }]
            ),
            "DEBUFF/100/10: {:?}",
            cleanse.effects
        );
    }

    /// G19 `PhysicalAttackRange`: real dist shapes — Archery (431, `DIFF
    /// +50`) and Rapid Fire (413, `PER -50`, a stance trading range for
    /// reload speed), both `<weaponType>BOW</weaponType>`-conditioned. Before
    /// this it was unregistered in `EFFECT_REGISTRY` and fell through.
    #[test]
    fn physical_attack_range_parses_diff_and_per_bow_conditioned() {
        let xml = r#"
        <list>
            <skill id="431" toLevel="1" name="Archery">
                <operateType>P</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="PhysicalAttackRange">
                        <amount>50</amount>
                        <mode>DIFF</mode>
                        <weaponType><item>BOW</item></weaponType>
                    </effect>
                </effects>
            </skill>
            <skill id="413" toLevel="1" name="Rapid Fire">
                <operateType>T</operateType>
                <targetType>SELF</targetType>
                <effects>
                    <effect name="PhysicalAttackRange">
                        <amount>-50</amount>
                        <mode>PER</mode>
                        <weaponType><item>BOW</item></weaponType>
                    </effect>
                </effects>
            </skill>
        </list>"#;
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);

        let archery = out.skills.get(&(431, 1)).expect("Archery parsed");
        assert!(
            matches!(
                archery.effects.as_slice(),
                [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttackRange, mode: StatModifierType::Diff, amount, weapon_condition, .. })]
                    if *amount == 50.0 && *weapon_condition != 0
            ),
            "DIFF +50, bow-conditioned: {:?}",
            archery.effects
        );
        let rapid_fire = out.skills.get(&(413, 1)).expect("Rapid Fire parsed");
        assert!(
            matches!(
                rapid_fire.effects.as_slice(),
                [SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttackRange, mode: StatModifierType::Per, amount, weapon_condition, .. })]
                    if *amount == -50.0 && *weapon_condition != 0
            ),
            "PER -50, bow-conditioned: {:?}",
            rapid_fire.effects
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
        let mut out = ParsedSkills::default();
        parse_str(xml, &mut out);
        let mut sd = SkillData {
            skills: out.skills,
            enchanted: out.enchanted,
            routes: out.routes,
        };

        let list = HashMap::from([(1078, 7200), (9999, 7200)]);
        sd.apply_skill_duration_list(&list);

        assert_eq!(
            sd.get(1078, 1).unwrap().abnormal_time,
            7200,
            "active buff time replaced"
        );
        assert_eq!(
            sd.get(9999, 1).unwrap().abnormal_time,
            1200,
            "toggle is exempt"
        );
        assert_eq!(
            sd.get(5555, 1).unwrap().abnormal_time,
            1200,
            "skill not in list is untouched"
        );

        // Enchanted levels (100..=140) add rather than replace.
        let enchanted = Skill {
            level: 101,
            ..sd.get(1078, 1).unwrap().clone()
        };
        sd.insert_for_test(enchanted);
        sd.apply_skill_duration_list(&HashMap::from([(1078, 100)]));
        assert_eq!(
            sd.get(1078, 101).unwrap().abnormal_time,
            7300,
            "enchanted level adds to base"
        );
    }
}
