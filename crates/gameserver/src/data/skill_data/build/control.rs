//! The action-gating abnormals: block/mute/root/fear/confuse, fake death,
//! silent move and transformation.

use super::super::{FEAR_TICKS, value_at};
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
        // Stun / sleep / paralyze (540 uses) and Root (79): no stat
        // modifier at all — the whole mechanic is the abnormal-state
        // flag they contribute (`Skill::effect_flags`).
        // The four bot-report punishment effects (skills
        // 6038/6039/6040/6055/6056). Each is a pure state
        // effect: the work happens on the buff's start and
        // exit, not at parse time.
        "BlockChat" => vec![skill::effects::SkillEffect::BlockChat],
        "BlockParty" => vec![skill::effects::SkillEffect::BlockParty],
        "BlockAction" => {
            // `<blockedActions>-2</blockedActions>` — Java
            // splits on ',' and parses each as an int.
            let blocked_actions = value_at(params, "blockedActions", level)
                .map(|v| {
                    v.split(',')
                        .filter_map(|a| a.trim().parse::<i32>().ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            vec![skill::effects::SkillEffect::BlockAction { blocked_actions }]
        }
        "BlockActions" => {
            // Java: a non-empty `allowedSkills` whitelist makes this
            // CONDITIONAL_BLOCK_ACTIONS instead. Both gate the same
            // way in `hasBlockActions()`.
            let conditional =
                value_at(params, "allowedSkills", level).is_some_and(|v| !v.trim().is_empty());
            vec![skill::effects::SkillEffect::BlockActions { conditional }]
        }
        "Root" => vec![skill::effects::SkillEffect::Root],
        // The rest of the state-flag CC family (Seal of Silence,
        // Shield Slam, Mystic Immunity, Horror): no parameters, the
        // mechanic is entirely the flag.
        "Mute" => vec![skill::effects::SkillEffect::Mute],
        "PhysicalMute" => vec![skill::effects::SkillEffect::PhysicalMute],
        "PhysicalAttackMute" => {
            vec![skill::effects::SkillEffect::PhysicalAttackMute]
        }
        "DebuffBlock" => vec![skill::effects::SkillEffect::DebuffBlock],
        "BuffBlock" => vec![skill::effects::SkillEffect::BuffBlock],
        "Untargetable" => vec![skill::effects::SkillEffect::Untargetable],
        "DisableTargeting" => vec![skill::effects::SkillEffect::DisableTargeting],
        "BlockResurrection" => vec![skill::effects::SkillEffect::BlockResurrection],
        "BlockEscape" => vec![skill::effects::SkillEffect::BlockEscape],
        "AbnormalShield" => vec![skill::effects::SkillEffect::AbnormalShield],
        "BlockControl" => vec![skill::effects::SkillEffect::BlockControl],
        "BlockMove" => vec![skill::effects::SkillEffect::BlockMove],
        // Fear (65/405/450/1092/1169/1272/1381/1400): forced flight.
        // The `<effect name="Fear"/>` element carries no params in
        // this dist — Java's `Fear` constructor ignores its `StatSet`
        // outright and `getTicks()` returns a hard-coded 5 — so the
        // cadence is a literal, not a parsed value. Every one of
        // these skills also carries `BlockControl`, so the *buff*
        // already landed before this arm existed (icon, duration and
        // the `BLOCK_CONTROL` flag); what was missing was the flight
        // itself, so the debuff simply never moved anyone.
        "Fear" => vec![skill::effects::SkillEffect::Fear { ticks: FEAR_TICKS }],
        // Java defaults `chance` to 100 when the tag is absent —
        // which is every Confuse skill on this dist (only the two
        // `RandomizeHate` ones declare 80).
        "Confuse" => vec![skill::effects::SkillEffect::Confuse {
            chance: value_at(params, "chance", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
        }],
        "Betray" => vec![skill::effects::SkillEffect::Betray],
        // Fake Death 60. Two halves: the `FAKE_DEATH` flag and an
        // MP upkeep with the same `power * getTicksMultiplier()`
        // shape as `ManaDamOverTime`, which it shares the tick
        // chain with. Skill 60 carries *only* this and
        // `SilentMove`, so with both unported the effect list came
        // out empty and the whole skill was dropped — it cast and
        // did nothing at all.
        "FakeDeath" => vec![skill::effects::SkillEffect::FakeDeath {
            power: param("power").unwrap_or(0.0),
            ticks: value_at(params, "ticks", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        }],
        "SilentMove" => vec![skill::effects::SkillEffect::SilentMove],
        "Bluff" => vec![skill::effects::SkillEffect::Bluff {
            chance: param("chance").unwrap_or(100.0) as i32,
        }],
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
                return Some(Vec::new());
            }
            vec![skill::effects::SkillEffect::BlockAbnormalSlot { slots }]
        }
        // "Transform <Monster>" scroll family (541-558, 617-674):
        // polymorph the caster into `transformationId`. No stat
        // modifier of its own — the transform template's own
        // stat/speed/skill overrides apply via
        // `admin::transforms::apply_transform_state` — so without
        // this arm the effect fell through to `EFFECT_REGISTRY`,
        // wasn't found, and the buff was dropped whole.
        "Transformation" => match param("transformationId") {
            Some(id) if id != 0.0 => vec![skill::effects::SkillEffect::Transform {
                transformation_id: id as i32,
            }],
            _ => Vec::new(),
        },
        _ => return None,
    })
}
