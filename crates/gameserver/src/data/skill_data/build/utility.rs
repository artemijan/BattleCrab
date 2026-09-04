//! Effects with a world-side action rather than a stat or an abnormal:
//! teleports, doors, recipe books, appearance changes, clan messages.

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
            vec![skill::effects::SkillEffect::Escape {
                dest: match value_at(params, "escapeType", level) {
                    Some("CLANHALL") => skill::effects::EscapeDest::ClanHall,
                    Some("CASTLE") => skill::effects::EscapeDest::Castle,
                    _ => skill::effects::EscapeDest::Town,
                },
            }]
        }
        // Fixed-destination teleports — the Scrolls of Escape.
        // Coordinates are per *level*: skill 2213 alone carries
        // 22 towns, one per level.
        "Teleport" => vec![skill::effects::SkillEffect::Teleport {
            x: param("x").unwrap_or(0.0) as i32,
            y: param("y").unwrap_or(0.0) as i32,
            z: param("z").unwrap_or(0.0) as i32,
        }],
        "TeleportToTarget" => vec![skill::effects::SkillEffect::TeleportToTarget],
        "GiveSp" => vec![skill::effects::SkillEffect::GiveSp {
            sp: param("sp").unwrap_or(0.0) as i64,
        }],
        // Java throws if amount is 0/missing; we drop the effect
        // (silent no-op) to match how other bad effect bodies fall
        // through, rather than panicking at data-load.
        "GiveRecommendation" => match param("amount") {
            Some(amount) if amount != 0.0 => {
                vec![skill::effects::SkillEffect::GiveRecommendation {
                    amount: amount as i32,
                }]
            }
            _ => Vec::new(),
        },
        "SetSkill" => vec![skill::effects::SkillEffect::SetSkill {
            skill_id: value_at(params, "skillId", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            // Java defaults this to 1, not 0.
            skill_level: value_at(params, "skillLevel", level)
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        }],
        // Java `params.getInt("sp", 0)` — an int on the XML, but
        // the award path takes the same i64 as every other SP
        // grant.
        // The three appearance potions, one variant.
        "ChangeFace" => vec![skill::effects::SkillEffect::ChangeAppearance {
            part: skill::effects::AppearancePart::Face,
            value: param("value").unwrap_or(0.0) as i32,
        }],
        "ChangeHairStyle" => vec![skill::effects::SkillEffect::ChangeAppearance {
            part: skill::effects::AppearancePart::HairStyle,
            value: param("value").unwrap_or(0.0) as i32,
        }],
        "ChangeHairColor" => vec![skill::effects::SkillEffect::ChangeAppearance {
            part: skill::effects::AppearancePart::HairColor,
            value: param("value").unwrap_or(0.0) as i32,
        }],
        "SendSystemMessageToClan" => {
            vec![skill::effects::SkillEffect::SendSystemMessageToClan {
                message_id: param("id").unwrap_or(0.0) as i16,
            }]
        }
        // Java's `chance` default is 0 — a door skill with no
        // `<chance>` never opens anything. Unlock declares one
        // at every level, so the default is only a guard.
        "OpenDoor" => vec![skill::effects::SkillEffect::OpenDoor {
            chance: param("chance").unwrap_or(0.0) as i32,
            is_item: value_at(params, "isItem", level) == Some("true"),
        }],
        "OpenChest" => vec![skill::effects::SkillEffect::OpenChest],
        // "Common Craft" (1322) / "Dwarven Craft" (1321): param-less
        // self-closing effects whose whole job is to open the recipe
        // window. Without these arms both skills parsed to zero
        // effects and the cast did nothing.
        "OpenCommonRecipeBook" => {
            vec![skill::effects::SkillEffect::OpenRecipeBook { dwarven: false }]
        }
        "OpenDwarfRecipeBook" => {
            vec![skill::effects::SkillEffect::OpenRecipeBook { dwarven: true }]
        }
        // Both the basic (247) and advanced (326) HQ skills
        // carry this; only 326 sets `<isAdvanced>true</…>`,
        // which halves the flag's incoming damage.
        "HeadquarterCreate" => {
            vec![skill::effects::SkillEffect::CreateHeadquarter {
                advanced: value_at(params, "isAdvanced", level)
                    .is_some_and(|v| v.eq_ignore_ascii_case("true")),
            }]
        }
        "Flag" => vec![skill::effects::SkillEffect::PvpFlag],
        // `CallSkill.java` — cast another skill outright.
        // Java's `skillLevel` default is 1; a declared 0 means
        // "the effector's own learned level", and
        // `skillLevelScaleTo` scales off an existing buff —
        // neither is used by any reachable carrier here.
        "CallSkill" => match param("skillId") {
            Some(sid) if sid > 0.0 => {
                vec![skill::effects::SkillEffect::CallSkill {
                    skill_id: sid as i32,
                    skill_level: param("skillLevel").unwrap_or(1.0).max(1.0) as i32,
                    chance: param("chance").unwrap_or(100.0) as i32,
                }]
            }
            _ => Vec::new(),
        },
        _ => return None,
    })
}
