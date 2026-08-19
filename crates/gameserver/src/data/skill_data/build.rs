use super::CondScope;
use super::EFFECT_REGISTRY;
use super::EffectScope;
use super::FEAR_TICKS;
use super::LeveledValues;
use super::ParsedCondition;
use super::ParsedEffect;
use super::SkillGaps;
use super::build_condition;
use super::effect_magic_type;
use super::record_unported_condition;
use super::value_at;
use crate::model::skill;

use crate::model::stats::Stat;
use crate::model::stats::StatModifierType;
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
            Some("A1" | "A2" | "A3" | "A4" | "A5" | "A6") => skill::OperateType::Active,
            Some("P") => skill::OperateType::Passive,
            Some("T") => skill::OperateType::Toggle,
            // `SkillOperateType.isChanneling()`: CA1, CA2, CA5.
            Some("CA1" | "CA2" | "CA5") => skill::OperateType::Channeling,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().operate_types, raw, id);
                }
                skill::OperateType::Other
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
            Some("SELF") => skill::TargetType::Self_,
            Some("TARGET") => skill::TargetType::Target,
            Some("ENEMY") => skill::TargetType::Enemy,
            Some("ENEMY_ONLY") => skill::TargetType::EnemyOnly,
            Some("ENEMY_NOT") => skill::TargetType::EnemyNot,
            Some("NPC_BODY") => skill::TargetType::NpcBody,
            Some("DOOR_TREASURE") => skill::TargetType::DoorTreasure,
            Some("OTHERS") => skill::TargetType::Others,
            Some("SUMMON") => skill::TargetType::Summon,
            Some("OWNER_PET") => skill::TargetType::OwnerPet,
            Some("PC_BODY") => skill::TargetType::PcBody,
            Some("GROUND") => skill::TargetType::Ground,
            Some("NONE") => skill::TargetType::None_,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().target_types, raw, id);
                }
                skill::TargetType::Other
            }
        };
        // `<abnormalVisualEffect>` is a `;`-separated list of enum names.
        let abnormal_visuals: Vec<i16> = value_at(values, "abnormalVisualEffect", level)
            .unwrap_or("")
            .split(';')
            .filter_map(|n| crate::model::skill::abnormal_visual_client_id(n.trim()))
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
            .map(crate::model::skill::TraitType::from_xml)
            .unwrap_or_default();
        // `affectScope` defaults to SINGLE when absent (Java's Skill ctor).
        let affect_scope = match value_at(values, "affectScope", level) {
            Some("RANGE") => skill::AffectScope::Range,
            Some("POINT_BLANK") => skill::AffectScope::PointBlank,
            Some("PARTY") => skill::AffectScope::Party,
            Some("PLEDGE") => skill::AffectScope::Pledge,
            Some("DEAD_PLEDGE") => skill::AffectScope::DeadPledge,
            Some("DEAD_PARTY") => skill::AffectScope::DeadParty,
            Some("DEAD_UNION") => skill::AffectScope::DeadUnion,
            Some("FAN") => skill::AffectScope::Fan,
            Some("FAN_PB") => skill::AffectScope::FanPointBlank,
            Some("SQUARE") => skill::AffectScope::Square,
            Some("SQUARE_PB") => skill::AffectScope::SquarePointBlank,
            Some("RING_RANGE") => skill::AffectScope::RingRange,
            Some("SINGLE") | Some("NONE") | None => skill::AffectScope::Single,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().affect_scopes, raw, id);
                }
                skill::AffectScope::Other
            }
        };
        // `affectObject` defaults to ALL. `*_PC` narrows Java's check to
        // players only; with no non-player creature able to be a "friend" in
        // the ported world they collapse onto the same filter.
        let affect_object = match value_at(values, "affectObject", level) {
            Some("NOT_FRIEND") | Some("NOT_FRIEND_PC") => skill::AffectObject::NotFriend,
            Some("FRIEND") | Some("FRIEND_PC") => skill::AffectObject::Friend,
            Some("CLAN") => skill::AffectObject::Clan,
            Some("UNDEAD_REAL_ENEMY") => skill::AffectObject::UndeadRealEnemy,
            Some("ALL") | None => skill::AffectObject::All,
            other => {
                if let Some(raw) = other {
                    SkillGaps::record(&mut gaps.borrow_mut().affect_objects, raw, id);
                }
                skill::AffectObject::Other
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
        let cond_scope = |want: CondScope| -> Vec<skill::SkillCondition> {
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
                .flat_map(|e| {
                    let (xml_name, params, mode, groups, armor_condition, weapon_condition) = (
                        &e.name,
                        &e.params,
                        &e.mode,
                        &e.groups,
                        &e.armor_condition,
                        &e.weapon_condition,
                    );
                    let param = |key: &str| -> Option<f64> {
                        value_at(params, key, level).and_then(|v| v.parse().ok())
                    };
                    let modifier_mode = if mode == "PER" {
                        StatModifierType::Per
                    } else {
                        StatModifierType::Diff
                    };
                    let stat_mod = |stat: Stat, amount: f64| {
                        skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                            stat,
                            mode: modifier_mode,
                            amount,
                            armor_condition: *armor_condition,
                            weapon_condition: *weapon_condition,
                            qualifier: None,
                            two_handed: false,
                        })
                    };
                    match xml_name.as_str() {
                        // Vital Force (148), Esprit (171), Acrobatic Move (225),
                        // Clear Mind (1297): a flat stat bonus that only counts
                        // while the creature is in the named locomotion state.
                        // Java names its own `<stat>`/`<type>`/`<value>` rather
                        // than using the generic `amount`/`mode` pair, and merges
                        // into `_moveTypeStats` — always additive, never percent —
                        // so `modifier_mode` is deliberately not consulted.
                        //
                        // Before this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and was dropped: Vital
                        // Force and Clear Mind carry *only* `StatByMoveType`, so
                        // both were passives that did precisely nothing.
                        "StatByMoveType" => {
                            let stat = value_at(params, "stat", level).and_then(Stat::from_xml);
                            let move_type = value_at(params, "type", level)
                                .and_then(crate::model::stats::MoveType::from_xml);
                            match (stat, move_type, param("value")) {
                                (Some(stat), Some(move_type), Some(amount)) => {
                                    vec![skill::SkillEffect::StatModifier(
                                        skill::StatModifierEffect {
                                            stat,
                                            mode: StatModifierType::Diff,
                                            amount,
                                            armor_condition: *armor_condition,
                                            weapon_condition: *weapon_condition,
                                            qualifier: Some(
                                                crate::model::stats::StatQualifier::MoveType(
                                                    move_type,
                                                ),
                                            ),
                                            two_handed: false,
                                        },
                                    )]
                                }
                                _ => Vec::new(),
                            }
                        }
                        // Guts (139) / Touch of Life (341) / Touch of Death (342):
                        // a multiplier on how likely an incoming *debuff* is to
                        // land. Java `mergeMul(RESIST_ABNORMAL_DEBUFF,
                        // 1 + amount/100)` — which is exactly what `Per` mode does
                        // here — so the mode is forced rather than read from the
                        // XML (these effects carry no `<mode>`, which would default
                        // to DIFF and silently mean something else entirely).
                        //
                        // Java's handler switches on `<slot>` and only implements
                        // DEBUFF ("only this one is in use it seems"); a different
                        // slot pumps nothing, so it is skipped here too.
                        "ResistAbnormalByCategory" => {
                            let slot = value_at(params, "slot", level).unwrap_or("DEBUFF");
                            param("amount")
                                .filter(|_| slot == "DEBUFF")
                                .map(|amount| {
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::ResistAbnormalDebuff,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        // Ultimate Defense (110) / Ultimate Evasion (111): the same
                        // shape for resisting *dispel*. Java only implements the
                        // BUFF slot.
                        "ResistDispelByCategory" => {
                            let slot = value_at(params, "slot", level).unwrap_or("BUFF");
                            param("amount")
                                .filter(|_| slot == "BUFF")
                                .map(|amount| {
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::ResistDispelBuff,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
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
                                return Vec::new();
                            }
                            vec![skill::SkillEffect::BlockAbnormalSlot { slots }]
                        }
                        // Stun / sleep / paralyze (540 uses) and Root (79): no stat
                        // modifier at all — the whole mechanic is the abnormal-state
                        // flag they contribute (`Skill::effect_flags`).
                        // The four bot-report punishment effects (skills
                        // 6038/6039/6040/6055/6056). Each is a pure state
                        // effect: the work happens on the buff's start and
                        // exit, not at parse time.
                        "BlockChat" => vec![skill::SkillEffect::BlockChat],
                        "BlockParty" => vec![skill::SkillEffect::BlockParty],
                        "Flag" => vec![skill::SkillEffect::PvpFlag],
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
                            vec![skill::SkillEffect::BlockAction { blocked_actions }]
                        }
                        "BlockActions" => {
                            // Java: a non-empty `allowedSkills` whitelist makes this
                            // CONDITIONAL_BLOCK_ACTIONS instead. Both gate the same
                            // way in `hasBlockActions()`.
                            let conditional = value_at(params, "allowedSkills", level)
                                .is_some_and(|v| !v.trim().is_empty());
                            vec![skill::SkillEffect::BlockActions { conditional }]
                        }
                        "Root" => vec![skill::SkillEffect::Root],
                        // The elemental attribute pair (PLAN_G19_ATTRIBUTES.md):
                        // one flat StatModifier per element named in the
                        // (comma-separable) `attribute` param, default FIRE —
                        // Java's `Stat.valueOf(attribute + "_POWER"/"_RES")`.
                        "AttackAttribute" | "DefenceAttribute" => {
                            let Some(amount) = param("amount") else {
                                return Vec::new();
                            };
                            let defence = xml_name.as_str() == "DefenceAttribute";
                            value_at(params, "attribute", level)
                                .unwrap_or("FIRE")
                                .split(',')
                                .filter_map(|n| crate::model::stats::Element::from_xml(n.trim()))
                                .map(|el| stat_mod(el.attribute_stat(defence), amount))
                                .collect()
                        }
                        // Polearm Mastery 216: `HitNumber` is a plain
                        // AbstractStatEffect over ATTACK_COUNT_MAX (amount 5).
                        "HitNumber" => param("amount")
                            .map(|amount| stat_mod(Stat::AttackCountMax, amount))
                            .into_iter()
                            .collect(),
                        // The rest of the state-flag CC family (Seal of Silence,
                        // Shield Slam, Mystic Immunity, Horror): no parameters, the
                        // mechanic is entirely the flag.
                        "Mute" => vec![skill::SkillEffect::Mute],
                        "PhysicalMute" => vec![skill::SkillEffect::PhysicalMute],
                        "DebuffBlock" => vec![skill::SkillEffect::DebuffBlock],
                        // G34 S3 — flag-only effects. Each maps to one
                        // `effect_flag` bit; see `Skill::effect_flags`.
                        // `SkillEvasion` is *not* a plain stat: Java keys it by
                        // `magicType` in a separate map, so a skill-dodge buff
                        // dodges only its own bucket (0 = physical skills).
                        "SkillEvasion" => vec![skill::SkillEffect::SkillEvasion {
                            magic_type: value_at(params, "magicType", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            amount: param("amount").unwrap_or(0.0),
                        }],
                        "SkillTurning" => vec![skill::SkillEffect::SkillTurning {
                            chance: param("chance").unwrap_or(100.0) as i32,
                            static_chance: value_at(params, "staticChance", level)
                                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
                        }],
                        // `EnlargeAbnormalSlot` reads `<slots>`, not the
                        // `<amount>` the generic registry expects, so it needs
                        // its own arm to become a stat modifier.
                        "EnlargeAbnormalSlot" => value_at(params, "slots", level)
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|slots| {
                                vec![skill::SkillEffect::StatModifier(
                                    skill::StatModifierEffect {
                                        stat: Stat::MaxBuffSlots,
                                        mode: StatModifierType::Diff,
                                        amount: slots,
                                        armor_condition: 0,
                                        weapon_condition: 0,
                                        qualifier: None,
                                        two_handed: false,
                                    },
                                )]
                            })
                            .unwrap_or_default(),
                        // `DispelBySlotMyself` — `<dispel>` is a `;`-separated
                        // list of abnormal *types* with no levels, unlike
                        // `DispelBySlot`'s `TYPE=level` pairs.
                        "DispelBySlotMyself" => value_at(params, "dispel", level)
                            .map(|d| {
                                vec![skill::SkillEffect::DispelBySlotMyself {
                                    dispel: d
                                        .split(';')
                                        .map(|t| t.trim().to_string())
                                        .filter(|t| !t.is_empty())
                                        .collect(),
                                }]
                            })
                            .unwrap_or_default(),
                        // `SkillMastery` stores the **BaseStat ordinal**, not a
                        // magnitude — `calcSkillMastery` reads it back through
                        // `BaseStat.values()[val]` to pick which stat's bonus
                        // drives the proc chance.
                        "SkillMastery" => vec![skill::SkillEffect::StatModifier(
                            skill::StatModifierEffect {
                                stat: Stat::SkillMastery,
                                mode: StatModifierType::Diff,
                                // The **Rust** discriminant, parsed by name — see
                                // `BaseStat::from_name` for why the Java ordinal
                                // must not be copied across.
                                amount: value_at(params, "stat", level)
                                    .and_then(crate::model::stats::BaseStat::from_name)
                                    .unwrap_or(crate::model::stats::BaseStat::Str)
                                    .ordinal() as f64,
                                armor_condition: 0,
                                weapon_condition: 0,
                                qualifier: None,
                                two_handed: false,
                            },
                        )],
                        // `Lucky` (194) is an **empty effect** in Java — its
                        // handler has only a `canStart` guard. The mechanic
                        // lives in `Player.isLucky()`, which asks whether the
                        // *buff* is present, so all this has to do is land.
                        "Lucky" => vec![skill::SkillEffect::Lucky],
                        // Java's `chance` default is 0 — a door skill with no
                        // `<chance>` never opens anything. Unlock declares one
                        // at every level, so the default is only a guard.
                        "OpenDoor" => vec![skill::SkillEffect::OpenDoor {
                            chance: param("chance").unwrap_or(0.0) as i32,
                            is_item: value_at(params, "isItem", level) == Some("true"),
                        }],
                        "OpenChest" => vec![skill::SkillEffect::OpenChest],
                        "Bluff" => vec![skill::SkillEffect::Bluff {
                            chance: param("chance").unwrap_or(100.0) as i32,
                        }],
                        // Java's default here is **-1**, not 100: a negative
                        // chance means "always", which is what Erase relies on.
                        "Unsummon" => vec![skill::SkillEffect::Unsummon {
                            chance: param("chance").unwrap_or(-1.0) as i32,
                        }],
                        "DeathLink" => param("power")
                            .map(|power| vec![skill::SkillEffect::DeathLink { power }])
                            .unwrap_or_default(),
                        // `CpHealPercent` — a share of **max CP**, clamped by
                        // `getMaxRecoverableCp()`. `power == 100` is the full
                        // pool (Java special-cases it to the same number).
                        "CpHealPercent" => param("power")
                            .map(|power| vec![skill::SkillEffect::CpHealPercent { power }])
                            .unwrap_or_default(),
                        // `HpByLevel` heals the **effector**, not the effected
                        // — Life Scavenge (46) and Corpse Life Drain (1151) top
                        // the *caster* up off a corpse.
                        "HpByLevel" => param("power")
                            .map(|power| vec![skill::SkillEffect::HpByLevel { power }])
                            .unwrap_or_default(),
                        // `MpVampiricAttack` pumps **two** values from one
                        // `<amount>`: the percentage (÷100) and a `sum`
                        // (`amount × chance`, default chance 30 — "Classic:
                        // 30% chance" in Java's own comment) that the chance
                        // finalizer divides back out.
                        "MpVampiricAttack" => param("amount")
                            .map(|amount| {
                                let chance = param("chance").unwrap_or(30.0);
                                vec![
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::AbsorbManaDamagePercent,
                                        mode: StatModifierType::Diff,
                                        amount: amount / 100.0,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    }),
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::MpVampiricSum,
                                        mode: StatModifierType::Diff,
                                        amount: amount * chance,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: None,
                                        two_handed: false,
                                    }),
                                ]
                            })
                            .unwrap_or_default(),
                        "TargetMe" => vec![skill::SkillEffect::TargetMe],
                        "TargetMeProbability" => vec![skill::SkillEffect::TargetMeProbability {
                            chance: param("chance").unwrap_or(100.0) as i32,
                        }],
                        "BuffBlock" => vec![skill::SkillEffect::BuffBlock],
                        "PhysicalShieldAngleAll" => {
                            vec![skill::SkillEffect::PhysicalShieldAngleAll]
                        }
                        "Passive" => vec![skill::SkillEffect::Passive],
                        "Untargetable" => vec![skill::SkillEffect::Untargetable],
                        "DisableTargeting" => vec![skill::SkillEffect::DisableTargeting],
                        "PhysicalAttackMute" => vec![skill::SkillEffect::PhysicalAttackMute],
                        "BlockResurrection" => vec![skill::SkillEffect::BlockResurrection],
                        "BlockEscape" => vec![skill::SkillEffect::BlockEscape],
                        "AbnormalShield" => vec![skill::SkillEffect::AbnormalShield],
                        "BlockControl" => vec![skill::SkillEffect::BlockControl],
                        "TargetCancel" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![skill::SkillEffect::TargetCancel { chance }]
                        }
                        // Aggression 28/18, Judgment 401, Tribunal 400: no params.
                        "GetAgro" => vec![skill::SkillEffect::GetAgro],
                        // Charm 15, Lure 51: `power` (default 0, Java always
                        // instantiates the handler even with no param).
                        "AddHate" => {
                            vec![skill::SkillEffect::AddHate {
                                power: param("power").unwrap_or(0.0),
                            }]
                        }
                        "DeleteHate" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![skill::SkillEffect::DeleteHate { chance }]
                        }
                        "DeleteHateOfMe" => {
                            let chance = value_at(params, "chance", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![skill::SkillEffect::DeleteHateOfMe { chance }]
                        }
                        // (`TargetMe` and `RandomizeHate`, once deferred here,
                        // both landed in G34 S4 — see their own arms in this
                        // match.)
                        // Java instantiates these handlers whenever the `<effect>` is
                        // present and reads `params.getDouble("power", 0)` — the
                        // effect is always created, `power` defaulting to 0 when the
                        // param is absent (e.g. skills 1011/4717/4718, whose
                        // `<item>power</item>` parses to the param key `item`, not
                        // `power`). Mirror that default here; do NOT drop the effect,
                        // or the skill becomes a silent no-op.
                        "MagicalAttack" => vec![skill::SkillEffect::MagicalAttack {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // The EffectPoint totem spawner (Symbol of Noise 455, Day
                        // of Doom 1422, Anti-summoning Field 1424; PLAN_G19_SYMBOLS.md).
                        "SummonNpc" => vec![skill::SkillEffect::SummonNpc {
                            npc_id: param("npcId").unwrap_or(0.0) as i32,
                            npc_count: param("npcCount").unwrap_or(1.0) as i32,
                            despawn_delay: param("despawnDelay").unwrap_or(0.0) as i32,
                        }],
                        // Ranged magical nuke (e.g. Prominence 1230). Java's
                        // `MagicalAttackRange` computes the same
                        // `calcMagicDam(mAtk, power, mDef, sps, bss, mcrit)` core as
                        // `MagicalAttack`, plus the `shieldDefPercent` shield-block
                        // term its own variant carries.
                        "MagicalAttackRange" => vec![skill::SkillEffect::MagicalAttackRange {
                            power: param("power").unwrap_or(0.0),
                            shield_def_percent: param("shieldDefPercent").unwrap_or(0.0),
                        }],
                        // Soul-charge magic nuke. Java's `MagicalSoulAttack` runs
                        // the identical `calcMagicDam` core as `MagicalAttack`;
                        // its only difference is scaling mAtk by
                        // `1.3 + souls*0.05` for charged souls. SKIP(census):
                        // like `PhysicalSoulAttack`, no reachable caster — all
                        // 15 carriers (Fallen Arrow 1431, Abyssal Blaze 1433, …)
                        // are Kamael skills with no skill-tree row, no item
                        // grant and no NPC carrier (verified 2026-08-06), and an
                        // NPC caster would NPE in Java's own handler. Same
                        // silent-drop trap as `MagicalAttackRange` if left
                        // unhandled, hence the arm.
                        "MagicalSoulAttack" => vec![skill::SkillEffect::MagicalAttack {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Vampiric Touch/Claw: magic damage + self-heal of
                        // `percentage`% of the drained HP.
                        "HpDrain" => vec![skill::SkillEffect::HpDrain {
                            power: param("power").unwrap_or(0.0),
                            percentage: param("percentage").unwrap_or(0.0),
                        }],
                        // Poison/bleed damage-over-time (e.g. Curse Poison 1168).
                        // Java always creates the effect and reads `power`/`ticks`
                        // (`ticks` is a scalar child → level-0 fallback); `canKill`
                        // defaults false. Without this arm the effect fell through
                        // to `EFFECT_REGISTRY`, wasn't found, and got dropped — the
                        // debuff landed but never dealt damage.
                        // Periodic HP / MP effects riding the same tick chain as DamOverTime.
                        // `HealEffect` scales the healing its bearer *receives* — a two-stat
                        // AbstractStatEffect like CriticalDamage: PER feeds the multiplier,
                        // DIFF the flat addend.
                        "HealEffect" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::HealEffect
                                } else {
                                    Stat::HealEffectAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        // Instant CP change (Braveheart, Wrath, Touch of Death).
                        "Cp" => param("amount")
                            .map(|amount| skill::SkillEffect::Cp {
                                amount,
                                percent: modifier_mode == StatModifierType::Per,
                            })
                            .into_iter()
                            .collect(),
                        "HealOverTime" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::HealOverTime { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        // `Relax` — the seated MP-upkeep toggle (skill 226).
                        "Relax" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::Relax { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "ChameleonRest" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::ChameleonRest { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "ManaHealOverTime" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::ManaHealOverTime { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "RebalanceHP" => vec![skill::SkillEffect::RebalanceHp],
                        "ManaDamOverTime" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::ManaDamOverTime { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        "DamOverTime" => vec![skill::SkillEffect::DamOverTime {
                            power: param("power").unwrap_or(0.0),
                            ticks: param("ticks").unwrap_or(0.0) as i32,
                            can_kill: value_at(params, "canKill", level) == Some("true"),
                        }],
                        // Dagger blows (calcBlowDamage). FatalBlow/Backstab roll
                        // `criticalChance` (default 0) to double; SoulBlow doesn't
                        // (its charged-soul boost is unmodeled → ×1). Backstab also
                        // requires flanking. Their `Lethal` sibling effect is a
                        // separate `<effect>` block, parsed in its own arm below.
                        "FatalBlow" => vec![skill::SkillEffect::Blow {
                            power: param("power").unwrap_or(0.0),
                            chance_boost: param("chanceBoost").unwrap_or(0.0),
                            critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
                            backstab: false,
                        }],
                        "Backstab" => vec![skill::SkillEffect::Blow {
                            power: param("power").unwrap_or(0.0),
                            chance_boost: param("chanceBoost").unwrap_or(0.0),
                            critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
                            backstab: true,
                        }],
                        "SoulBlow" => vec![skill::SkillEffect::Blow {
                            power: param("power").unwrap_or(0.0),
                            chance_boost: param("chanceBoost").unwrap_or(0.0),
                            critical_chance: None,
                            backstab: false,
                        }],
                        // Backstab (30), Lethal Blow (344), Deadly Blow (263),
                        // Critical Blow (409), Lethal Shot (343), Turn/Banish
                        // Undead/Seraph (1400/405/450): without this arm the
                        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
                        // and the bonus instant-kill/half-kill chance never
                        // rolled — only these skills' other (already-ported)
                        // effect landed.
                        "Lethal" => vec![skill::SkillEffect::Lethal {
                            full_lethal: param("fullLethal").unwrap_or(0.0),
                            half_lethal: param("halfLethal").unwrap_or(0.0),
                        }],
                        // Physical skill damage. `PhysicalSoulAttack` runs the
                        // identical `77·((pAtk·pAtkMod)·levelMod + power)/(pDef·pDefMod)`
                        // core, so it routes here too (like
                        // MagicalSoulAttack→MagicalAttack). SKIP(census): its
                        // extra — the `1 + souls·0.04` charged-soul boost — has
                        // no reachable caster on this dist: all 30 carriers are
                        // Kamael skills with no skill-tree row and no item
                        // grant, and the one NPC carrier (Twin Shot 507) would
                        // NPE in Java's own handler (`getActingPlayer()` is
                        // null for an NPC). Verified 2026-08-06. (The
                        // `FatalBlow`/`Backstab` blow skills parse to their own
                        // `SkillEffect::Blow` above — `calcBlowDamage` is
                        // ported.)
                        // Java's `criticalChance` default here is **0**, not
                        // `PhysicalAttack`'s 10, and it has no
                        // `ignoreShieldDefence` param at all.
                        "PhysicalAttackHpLink" => {
                            vec![skill::SkillEffect::PhysicalAttackHpLink {
                                power: param("power").unwrap_or(0.0),
                                p_atk_mod: 1.0,
                                p_def_mod: 1.0,
                                critical_chance: param("criticalChance").unwrap_or(0.0),
                                ignore_shield_defence: false,
                            }]
                        }
                        // `CallSkill.java` — cast another skill outright.
                        // Java's `skillLevel` default is 1; a declared 0 means
                        // "the effector's own learned level", and
                        // `skillLevelScaleTo` scales off an existing buff —
                        // neither is used by any reachable carrier here.
                        "CallSkill" => match param("skillId") {
                            Some(sid) if sid > 0.0 => vec![skill::SkillEffect::CallSkill {
                                skill_id: sid as i32,
                                skill_level: param("skillLevel").unwrap_or(1.0).max(1.0) as i32,
                                chance: param("chance").unwrap_or(100.0) as i32,
                            }],
                            _ => Vec::new(),
                        },
                        "PolearmSingleTarget" => vec![skill::SkillEffect::PolearmSingleTarget],
                        "ReduceDropPenalty" => {
                            use crate::model::skill::ReduceDropKind;
                            vec![skill::SkillEffect::ReduceDropPenalty {
                                // Java `mergeMul(stat, amount/100 + 1)`.
                                exp_mul: param("exp").unwrap_or(0.0) / 100.0 + 1.0,
                                kind: match value_at(params, "type", level) {
                                    Some("PK") => ReduceDropKind::Pk,
                                    Some("RAID") => ReduceDropKind::Raid,
                                    _ => ReduceDropKind::Mob,
                                },
                            }]
                        }
                        "ResurrectionSpecial" => vec![skill::SkillEffect::ResurrectionSpecial {
                            power: param("power").unwrap_or(0.0) as i32,
                            hp_percent: param("hpPercent").unwrap_or(0.0) as i32,
                            mp_percent: param("mpPercent").unwrap_or(0.0) as i32,
                            cp_percent: param("cpPercent").unwrap_or(0.0) as i32,
                        }],
                        // Unlike every other stat effect, this one names its
                        // target with Java's **`Stat` enum name** in a `<stat>`
                        // child rather than through the effect name, so it
                        // needs its own lookup. `ACCURACY_COMBAT` is the only
                        // one on this dist (Shadow Sense 294); an unknown name
                        // yields no effect and is recorded as a gap.
                        //
                        // The grant is night-gated and lands through
                        // `game_loop::night_stats`, not the ordinary stat
                        // pipeline — see the variant's docs.
                        "NightStatModify" => match value_at(params, "stat", level) {
                            Some("ACCURACY_COMBAT") => vec![skill::SkillEffect::NightStatModify {
                                stat: Stat::AccuracyCombat,
                                amount: param("amount").unwrap_or(0.0),
                                mode: modifier_mode,
                            }],
                            _ => Vec::new(),
                        },
                        "Betray" => vec![skill::SkillEffect::Betray],
                        "ImmobilePetBuff" => vec![skill::SkillEffect::ImmobilePetBuff],
                        "CallParty" => vec![skill::SkillEffect::CallParty],
                        "TriggerSkillByDamage" => vec![skill::SkillEffect::TriggerSkillByDamage {
                            min_damage: param("minDamage").unwrap_or(1.0) as i32,
                            chance: param("chance").unwrap_or(100.0) as i32,
                            skill_id: param("skillId").unwrap_or(0.0) as i32,
                            skill_level: param("skillLevel").unwrap_or(1.0) as i32,
                            hp_percent: param("hpPercent").unwrap_or(100.0) as i32,
                            attacker_playable_only: value_at(params, "attackerType", level)
                                == Some("Playable"),
                            // Java's default is SELF; ENEMY is what casts the
                            // trigger back at whoever hit you.
                            on_attacker: value_at(params, "targetType", level) == Some("ENEMY"),
                        }],
                        "TriggerSkillByMagicType" => {
                            vec![skill::SkillEffect::TriggerSkillByMagicType {
                                magic_types: value_at(params, "magicTypes", level)
                                    .map(|v| {
                                        v.split(';').filter_map(|t| t.trim().parse().ok()).collect()
                                    })
                                    .unwrap_or_default(),
                                chance: param("chance").unwrap_or(100.0) as i32,
                                skill_id: param("skillId").unwrap_or(0.0) as i32,
                                // Java's default here is 0, which disables the
                                // effect — unlike the damage twin's 1.
                                skill_level: param("skillLevel").unwrap_or(0.0) as i32,
                                on_party: value_at(params, "targetType", level) == Some("MY_PARTY"),
                            }]
                        }
                        "PhysicalAttack" | "PhysicalSoulAttack" => {
                            vec![skill::SkillEffect::PhysicalAttack {
                                power: param("power").unwrap_or(0.0),
                                p_atk_mod: param("pAtkMod").unwrap_or(1.0),
                                p_def_mod: param("pDefMod").unwrap_or(1.0),
                                critical_chance: param("criticalChance").unwrap_or(10.0),
                                ignore_shield_defence: value_at(
                                    params,
                                    "ignoreShieldDefence",
                                    level,
                                ) == Some("true"),
                            }]
                        }
                        "Heal" => vec![skill::SkillEffect::Heal {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Miracle (1426), Benediction (1271), Restore Life (1258),
                        // Revival (181), Touch of Life (341): without this arm the
                        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
                        // and the heal amount was silently 0.
                        "HealPercent" => vec![skill::SkillEffect::HealPercent {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Sonic Focus (8), Focus Force (50), Sonic Rage (345), …:
                        // without this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and the "build Force"
                        // toggle/skill did nothing.
                        "FocusMomentum" => vec![skill::SkillEffect::FocusMomentum {
                            amount: param("amount").unwrap_or(1.0) as i32,
                            max_charges: param("maxCharges").unwrap_or(0.0) as i32,
                        }],
                        // Double Sonic Slash (5), Sonic Blaster (6), Force Burst
                        // (17), …: `chargeConsume` is a *skill-level* tag (a
                        // sibling of `<targetType>`), not a child of the
                        // `<effect name="EnergyAttack">` element itself — Java's
                        // effect constructors read the skill's whole merged param
                        // set, so it reaches `_chargeConsume` the same way. Without
                        // this arm the effect fell through to `EFFECT_REGISTRY`,
                        // wasn't found, and every Force-spend attack did nothing.
                        "EnergyAttack" => vec![skill::SkillEffect::EnergyAttack {
                            power: param("power").unwrap_or(0.0),
                            critical_chance: param("criticalChance").unwrap_or(10.0),
                            p_def_mod: param("pDefMod").unwrap_or(1.0),
                            charge_consume: value_at(values, "chargeConsume", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            ignore_shield_defence: value_at(params, "ignoreShieldDefence", level)
                                == Some("true"),
                        }],
                        // Pet food (Wolf Food 2048, etc.). Without this arm the
                        // food item was consumed and restored nothing.
                        "Feed" => vec![skill::SkillEffect::Feed {
                            normal: param("normal").unwrap_or(0.0) as i32,
                            ride: param("ride").unwrap_or(0.0) as i32,
                            wyvern: param("wyvern").unwrap_or(0.0) as i32,
                        }],
                        "SummonCubic" => vec![skill::SkillEffect::SummonCubic {
                            cubic_id: param("cubicId").unwrap_or(-1.0) as i32,
                            cubic_level: param("cubicLvl").unwrap_or(0.0) as i32,
                        }],
                        "Restoration" => match (param("itemId"), param("itemCount")) {
                            (Some(item_id), Some(item_count)) => {
                                vec![skill::SkillEffect::GiveItem {
                                    item_id: item_id as i32,
                                    item_count: item_count as i64,
                                    item_enchant_level: param("itemEnchantmentLevel").unwrap_or(0.0)
                                        as i32,
                                }]
                            }
                            _ => Vec::new(),
                        },
                        "RestorationRandom" => vec![skill::SkillEffect::GiveItemRandom {
                            groups: groups.clone(),
                        }],
                        // Spoil (254/…): mark the mob spoiled. No params — the
                        // landing roll and target checks live in the effect handler.
                        "Spoil" => vec![skill::SkillEffect::Spoil],
                        // Sweeper (42/474): claim the dead mob's spoil loot.
                        "Sweeper" => vec![skill::SkillEffect::Sweeper],
                        // ConsumeBody (paired with Sweeper on 42): decay the corpse.
                        "ConsumeBody" => vec![skill::SkillEffect::ConsumeBody],
                        // Sow (2097): the manor sow, cast via a Seed item.
                        "Sow" => vec![skill::SkillEffect::Sow],
                        // Harvesting (2098): claim a sown corpse's crop.
                        "Harvesting" => vec![skill::SkillEffect::Harvesting],
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
                                return Vec::new();
                            }
                            let rate = value_at(params, "rate", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(100);
                            vec![skill::SkillEffect::DispelBySlotProbability { dispel, rate }]
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
                                    vec![skill::SkillEffect::DispelBySlot { dispel }]
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
                                Some("DEBUFF") => skill::DispelSlot::Debuff,
                                Some("ALL") => skill::DispelSlot::All,
                                _ => skill::DispelSlot::Buff,
                            };
                            let rate = value_at(params, "rate", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(0);
                            let max = value_at(params, "max", level)
                                .and_then(|v| v.parse::<i32>().ok())
                                .unwrap_or(0);
                            vec![skill::SkillEffect::DispelByCategory { slot, rate, max }]
                        }
                        // Both the basic (247) and advanced (326) HQ skills
                        // carry this; only 326 sets `<isAdvanced>true</…>`,
                        // which halves the flag's incoming damage.
                        "HeadquarterCreate" => vec![skill::SkillEffect::CreateHeadquarter {
                            advanced: value_at(params, "isAdvanced", level)
                                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
                        }],
                        // "Common Craft" (1322) / "Dwarven Craft" (1321): param-less
                        // self-closing effects whose whole job is to open the recipe
                        // window. Without these arms both skills parsed to zero
                        // effects and the cast did nothing.
                        "OpenCommonRecipeBook" => {
                            vec![skill::SkillEffect::OpenRecipeBook { dwarven: false }]
                        }
                        "OpenDwarfRecipeBook" => {
                            vec![skill::SkillEffect::OpenRecipeBook { dwarven: true }]
                        }
                        // Java throws if amount is 0/missing; we drop the effect
                        // (silent no-op) to match how other bad effect bodies fall
                        // through, rather than panicking at data-load.
                        "GiveRecommendation" => match param("amount") {
                            Some(amount) if amount != 0.0 => {
                                vec![skill::SkillEffect::GiveRecommendation {
                                    amount: amount as i32,
                                }]
                            }
                            _ => Vec::new(),
                        },
                        // Fixed-destination teleports — the Scrolls of Escape.
                        // Coordinates are per *level*: skill 2213 alone carries
                        // 22 towns, one per level.
                        "Teleport" => vec![skill::SkillEffect::Teleport {
                            x: param("x").unwrap_or(0.0) as i32,
                            y: param("y").unwrap_or(0.0) as i32,
                            z: param("z").unwrap_or(0.0) as i32,
                        }],
                        // `Hp.java` — a raw instant HP change, not a `Heal`:
                        // no `calcHeal`, no healing-stat scaling, no overheal
                        // message. `DIFF` is a flat amount, `PER` a share of
                        // **max** HP.
                        "Hp" => vec![skill::SkillEffect::Hp {
                            amount: param("amount").unwrap_or(0.0),
                            percent: mode == "PER",
                        }],
                        // `FORTRESS` has no arm on purpose — with no fortress
                        // system it would be a destination that cannot be
                        // resolved, and letting it drop keeps it visible in the
                        // census instead of silently teleporting to town.
                        "Escape"
                            if matches!(
                                value_at(params, "escapeType", level),
                                Some("TOWN" | "CLANHALL" | "CASTLE")
                            ) =>
                        {
                            vec![skill::SkillEffect::Escape {
                                dest: match value_at(params, "escapeType", level) {
                                    Some("CLANHALL") => skill::EscapeDest::ClanHall,
                                    Some("CASTLE") => skill::EscapeDest::Castle,
                                    _ => skill::EscapeDest::Town,
                                },
                            }]
                        }
                        "DispelAll" => vec![skill::SkillEffect::DispelAll],
                        "Grow" => vec![skill::SkillEffect::Grow],
                        // Java `params.getInt("sp", 0)` — an int on the XML, but
                        // the award path takes the same i64 as every other SP
                        // grant.
                        // The three appearance potions, one variant.
                        "ChangeFace" => vec![skill::SkillEffect::ChangeAppearance {
                            part: skill::AppearancePart::Face,
                            value: param("value").unwrap_or(0.0) as i32,
                        }],
                        "ChangeHairStyle" => vec![skill::SkillEffect::ChangeAppearance {
                            part: skill::AppearancePart::HairStyle,
                            value: param("value").unwrap_or(0.0) as i32,
                        }],
                        "ChangeHairColor" => vec![skill::SkillEffect::ChangeAppearance {
                            part: skill::AppearancePart::HairColor,
                            value: param("value").unwrap_or(0.0) as i32,
                        }],
                        "SendSystemMessageToClan" => {
                            vec![skill::SkillEffect::SendSystemMessageToClan {
                                message_id: param("id").unwrap_or(0.0) as i16,
                            }]
                        }
                        // Empty in Java — see the variant.
                        "Recovery" => vec![skill::SkillEffect::Recovery],
                        "GiveSp" => vec![skill::SkillEffect::GiveSp {
                            sp: param("sp").unwrap_or(0.0) as i64,
                        }],
                        "TeleportToTarget" => vec![skill::SkillEffect::TeleportToTarget],
                        "SetSkill" => vec![skill::SkillEffect::SetSkill {
                            skill_id: value_at(params, "skillId", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            // Java defaults this to 1, not 0.
                            skill_level: value_at(params, "skillLevel", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1),
                        }],
                        // `CallPc.java`. `itemId`/`itemCount` are the Summon
                        // Friend toll, charged to the **target**; the monster
                        // half reads neither and every monster carrier omits
                        // them, which is why they default to 0.
                        "CallPc" => vec![skill::SkillEffect::CallPc {
                            item_id: value_at(params, "itemId", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            item_count: value_at(params, "itemCount", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        }],
                        // `Speed` pumps four move-speed stats at once (Java
                        // `Speed.pump`); the 1-name→1-stat `EFFECT_REGISTRY` can't
                        // express that, so expand it here. Without this, movement
                        // buffs (Wind Walk, Agility) loaded with an empty effect
                        // list and did nothing — server or client.
                        "Speed" => match param("amount") {
                            Some(amount) => [
                                Stat::RunSpeed,
                                Stat::WalkSpeed,
                                Stat::SwimRunSpeed,
                                Stat::SwimWalkSpeed,
                            ]
                            .into_iter()
                            .map(|stat| stat_mod(stat, amount))
                            .collect(),
                            None => Vec::new(),
                        },
                        // Blessing of Protection (5182): PK-damage immunity
                        // (`pvp::protection_blessing_blocks`). No stat
                        // modifier, so it would otherwise fall through to an empty
                        // effect list and never land as a buff — carry a marker so
                        // `apply_skill_effects` still creates the icon-only timed buff.
                        "ProtectionBlessing" => vec![skill::SkillEffect::ProtectionBlessing],
                        // Noblesse Blessing (1323): no params, no stat modifier —
                        // the whole mechanic is the `NOBLESS_BLESSING` flag the
                        // death path reads. Without this arm the effect fell through
                        // to `EFFECT_REGISTRY`, wasn't found, and the buff was
                        // dropped whole (the skill cast but nothing landed).
                        "NoblesseBless" => vec![skill::SkillEffect::NoblesseBless],
                        // Fear (65/405/450/1092/1169/1272/1381/1400): forced flight.
                        // The `<effect name="Fear"/>` element carries no params in
                        // this dist — Java's `Fear` constructor ignores its `StatSet`
                        // outright and `getTicks()` returns a hard-coded 5 — so the
                        // cadence is a literal, not a parsed value. Every one of
                        // these skills also carries `BlockControl`, so the *buff*
                        // already landed before this arm existed (icon, duration and
                        // the `BLOCK_CONTROL` flag); what was missing was the flight
                        // itself, so the debuff simply never moved anyone.
                        "Fear" => vec![skill::SkillEffect::Fear { ticks: FEAR_TICKS }],
                        // Silent Move 221, Stealth 411, Dance of Shadows 366, and
                        // the stealth half of Fake Death 60. Java's handler is an
                        // empty constructor plus `getEffectFlags` — a pure state
                        // flag, no params at all.
                        // Mana Burn 1398, Mana Storm 1399, Aura Sink 1102, Seal of
                        // Gloom 1210 — MP drain. `critical`/`criticalLimit` are the
                        // effect's own params (all four declare `critical=true`);
                        // the crit *rate* comes from the skill's
                        // `<magicCriticalRate>`, not from here.
                        //
                        // Mana Burn and Mana Storm carry only this effect, so before
                        // this arm both parsed to an empty effect list and were
                        // dropped whole — the nukes cast and drained nothing.
                        "MagicalAttackMp" => vec![skill::SkillEffect::MagicalAttackMp {
                            power: param("power").unwrap_or(0.0),
                            critical: value_at(params, "critical", level) == Some("true"),
                            critical_limit: param("criticalLimit").unwrap_or(0.0),
                        }],
                        // The MP-restore family. All four are instant effects that
                        // differ only in how the amount is computed; the shared
                        // apply path lives in `restore_mp`.
                        "ManaHeal" => vec![skill::SkillEffect::ManaHeal {
                            power: param("power").unwrap_or(0.0),
                        }],
                        "ManaHealByLevel" => vec![skill::SkillEffect::ManaHealByLevel {
                            power: param("power").unwrap_or(0.0),
                        }],
                        "ManaHealPercent" => vec![skill::SkillEffect::ManaHealPercent {
                            power: param("power").unwrap_or(0.0),
                        }],
                        // Java's `Mp` handler reads `amount`/`mode`, not `power`.
                        "Mp" => vec![skill::SkillEffect::MpRestore {
                            amount: param("amount").unwrap_or(0.0),
                            percent: modifier_mode == StatModifierType::Per,
                        }],
                        // Java defaults `chance` to 100 when the tag is absent —
                        // which is every Confuse skill on this dist (only the two
                        // `RandomizeHate` ones declare 80).
                        "Confuse" => vec![skill::SkillEffect::Confuse {
                            chance: value_at(params, "chance", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(100),
                        }],
                        "RandomizeHate" => vec![skill::SkillEffect::RandomizeHate {
                            chance: value_at(params, "chance", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(100),
                        }],
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
                                            crate::data::item_data::WeaponType::from_name(w.trim())
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
                                vec![skill::SkillEffect::TriggerSkillByAttack {
                                    min_damage: int_param("minDamage", 1),
                                    chance: int_param("chance", 100),
                                    skill_id,
                                    skill_level: int_param("skillLevel", 1),
                                    on_party: value_at(params, "targetType", level)
                                        == Some("MY_PARTY"),
                                    is_critical: value_at(params, "isCritical", level)
                                        == Some("true"),
                                    allow_weapons,
                                }]
                            }
                        }
                        // Rage 94, Frenzy 176, Two-handed Weapon Mastery 293.
                        // Java's handler carries eleven stat/mode pairs; the only
                        // ones any reachable skill sets are `pAtk` and
                        // `pAccuracy`, so those two are read and the rest keep
                        // their zero default (the same
                        // scope-to-what-the-dist-reaches call `TriggerSkillByAttack`
                        // made).
                        //
                        // Two conditions, both from Java's static fields:
                        // `ConditionUsingItemType(BLUNT|SWORD)` — expressed through
                        // the existing `weapon_condition` mask — and
                        // `ConditionUsingSlotType(SLOT_LR_HAND)`, the new
                        // `two_handed` axis.
                        "TwoHandedBluntBonus" | "TwoHandedSwordBonus" => {
                            let weapon = if xml_name == "TwoHandedBluntBonus" {
                                crate::data::item_data::WeaponType::Blunt.mask_bit()
                            } else {
                                crate::data::item_data::WeaponType::Sword.mask_bit()
                            };
                            let pair = |amount_key: &str, mode_key: &str, stat: Stat| {
                                let amount = value_at(params, amount_key, level)
                                    .and_then(|v| v.parse::<f64>().ok())?;
                                if amount == 0.0 {
                                    return None;
                                }
                                let mode = if value_at(params, mode_key, level) == Some("PER") {
                                    StatModifierType::Per
                                } else {
                                    StatModifierType::Diff
                                };
                                Some(skill::SkillEffect::StatModifier(
                                    skill::StatModifierEffect {
                                        stat,
                                        mode,
                                        amount,
                                        weapon_condition: weapon,
                                        two_handed: true,
                                        ..Default::default()
                                    },
                                ))
                            };
                            [
                                pair("pAtkAmount", "pAtkMode", Stat::PhysicalAttack),
                                pair("pAccuracyAmount", "pAccuracyMode", Stat::AccuracyCombat),
                            ]
                            .into_iter()
                            .flatten()
                            .collect()
                        }
                        "Resurrection" => {
                            let int_param = |key: &str, d: i32| {
                                value_at(params, key, level)
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(d)
                            };
                            vec![skill::SkillEffect::Resurrection {
                                power: int_param("power", 0),
                                hp_percent: int_param("hpPercent", 0),
                                mp_percent: int_param("mpPercent", 0),
                                cp_percent: int_param("cpPercent", 0),
                            }]
                        }
                        // Java throws on an empty `Summon` param set; here a
                        // missing/zero `npcId` simply yields no effect, matching how
                        // every other arm handles unusable params.
                        "Summon" => {
                            let int_param = |key: &str, d: i32| {
                                value_at(params, key, level)
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(d)
                            };
                            let npc_id = int_param("npcId", 0);
                            if npc_id == 0 {
                                Vec::new()
                            } else {
                                vec![skill::SkillEffect::Summon {
                                    npc_id,
                                    life_time: int_param("lifeTime", 0),
                                    consume_item_id: int_param("consumeItemId", 0),
                                    consume_item_count: int_param("consumeItemCount", 1) as i64,
                                }]
                            }
                        }
                        "SummonPet" => vec![skill::SkillEffect::SummonPet],
                        "BlockMove" => vec![skill::SkillEffect::BlockMove],
                        // `type` picks the Java stat: PHYSICAL (the default) or
                        // MAGICAL. Physical Mirror 350 and Magical Mirror 351 carry
                        // *only* this effect, so both were dropped whole before it.
                        // `type` is a `BasicProperty`: `NONE`, `PHYSICAL` (the
                        // default) or **`MAGIC`** — not "MAGICAL", which is the
                        // spelling this port first guessed and which would have
                        // silently routed every magic reflect into the physical
                        // stat. Both Mirrors carry one effect of each kind.
                        //
                        // Their `<armorTYpe>SHIELD</armorTYpe>` gate is a datapack
                        // typo (10 occurrences against 220 correct `<armorType>`).
                        // Java matches element names exactly too, so the condition
                        // is inert on both sides and is faithfully reproduced by
                        // not special-casing it.
                        "ReflectSkill" => vec![skill::SkillEffect::ReflectSkill {
                            magic: value_at(params, "type", level) == Some("MAGIC"),
                            amount: param("amount").unwrap_or(0.0),
                        }],
                        "SilentMove" => vec![skill::SkillEffect::SilentMove],
                        // Fake Death 60. Two halves: the `FAKE_DEATH` flag and an
                        // MP upkeep with the same `power * getTicksMultiplier()`
                        // shape as `ManaDamOverTime`, which it shares the tick
                        // chain with. Skill 60 carries *only* this and
                        // `SilentMove`, so with both unported the effect list came
                        // out empty and the whole skill was dropped — it cast and
                        // did nothing at all.
                        "FakeDeath" => vec![skill::SkillEffect::FakeDeath {
                            power: param("power").unwrap_or(0.0),
                            ticks: value_at(params, "ticks", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        }],
                        // "Transform <Monster>" scroll family (541-558, 617-674):
                        // polymorph the caster into `transformationId`. No stat
                        // modifier of its own — the transform template's own
                        // stat/speed/skill overrides apply via
                        // `admin::transforms::apply_transform_state` — so without
                        // this arm the effect fell through to `EFFECT_REGISTRY`,
                        // wasn't found, and the buff was dropped whole.
                        "Transformation" => match param("transformationId") {
                            Some(id) if id != 0.0 => vec![skill::SkillEffect::Transform {
                                transformation_id: id as i32,
                            }],
                            _ => Vec::new(),
                        },
                        // Fighter-class toggle upkeep (Accuracy 256, Guard Stance
                        // 288, War Frenzy 424, Super Haste 7029, …): without this
                        // arm the effect fell through to `EFFECT_REGISTRY`, wasn't
                        // found, and the toggle's *stat* half (parsed separately,
                        // below) landed as a free buff with no MP cost at all.
                        "MpConsumePerLevel" => {
                            match (
                                param("power"),
                                value_at(params, "ticks", level)
                                    .and_then(|v| v.parse::<i32>().ok()),
                            ) {
                                (Some(power), Some(ticks)) if ticks > 0 => {
                                    vec![skill::SkillEffect::MpConsumePerLevel { power, ticks }]
                                }
                                _ => Vec::new(),
                            }
                        }
                        // Death Whisper (1242) & co.: Java `CriticalDamage extends
                        // AbstractStatEffect(params, CRITICAL_DAMAGE, CRITICAL_DAMAGE_ADD)`
                        // — a two-stat effect that pumps the multiplicative
                        // `CRITICAL_DAMAGE` in `PER` mode and the additive
                        // `CRITICAL_DAMAGE_ADD` in `DIFF` mode. The 1-name→1-stat
                        // `EFFECT_REGISTRY` can't express that, so pick the stat by
                        // mode here (like `Speed`). Without this the effect fell
                        // through, produced no modifier, and the buff was dropped
                        // whole (community-board "Death Whisper doesn't apply").
                        // The `AbstractStatEffect` crit-damage family: one handler,
                        // two stats, picked by mode (PER → the multiplier, DIFF →
                        // the flat add). Every one of these was parsed *before* this
                        // slice and pumped a stat that nothing read — see
                        // `formulas::crit_damage_multiplier`.
                        "CriticalDamage" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::CriticalDamage
                                } else {
                                    Stat::CriticalDamageAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        "DefenceCriticalRate" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::DefenceCriticalRate
                                } else {
                                    Stat::DefenceCriticalRateAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        "DefenceCriticalDamage" => param("amount")
                            .map(|amount| {
                                let stat = if modifier_mode == StatModifierType::Per {
                                    Stat::DefenceCriticalDamage
                                } else {
                                    Stat::DefenceCriticalDamageAdd
                                };
                                stat_mod(stat, amount)
                            })
                            .into_iter()
                            .collect(),
                        // Prophecy of Wind (1357), Victories of Pa'agrio (1414).
                        // Java's `MAGIC_CRITICAL_DAMAGE_ADD` half is dropped: the
                        // magic branch of `calcCritDamage` reads only the
                        // multiplier, and `calcCritDamageAdd`'s magic result is
                        // never applied (`Formulas.calcMagicDam` says as much in its own comment).
                        "MagicCriticalDamage" => param("amount")
                            .filter(|_| modifier_mode == StatModifierType::Per)
                            .map(|amount| stat_mod(Stat::MagicCriticalDamage, amount))
                            .into_iter()
                            .collect(),
                        "DefenceMagicCriticalDamage" => param("amount")
                            .filter(|_| modifier_mode == StatModifierType::Per)
                            .map(|amount| stat_mod(Stat::DefenceMagicCriticalDamage, amount))
                            .into_iter()
                            .collect(),
                        // Focus Death (355), Focus Power (357): a crit-damage
                        // multiplier that applies only from a given attack
                        // position. Java merges `(amount/100)+1` multiplicatively
                        // into `_positionTypeStats` — a different map, merge and
                        // identity from the move-type one, so the qualifier routes
                        // it accordingly. Read only by the *autoattack* branch of
                        // `calcCritDamage`, matching Java.
                        // `CriticalRatePositionBonus` (Focus Chance 356) — the
                        // crit-*rate* twin of `CriticalDamagePosition`, and the
                        // only skill on this dist that declares all three
                        // positions at once (−30 front, +30 side, +60 back).
                        "CriticalRatePositionBonus" => {
                            let position = match value_at(params, "position", level) {
                                Some("BACK") => crate::model::movement::Position::Back,
                                Some("SIDE") => crate::model::movement::Position::Side,
                                _ => crate::model::movement::Position::Front,
                            };
                            param("amount")
                                .map(|amount| {
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::CriticalRate,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: Some(
                                            crate::model::stats::StatQualifier::Position(position),
                                        ),
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        "CriticalDamagePosition" => {
                            let position = match value_at(params, "position", level) {
                                // Java `params.getEnum("position", Position.class, Position.FRONT)`.
                                Some("BACK") => crate::model::movement::Position::Back,
                                Some("SIDE") => crate::model::movement::Position::Side,
                                _ => crate::model::movement::Position::Front,
                            };
                            param("amount")
                                .map(|amount| {
                                    skill::SkillEffect::StatModifier(skill::StatModifierEffect {
                                        stat: Stat::CriticalDamage,
                                        mode: StatModifierType::Per,
                                        amount,
                                        armor_condition: *armor_condition,
                                        weapon_condition: *weapon_condition,
                                        qualifier: Some(
                                            crate::model::stats::StatQualifier::Position(position),
                                        ),
                                        two_handed: false,
                                    })
                                })
                                .into_iter()
                                .collect()
                        }
                        // Mental Shield (1035) / Stun Resistance ("Resist Shock",
                        // 1259): Java `DefenceTrait` raises per-`TraitType`
                        // resistance (HOLD/SLEEP/SHOCK…). Its params are the trait
                        // *names*, not `amount`, so they are read straight off the
                        // param map rather than through the usual `amount` lookup.
                        "DefenceTrait" => {
                            // Every param is a trait name → percent; Java
                            // divides by 100 and treats >= 1.0 as invulnerable.
                            let traits: Vec<(crate::model::skill::TraitType, f64)> = params
                                .keys()
                                .filter_map(|key| {
                                    let raw = value_at(params, key, level)?;
                                    let pct: f64 = raw.parse().ok()?;
                                    Some((
                                        crate::model::skill::TraitType::from_xml(key),
                                        pct / 100.0,
                                    ))
                                })
                                .collect();
                            vec![skill::SkillEffect::DefenceTrait { traits }]
                        }
                        // Vampiric Rage (1268): Java `VampiricAttack` grants a chance
                        // to absorb a % of melee damage as HP. The melee-absorb path
                        // isn't modeled, so carry an icon-only marker rather than
                        // dropping the buff.
                        "VampiricAttack" => vec![skill::SkillEffect::VampiricAttack {
                            amount: param("amount").unwrap_or(0.0),
                            chance: param("chance").unwrap_or(0.0),
                        }],
                        // "Detect <Category> Weakness" (75/80/87/88/104, 359/360):
                        // Java `AttackTrait` merges a `*_WEAKNESS` bonus onto the
                        // caster — genuinely inert in the reference server too (see
                        // the doc comment on `SkillEffect::AttackTrait`), so this
                        // carries an icon-only marker like `DefenceTrait`/
                        // `VampiricAttack` rather than the per-trait param map.
                        // Same shape as `DefenceTrait`: every param is a trait
                        // name → percent, divided by 100.
                        "AttackTrait" => {
                            let traits: Vec<(crate::model::skill::TraitType, f64)> = params
                                .keys()
                                .filter_map(|key| {
                                    let raw = value_at(params, key, level)?;
                                    let pct: f64 = raw.parse().ok()?;
                                    Some((
                                        crate::model::skill::TraitType::from_xml(key),
                                        pct / 100.0,
                                    ))
                                })
                                .collect();
                            vec![skill::SkillEffect::AttackTrait { traits }]
                        }
                        // Celestial Shield (1418), Flames of Invincibility (1427),
                        // Dance of Medusa (367), Sonic/Force Barrier (442/443): a
                        // skill carries two of these, one `BLOCK_HP` and one
                        // `BLOCK_MP` (`<effect name="DamageBlock"><type>BLOCK_HP
                        // </type></effect>`, a plain string param, not `param()`'s
                        // f64). Without this arm the effect fell through to
                        // `EFFECT_REGISTRY`, wasn't found, and these short
                        // invulnerability shields did nothing.
                        "DamageBlock" => {
                            let ty = value_at(params, "type", level);
                            vec![skill::SkillEffect::DamageBlock {
                                block_hp: ty == Some("BLOCK_HP"),
                                block_mp: ty == Some("BLOCK_MP"),
                            }]
                        }
                        // `MagicMpCost` / `Reuse` — a percentage on one
                        // `magicType` bucket. Java's handlers read `magicType`
                        // (default 0 = physical) and `amount`; the `<mode>PER`
                        // that every carrier also declares is decorative, the
                        // handlers never read it. `amount` can be per level
                        // (Clarity 1397, Quick Recovery 164), hence `value_at`.
                        //
                        // A missing/unparsable `amount` yields a factor of 1
                        // downstream rather than dropping the buff — `Holy
                        // Squad` (615) really does carry `0` for its first two
                        // levels.
                        "MagicMpCost" => vec![skill::SkillEffect::MagicMpCost {
                            magic_type: effect_magic_type(params, level),
                            amount: value_at(params, "amount", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.0),
                        }],
                        "Reuse" => vec![skill::SkillEffect::Reuse {
                            magic_type: effect_magic_type(params, level),
                            amount: value_at(params, "amount", level)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.0),
                        }],
                        // Song of Vengeance (305): the combat damage-reflect
                        // path isn't modeled, so carry an icon-only marker
                        // (like `VampiricAttack`) rather than dropping the buff
                        // whole at the empty-effects guard — it must still show
                        // and expire.
                        "DamageShield" => vec![skill::SkillEffect::DamageShield {
                            amount: param("amount").unwrap_or(0.0),
                        }],
                        // Expand Inventory/Warehouse/Trade/Common Craft/Dwarven
                        // Craft (1368-1372, the craftsman-guild storage passives):
                        // Java `EnlargeSlot extends AbstractStatEffect` reads
                        // `amount` + a `type` string picking one of 6 `Stat`s; an
                        // absent `type` (Expand Inventory) defaults to
                        // INVENTORY_NORMAL. Expand Trade carries two effect blocks
                        // per level, one TRADE_BUY one TRADE_SELL. The 1-name-1-stat
                        // `EFFECT_REGISTRY` can't express the type-selected stat, so
                        // without this arm the effect fell through and these
                        // passives did nothing.
                        "EnlargeSlot" => {
                            let stat = match value_at(params, "type", level) {
                                Some("STORAGE_PRIVATE") => Stat::StoragePrivate,
                                Some("TRADE_SELL") => Stat::TradeSell,
                                Some("TRADE_BUY") => Stat::TradeBuy,
                                Some("RECIPE_DWARVEN") => Stat::RecipeDwarven,
                                Some("RECIPE_COMMON") => Stat::RecipeCommon,
                                _ => Stat::InventoryNormal,
                            };
                            param("amount")
                                .map(|amount| stat_mod(stat, amount))
                                .into_iter()
                                .collect()
                        }
                        _ => match EFFECT_REGISTRY
                            .iter()
                            .find(|(n, _)| n == xml_name)
                            .map(|(_, s)| *s)
                        {
                            Some(stat) => param("amount")
                                .map(|amount| stat_mod(stat, amount))
                                .into_iter()
                                .collect(),
                            // Nothing recognised this name: the effect is
                            // dropped and, if it was the skill's only one, so
                            // is the whole buff. Recorded, not silent.
                            None => {
                                SkillGaps::record(&mut gaps.borrow_mut().effects, xml_name, id);
                                Vec::new()
                            }
                        },
                    }
                })
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
            // `<attributeType>FIRE</attributeType>` + `<attributeValue>20`
            // — the skill's element for `calcAttributeBonus`. `NONE` and
            // unknown names read as no element, like Java's enum default.
            attribute_type: value_at(values, "attributeType", level)
                .and_then(crate::model::stats::Element::from_xml),
            attribute_value: get_i("attributeValue", 0),
        }
    }
}
