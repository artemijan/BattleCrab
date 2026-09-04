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
        "MagicalAttack" => vec![skill::effects::SkillEffect::MagicalAttack {
            power: param("power").unwrap_or(0.0),
        }],
        // Ranged magical nuke (e.g. Prominence 1230). Java's
        // `MagicalAttackRange` computes the same
        // `calcMagicDam(mAtk, power, mDef, sps, bss, mcrit)` core as
        // `MagicalAttack`, plus the `shieldDefPercent` shield-block
        // term its own variant carries.
        "MagicalAttackRange" => {
            vec![skill::effects::SkillEffect::MagicalAttackRange {
                power: param("power").unwrap_or(0.0),
                shield_def_percent: param("shieldDefPercent").unwrap_or(0.0),
            }]
        }
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
        "MagicalSoulAttack" => vec![skill::effects::SkillEffect::MagicalAttack {
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
        "MagicalAttackMp" => vec![skill::effects::SkillEffect::MagicalAttackMp {
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
        }],
        "Backstab" => vec![skill::effects::SkillEffect::Blow {
            power: param("power").unwrap_or(0.0),
            chance_boost: param("chanceBoost").unwrap_or(0.0),
            critical_chance: Some(param("criticalChance").unwrap_or(0.0)),
            backstab: true,
        }],
        "SoulBlow" => vec![skill::effects::SkillEffect::Blow {
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
        "Lethal" => vec![skill::effects::SkillEffect::Lethal {
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
            vec![skill::effects::SkillEffect::PhysicalAttackHpLink {
                power: param("power").unwrap_or(0.0),
                p_atk_mod: 1.0,
                p_def_mod: 1.0,
                critical_chance: param("criticalChance").unwrap_or(0.0),
                ignore_shield_defence: false,
            }]
        }
        "PhysicalAttack" | "PhysicalSoulAttack" => {
            vec![skill::effects::SkillEffect::PhysicalAttack {
                power: param("power").unwrap_or(0.0),
                p_atk_mod: param("pAtkMod").unwrap_or(1.0),
                p_def_mod: param("pDefMod").unwrap_or(1.0),
                critical_chance: param("criticalChance").unwrap_or(10.0),
                ignore_shield_defence: value_at(params, "ignoreShieldDefence", level)
                    == Some("true"),
            }]
        }
        // Double Sonic Slash (5), Sonic Blaster (6), Force Burst
        // (17), …: `chargeConsume` is a *skill-level* tag (a
        // sibling of `<targetType>`), not a child of the
        // `<effect name="EnergyAttack">` element itself — Java's
        // effect constructors read the skill's whole merged param
        // set, so it reaches `_chargeConsume` the same way. Without
        // this arm the effect fell through to `EFFECT_REGISTRY`,
        // wasn't found, and every Force-spend attack did nothing.
        "EnergyAttack" => vec![skill::effects::SkillEffect::EnergyAttack {
            power: param("power").unwrap_or(0.0),
            critical_chance: param("criticalChance").unwrap_or(10.0),
            p_def_mod: param("pDefMod").unwrap_or(1.0),
            charge_consume: value_at(values, "chargeConsume", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            ignore_shield_defence: value_at(params, "ignoreShieldDefence", level) == Some("true"),
        }],
        "DeathLink" => param("power")
            .map(|power| vec![skill::effects::SkillEffect::DeathLink { power }])
            .unwrap_or_default(),
        _ => return None,
    })
}
