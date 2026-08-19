//! Port of `data/xml/OptionData` — `data/stats/augmentation/options/*.xml`, the
//! bonuses an augmented weapon's two option ids grant while it is equipped.
//!
//! Each `<option>` is one bonus: a list of `<effects>` (the same
//! `<effect name>` vocabulary skills use, so the stat mapping is shared with
//! [`crate::data::skill_data::stat_for_effect_name`]), plus optional skills —
//! `passive_skill` (always on), `active_skill` (added to the skill list), and
//! the three activation kinds (`attack_skill` / `magic_skill` /
//! `critical_skill`, which Java feeds to `Player.addTriggerSkill`).
//!
//! Of the ~19 400 option ids this dist's `Variations.xml` can actually roll,
//! about 80 % are stat-only; the rest carry a skill. This loader keeps all of
//! it; what the runtime *applies* is documented in
//! [`crate::game_loop::options`].

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

use crate::model::skill::StatModifierEffect;
use crate::model::stats::StatModifierType;

pub const OPTIONS_DIR: &str = "data/stats/augmentation/options";

/// Java `OptionSkillType` — when an activation skill fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OptionSkillType {
    Attack,
    Magic,
    Critical,
}

/// Java `OptionSkillHolder` — a skill that fires on a chance when the wearer
/// attacks / casts / crits.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct OptionTrigger {
    pub skill_id: i32,
    pub skill_level: i32,
    pub chance: f64,
    pub kind: OptionSkillType,
}

/// One `<option>` (Java `Options`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct OptionEntry {
    pub id: i32,
    /// Stat contributions, already mapped to the port's stat vocabulary.
    /// Effect names the port doesn't model yet (`DefenceTrait`,
    /// `DefenceAttribute`, `AttackTrait`, `StatUp`, …) are dropped here, the
    /// same way an unmapped skill effect is.
    pub effects: Vec<StatModifierEffect>,
    pub passive_skills: Vec<(i32, i32)>,
    pub active_skills: Vec<(i32, i32)>,
    pub triggers: Vec<OptionTrigger>,
}

impl OptionEntry {
    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct OptionData {
    by_id: HashMap<i32, OptionEntry>,
}

impl OptionData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut by_id = HashMap::new();
        let dir = format!("{file_path}{OPTIONS_DIR}");
        for file in &super::xml::xml_files_in(&dir) {
            parse_file(file, &mut by_id);
        }
        let with_skills = by_id
            .values()
            .filter(|o| {
                !o.passive_skills.is_empty()
                    || !o.active_skills.is_empty()
                    || !o.triggers.is_empty()
            })
            .count();
        info!(
            "OptionData: Loaded {} options ({with_skills} carrying skills).",
            by_id.len()
        );
        Self { by_id }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, entry: OptionEntry) {
        self.by_id.insert(entry.id, entry);
    }

    /// Java `OptionData.getOptions(id)`.
    pub fn get(&self, id: i32) -> Option<&OptionEntry> {
        self.by_id.get(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn parse_file(path: &std::path::Path, out: &mut HashMap<i32, OptionEntry>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut current: Option<OptionEntry> = None;
    // `<effect name="X"><amount>N</amount><mode>DIFF</mode></effect>` — the
    // parameters are child *nodes*, so they are accumulated as text lands.
    let mut effect_name: Option<String> = None;
    let mut amount: Option<f64> = None;
    let mut mode = StatModifierType::Diff;
    let mut text_target: Option<&'static str> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let attr = |key: &str| super::xml::attr_strict(&e, key);
                let num = |key: &str| attr(key).and_then(|v| v.parse::<f64>().ok());
                match e.name().as_ref() {
                    b"option" => {
                        current =
                            attr("id")
                                .and_then(|v| v.parse::<i32>().ok())
                                .map(|id| OptionEntry {
                                    id,
                                    ..Default::default()
                                });
                    }
                    b"effect" => {
                        effect_name = attr("name");
                        amount = None;
                        mode = StatModifierType::Diff;
                    }
                    b"amount" => text_target = Some("amount"),
                    b"mode" => text_target = Some("mode"),
                    tag @ (b"passive_skill" | b"active_skill" | b"attack_skill"
                    | b"magic_skill" | b"critical_skill") => {
                        if let (Some(o), Some(id), Some(level)) = (
                            current.as_mut(),
                            num("id").map(|v| v as i32),
                            num("level").map(|v| v as i32),
                        ) {
                            match tag {
                                b"passive_skill" => o.passive_skills.push((id, level)),
                                b"active_skill" => o.active_skills.push((id, level)),
                                _ => o.triggers.push(OptionTrigger {
                                    skill_id: id,
                                    skill_level: level,
                                    chance: num("chance").unwrap_or(0.0),
                                    kind: match tag {
                                        b"attack_skill" => OptionSkillType::Attack,
                                        b"magic_skill" => OptionSkillType::Magic,
                                        _ => OptionSkillType::Critical,
                                    },
                                }),
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let value = t.unescape().unwrap_or_default().trim().to_string();
                match text_target.take() {
                    Some("amount") => amount = value.parse().ok(),
                    Some("mode") => {
                        mode = if value == "PER" {
                            StatModifierType::Per
                        } else {
                            StatModifierType::Diff
                        };
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"effect" => {
                    if let (Some(o), Some(name), Some(amount)) =
                        (current.as_mut(), effect_name.take(), amount.take())
                        && let Some(stat) = crate::data::skill_data::stat_for_effect_name(&name)
                    {
                        o.effects.push(StatModifierEffect {
                            stat,
                            mode,
                            amount,
                            armor_condition: 0,
                            weapon_condition: 0,
                            qualifier: None,
                            two_handed: false,
                            // Augment options carry no `hpPercent`: the four
                            // `AbstractConditionalHpEffect` names that could
                            // (`PAtk`, `PhysicalDefence`, `PhysicalEvasion`,
                            // `CriticalRate`) appear here as plain stat rows.
                            hp_percent: 0,
                        });
                    }
                }
                b"option" => {
                    if let Some(o) = current.take() {
                        out.insert(o.id, o);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    #[test]
    fn loads_the_dist_option_catalogue() {
        let data = OptionData::load_from(DIST);
        // 342 files; every `<option>` in them lands.
        assert!(data.len() > 30_000, "loaded {}", data.len());

        // Option 1 is the first line of the first file: `P. Def. + 15.45` DIFF.
        let one = data.get(1).expect("option 1");
        assert_eq!(one.effects.len(), 1);
        assert_eq!(
            one.effects[0].stat,
            crate::model::stats::Stat::PhysicalDefence
        );
        assert_eq!(one.effects[0].mode, StatModifierType::Diff);
        assert!((one.effects[0].amount - 15.45).abs() < 1e-9);

        // Skill-carrying options load their skills (14578+ are the active/passive
        // block `Variations.xml` rolls for a Lv. 46 life stone).
        let with_skills = (14578..=14738)
            .filter_map(|id| data.get(id))
            .filter(|o| !o.active_skills.is_empty() || !o.passive_skills.is_empty())
            .count();
        assert!(with_skills > 100, "skill options load: {with_skills}");
    }
}
