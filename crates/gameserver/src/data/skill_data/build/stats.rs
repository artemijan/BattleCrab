//! Stat-modifier effects — the `<effect>` names that reduce to one or more
//! `StatModifier`s, plus the shield/reflect stats built the same way.

use super::super::{effect_magic_type, value_at};
use super::effect::Cx;
use crate::model::skill;
use crate::model::stats::{Stat, StatModifierType};

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
    let stat_mod = |stat: Stat, amount: f64| cx.stat_mod(stat, amount);

    Some(match xml_name.as_str() {
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
            let move_type =
                value_at(params, "type", level).and_then(crate::model::stats::MoveType::from_xml);
            match (stat, move_type, param("value")) {
                (Some(stat), Some(move_type), Some(amount)) => {
                    vec![skill::effects::SkillEffect::StatModifier(
                        skill::effects::StatModifierEffect {
                            stat,
                            mode: StatModifierType::Diff,
                            amount,
                            armor_condition: *armor_condition,
                            weapon_condition: *weapon_condition,
                            qualifier: Some(crate::model::stats::StatQualifier::MoveType(
                                move_type,
                            )),
                            two_handed: false,
                            hp_percent: 0,
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
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::ResistAbnormalDebuff,
                        mode: StatModifierType::Per,
                        amount,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: None,
                        two_handed: false,
                        hp_percent: 0,
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
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::ResistDispelBuff,
                        mode: StatModifierType::Per,
                        amount,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: None,
                        two_handed: false,
                        hp_percent: 0,
                    })
                })
                .into_iter()
                .collect()
        }
        // `EnlargeAbnormalSlot` reads `<slots>`, not the
        // `<amount>` the generic registry expects, so it needs
        // its own arm to become a stat modifier.
        "EnlargeAbnormalSlot" => value_at(params, "slots", level)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|slots| {
                vec![skill::effects::SkillEffect::StatModifier(
                    skill::effects::StatModifierEffect {
                        stat: Stat::MaxBuffSlots,
                        mode: StatModifierType::Diff,
                        amount: slots,
                        armor_condition: 0,
                        weapon_condition: 0,
                        qualifier: None,
                        two_handed: false,
                        hp_percent: 0,
                    },
                )]
            })
            .unwrap_or_default(),
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
        // `SkillMastery` stores the **BaseStat ordinal**, not a
        // magnitude — `calcSkillMastery` reads it back through
        // `BaseStat.values()[val]` to pick which stat's bonus
        // drives the proc chance.
        "SkillMastery" => vec![skill::effects::SkillEffect::StatModifier(
            skill::effects::StatModifierEffect {
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
                hp_percent: 0,
            },
        )],
        // `MpVampiricAttack` pumps **two** values from one
        // `<amount>`: the percentage (÷100) and a `sum`
        // (`amount × chance`, default chance 30 — "Classic:
        // 30% chance" in Java's own comment) that the chance
        // finalizer divides back out.
        // `VampiricDefence` is an `AbstractStatPercentEffect`,
        // so it merges as `mergeMul(stat, (amount/100)+1)`
        // **regardless of any declared `<mode>`** — hence an
        // explicit `Per` here rather than the registry path,
        // which would honour the (absent) mode and read `Diff`.
        "VampiricDefence" => param("amount")
            .map(|amount| {
                skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                    stat: Stat::AbsorbDamageDefence,
                    mode: StatModifierType::Per,
                    amount,
                    armor_condition: *armor_condition,
                    weapon_condition: *weapon_condition,
                    qualifier: None,
                    two_handed: false,
                    hp_percent: 0,
                })
            })
            .into_iter()
            .collect(),
        "MpVampiricAttack" => param("amount")
            .map(|amount| {
                let chance = param("chance").unwrap_or(30.0);
                vec![
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::AbsorbManaDamagePercent,
                        mode: StatModifierType::Diff,
                        amount: amount / 100.0,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: None,
                        two_handed: false,
                        hp_percent: 0,
                    }),
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::MpVampiricSum,
                        mode: StatModifierType::Diff,
                        amount: amount * chance,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: None,
                        two_handed: false,
                        hp_percent: 0,
                    }),
                ]
            })
            .unwrap_or_default(),
        // Vampiric Rage (1268): Java `VampiricAttack` grants a chance
        // to absorb a % of melee damage as HP. The melee-absorb path
        // isn't modeled, so carry an icon-only marker rather than
        // dropping the buff.
        "VampiricAttack" => vec![skill::effects::SkillEffect::VampiricAttack {
            amount: param("amount").unwrap_or(0.0),
            chance: param("chance").unwrap_or(0.0),
        }],
        // The elemental attribute pair (PLAN_G19_ATTRIBUTES.md):
        // one flat StatModifier per element named in the
        // (comma-separable) `attribute` param, default FIRE —
        // Java's `Stat.valueOf(attribute + "_POWER"/"_RES")`.
        "AttackAttribute" | "DefenceAttribute" => {
            let Some(amount) = param("amount") else {
                return Some(Vec::new());
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
        // G34 S3 — flag-only effects. Each maps to one
        // `effect_flag` bit; see `Skill::effect_flags`.
        // `SkillEvasion` is *not* a plain stat: Java keys it by
        // `magicType` in a separate map, so a skill-dodge buff
        // dodges only its own bucket (0 = physical skills).
        "SkillEvasion" => vec![skill::effects::SkillEffect::SkillEvasion {
            magic_type: value_at(params, "magicType", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            amount: param("amount").unwrap_or(0.0),
        }],
        "SkillTurning" => vec![skill::effects::SkillEffect::SkillTurning {
            chance: param("chance").unwrap_or(100.0) as i32,
            static_chance: value_at(params, "staticChance", level)
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        }],
        // `Speed` pumps four move-speed stats at once (Java
        // `Speed.pump`); the 1-name→1-stat `EFFECT_REGISTRY` can't
        // express that, so expand it here. Without this, movement
        // buffs (Wind Walk, Agility) loaded with an empty effect
        // list and did nothing — server or client.
        // `Speed.pump` merges the amount onto **six** speed
        // stats: RUN/WALK, SWIM_RUN/SWIM_WALK and
        // FLY_RUN/FLY_WALK. The two fly stats are dropped here
        // and the drop is inert: nothing on this port reads
        // `FLY_RUN_SPEED`, because `UserInfo` derives a rider's
        // flight speed from the *run* speed
        // (`isFlying() ? runSpd : 0`, Java's own shape) — so a
        // Speed buff already reaches a wyvern rider through the
        // stat that is modelled.
        //
        // The handler's `weaponType` gate (`ConditionUsingItemType`)
        // is not dropped: it rides `weapon_condition` on the
        // effect, which `stat_mod` copies and
        // `conditioned_passive_buffs` filters on.
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
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::CriticalRate,
                        mode: StatModifierType::Per,
                        amount,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: Some(crate::model::stats::StatQualifier::Position(position)),
                        two_handed: false,
                        hp_percent: 0,
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
                    skill::effects::SkillEffect::StatModifier(skill::effects::StatModifierEffect {
                        stat: Stat::CriticalDamage,
                        mode: StatModifierType::Per,
                        amount,
                        armor_condition: *armor_condition,
                        weapon_condition: *weapon_condition,
                        qualifier: Some(crate::model::stats::StatQualifier::Position(position)),
                        two_handed: false,
                        hp_percent: 0,
                    })
                })
                .into_iter()
                .collect()
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
                crate::data::item_data::kinds::WeaponType::Blunt.mask_bit()
            } else {
                crate::data::item_data::kinds::WeaponType::Sword.mask_bit()
            };
            let pair = |amount_key: &str, mode_key: &str, stat: Stat| {
                let amount =
                    value_at(params, amount_key, level).and_then(|v| v.parse::<f64>().ok())?;
                if amount == 0.0 {
                    return None;
                }
                let mode = if value_at(params, mode_key, level) == Some("PER") {
                    StatModifierType::Per
                } else {
                    StatModifierType::Diff
                };
                Some(skill::effects::SkillEffect::StatModifier(
                    skill::effects::StatModifierEffect {
                        stat,
                        mode,
                        amount,
                        weapon_condition: weapon,
                        two_handed: true,
                        hp_percent: 0,
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
        // Unlike every other stat effect, this one names its
        // target with Java's **`Stat` enum name** in a `<stat>`
        // child rather than through the effect name, so it
        // needs its own lookup. `ACCURACY_COMBAT` is the only
        // one on this dist (Shadow Sense 294); an unknown name
        // yields no effect and is recorded as a gap.
        //
        // The grant is night-gated and lands through
        // `game_loop::stats::night_stats`, not the ordinary stat
        // pipeline — see the variant's docs.
        "NightStatModify" => match value_at(params, "stat", level) {
            Some("ACCURACY_COMBAT") => {
                vec![skill::effects::SkillEffect::NightStatModify {
                    stat: Stat::AccuracyCombat,
                    amount: param("amount").unwrap_or(0.0),
                    mode: modifier_mode,
                }]
            }
            _ => Vec::new(),
        },
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
        "MagicMpCost" => vec![skill::effects::SkillEffect::MagicMpCost {
            magic_type: effect_magic_type(params, level),
            amount: value_at(params, "amount", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
        }],
        "Reuse" => vec![skill::effects::SkillEffect::Reuse {
            magic_type: effect_magic_type(params, level),
            amount: value_at(params, "amount", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
        }],
        // Fighter-class toggle upkeep (Accuracy 256, Guard Stance
        // 288, War Frenzy 424, Super Haste 7029, …): without this
        // arm the effect fell through to `EFFECT_REGISTRY`, wasn't
        // found, and the toggle's *stat* half (parsed separately,
        // below) landed as a free buff with no MP cost at all.
        "MpConsumePerLevel" => {
            match (
                param("power"),
                value_at(params, "ticks", level).and_then(|v| v.parse::<i32>().ok()),
            ) {
                (Some(power), Some(ticks)) if ticks > 0 => {
                    vec![skill::effects::SkillEffect::MpConsumePerLevel { power, ticks }]
                }
                _ => Vec::new(),
            }
        }
        "PhysicalShieldAngleAll" => {
            vec![skill::effects::SkillEffect::PhysicalShieldAngleAll]
        }
        // `Lucky` (194) is an **empty effect** in Java — its
        // handler has only a `canStart` guard. The mechanic
        // lives in `Player.isLucky()`, which asks whether the
        // *buff* is present, so all this has to do is land.
        "Lucky" => vec![skill::effects::SkillEffect::Lucky],
        "ReduceDropPenalty" => {
            use crate::model::skill::ReduceDropKind;
            vec![skill::effects::SkillEffect::ReduceDropPenalty {
                // Java `mergeMul(stat, amount/100 + 1)`.
                exp_mul: param("exp").unwrap_or(0.0) / 100.0 + 1.0,
                kind: match value_at(params, "type", level) {
                    Some("PK") => ReduceDropKind::Pk,
                    Some("RAID") => ReduceDropKind::Raid,
                    _ => ReduceDropKind::Mob,
                },
            }]
        }
        // Sonic Focus (8), Focus Force (50), Sonic Rage (345), …:
        // without this arm the effect fell through to
        // `EFFECT_REGISTRY`, wasn't found, and the "build Force"
        // toggle/skill did nothing.
        "FocusMomentum" => vec![skill::effects::SkillEffect::FocusMomentum {
            amount: param("amount").unwrap_or(1.0) as i32,
            max_charges: param("maxCharges").unwrap_or(0.0) as i32,
        }],
        // Blessing of Protection (5182): PK-damage immunity
        // (`pvp::protection_blessing_blocks`). No stat
        // modifier, so it would otherwise fall through to an empty
        // effect list and never land as a buff — carry a marker so
        // `apply_skill_effects` still creates the icon-only timed buff.
        "ProtectionBlessing" => {
            vec![skill::effects::SkillEffect::ProtectionBlessing]
        }
        // Noblesse Blessing (1323): no params, no stat modifier —
        // the whole mechanic is the `NOBLESS_BLESSING` flag the
        // death path reads. Without this arm the effect fell through
        // to `EFFECT_REGISTRY`, wasn't found, and the buff was
        // dropped whole (the skill cast but nothing landed).
        "NoblesseBless" => vec![skill::effects::SkillEffect::NoblesseBless],
        // Song of Vengeance (305): the combat damage-reflect
        // path isn't modeled, so carry an icon-only marker
        // (like `VampiricAttack`) rather than dropping the buff
        // whole at the empty-effects guard — it must still show
        // and expire.
        "DamageShield" => vec![skill::effects::SkillEffect::DamageShield {
            amount: param("amount").unwrap_or(0.0),
        }],
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
            vec![skill::effects::SkillEffect::DamageBlock {
                block_hp: ty == Some("BLOCK_HP"),
                block_mp: ty == Some("BLOCK_MP"),
            }]
        }
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
        "ReflectSkill" => vec![skill::effects::SkillEffect::ReflectSkill {
            magic: value_at(params, "type", level) == Some("MAGIC"),
            amount: param("amount").unwrap_or(0.0),
        }],
        "PolearmSingleTarget" => {
            vec![skill::effects::SkillEffect::PolearmSingleTarget]
        }
        "Passive" => vec![skill::effects::SkillEffect::Passive],
        _ => return None,
    })
}
