mod aggro;
mod control;
mod damage;
mod dispel;
mod effect;
mod gathering;
mod stats;
mod summoning;
mod support;
mod ticks;
mod traits;
mod triggers;
mod utility;

use super::CondScope;
use super::EffectScope;
use super::LeveledValues;
use super::ParsedCondition;
use super::ParsedEffect;
use super::SkillGaps;
use super::build_condition;
use super::record_unported_condition;
use super::value_at;
use crate::model::skill;

use std::cell::RefCell;
pub(crate) fn build_skill(
    id: i32,
    name: &str,
    level: i32,
    sub: i32,
    values: &LeveledValues,
    effects: &[ParsedEffect],
    conditions: &[ParsedCondition],
    gaps: &RefCell<SkillGaps>,
) -> skill::Skill {
    {
        // Integer reads fall back through f64 truncation — an enchant-route
        // expression can evaluate fractionally (`Curse Gloom +1` abnormalTime
        // = 10.5) and Java's `StatSet.getInt` truncates via `Number.intValue`.
        let get_i = |field: &str, default: i32| {
            value_at(values, field, level)
                .and_then(|v| {
                    v.parse::<i32>()
                        .ok()
                        .or_else(|| v.parse::<f64>().ok().map(|f| f as i32))
                })
                .unwrap_or(default)
        };
        let get_f = |field: &str, default: f64| {
            value_at(values, field, level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        // `SkillOperateType.isSelfContinuous()` — A3 and nothing else.
        let self_continuous = value_at(values, "operateType", level) == Some("A3");
        let operate_type = match value_at(values, "operateType", level) {
            // `SkillOperateType.isActive()` minus the channeling and fly
            // families. A1..A6 differ in continuity, which is read off the
            // `operateType` string into `is_continuous` rather than from this
            // enum, so they collapse here.
            //
            // A3 additionally sets `isSelfContinuous()`, which
            // `BuffInfo.isDisplayedForEffected` reads — see
            // `ActiveBuff::displayed`. It is carried on the skill separately
            // (`self_continuous`) rather than through this enum, because the
            // enum is about *castability* and A3 is an ordinary active here.
            //
            // Falling to `Other` is not a cosmetic gap: `use_magic_on` bails
            // outright on anything that is neither `Active` nor `Channeling`,
            // so an unmapped active operate type means the skill **cannot be
            // cast at all**. A3 (Blinding Blow 321, Vengeance 368, Evade Shot
            // 369, Critical Blow 409, Aura Flare 1231) and CA5 (Battle Stance
            // 426, Spell Stance 427) were seven learnable skills in exactly
            // that state.
            Some("A1" | "A2" | "A3" | "A4" | "A5" | "A6") => skill::target::OperateType::Active,
            Some("P") => skill::target::OperateType::Passive,
            Some("T") => skill::target::OperateType::Toggle,
            // `SkillOperateType.isChanneling()`: CA1, CA2, CA5.
            Some("CA1" | "CA2" | "CA5") => skill::target::OperateType::Channeling,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().operate_types, raw, id);
                }
                skill::target::OperateType::Other
            }
        };
        // Java `SkillOperateType.isContinuous()` — the A2..A6/DA2..DA5 family.
        // `OperateType` above collapses A1 and A2 into `Active` (the cast
        // pipeline treats them alike), so continuity is read from the raw
        // string instead of derived from it. The NPC AI needs it to tell a
        // buff/debuff apart from an instant nuke when bucketing skills.
        let is_continuous = matches!(
            value_at(values, "operateType", level),
            Some("A2" | "A3" | "A4" | "A5" | "A6" | "DA2" | "DA4" | "DA5")
        );
        let target_type = match value_at(values, "targetType", level) {
            Some("SELF") => skill::target::TargetType::Self_,
            Some("TARGET") => skill::target::TargetType::Target,
            Some("ENEMY") => skill::target::TargetType::Enemy,
            Some("ENEMY_ONLY") => skill::target::TargetType::EnemyOnly,
            Some("ENEMY_NOT") => skill::target::TargetType::EnemyNot,
            Some("NPC_BODY") => skill::target::TargetType::NpcBody,
            Some("DOOR_TREASURE") => skill::target::TargetType::DoorTreasure,
            Some("OTHERS") => skill::target::TargetType::Others,
            Some("SUMMON") => skill::target::TargetType::Summon,
            Some("OWNER_PET") => skill::target::TargetType::OwnerPet,
            Some("PC_BODY") => skill::target::TargetType::PcBody,
            Some("GROUND") => skill::target::TargetType::Ground,
            Some("NONE") => skill::target::TargetType::None_,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().target_types, raw, id);
                }
                skill::target::TargetType::Other
            }
        };
        // `<abnormalVisualEffect>` is a `;`-separated list of enum names.
        let abnormal_visuals: Vec<i16> = value_at(values, "abnormalVisualEffect", level)
            .unwrap_or("")
            .split(';')
            .filter_map(|n| crate::model::skill::abnormal::abnormal_visual_client_id(n.trim()))
            .collect();
        // `overHit` is an **effect** parameter, not a skill field — the damage
        // handlers (Backstab, EnergyAttack, PhysicalAttack, …) each read
        // `params.getBoolean("overHit", false)`. A skill carries at most one
        // damage effect in practice, so hoisting "any effect declares it" to the
        // skill is behaviourally identical and avoids threading the flag
        // through every `SkillEffect` variant.
        let over_hit = effects
            .iter()
            .filter(|e| e.applies_at(level, sub))
            .any(|e| {
                value_at(&e.params, "overHit", level)
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
            });
        let toggle_group_id = get_i("toggleGroupId", 0);
        // `<trait>` — the debuff's own trait, matched against the target's
        // `DefenceTrait` resistances when it lands.
        let trait_type = value_at(values, "trait", level)
            .map(crate::model::skill::traits::TraitType::from_xml)
            .unwrap_or_default();
        // `affectScope` defaults to SINGLE when absent (Java's Skill ctor).
        let affect_scope = match value_at(values, "affectScope", level) {
            Some("RANGE") => skill::target::AffectScope::Range,
            Some("POINT_BLANK") => skill::target::AffectScope::PointBlank,
            Some("PARTY") => skill::target::AffectScope::Party,
            Some("PLEDGE") => skill::target::AffectScope::Pledge,
            Some("DEAD_PLEDGE") => skill::target::AffectScope::DeadPledge,
            Some("DEAD_PARTY") => skill::target::AffectScope::DeadParty,
            Some("DEAD_UNION") => skill::target::AffectScope::DeadUnion,
            Some("FAN") => skill::target::AffectScope::Fan,
            Some("FAN_PB") => skill::target::AffectScope::FanPointBlank,
            Some("SQUARE") => skill::target::AffectScope::Square,
            Some("SQUARE_PB") => skill::target::AffectScope::SquarePointBlank,
            Some("RING_RANGE") => skill::target::AffectScope::RingRange,
            Some("SINGLE") | Some("NONE") | None => skill::target::AffectScope::Single,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().affect_scopes, raw, id);
                }
                skill::target::AffectScope::Other
            }
        };
        // `affectObject` defaults to ALL. `*_PC` narrows Java's check to
        // players only; with no non-player creature able to be a "friend" in
        // the ported world they collapse onto the same filter.
        let affect_object = match value_at(values, "affectObject", level) {
            Some("NOT_FRIEND") | Some("NOT_FRIEND_PC") => skill::target::AffectObject::NotFriend,
            Some("FRIEND") | Some("FRIEND_PC") => skill::target::AffectObject::Friend,
            Some("CLAN") => skill::target::AffectObject::Clan,
            Some("UNDEAD_REAL_ENEMY") => skill::target::AffectObject::UndeadRealEnemy,
            Some("ALL") | None => skill::target::AffectObject::All,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().affect_objects, raw, id);
                }
                skill::target::AffectObject::Other
            }
        };
        let affect_range = get_i("affectRange", 0);
        // `<affectLimit>min-max</affectLimit>`; a bare value sets min only.
        let affect_limit = value_at(values, "affectLimit", level)
            .map(|v| {
                let mut parts = v.split('-').map(|p| p.trim().parse::<i32>().unwrap_or(0));
                (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
            })
            .unwrap_or((0, 0));
        // `<fanRange>unk;startDegree;fanAffectRange;fanAffectAngle</fanRange>`
        // (Java splits on ';' into `_fanRange[4]`); level-valued in the XML.
        let fan_range = value_at(values, "fanRange", level)
            .map(|v| {
                let mut out = [0i32; 4];
                for (slot, part) in out.iter_mut().zip(v.split(';')) {
                    *slot = part.trim().parse().unwrap_or(0);
                }
                out
            })
            .unwrap_or([0; 4]);

        // Java keeps one condition list per `SkillConditionScope`; unported
        // names drop out here (see `build_condition`'s fail-open note).
        let cond_scope = |want: CondScope| -> Vec<skill::condition::SkillCondition> {
            conditions
                .iter()
                .filter(|c| c.scope == want)
                .filter_map(|c| {
                    let built = build_condition(c, level, sub);
                    if built.is_none() {
                        record_unported_condition(&mut gaps.borrow_mut(), c, id);
                    }
                    built
                })
                .collect()
        };
        let build_scope = |want: EffectScope| {
            effects
                .iter()
                // Java `forEachNamedParamInfoParam`: an effect whose declared level
                // range excludes this level is simply not part of the skill here.
                .filter(|e| e.applies_at(level, sub) && e.scope == want)
                .flat_map(|e| effect::of(e, values, level, id, gaps))
                .collect::<Vec<_>>()
        };
        // Java keeps one effect list per `EffectScope`; the port carries the
        // ones it can act on. `START`/`END` parse as `Other` and are dropped —
        // they hang off lifecycle hooks this port doesn't have.
        let skill_effects = build_scope(EffectScope::General);
        let self_effects = build_scope(EffectScope::SelfScope);
        let pve_effects = build_scope(EffectScope::Pve);
        let pvp_effects = build_scope(EffectScope::Pvp);
        let channeling_effects = build_scope(EffectScope::Channeling);
        let end_effects = build_scope(EffectScope::End);

        // Effect names present in the XML but not in `EFFECT_REGISTRY` are
        // silently dropped (see module docs) — expected for the vast majority
        // of skills, which are outside G6's scope.
        skill::Skill {
            id,
            level,
            sub_level: sub,
            name: name.to_string(),
            // Java `set.getString("icon", "icon.skill0000")`.
            icon: value_at(values, "icon", level)
                .unwrap_or("icon.skill0000")
                .to_string(),
            operate_type,
            is_continuous,
            target_type,
            over_hit,
            abnormal_visuals,
            toggle_group_id,
            affect_scope,
            trait_type,
            affect_object,
            affect_range,
            affect_limit,
            fan_range,
            magic_type: get_i("isMagic", 0),
            static_reuse: value_at(values, "staticReuse", level) == Some("true"),
            magic_level: get_i("magicLevel", 0),
            activate_rate: get_i("activateRate", -1),
            lvl_bonus_rate: get_i("lvlBonusRate", 0),
            effect_point: get_i("effectPoint", 0),
            cast_range: get_i("castRange", 0),
            effect_range: get_i("effectRange", 0),
            hit_time: get_i("hitTime", 0),
            next_action: match value_at(values, "nextAction", level) {
                Some("ATTACK") => crate::model::skill::NextAction::Attack,
                Some("CAST") => crate::model::skill::NextAction::Cast,
                _ => crate::model::skill::NextAction::None,
            },
            // `;`-separated abnormal type names, kept as strings because that
            // is how `abnormal_type` itself is stored — the comparison is a
            // name match, not an enum one.
            abnormal_resists: value_at(values, "abnormalResists", level)
                .map(|v| {
                    v.split(';')
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            hit_cancel_time: get_f("hitCancelTime", 0.0),
            cool_time: get_i("coolTime", 0),
            reuse_delay: get_i("reuseDelay", 0),
            reuse_delay_group: get_i("reuseDelayGroup", -1),
            mp_consume: get_i("mpConsume", 0),
            mp_initial_consume: get_i("mpInitialConsume", 0),
            hp_consume: get_i("hpConsume", 0),
            without_action: value_at(values, "withoutAction", level) == Some("true"),
            is_suicide_attack: value_at(values, "isSuicideAttack", level) == Some("true"),
            item_consume_id: get_i("itemConsumeId", 0),
            item_consume_count: get_i("itemConsumeCount", 0),
            abnormal_time: get_i("abnormalTime", 0),
            abnormal_level: get_i("abnormalLevel", 0),
            abnormal_type: value_at(values, "abnormalType", level)
                .unwrap_or("NONE")
                .to_string(),
            // Java `set.getBoolean("canBeDispelled", true)` / `("isDebuff", false)`.
            can_be_dispelled: value_at(values, "canBeDispelled", level).is_none_or(|v| v == "true"),
            is_debuff: value_at(values, "isDebuff", level) == Some("true"),
            // Java `set.getBoolean("excludedFromCheck", false)`.
            excluded_from_check: value_at(values, "excludedFromCheck", level)
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
            // Java `set.getBoolean("isSharedWithSummon", true)` — note the
            // `true` default: absent tag means shared.
            shared_with_summon: value_at(values, "isSharedWithSummon", level)
                .is_none_or(|v| v.eq_ignore_ascii_case("true")),
            // Java `set.getBoolean("stayAfterDeath", false)`. The dist writes
            // both `true` and `True` for this tag and `Boolean.parseBoolean`
            // is case-insensitive, so compare loosely.
            // Java `isStayAfterDeath()` is `_stayAfterDeath || _irreplacableBuff
            // || _isNecessaryToggle` — one getter over three tags, so all three
            // are folded here (G34 S3). `irreplacableBuff` alone is on 30
            // learnable skills (the clan/pledge buffs and the noblesse line);
            // reading only `<stayAfterDeath>` stripped every one of them on
            // death.
            stay_after_death: ["stayAfterDeath", "irreplacableBuff", "isNecessaryToggle"]
                .iter()
                .any(|tag| {
                    value_at(values, tag, level).is_some_and(|v| v.eq_ignore_ascii_case("true"))
                }),
            // Java `set.getBoolean("removedOnDamage", false)` — same loose
            // compare as above, the dist writes `true` and `True` both.
            removed_on_damage: value_at(values, "removedOnDamage", level)
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
            effects: skill_effects,
            self_continuous,
            self_effects,
            pve_effects,
            pvp_effects,
            channeling_effects,
            end_effects,
            basic_property: value_at(values, "basicProperty", level)
                .map(skill::BasicProperty::from_xml)
                .unwrap_or_default(),
            conditions: cond_scope(CondScope::General),
            target_conditions: cond_scope(CondScope::Target),
            passive_conditions: cond_scope(CondScope::Passive),
            // Java `set.getInt("mpPerChanneling", _mpConsume)` — the
            // default is the skill's own mpConsume, not 0.
            mp_per_channeling: get_i("mpPerChanneling", get_i("mpConsume", 0)),
            channeling_skill_id: get_i("channelingSkillId", 0),
            // XML values are seconds; Java stores ms (`getFloat × 1000`).
            channeling_tick_ms: (get_f("channelingTickInterval", 0.0) * 1000.0) as i32,
            channeling_start_ms: (get_f("channelingStart", 0.0) * 1000.0) as i32,
            // `<attributeType>FIRE</attributeType>` + `<attributeValue>20` — the skill's element for `calcAttributeBonus`. `NONE` and
            // unknown names read as no element, like Java's enum default.
            attribute_type: value_at(values, "attributeType", level)
                .and_then(crate::model::stats::Element::from_xml),
            attribute_value: get_i("attributeValue", 0),
        }
    }
}
