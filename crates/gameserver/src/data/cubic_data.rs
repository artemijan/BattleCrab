//! Port of `data/xml/CubicData` + `model/cubic/CubicTemplate` (G29): the cubic
//! templates in `dist/game/data/stats/cubics/*.xml`.
//!
//! A cubic is a floating satellite that periodically casts a skill on its
//! owner's behalf. It is **not** a world object — it lives on the player, which
//! is why this needs no NPC template and no spawn.
//!
//! 12 of the 28 `SummonCubic` skills are learnable on this dist, which is what
//! made cubics the next slice ahead of agathions (166 `SummonAgathion` skills,
//! **zero** of them on any skill tree).

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::info;

const CUBICS_DIR: &str = "data/stats/cubics";

/// Java `CubicTargetType`. `MASTER` exists in the enum but no template on this
/// dist uses it at the template level (only nested skills do).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CubicTargetType {
    /// Whatever the owner currently has targeted — the attack cubics.
    #[default]
    Target,
    /// The nested skill's own target type decides.
    BySkill,
    /// The most-wounded of the owner, their party, and their summons.
    Heal,
    /// The owner themselves.
    Master,
}

impl CubicTargetType {
    fn parse(s: &str) -> Self {
        match s {
            "BY_SKILL" => Self::BySkill,
            "HEAL" => Self::Heal,
            "MASTER" => Self::Master,
            _ => Self::Target,
        }
    }
}

/// One `<skill>` a cubic can cast.
#[derive(Debug, Clone, Default)]
pub struct CubicSkill {
    pub skill_id: i32,
    pub skill_level: i32,
    /// `successRate` — rolled as `Rnd.get(100) < rate` *after* the skill is
    /// chosen, so it gates the cast rather than the choice.
    pub success_rate: i32,
    /// `triggerRate` — the cumulative weight used to *choose* between a
    /// cubic's skills. Absent means 100 (a lone skill always wins the roll).
    pub trigger_rate: i32,
    /// `canUseOnStaticObjects` — whether a door is a valid target.
    pub can_use_on_static_objects: bool,
    /// A nested `targetType`, consulted only when the template's own type is
    /// `BY_SKILL`.
    pub target_type: Option<CubicTargetType>,
}

/// `<hp type="GREATER" percent="33"/>` — the owner must be above (or below)
/// this share of max HP for the cubic to act.
#[derive(Debug, Clone, Copy)]
pub struct HpCondition {
    pub percent: i32,
    /// `type="GREATER"` (the only value on this dist) vs `LESS`.
    pub greater: bool,
}

/// One cubic template — `(id, level)` keyed, exactly as Java stores them.
#[derive(Debug, Clone, Default)]
pub struct CubicTemplate {
    pub id: i32,
    pub level: i32,
    /// Which of the owner's cubic slots this occupies (display position).
    pub slot: i32,
    /// Seconds the cubic lives before expiring.
    pub duration: i32,
    /// Seconds between action attempts.
    pub delay: i32,
    /// How many times it may act before going away. 0 = unlimited.
    pub max_count: i32,
    pub power: f64,
    pub target_type: CubicTargetType,
    pub skills: Vec<CubicSkill>,
    /// `<hp>` condition on the **owner**.
    pub hp_condition: Option<HpCondition>,
    /// `<range value="1000"/>` — max distance to the target.
    pub range: Option<i32>,
    /// `<healthPercent min max/>` — a condition on the **target**'s HP share,
    /// used by the debuff cubics to focus wounded enemies.
    pub health_percent: Option<(i32, i32)>,
}

#[derive(Debug, Default)]
pub struct CubicData {
    by_key: HashMap<(i32, i32), CubicTemplate>,
}

