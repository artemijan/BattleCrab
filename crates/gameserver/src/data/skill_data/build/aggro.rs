//! Hate and target-me effects.

use super::super::value_at;
use super::effect::Cx;
use crate::model::skill;
use crate::model::stats::Stat;

pub(super) fn build(cx: &Cx<'_>) -> Option<Vec<skill::effects::SkillEffect>> {
    let &Cx {
        xml_name,
        params,
        mode,
        groups,
        armor_condition,
        weapon_condition,
        values,
        level,
        modifier_mode,
        hp_percent,
    } = cx;
    let _ = (
        mode,
        groups,
        armor_condition,
        weapon_condition,
        values,
        modifier_mode,
        hp_percent,
    );
    let param = |key: &str| cx.param(key);
    let _stat_mod = |stat: Stat, amount: f64| cx.stat_mod(stat, amount);

    Some(match xml_name.as_str() {
        "TargetMe" => vec![skill::effects::SkillEffect::TargetMe],
        "TargetMeProbability" => {
            vec![skill::effects::SkillEffect::TargetMeProbability {
                chance: param("chance").unwrap_or(100.0) as i32,
            }]
        }
        "TargetCancel" => {
            let chance = value_at(params, "chance", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(100);
            vec![skill::effects::SkillEffect::TargetCancel { chance }]
        }
        // Aggression 28/18, Judgment 401, Tribunal 400: no params.
        "GetAgro" => vec![skill::effects::SkillEffect::GetAgro],
        // Charm 15, Lure 51: `power` (default 0, Java always
        // instantiates the handler even with no param).
        "AddHate" => {
            vec![skill::effects::SkillEffect::AddHate {
                power: param("power").unwrap_or(0.0),
            }]
        }
        "DeleteHate" => {
            let chance = value_at(params, "chance", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(100);
            vec![skill::effects::SkillEffect::DeleteHate { chance }]
        }
        "DeleteHateOfMe" => {
            let chance = value_at(params, "chance", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(100);
            vec![skill::effects::SkillEffect::DeleteHateOfMe { chance }]
        }
        "RandomizeHate" => vec![skill::effects::SkillEffect::RandomizeHate {
            chance: value_at(params, "chance", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
        }],
        _ => return None,
    })
}
