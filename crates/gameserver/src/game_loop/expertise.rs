//! Port of `Player.refreshExpertisePenalty` (gated on `Config.ExpertisePenalty`).
//!
//! Equipping a weapon or piece of armor whose grade is higher than the grade
//! the character may wear (`Player.getExpertiseLevel`, the level of the
//! `EXPERTISE` skill) applies a grade-penalty debuff — the weapon penalty skill
//! `6209` and/or the armor penalty skill `6213`, each at a level of 1-4 for how
//! many grades over the character is. Those passive skills carry the actual
//! stat malus (accuracy/atk-speed/speed/… down); the penalty level is also sent
//! to the client in `EtcStatusUpdate` so it can draw the penalty icons.

use crate::data::item_data::ItemKind;
use crate::model::components::{BaseStats, Buffs, CombatStats, ExpertisePenalty, SkillBook, Speeds, StatModifiers};
use crate::model::inventory::Inventory;
use crate::model::skill::{ActiveBuff, StatModifierEffect};
use crate::model::Player;
use crate::world::World;

/// Java `CommonSkill.WEAPON_GRADE_PENALTY`.
const WEAPON_GRADE_PENALTY: i32 = 6209;
/// Java `CommonSkill.ARMOR_GRADE_PENALTY`.
const ARMOR_GRADE_PENALTY: i32 = 6213;
/// Java `CommonSkill.EXPERTISE` — the skill whose level *is* the highest grade
/// the character may wear without penalty (`Player.getExpertiseLevel`).
const EXPERTISE_SKILL: i32 = 239;

/// Port of `Player.refreshExpertisePenalty`. Recomputes the weapon/armor grade
/// penalties from currently-equipped gear versus the character's expertise
/// level, (re)applies the penalty debuff skills, and resends `EtcStatusUpdate`
/// and `UserInfo` when the penalty changed. No-op when `Config.ExpertisePenalty`
/// is off (Java returns early) or when nothing changed. Call after any
/// equip/unequip and at enter-world.
pub(crate) fn refresh_expertise_penalty(world: &mut World, object_id: i32) {
    if !world.cfg.character.expertise_penalty {
        return;
    }

    // --- read phase: expertise level + highest over-grade weapon/armor. ---
    let (weapon_penalty, armor_penalty) = {
        let expertise = world
            .objects
            .get_component::<SkillBook>(&object_id)
            .and_then(|s| s.0.get(&EXPERTISE_SKILL).copied())
            .unwrap_or(0)
            .max(0);
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        // Java: weapons feed the weapon penalty, everything else the armor
        // penalty (arrows/bolts are `EtcItem`s and never equip into the
        // paperdoll here, so no explicit skip is needed).
        let equipped = inventory.equipped_items().into_iter().filter_map(|item| {
            let t = world.data.item_data.get(item.item_id)?;
            Some((t.kind == ItemKind::Weapon, t.crystal_type.level()))
        });
        compute_penalties(expertise, equipped)
    };

    let current = world.objects.get_component::<ExpertisePenalty>(&object_id).copied().unwrap_or_default();
    if current.weapon == weapon_penalty && current.armor == armor_penalty {
        return;
    }

    // Clone the penalty skills' stat effects before borrowing objects mutably.
    let weapon_effects = penalty_effects(world, WEAPON_GRADE_PENALTY, weapon_penalty);
    let armor_effects = penalty_effects(world, ARMOR_GRADE_PENALTY, armor_penalty);

    // --- apply phase: swap the penalty buffs, recompute stats. Scoped so the
    // entity-store borrow ends before the component/packet work below. ---
    {
        let Some((player, base, mut mods, mut buffs, mut speeds, mut combat)) = world.objects.get_many_mut::<(
            &Player,
            &BaseStats,
            &mut StatModifiers,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&object_id) else {
            return;
        };
        // `remove_buff` rebuilds the modifier maps from the *remaining* buffs,
        // so removing then re-adding at the new level leaves no stale penalty
        // modifiers (a no-op when the penalty wasn't present).
        player.remove_buff(&world.data, base, &mut mods, &mut buffs, &mut speeds, &mut combat, WEAPON_GRADE_PENALTY);
        player.remove_buff(&world.data, base, &mut mods, &mut buffs, &mut speeds, &mut combat, ARMOR_GRADE_PENALTY);
        if let Some((level, effects)) = weapon_effects {
            player.apply_buff(&world.data, base, &mut mods, &mut buffs, &mut speeds, &mut combat, passive_penalty_buff(WEAPON_GRADE_PENALTY, level, effects));
        }
        if let Some((level, effects)) = armor_effects {
            player.apply_buff(&world.data, base, &mut mods, &mut buffs, &mut speeds, &mut combat, passive_penalty_buff(ARMOR_GRADE_PENALTY, level, effects));
        }
    }

    if let Some(ep) = world.objects.get_component_mut::<ExpertisePenalty>(&object_id) {
        ep.weapon = weapon_penalty;
        ep.armor = armor_penalty;
    }

    // Java sends `EtcStatusUpdate` on change; the stat change (Java's
    // addSkill/removeSkill → broadcastUserInfo) also needs a fresh `UserInfo`.
    if let Some(client_id) = crate::game_loop::helpers::client_for_player(world, object_id) {
        if let Some(view) = crate::model::PlayerView::of(&world.objects, object_id) {
            let user_info = crate::network::user_info::user_info(&view, &world.data, &world.cfg.character);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(crate::network::enter_world::etc_status_update(weapon_penalty, armor_penalty));
                cs.send(user_info);
            }
        }
    }
}

