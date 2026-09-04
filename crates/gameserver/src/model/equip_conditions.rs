//! The `<condition>` gates on a passive skill's stat modifiers — Java's
//! `ConditionUsingItemType` / `ConditionUsingSlotType` armour and weapon
//! checks, applied when composing a creature's modifier set.

use crate::data::GameData;

use super::components::skills::SkillBook;
use super::inventory::Inventory;
use super::skill::active_buff::ActiveBuff;
use super::skill::effects::StatModifierEffect;

/// Port of `ConditionUsingItemType.testImpl`'s armor branch (the only branch a
/// robe passive's `<armorType>` mask reaches): the condition passes when the
/// worn chest — and, unless the chest is full-armor, the worn legs — matches the
/// mask, treating a bare slot as `ArmorType::NONE`.
pub(crate) fn armor_condition_passes(
    mask: u8,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::data::item_data::SLOT_FULL_ARMOR;
    use crate::data::item_data::kinds::ArmorType;
    use crate::model::inventory::PaperdollSlot;
    const NONE_BIT: u8 = ArmorType::None.mask_bit();
    let Some(chest) = inventory.paperdoll_item(PaperdollSlot::Chest) else {
        return mask & NONE_BIT != 0;
    };
    if mask & items.armor_type(chest.item_id).mask_bit() == 0 {
        return false;
    }
    if items
        .get(chest.item_id)
        .map(|t| t.body_part == SLOT_FULL_ARMOR)
        .unwrap_or(false)
    {
        return true;
    }
    let Some(legs) = inventory.paperdoll_item(PaperdollSlot::Legs) else {
        return mask & NONE_BIT != 0;
    };
    mask & items.armor_type(legs.item_id).mask_bit() != 0
}

/// Whether a skill effect's `<weaponType>` condition (`mask`, an OR of
/// `WeaponType::mask_bit`s) is satisfied by the currently equipped weapon. No
/// weapon (or a type not in the mask) → `false`, so e.g. Weapon Mastery 249's
/// `-30% MagicalAttackSpeed` only bites a BOW/POLE user, not a staff caster.
/// Java `ConditionUsingSlotType(ItemTemplate.SLOT_LR_HAND)` — the equipped
/// weapon occupies **both** hands.
///
/// Read off the weapon template's `bodypart`, which is how the datapack marks
/// a two-hander, rather than by inferring it from the left hand being empty
/// (that would also match an unarmed or shield-less one-hander).
pub(crate) fn two_handed_weapon_equipped(
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::model::inventory::PaperdollSlot;
    inventory
        .paperdoll_item(PaperdollSlot::RHand)
        .and_then(|w| items.get(w.item_id))
        .is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
}

pub(crate) fn weapon_condition_passes(
    mask: u32,
    inventory: &Inventory,
    items: &crate::data::item_data::ItemData,
) -> bool {
    use crate::model::inventory::PaperdollSlot;
    let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand) else {
        return false;
    };
    mask & items.weapon_type(weapon.item_id).mask_bit() != 0
}

/// The passive buffs a player's skill book contributes **right now**, with each
/// effect's own conditions evaluated against the state they name.
///
/// `hp_percent_now` is Java's `effected.getCurrentHpPercent()` —
/// `(int) ((currentHp * 100) / maxHp)` — read by the
/// `AbstractConditionalHpEffect` family. It is a parameter rather than a
/// component read because this runs from `Player::from_char`, before the
/// entity exists.
pub(crate) fn conditioned_passive_buffs(
    data: &GameData,
    skills: &SkillBook,
    inventory: &Inventory,
    hp_percent_now: i32,
) -> Vec<ActiveBuff> {
    use crate::model::skill::effects::SkillEffect;
    use crate::model::skill::target::OperateType;
    let mut out = Vec::new();
    for (&skill_id, &level) in &skills.0 {
        let Some(skill) = data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        // Java `checkConditions(PASSIVE, …)` — a passive whose own
        // `<passiveConditions>` don't hold contributes nothing (G34 S1).
        if !crate::game_loop::skills::conditions::passive_stat_gate(
            skill,
            inventory,
            &data.item_data,
        ) {
            continue;
        }
        let applicable: Vec<StatModifierEffect> = skill
            .effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                _ => None,
            })
            .filter(|m| {
                (m.armor_condition == 0 || armor_condition_passes(m.armor_condition, inventory, &data.item_data))
                    && (m.weapon_condition == 0 || weapon_condition_passes(m.weapon_condition, inventory, &data.item_data))
                    // `ConditionUsingSlotType(SLOT_LR_HAND)` — a *separate*
                    // axis from the weapon type: the same blunt bonus is off
                    // while a one-handed mace is equipped.
                    && (!m.two_handed || two_handed_weapon_equipped(inventory, &data.item_data))
                    // `AbstractConditionalHpEffect.canPump`:
                    // `(_hpPercent <= 0) || (effected.getCurrentHpPercent() <= _hpPercent)`.
                    && (m.hp_percent <= 0 || hp_percent_now <= m.hp_percent)
            })
            .collect();
        if applicable.is_empty() {
            continue;
        }
        out.push(ActiveBuff::passive_pump(skill_id, level, applicable));
    }
    out
}
