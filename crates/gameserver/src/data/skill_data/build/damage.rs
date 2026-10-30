//! The direct-damage effects — magical and physical attacks, drains,
//! blows, lethal and the damage-linked variants.

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
    // Java's `params.getString(key).split(";")` lists: `abnormalType` names
    // (kept as strings, the way `Skill::abnormal_type` is stored) and
    // `races` (`Race.valueOf`).
    let list = |key: &str| -> Vec<String> {
        value_at(params, key, level)
            .map(|v| {
                v.split(';')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let races = || -> Vec<crate::enums::Race> {
        list("races")
            .iter()
            .filter_map(|r| crate::enums::Race::from_name(r))
            .collect()
    };

    Some(match xml_name.as_str() {
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
        "MagicalDamage" => vec![skill::effects::SkillEffect::MagicalDamage {
            power: param("power").unwrap_or(0.0),
            debuff_modifier: param("debuffModifier").unwrap_or(1.0),
            races: races(),
            race_modifier: param("raceModifier").unwrap_or(1.0),
        }],
        // Ranged magical nuke (e.g. Prominence 1230). Java's
        // `MagicalDamageRange` computes the same
        // `calcMagicDam(mAtk, power, mDef, sps, bss, mcrit)` core as
        // `MagicalDamage`, plus the `shieldDefPercent` shield-block
        // term its own variant carries.
        "MagicalDamageRange" => {
            vec![skill::effects::SkillEffect::MagicalDamageRange {
                power: param("power").unwrap_or(0.0),
                shield_def_percent: param("shieldDefPercent").unwrap_or(0.0),
            }]
        }
        // Soul-charge magic nuke (Fallen Arrow 1431, Abyssal Blaze 1433, …):
        // `power` is its only param.
        "MagicalSoulDamage" => vec![skill::effects::SkillEffect::MagicalSoulDamage {
            power: param("power").unwrap_or(0.0),
        }],
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
        "MagicalDamageMp" => vec![skill::effects::SkillEffect::MagicalDamageMp {
            power: param("power").unwrap_or(0.0),
            critical: value_at(params, "critical", level) == Some("true"),
            critical_limit: param("criticalLimit").unwrap_or(0.0),
        }],
        // Vampiric Touch/Claw: magic damage + self-heal of
        // `percentage`% of the drained HP.
        "HpDrain" => vec![skill::effects::SkillEffect::HpDrain {
            power: param("power").unwrap_or(0.0),
            percentage: param("percentage").unwrap_or(0.0),
        }],
        // Dagger blows (calcBlowDamage). FatalBlow/Backstab roll
        // `criticalChance` (default 0) to double; SoulBlow doesn't
        // (its charged-soul boost is unmodeled → ×1). Backstab also
        // requires flanking. Their `Lethal` sibling effect is a
        // separate `<effect>` block, parsed in its own arm below.
        "FatalBlow" => vec![skill::effects::SkillEffect::Blow {
            power: param("power").unwrap_or(0.0),
            chance_boost: param("chanceBoost").unwrap_or(0.0),
            critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
            backstab: false,
            abnormals: list("abnormalType"),
            abnormal_power: param("abnormalPower").unwrap_or(1.0),
        }],
        "Backstab" => vec![skill::effects::SkillEffect::Blow {
            power: param("power").unwrap_or(0.0),
            chance_boost: param("chanceBoost").unwrap_or(0.0),
            critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
            backstab: true,
            abnormals: Vec::new(),
            abnormal_power: 0.0,
        }],
        "SoulBlow" => vec![skill::effects::SkillEffect::Blow {
            power: param("power").unwrap_or(0.0),
            chance_boost: param("chanceBoost").unwrap_or(0.0),
            critical_chance: None,
            backstab: false,
            abnormals: Vec::new(),
            abnormal_power: 0.0,
        }],
        // Backstab (30), Lethal Blow (344), Deadly Blow (263),
        // Critical Blow (409), Lethal Shot (343), Turn/Banish
        // Undead/Seraph (1400/405/450): without this arm the
        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
        // and the bonus instant-kill/half-kill chance never
        // rolled — only these skills' other (already-ported)
        // effect landed.
        "Lethal" => vec![skill::effects::SkillEffect::Lethal {
            full_lethal: param("fullLethal").unwrap_or(0.0),
            half_lethal: param("halfLethal").unwrap_or(0.0),
        }],
        // Java's `criticalChance` default here is **0**, not
        // `PhysicalDamage`'s 10, and it has no `ignoreShieldDefence` param
        // at all.
        "PhysicalDamageHpLink" => {
            vec![skill::effects::SkillEffect::PhysicalDamageHpLink {
                power: param("power").unwrap_or(0.0),
                p_atk_mod: 1.0,
                p_def_mod: 1.0,
                critical_chance: param("criticalChance").unwrap_or(0.0),
                ignore_shield_defence: false,
            }]
        }
        "PhysicalDamage" => vec![skill::effects::SkillEffect::PhysicalDamage {
            power: param("power").unwrap_or(0.0),
            p_atk_mod: param("pAtkMod").unwrap_or(1.0),
            p_def_mod: param("pDefMod").unwrap_or(1.0),
            critical_chance: param("criticalChance").unwrap_or(10.0),
            ignore_shield_defence: value_at(params, "ignoreShieldDefence", level) == Some("true"),
            abnormals: list("abnormalType"),
            abnormal_damage_mod: param("damageModifier").unwrap_or(1.0),
            abnormal_power_mod: param("powerModifier").unwrap_or(1.0),
            races: races(),
            race_modifier: param("raceModifier").unwrap_or(1.0),
        }],
        // Its `criticalChance` default is 0, and it has no pAtk/pDef mods.
        "PhysicalSoulDamage" => vec![skill::effects::SkillEffect::PhysicalSoulDamage {
            power: param("power").unwrap_or(0.0),
            critical_chance: param("criticalChance").unwrap_or(0.0),
            ignore_shield_defence: value_at(params, "ignoreShieldDefence", level) == Some("true"),
        }],
        // Double Sonic Slash (5), Sonic Blaster (6), Force Burst (17), …:
        // `chargeConsume` is read from the effect's own params, like every
        // other Salvation handler param. Twelve carriers declare it there;
        // the ones that declare it at skill level instead spend it at cast
        // time (`Skill::charge_consume_count`) and land here with 0.
        "EnergyDamage" => vec![skill::effects::SkillEffect::EnergyDamage {
            power: param("power").unwrap_or(0.0),
            critical_chance: param("criticalChance").unwrap_or(10.0),
            p_def_mod: param("pDefMod").unwrap_or(1.0),
            charge_consume: cx.int_param("chargeConsume", 0),
            ignore_shield_defence: value_at(params, "ignoreShieldDefence", level) == Some("true"),
        }],
        "DeathLink" => param("power")
            .map(|power| vec![skill::effects::SkillEffect::DeathLink { power }])
            .unwrap_or_default(),
        _ => return None,
    })
}