/// Pure port of the penalty arithmetic in `refreshExpertisePenalty`: given the
/// character's expertise level and each equipped item's `(is_weapon, grade)`,
/// return the `(weapon, armor)` penalty levels. Only over-grade items count;
/// the penalty is "how many grades over" the highest such item is, clamped to
/// 0-4 (Java also subtracts an expertise bonus that is always 0 here).
fn compute_penalties(expertise: i32, equipped: impl IntoIterator<Item = (bool, i32)>) -> (i32, i32) {
    let mut weapon_grade = 0;
    let mut armor_grade = 0;
    for (is_weapon, grade) in equipped {
        if grade > expertise {
            if is_weapon {
                weapon_grade = weapon_grade.max(grade);
            } else {
                armor_grade = armor_grade.max(grade);
            }
        }
    }
    ((weapon_grade - expertise).clamp(0, 4), (armor_grade - expertise).clamp(0, 4))
}

/// The penalty skill's continuous stat effects at `level`, paired with the
/// level — or `None` when `level == 0` (no penalty) or the skill level isn't
/// loaded. Cloned out so the caller can drop the `GameData` borrow before
/// mutating the entity store.
fn penalty_effects(world: &World, skill_id: i32, level: i32) -> Option<(i32, Vec<StatModifierEffect>)> {
    if level <= 0 {
        return None;
    }
    let skill = world.data.skill_data.get(skill_id, level)?;
    Some((level, skill.stat_modifier_effects()))
}

/// A grade-penalty `ActiveBuff`: `passive` (hidden from `AbnormalStatusUpdate`,
/// never scheduled to expire) and permanent while the gear stays equipped.
fn passive_penalty_buff(skill_id: i32, level: i32, effects: Vec<StatModifierEffect>) -> ActiveBuff {
    ActiveBuff {
        skill_id,
        skill_level: level,
        abnormal_type_client_id: -1,
        expires_at_tick: u64::MAX,
        passive: true,
        effects,
    }
}

#[cfg(test)]
mod tests {
    use super::compute_penalties;

    // Grades: D=1, C=2, B=3, A=4, S=5 (CrystalType::level).
    const D: i32 = 1;
    const C: i32 = 2;
    const B: i32 = 3;

    #[test]
    fn no_penalty_when_gear_is_at_or_below_expertise() {
        // Expertise C (2); wearing a C weapon + D armor — nothing over-grade.
        assert_eq!(compute_penalties(C, [(true, C), (false, D)]), (0, 0));
    }

    #[test]
    fn penalty_is_grades_over_expertise_per_slot_kind() {
        // Expertise D (1); a B weapon (3) is 2 grades over → 2, a C armor (2)
        // is 1 grade over → 1.
        assert_eq!(compute_penalties(D, [(true, B), (false, C)]), (2, 1));
    }

    #[test]
    fn only_the_highest_over_grade_item_of_each_kind_counts() {
        // Two armor pieces (C and B) over a D character → the B one (2 over).
        assert_eq!(compute_penalties(D, [(false, C), (false, B)]), (0, 2));
    }

    #[test]
    fn penalty_clamps_to_four() {
        // No expertise (0) wearing an S-grade weapon (5) → clamps at 4.
        assert_eq!(compute_penalties(0, [(true, 5)]), (4, 0));
    }
}
