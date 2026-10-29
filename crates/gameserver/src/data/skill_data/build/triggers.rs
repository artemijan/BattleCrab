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
        "TriggerSkillByDamageReceived" => {
            let Some(trigger) = damage_trigger(cx) else {
                return Some(Vec::new());
            };
            vec![skill::effects::SkillEffect::TriggerSkillByDamageReceived {
                trigger,
                hp_percent: cx.int_param("hpPercent", 100),
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
        "TriggerSkillByDamageDealt" => {
            let Some(trigger) = damage_trigger(cx) else {
                return Some(Vec::new());
            };
            // `"ALL"` (the default) is 0; otherwise `WeaponType.valueOf` per
            // `,`-separated name, OR'd into a mask.
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
            let flag = |key: &str, default: bool| {
                value_at(params, key, level).map_or(default, |v| v.eq_ignore_ascii_case("true"))
            };
            vec![skill::effects::SkillEffect::TriggerSkillByDamageDealt {
                trigger,
                is_critical: flag("isCritical", false),
                allow_normal_attack: flag("allowNormalAttack", true),
                allow_skill_attack: flag("allowSkillAttack", false),
                allow_reflect: flag("allowReflect", false),
                allow_weapons,
            }]
        }
        _ => return None,
    })
}

/// The constructor half both damage triggers share. `None` is Java's
/// `getEnum` throwing on an `attackerType` it does not know, which fails the
/// effect's construction; the dist names none.
fn damage_trigger(cx: &Cx<'_>) -> Option<skill::effects::DamageTrigger> {
    use skill::effects::AttackerType;
    let text = |key: &str| value_at(cx.params, key, cx.level);
    let attacker_type = match text("attackerType") {
        None | Some("Creature") => AttackerType::Creature,
        Some("Playable") => AttackerType::Playable,
        Some("Attackable") => AttackerType::Attackable,
        Some(_) => return None,
    };
    let target_type = match text("targetType") {
        None => skill::target::TargetType::Self_,
        Some(raw) => super::target_type_from_name(raw)?,
    };
    // `triggerSkills="id,level;id,level"` — Java `Integer.parseInt`s both
    // halves of every entry.
    let trigger_skills = text("triggerSkills")
        .filter(|v| !v.is_empty())
        .map(|v| {
            v.split(';')
                .filter_map(|entry| {
                    let (id, lvl) = entry.split_once(',')?;
                    Some((id.trim().parse().ok()?, lvl.trim().parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(skill::effects::DamageTrigger {
        min_attacker_level: cx.int_param("minAttackerLevel", 1),
        max_attacker_level: cx.int_param("maxAttackerLevel", i32::MAX),
        min_damage: cx.int_param("minDamage", 1),
        chance: cx.int_param("chance", 100),
        skill_id: cx.int_param("skillId", 0),
        skill_level: cx.int_param("skillLevel", 1),
        target_type,
        attacker_type,
        skill_level_scale_to: cx.int_param("skillLevelScaleTo", 0),
        trigger_skills,
    })
}
