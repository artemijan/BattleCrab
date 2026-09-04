//! `TriggerSkillBy*` — the chance-on-event effects.

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
        "TriggerSkillByDamage" => {
            vec![skill::effects::SkillEffect::TriggerSkillByDamage {
                min_damage: param("minDamage").unwrap_or(1.0) as i32,
                chance: param("chance").unwrap_or(100.0) as i32,
                skill_id: param("skillId").unwrap_or(0.0) as i32,
                skill_level: param("skillLevel").unwrap_or(1.0) as i32,
                hp_percent: param("hpPercent").unwrap_or(100.0) as i32,
                attacker_playable_only: value_at(params, "attackerType", level) == Some("Playable"),
                // Java's default is SELF; ENEMY is what casts the
                // trigger back at whoever hit you.
                on_attacker: value_at(params, "targetType", level) == Some("ENEMY"),
            }]
        }
        "TriggerSkillByMagicType" => {
            vec![skill::effects::SkillEffect::TriggerSkillByMagicType {
                magic_types: value_at(params, "magicTypes", level)
                    .map(|v| v.split(';').filter_map(|t| t.trim().parse().ok()).collect())
                    .unwrap_or_default(),
                chance: param("chance").unwrap_or(100.0) as i32,
                skill_id: param("skillId").unwrap_or(0.0) as i32,
                // Java's default here is 0, which disables the
                // effect — unlike the damage twin's 1.
                skill_level: param("skillLevel").unwrap_or(0.0) as i32,
                on_party: value_at(params, "targetType", level) == Some("MY_PARTY"),
            }]
        }
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
                            crate::data::item_data::kinds::WeaponType::from_name(w.trim())
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
                vec![skill::effects::SkillEffect::TriggerSkillByAttack {
                    min_damage: int_param("minDamage", 1),
                    chance: int_param("chance", 100),
                    skill_id,
                    skill_level: int_param("skillLevel", 1),
                    on_party: value_at(params, "targetType", level) == Some("MY_PARTY"),
                    is_critical: value_at(params, "isCritical", level) == Some("true"),
                    allow_weapons,
                }]
            }
        }
        _ => return None,
    })
}
