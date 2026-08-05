//! Augment option bonuses at runtime — Java `VariationInstance.applyBonus` /
//! `removeBonus` (`Options.apply`/`remove`), fired from the equip/unequip
//! listeners in `Inventory.equipItem`/`unEquipItemInBodySlot`.
//!
//! An augmented item carries two option ids ([`crate::data::option_data`]).
//! While it is **equipped**, each option's stat effects are folded into the
//! wearer's modifier maps, exactly as Java folds them in as a `BuffInfo` on the
//! effect list. The port carries them as **passive buffs** (the same mechanism
//! the grade-penalty and clan-skill pumps use), keyed by a synthetic negative
//! skill id so they can be removed again and never collide with a real skill.
//!
//! Also applied: the option's **passive skills**, whose own stat effects are
//! folded the same way (a passive skill is a stat pump with no icon).
//!
//! TODO(G15.5): the option's **active** skills (Java `addSkill` → they appear
//! on the skill bar) and its **activation** skills (`attack_skill` /
//! `magic_skill` / `critical_skill` → `Player.addTriggerSkill`) are parsed and
//! carried on the entry but not yet granted; both need a temporary-skill grant
//! path (`SkillBook` + `SkillList`) and a trigger registry keyed by something
//! other than a learned skill, which the port doesn't have yet.

use crate::model::Player;
use crate::model::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers};
use crate::model::inventory::Inventory;
use crate::model::skill::{ActiveBuff, BuffSlot, StatModifierEffect};
use crate::world::World;

/// Option buffs live at `OPTION_BUFF_ID_BASE - option_id`, well below any real
/// skill id, so `remove_buff` can target exactly one option's contribution.
const OPTION_BUFF_ID_BASE: i32 = -1_000_000;

fn option_buff_id(option_id: i32) -> i32 {
    OPTION_BUFF_ID_BASE - option_id
}

/// Java `VariationInstance.applyBonus` — fold both options of a newly equipped
/// augmented item into the wearer. No-op for an unaugmented item.
pub(crate) fn apply_item_options(world: &mut World, player_oid: i32, item_object_id: i32) {
    for option_id in option_ids_of(world, player_oid, item_object_id) {
        apply_option(world, player_oid, option_id);
    }
}

/// Java `VariationInstance.removeBonus` — drop both options of an item that was
/// just unequipped (or whose augmentation was cancelled).
pub(crate) fn remove_item_options(world: &mut World, player_oid: i32, item_object_id: i32) {
    for option_id in option_ids_of(world, player_oid, item_object_id) {
        remove_option(world, player_oid, option_id);
    }
}

/// The two option ids of an inventory instance, empty when it isn't augmented.
fn option_ids_of(world: &World, player_oid: i32, item_object_id: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<Inventory>(&player_oid)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|it| it.object_id == item_object_id)
                .map(|it| (it.augment_option1, it.augment_option2))
        })
        .map(|(a, b)| [a, b].into_iter().filter(|&id| id != 0).collect())
        .unwrap_or_default()
}

/// `Options.apply(player)`, narrowed to the stat half: the option's own effects
/// plus every effect of its passive skills.
fn apply_option(world: &mut World, player_oid: i32, option_id: i32) {
    let Some(effects) = option_effects(world, option_id) else {
        return;
    };
    if effects.is_empty() {
        return;
    }
    let buff = ActiveBuff {
        displayed: true,
        skill_id: option_buff_id(option_id),
        skill_level: 1,
        abnormal_type_client_id: 0,
        abnormal_type: String::new(),
        abnormal_level: 0,
        slot: BuffSlot::Uncapped,
        expires_at_tick: 0,
        passive: true,
        effect_flags: 0,
        abnormal_visuals: Vec::new(),
        blocked_abnormals: Vec::new(),
        effects,
    };
    if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &mut Player,
            &BaseStats,
            &mut StatModifiers,
            &Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&player_oid)
    {
        target.apply_buff(
            &world.data,
            base,
            &mut mods,
            inventory,
            &mut buffs,
            &mut speeds,
            &mut combat,
            buff,
        );
    }
    // MaxHp/MaxMp/MaxCp options don't reach `recalculate_stats`, which is why
    // the buff paths all follow up with this.
    super::skills::effects::recompute_max_vitals(world, player_oid);
}

/// `Options.remove(player)` — drop this option's contribution and rebuild.
fn remove_option(world: &mut World, player_oid: i32, option_id: i32) {
    let skill_id = option_buff_id(option_id);
    if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &mut Player,
            &BaseStats,
            &mut StatModifiers,
            &Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&player_oid)
    {
        target.remove_buff(
            &world.data,
            base,
            &mut mods,
            inventory,
            &mut buffs,
            &mut speeds,
            &mut combat,
            skill_id,
        );
    }
    super::skills::effects::recompute_max_vitals(world, player_oid);
}

/// An option's stat contributions: its own `<effects>` plus the effects of the
/// passive skills it grants (Java applies both through the same effect list).
fn option_effects(world: &World, option_id: i32) -> Option<Vec<StatModifierEffect>> {
    use crate::model::skill::{OperateType, SkillEffect};

    let entry = world.data.options.get(option_id)?;
    let mut effects = entry.effects.clone();
    for &(skill_id, level) in &entry.passive_skills {
        let Some(skill) = world.data.skill_data.get(skill_id, level) else {
            continue;
        };
        if skill.operate_type != OperateType::Passive {
            continue;
        }
        for effect in &skill.effects {
            if let SkillEffect::StatModifier(m) = effect {
                effects.push(*m);
            }
        }
    }
    Some(effects)
}
