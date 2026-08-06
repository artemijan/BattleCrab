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
//! The option's **active** skills (Java `addSkill`) are granted into the
//! `SkillBook` while the item is worn — they show on the skill bar and cast
//! like any known skill — and removed on unequip; like Java (`Options.
//! addSkill` never stores), they are re-derived from the worn augments at
//! login, with a stale-cleanup pass for ids the book carries but no worn
//! option grants (the cursed-weapon pattern). The **activation** skills
//! (`attack_skill` / `magic_skill` / `critical_skill` → Java
//! `Player.addTriggerSkill`) live on the [`AugmentTriggers`] registry and
//! fire from the two Java consumption sites: the auto-attack hit (ATTACK on
//! a plain hit, CRITICAL on a crit — `Creature.onHitTimer`) and the cast
//! launch (MAGIC for magic skills, ATTACK for physical — `SkillCaster`).

use crate::data::option_data::OptionSkillType;
use crate::model::Player;
use crate::model::components::{
    AugmentTriggers, BaseStats, Buffs, CombatStats, SkillBook, Speeds, StatModifiers,
};
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
        apply_option_skills(world, player_oid, option_id);
    }
}

/// Java `VariationInstance.removeBonus` — drop both options of an item that was
/// just unequipped (or whose augmentation was cancelled).
pub(crate) fn remove_item_options(world: &mut World, player_oid: i32, item_object_id: i32) {
    for option_id in option_ids_of(world, player_oid, item_object_id) {
        remove_option(world, player_oid, option_id);
        remove_option_skills(world, player_oid, option_id);
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

/// The skill/trigger half of `Options.apply`: grant the active skills into
/// the book and register the activation skills. Split from [`apply_option`]
/// so the stat half stays a pure buff.
fn apply_option_skills(world: &mut World, player_oid: i32, option_id: i32) {
    let Some(entry) = world.data.options.get(option_id) else {
        return;
    };
    let actives = entry.active_skills.clone();
    let triggers = entry.triggers.clone();
    let mut granted = false;
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        for (id, level) in actives {
            // Java `addSkill` replaces; a lower level never downgrades one
            // the player somehow knows higher.
            let e = book.0.entry(id).or_insert(0);
            if *e < level {
                *e = level;
                granted = true;
            }
        }
    }
    if !triggers.is_empty() {
        if world
            .objects
            .get_component::<AugmentTriggers>(&player_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&player_oid, AugmentTriggers::default());
        }
        if let Some(reg) = world
            .objects
            .get_component_mut::<AugmentTriggers>(&player_oid)
        {
            for t in triggers {
                reg.0.push((option_id, t));
            }
        }
    }
    if granted {
        super::admin::refresh_skill_list(world, player_oid);
    }
}

/// The skill/trigger half of `Options.remove`.
fn remove_option_skills(world: &mut World, player_oid: i32, option_id: i32) {
    let Some(entry) = world.data.options.get(option_id) else {
        return;
    };
    let actives = entry.active_skills.clone();
    let mut removed = false;
    for (id, _) in actives {
        if world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .is_some_and(|b| b.0.contains_key(&id))
        {
            super::skills::remove_player_skill(world, player_oid, id);
            removed = true;
        }
    }
    if let Some(reg) = world
        .objects
        .get_component_mut::<AugmentTriggers>(&player_oid)
    {
        reg.0.retain(|(oid, _)| *oid != option_id);
    }
    if removed {
        super::admin::refresh_skill_list(world, player_oid);
    }
}

/// Login re-derivation (Java's equip listeners ran during `restoreInventory`):
/// fold every worn augmented item's options in — stats, actives, triggers —
/// and sweep book entries that are option actives no worn option grants (the
/// item was de-augmented or the data changed while the row persisted).
pub(crate) fn apply_worn_options_at_login(world: &mut World, player_oid: i32) {
    let worn: Vec<i32> = world
        .objects
        .get_component::<Inventory>(&player_oid)
        .map(|inv| {
            inv.items()
                .iter()
                .filter(|it| {
                    inv.paperdoll_slot_of(it.object_id).is_some()
                        && (it.augment_option1 != 0 || it.augment_option2 != 0)
                })
                .map(|it| it.object_id)
                .collect()
        })
        .unwrap_or_default();
    let mut granted_ids: Vec<i32> = Vec::new();
    for item_oid in worn {
        for option_id in option_ids_of(world, player_oid, item_oid) {
            if let Some(e) = world.data.options.get(option_id) {
                granted_ids.extend(e.active_skills.iter().map(|&(id, _)| id));
            }
            apply_option(world, player_oid, option_id);
            apply_option_skills(world, player_oid, option_id);
        }
    }
    // Stale sweep: any augment-active id in the book that no worn option
    // grants was persisted past its item — drop it (the curse pattern).
    let all_option_actives = world.data.options.all_active_skill_ids();
    let stale: Vec<i32> = world
        .objects
        .get_component::<SkillBook>(&player_oid)
        .map(|b| {
            b.0.keys()
                .filter(|id| all_option_actives.contains(id) && !granted_ids.contains(id))
                .copied()
                .collect()
        })
        .unwrap_or_default();
    for id in stale {
        super::skills::remove_player_skill(world, player_oid, id);
    }
}

// ---------------------------------------------------------------------------
// Trigger firing (Java's two `getTriggerSkills` consumption sites)
// ---------------------------------------------------------------------------

/// `Creature.onHitTimer`'s trigger block: a plain hit fires ATTACK-type
/// activation skills, a critical fires CRITICAL-type — each at
/// `Rnd.get(100) < chance`, cast at the hit target with no cast time.
pub(crate) fn fire_augment_attack_triggers(
    world: &mut World,
    attacker: i32,
    target: i32,
    crit: bool,
) {
    fire_triggers(world, attacker, target, |kind| {
        (!crit && kind == OptionSkillType::Attack) || (crit && kind == OptionSkillType::Critical)
    });
}

/// `SkillCaster`'s trigger block, inside `!skill.isStatic()`: a magic cast
/// fires MAGIC-type activation skills, a physical one ATTACK-type.
pub(crate) fn fire_augment_cast_triggers(
    world: &mut World,
    caster: i32,
    target: i32,
    magic_type: i32,
) {
    if magic_type == 2 {
        return; // static skills never trigger (Java's `!skill.isStatic()`)
    }
    let magic = magic_type == 1;
    fire_triggers(world, caster, target, |kind| {
        (magic && kind == OptionSkillType::Magic) || (!magic && kind == OptionSkillType::Attack)
    });
}

fn fire_triggers(
    world: &mut World,
    caster: i32,
    target: i32,
    wants: impl Fn(OptionSkillType) -> bool,
) {
    let Some(reg) = world.objects.get_component::<AugmentTriggers>(&caster) else {
        return;
    };
    let candidates: Vec<(i32, i32, f64)> = reg
        .0
        .iter()
        .filter(|(_, t)| wants(t.kind))
        .map(|(_, t)| (t.skill_id, t.skill_level, t.chance))
        .collect();
    for (skill_id, skill_level, chance) in candidates {
        // Java `Rnd.get(100) < holder.getChance()`.
        if f64::from(world.roll(100)) >= chance {
            continue;
        }
        let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
            continue;
        };
        // `SkillCaster.triggerCast` — no cast time, no MP, no reuse.
        super::skills::effects::apply_skill_effects(world, caster, target, &skill);
    }
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
