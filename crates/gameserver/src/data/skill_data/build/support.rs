//! Heal, mana, CP and the restore/resurrect effects.

use super::super::value_at;
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
        "Heal" => vec![skill::effects::SkillEffect::Heal {
            power: param("power").unwrap_or(0.0),
        }],
        // Miracle (1426), Benediction (1271), Restore Life (1258),
        // Revival (181), Touch of Life (341): without this arm the
        // effect fell through to `EFFECT_REGISTRY`, wasn't found,
        // and the heal amount was silently 0.
        "HealPercent" => vec![skill::effects::SkillEffect::HealPercent {
            power: param("power").unwrap_or(0.0),
        }],
        // Instant CP change (Braveheart, Wrath, Touch of Death).
        "Cp" => param("amount")
            .map(|amount| skill::effects::SkillEffect::Cp {
                amount,
                percent: modifier_mode == StatModifierType::Per,
            })
            .into_iter()
            .collect(),
        // `CpHealPercent` — a share of **max CP**, clamped by
        // `getMaxRecoverableCp()`. `power == 100` is the full
        // pool (Java special-cases it to the same number).
        "CpHealPercent" => param("power")
            .map(|power| vec![skill::effects::SkillEffect::CpHealPercent { power }])
            .unwrap_or_default(),
        // `HpByLevel` heals the **effector**, not the effected
        // — Life Scavenge (46) and Corpse Life Drain (1151) top
        // the *caster* up off a corpse.
        "HpByLevel" => param("power")
            .map(|power| vec![skill::effects::SkillEffect::HpByLevel { power }])
            .unwrap_or_default(),
        // `Hp.java` — a raw instant HP change, not a `Heal`:
        // no `calcHeal`, no healing-stat scaling, no overheal
        // message. `DIFF` is a flat amount, `PER` a share of
        // **max** HP.
        "Hp" => vec![skill::effects::SkillEffect::Hp {
            amount: param("amount").unwrap_or(0.0),
            percent: mode == "PER",
        }],
        // Java's `Mp` handler reads `amount`/`mode`, not `power`.
        "Mp" => vec![skill::effects::SkillEffect::MpRestore {
            amount: param("amount").unwrap_or(0.0),
            percent: modifier_mode == StatModifierType::Per,
        }],
        // The MP-restore family. All four are instant effects that
        // differ only in how the amount is computed; the shared
        // apply path lives in `restore_mp`.
        "ManaHeal" => vec![skill::effects::SkillEffect::ManaHeal {
            power: param("power").unwrap_or(0.0),
        }],
        "ManaHealByLevel" => vec![skill::effects::SkillEffect::ManaHealByLevel {
            power: param("power").unwrap_or(0.0),
        }],
        "ManaHealPercent" => vec![skill::effects::SkillEffect::ManaHealPercent {
            power: param("power").unwrap_or(0.0),
        }],
        "RebalanceHP" => vec![skill::effects::SkillEffect::RebalanceHp],
        // Empty in Java — see the variant.
        "Recovery" => vec![skill::effects::SkillEffect::Recovery],
        // Pet food (Wolf Food 2048, etc.). Without this arm the
        // food item was consumed and restored nothing.
        "Feed" => vec![skill::effects::SkillEffect::Feed {
            normal: param("normal").unwrap_or(0.0) as i32,
            ride: param("ride").unwrap_or(0.0) as i32,
            wyvern: param("wyvern").unwrap_or(0.0) as i32,
        }],
        "Resurrection" => {
            let int_param = |key: &str, d: i32| {
                value_at(params, key, level)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(d)
            };
            vec![skill::effects::SkillEffect::Resurrection {
                power: int_param("power", 0),
                hp_percent: int_param("hpPercent", 0),
                mp_percent: int_param("mpPercent", 0),
                cp_percent: int_param("cpPercent", 0),
            }]
        }
        "ResurrectionSpecial" => {
            vec![skill::effects::SkillEffect::ResurrectionSpecial {
                power: param("power").unwrap_or(0.0) as i32,
                hp_percent: param("hpPercent").unwrap_or(0.0) as i32,
                mp_percent: param("mpPercent").unwrap_or(0.0) as i32,
                cp_percent: param("cpPercent").unwrap_or(0.0) as i32,
            }]
        }
        "Restoration" => match (param("itemId"), param("itemCount")) {
            (Some(item_id), Some(item_count)) => {
                vec![skill::effects::SkillEffect::GiveItem {
                    item_id: item_id as i32,
                    item_count: item_count as i64,
                    item_enchant_level: param("itemEnchantmentLevel").unwrap_or(0.0) as i32,
                }]
            }
            _ => Vec::new(),
        },
        "RestorationRandom" => vec![skill::effects::SkillEffect::GiveItemRandom {
            groups: groups.clone(),
        }],
        _ => return None,
    })
}
