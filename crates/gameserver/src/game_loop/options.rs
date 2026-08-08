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
//! Also applied, since G15.5: the option's **active** skills (Java `addSkill`
//! → they appear on the skill bar) and its **activation** skills
//! (`attack_skill` / `magic_skill` / `critical_skill` → `addTriggerSkill`).
//!
//! Both live in their own transient components rather than the
//! [`crate::model::components::SkillBook`], because Java grants them with
//! `store = false` and this port persists the whole book — an option skill
//! filed there would outlive the item. See
//! [`crate::model::components::OptionSkills`] /
//! [`crate::model::components::OptionTriggers`].

use crate::game_loop::stat_ctx::with_stat_ctx;
use crate::model::components::{OptionSkills, OptionTriggers};
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

/// Every currently-equipped item's options, applied at enter-world.
///
/// Java reaches this through `restoreCharData` → the equip listeners; there is
/// no equivalent replay here, so it is done explicitly. Idempotent by
/// construction: each option lands as one buff keyed by
/// [`option_buff_id`], and the skill grants are map inserts.
pub(crate) fn apply_equipped_item_options(world: &mut World, player_oid: i32) {
    let equipped: Vec<i32> = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|it| it.is_augmented() && inv.paperdoll_slot_of(it.object_id).is_some())
                .map(|it| it.object_id)
                .collect()
        })
        .unwrap_or_default();
    for item_oid in equipped {
        apply_item_options(world, player_oid, item_oid);
    }
}

/// Java `VariationInstance.removeBonus` — drop both options of an item that was
/// just unequipped (or whose augmentation was cancelled).
///
/// Reads the ids off the instance **in the bag**, so this only works while the
/// item is still there. A destroyed item is already gone by the time anything
/// can react to it — use [`remove_option_ids`] with ids captured beforehand.
pub(crate) fn remove_item_options(world: &mut World, player_oid: i32, item_object_id: i32) {
    remove_option_ids(
        world,
        player_oid,
        &option_ids_of(world, player_oid, item_object_id),
    );
}

/// [`remove_item_options`] for an item that has already left the inventory:
/// the option ids come from the caller's snapshot instead of a lookup.
///
/// Destroying a *worn* augmented item used to leak its bonuses permanently —
/// the removal ran, took the "unequipped" branch, and then found no instance to
/// read option ids from, so it removed nothing. Zeroes are ignored, so an
/// unaugmented item's `[0, 0]` is a no-op.
pub(crate) fn remove_option_ids(world: &mut World, player_oid: i32, option_ids: &[i32]) {
    for &option_id in option_ids.iter().filter(|&&id| id != 0) {
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
    // The skill halves run first and unconditionally: an option can grant an
    // active skill or a trigger while contributing no stats at all (1793
    // `<active_skill>` rows on this dist, and plenty of options carry nothing
    // else), so they must not sit behind the `effects.is_empty()` bail below.
    apply_option_skills(world, player_oid, option_id);

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
    with_stat_ctx(world, player_oid, |ctx| ctx.apply(buff));
    // MaxHp/MaxMp/MaxCp options don't reach `recalculate_stats`, which is why
    // the buff paths all follow up with this.
    super::skills::effects::recompute_max_vitals(world, player_oid);
}

/// `Options.remove(player)` — drop this option's contribution and rebuild.
fn remove_option(world: &mut World, player_oid: i32, option_id: i32) {
    remove_option_skills(world, player_oid, option_id);
    let skill_id = option_buff_id(option_id);
    with_stat_ctx(world, player_oid, |ctx| ctx.remove(skill_id));
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

/// `Options.apply`'s skill half — the active skills and the activation
/// (trigger) skills.
///
/// Java grants the actives with `addSkill(skill, false)`; the `false` is
/// `store`, so they never reach `character_skills`. That distinction is
/// load-bearing here, where the whole [`crate::model::components::SkillBook`]
/// is persisted: they go into [`OptionSkills`] instead.
///
/// The reuse-timestamp restore Java does alongside (`getSkillRemainingReuseTime`
/// → `addTimeStamp`/`disableSkill` + `SkillCoolTime`) is a no-op here: this
/// port keeps reuse entries keyed by skill id in `Reuses` whether or not the
/// skill is currently known, so a cooldown already survives unequip/re-equip
/// without being re-armed.
fn apply_option_skills(world: &mut World, player_oid: i32, option_id: i32) {
    let Some(entry) = world.data.options.get(option_id) else {
        return;
    };
    let actives = entry.active_skills.clone();
    let triggers = entry.triggers.clone();
    if actives.is_empty() && triggers.is_empty() {
        return;
    }
    if let Some(skills) = world.objects.get_component_mut::<OptionSkills>(&player_oid) {
        for &(skill_id, level) in &actives {
            skills.0.insert(skill_id, level);
        }
    }
    if let Some(reg) = world
        .objects
        .get_component_mut::<OptionTriggers>(&player_oid)
    {
        // Java's `_triggerSkills` is keyed by the *triggered* skill's id, so a
        // second option granting the same proc replaces rather than doubles it.
        for trigger in &triggers {
            reg.0.insert(trigger.skill_id, *trigger);
        }
    }
    send_skill_list(world, player_oid);
}

/// `Options.remove`'s skill half.
///
/// **Java's own quirk, kept:** the removal is unconditional, so when two
/// equipped items carry options granting the *same* skill, unequipping either
/// one takes the skill away even though the other still grants it. Re-equipping
/// anything restores it. Reproducing this is deliberate — the alternative is
/// refcounting that Java does not do, which would diverge on a case the
/// datapack can actually produce.
fn remove_option_skills(world: &mut World, player_oid: i32, option_id: i32) {
    let Some(entry) = world.data.options.get(option_id) else {
        return;
    };
    let actives = entry.active_skills.clone();
    let triggers = entry.triggers.clone();
    if actives.is_empty() && triggers.is_empty() {
        return;
    }
    if let Some(skills) = world.objects.get_component_mut::<OptionSkills>(&player_oid) {
        for (skill_id, _) in &actives {
            skills.0.remove(skill_id);
        }
    }
    if let Some(reg) = world
        .objects
        .get_component_mut::<OptionTriggers>(&player_oid)
    {
        for trigger in &triggers {
            reg.0.remove(&trigger.skill_id);
        }
    }
    send_skill_list(world, player_oid);
}

/// `player.sendSkillList()`, which both halves of `Options` end with — the
/// client only shows a granted active once the list is resent.
fn send_skill_list(world: &World, player_oid: i32) {
    if let Some(pkt) = super::helpers::skill_list_packet(world, player_oid)
        && let Some(cs) =
            super::helpers::client_for_player(world, player_oid).and_then(|c| world.clients.get(&c))
    {
        cs.send(pkt);
    }
}
