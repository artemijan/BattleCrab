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

use crate::model::skill::{OperateType, Skill, SkillEffect, StatModifierEffect, TargetType};
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
    // instant damage/heal handlers —, mode).
    let mut effects: Vec<(String, LeveledValues, String)> = Vec::new();
    let mut in_effects = false;
    let mut in_conditions = false;
    let mut cur_effect_name: Option<String> = None;
    let mut cur_effect_params: LeveledValues = HashMap::new();
    let mut cur_effect_mode = String::from("DIFF");
    let mut cur_effect_field = String::new();

    // Tag-name stack relative to `<skill>` (path[0] == "skill" once inside one).
    let mut path: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(_)) => {
                // Self-closing leaf (e.g. an attribute-only tag with no text) —
                // none of the fields this loader reads use this shape, so
                // there's nothing to record; explicitly not pushed onto `path`
                // since no matching `End` event follows a self-closing tag.
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
                } else if path.len() == 3 && in_effects {
                    cur_effect_field = name.clone();
                } else if path.len() == 4 && in_effects && name == "value" {
                    pending_level = attr_i32(&e, b"level").unwrap_or(0);
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
                } else if closed == "effect" && in_effects {
                    if let Some(name) = cur_effect_name.take() {
                        effects.push((name, cur_effect_params.clone(), cur_effect_mode.clone()));
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
    effects: &[(String, LeveledValues, String)],
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
            _ => TargetType::Other,
        };

        let skill_effects = effects
            .iter()
            .filter_map(|(xml_name, params, mode)| {
                let param = |key: &str| -> Option<f64> { value_at(params, key, level).and_then(|v| v.parse().ok()) };
                match xml_name.as_str() {
                    "MagicalAttack" => Some(SkillEffect::MagicalAttack { power: param("power")? }),
                    "Heal" => Some(SkillEffect::Heal { power: param("power")? }),
                    _ => {
                        let stat = EFFECT_REGISTRY.iter().find(|(n, _)| n == xml_name).map(|(_, s)| *s)?;
                        let mode = if mode == "PER" { StatModifierType::Per } else { StatModifierType::Diff };
                        Some(SkillEffect::StatModifier(StatModifierEffect { stat, mode, amount: param("amount")? }))
                    }
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

        // "Knight - Individual" shares reuse group 10008 with its siblings.
        let ki = sd.get(10248, 1).expect("Knight - Individual lvl 1");
        assert_eq!(ki.reuse_delay_group, 10008);
        assert_eq!(ki.reuse_key(), 10008);
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
            SkillEffect::StatModifier(StatModifierEffect { stat: Stat::PhysicalAttack, mode: StatModifierType::Per, amount }) if amount == 10.0
        ));
    }
}