impl CubicData {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Java `CubicData.getCubicTemplate(id, level)`.
    pub fn get(&self, id: i32, level: i32) -> Option<&CubicTemplate> {
        self.by_key.get(&(id, level))
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    pub fn insert_for_test(&mut self, t: CubicTemplate) {
        self.by_key.insert((t.id, t.level), t);
    }

    pub fn load_from(base: &str) -> Self {
        let dir = std::path::Path::new(base).join(CUBICS_DIR);
        let mut out = Self::default();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            info!("cubic data: {} not found, no cubics loaded", dir.display());
            return out;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "xml"))
            .collect();
        files.sort();
        for f in files {
            out.load_file(&f);
        }
        info!("cubic data: {} templates", out.by_key.len());
        out
    }

    fn load_file(&mut self, path: &std::path::Path) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let mut reader = Reader::from_str(&text);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut cur: Option<CubicTemplate> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = e.name();
                    let tag = String::from_utf8_lossy(name.as_ref()).to_string();
                    let attr = |key: &str| super::xml::attr_str(&e, key.as_bytes());
                    match tag.as_str() {
                        "cubic" => {
                            let mut t = CubicTemplate {
                                id: attr("id").and_then(|v| v.parse().ok()).unwrap_or(0),
                                level: attr("level").and_then(|v| v.parse().ok()).unwrap_or(1),
                                slot: attr("slot").and_then(|v| v.parse().ok()).unwrap_or(1),
                                duration: attr("duration")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0),
                                delay: attr("delay").and_then(|v| v.parse().ok()).unwrap_or(0),
                                max_count: attr("maxCount")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0),
                                power: attr("power").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                                target_type: attr("targetType")
                                    .map(|v| CubicTargetType::parse(&v))
                                    .unwrap_or_default(),
                                ..Default::default()
                            };
                            // `<cubic .../>` self-closing would end here; the
                            // real files always nest, but don't assume it.
                            if matches!(reader.read_event_into(&mut Vec::new()), Ok(Event::Eof)) {
                                self.by_key.insert((t.id, t.level), std::mem::take(&mut t));
                                break;
                            }
                            cur = Some(t);
                        }
                        "skill" => {
                            if let Some(t) = cur.as_mut() {
                                t.skills.push(CubicSkill {
                                    skill_id: attr("id").and_then(|v| v.parse().ok()).unwrap_or(0),
                                    skill_level: attr("level")
                                        .and_then(|v| v.parse().ok())
                                        .unwrap_or(1),
                                    success_rate: attr("successRate")
                                        .and_then(|v| v.parse().ok())
                                        .unwrap_or(100),
                                    // A lone skill with no triggerRate must
                                    // still win the cumulative roll.
                                    trigger_rate: attr("triggerRate")
                                        .and_then(|v| v.parse().ok())
                                        .unwrap_or(100),
                                    can_use_on_static_objects: attr("canUseOnStaticObjects")
                                        .map(|v| v == "true")
                                        .unwrap_or(false),
                                    target_type: attr("targetType")
                                        .map(|v| CubicTargetType::parse(&v)),
                                });
                            }
                        }
                        "hp" => {
                            if let Some(t) = cur.as_mut() {
                                t.hp_condition = Some(HpCondition {
                                    percent: attr("percent")
                                        .and_then(|v| v.parse().ok())
                                        .unwrap_or(0),
                                    greater: attr("type").map(|v| v != "LESS").unwrap_or(true),
                                });
                            }
                        }
                        "range" => {
                            if let Some(t) = cur.as_mut() {
                                t.range = attr("value").and_then(|v| v.parse().ok());
                            }
                        }
                        "healthPercent" => {
                            if let Some(t) = cur.as_mut() {
                                let min = attr("min").and_then(|v| v.parse().ok()).unwrap_or(0);
                                let max = attr("max").and_then(|v| v.parse().ok()).unwrap_or(100);
                                t.health_percent = Some((min, max));
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"cubic"
                        && let Some(t) = cur.take()
                    {
                        self.by_key.insert((t.id, t.level), t);
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIST: &str = crate::data::DIST_GAME;

    /// The real datapack parses, and the first Storm Cubic level carries the
    /// values the XML shows — a guard on the whole attribute set at once.
    #[test]
    fn the_real_cubic_table_parses() {
        let data = CubicData::load_from(DIST);
        assert!(
            data.len() > 100,
            "expected the full cubic table, got {}",
            data.len()
        );

        let storm = data.get(1, 1).expect("cubic 1 level 1");
        assert_eq!(storm.duration, 900);
        assert_eq!(storm.delay, 10);
        assert_eq!(storm.max_count, 30);
        assert_eq!(storm.power, 282.0);
        assert_eq!(storm.target_type, CubicTargetType::Target);
        assert_eq!(storm.range, Some(1000));
        let hp = storm.hp_condition.expect("hp condition");
        assert_eq!((hp.percent, hp.greater), (33, true));

        assert_eq!(storm.skills.len(), 1);
        let s = &storm.skills[0];
        assert_eq!((s.skill_id, s.skill_level, s.success_rate), (4049, 1, 12));
        assert!(s.can_use_on_static_objects);
        assert_eq!(
            s.trigger_rate, 100,
            "a lone skill with no triggerRate must still win the roll"
        );
    }

    /// Multi-skill cubics carry real `triggerRate` weights — the parse would
    /// look fine on single-skill cubics alone.
    #[test]
    fn multi_skill_cubics_carry_trigger_rates() {
        let data = CubicData::load_from(DIST);
        let multi = (1..=40)
            .flat_map(|id| (1..=12).map(move |lvl| (id, lvl)))
            .filter_map(|(id, lvl)| data.get(id, lvl))
            .find(|t| t.skills.len() > 1)
            .expect("at least one cubic casts more than one skill");
        assert!(
            multi.skills.iter().all(|s| s.trigger_rate > 0),
            "every skill of a multi-skill cubic needs a weight, else it can never be chosen"
        );
    }
}
