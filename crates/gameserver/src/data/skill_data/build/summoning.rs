//! Summons, servitors, cubics and the pet-side effects.

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
        // The EffectPoint totem spawner (Symbol of Noise 455, Day
        // of Doom 1422, Anti-summoning Field 1424; PLAN_G19_SYMBOLS.md).
        "SummonNpc" => vec![skill::effects::SkillEffect::SummonNpc {
            npc_id: param("npcId").unwrap_or(0.0) as i32,
            npc_count: param("npcCount").unwrap_or(1.0) as i32,
            despawn_delay: param("despawnDelay").unwrap_or(0.0) as i32,
        }],
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
                vec![skill::effects::SkillEffect::Summon {
                    npc_id,
                    life_time: int_param("lifeTime", 0),
                    consume_item_id: int_param("consumeItemId", 0),
                    consume_item_count: int_param("consumeItemCount", 1) as i64,
                }]
            }
        }
        "SummonPet" => vec![skill::effects::SkillEffect::SummonPet],
        "SummonCubic" => vec![skill::effects::SkillEffect::SummonCubic {
            cubic_id: param("cubicId").unwrap_or(-1.0) as i32,
            cubic_level: param("cubicLvl").unwrap_or(0.0) as i32,
        }],
        // Java's default here is **-1**, not 100: a negative
        // chance means "always", which is what Erase relies on.
        "Unsummon" => vec![skill::effects::SkillEffect::Unsummon {
            chance: param("chance").unwrap_or(-1.0) as i32,
        }],
        // `CallPc.java`. `itemId`/`itemCount` are the Summon
        // Friend toll, charged to the **target**; the monster
        // half reads neither and every monster carrier omits
        // them, which is why they default to 0.
        "CallPc" => vec![skill::effects::SkillEffect::CallPc {
            item_id: value_at(params, "itemId", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            item_count: value_at(params, "itemCount", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        }],
        "CallParty" => vec![skill::effects::SkillEffect::CallParty],
        "ImmobilePetBuff" => vec![skill::effects::SkillEffect::ImmobilePetBuff],
        "Grow" => vec![skill::effects::SkillEffect::Grow],
        _ => return None,
    })
}
