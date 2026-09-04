//! Over-time effects and the rest states that tick with them.

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
        "DamOverTime" => vec![skill::effects::SkillEffect::DamOverTime {
            power: param("power").unwrap_or(0.0),
            ticks: param("ticks").unwrap_or(0.0) as i32,
            can_kill: value_at(params, "canKill", level) == Some("true"),
        }],
        "ManaDamOverTime" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::ManaDamOverTime { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        "HealOverTime" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::HealOverTime { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        "ManaHealOverTime" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::ManaHealOverTime { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        // `Relax` — the seated MP-upkeep toggle (skill 226).
        "Relax" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::Relax { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        "ChameleonRest" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::ChameleonRest { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        _ => return None,
    })
}
