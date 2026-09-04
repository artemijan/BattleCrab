//! The `DispelBy*` family.

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
        // `DispelBySlotMyself` — `<dispel>` is a `;`-separated
        // list of abnormal *types* with no levels, unlike
        // `DispelBySlot`'s `TYPE=level` pairs.
        "DispelBySlotMyself" => value_at(params, "dispel", level)
            .map(|d| {
                vec![skill::effects::SkillEffect::DispelBySlotMyself {
                    dispel: d
                        .split(';')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect(),
                }]
            })
            .unwrap_or_default(),
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
                return Some(Vec::new());
            }
            let rate = value_at(params, "rate", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(100);
            vec![skill::effects::SkillEffect::DispelBySlotProbability { dispel, rate }]
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
                    vec![skill::effects::SkillEffect::DispelBySlot { dispel }]
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
                Some("DEBUFF") => skill::effects::DispelSlot::Debuff,
                Some("ALL") => skill::effects::DispelSlot::All,
                _ => skill::effects::DispelSlot::Buff,
            };
            let rate = value_at(params, "rate", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            let max = value_at(params, "max", level)
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            vec![skill::effects::SkillEffect::DispelByCategory { slot, rate, max }]
        }
        "DispelAll" => vec![skill::effects::SkillEffect::DispelAll],
        _ => return None,
    })
}
