//! `AttackTrait` / `DefenceTrait` — the trait tables a skill merges.

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
    let _param = |key: &str| cx.param(key);
    let _stat_mod = |stat: Stat, amount: f64| cx.stat_mod(stat, amount);

    Some(match xml_name.as_str() {
        // Mental Shield (1035) / Stun Resistance ("Resist Shock",
        // 1259): Java `DefenceTrait` raises per-`TraitType`
        // resistance (HOLD/SLEEP/SHOCK…). Its params are the trait
        // *names*, not `amount`, so they are read straight off the
        // param map rather than through the usual `amount` lookup.
        //
        // "Detect <Category> Weakness" (75/80/87/88/104, 359/360):
        // Java `AttackTrait` merges a `*_WEAKNESS` bonus onto the
        // caster — genuinely inert in the reference server too (see
        // the doc comment on `SkillEffect::AttackTrait`), so this
        // carries an icon-only marker like `DefenceTrait`/
        // `VampiricAttack` rather than the per-trait param map.
        "DefenceTrait" | "AttackTrait" => {
            // Every param is a trait name → percent; Java divides by
            // 100 (and a defence value >= 1.0 is invulnerable).
            let traits: Vec<(crate::model::skill::traits::TraitType, f64)> = params
                .keys()
                .filter_map(|key| {
                    let raw = value_at(params, key, level)?;
                    let pct: f64 = raw.parse().ok()?;
                    Some((
                        crate::model::skill::traits::TraitType::from_xml(key),
                        pct / 100.0,
                    ))
                })
                .collect();
            vec![if xml_name == "DefenceTrait" {
                skill::effects::SkillEffect::DefenceTrait { traits }
            } else {
                skill::effects::SkillEffect::AttackTrait { traits }
            }]
        }
        _ => return None,
    })
}
