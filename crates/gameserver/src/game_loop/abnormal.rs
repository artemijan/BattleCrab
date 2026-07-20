//! Abnormal-state flags — the crowd-control gates.
//!
//! Java keeps a cached `EffectFlag` bitmask on `EffectList`, recomputed on
//! every effect add/remove (`computeEffectFlags`), and reads it through
//! `Creature.isAffected(flag)`. This port stamps each [`ActiveBuff`] with the
//! flags its skill contributes ([`Skill::effect_flags`]) and ORs the live buff
//! list here instead: the same answer with no cached value that could go stale
//! across the several places buffs are added and removed (skill application,
//! NPC buffs, expiry, dispel, toggle-off).
//!
//! The three predicates below are the ported subset of `Creature`'s gate
//! family:
//!
//! | Java | here | blocks |
//! |---|---|---|
//! | `hasBlockActions()` | [`is_blocked_from_actions`] | attack, cast, move |
//! | `isRooted()` | (folded into the below) | move |
//! | `isMovementDisabled()` | [`is_movement_disabled`] | move |
//!
//! Java's `isMovementDisabled` also ORs `_isOverloaded`, `_isImmobilized`,
//! `isAlikeDead()` and `_isTeleporting`. Overload/immobilise have no ported
//! source; death and teleport are already gated separately at every call site
//! here, so only the two effect-driven terms are folded in.

use crate::model::components::Buffs;
use crate::model::skill::effect_flag;
use crate::world::World;

/// The creature's live abnormal-state mask — Java `EffectList.getEffectFlags`.
pub(crate) fn flags_of(world: &World, object_id: i32) -> u32 {
    world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|b| b.0.iter().fold(0, |acc, buff| acc | buff.effect_flags))
        .unwrap_or(0)
}

/// Java `Creature.hasBlockActions()` — stunned, asleep or paralyzed. Blocks
/// attacking, casting **and** moving.
pub(crate) fn is_blocked_from_actions(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::BLOCK_ACTIONS != 0
}

/// Java `EffectList.getCurrentAbnormalVisualEffects()` — every visual effect
/// the creature's live buffs contribute, de-duplicated (two poisons draw one
/// tint). Order is the buff order, which is what Java's `LinkedHashSet`
/// preserves.
pub(crate) fn visual_effects(world: &World, object_id: i32) -> Vec<i16> {
    let mut out: Vec<i16> = Vec::new();
    // GM-pinned effects first, so `//ave_abnormal` shows even on a buff-less
    // creature.
    if let Some(admin) = world.objects.get_component::<crate::model::components::AdminVisuals>(&object_id) {
        out.extend(admin.0.iter().copied());
    }
    if let Some(buffs) = world.objects.get_component::<Buffs>(&object_id) {
        for buff in &buffs.0 {
            for &id in &buff.abnormal_visuals {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// Java `Creature.isMuted()` — silenced against **magic** skills.
pub(crate) fn is_muted(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::MUTED != 0
}

/// Java `Creature.isPhysicalMuted()` — the non-magic twin.
pub(crate) fn is_physical_muted(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::PHYSICAL_MUTED != 0
}

/// Java `Creature.isMpBlocked()` — MP cannot be drained *or restored* while
/// this is up.
///
/// **This flag was previously documented as having no callers.** That grep
/// covered `java/` only; every effect handler lives under
/// `dist/game/data/scripts/handlers/effecthandlers/`, and five of them read it
/// (`MagicalAttackMp`, `Mp`, `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`).
/// The `MP_BLOCK` doc comment has been corrected accordingly.
pub(crate) fn is_mp_blocked(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::MP_BLOCK != 0
}

/// Java `Creature.isDebuffBlocked()` — incoming debuffs fail outright. (Java
/// also ORs `isInvul()`; GM invulnerability is tracked separately in this port
/// and is not folded in here.)
pub(crate) fn is_debuff_blocked(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::DEBUFF_BLOCK != 0
}

/// Java `Creature.isControlBlocked()` — "out of control"; blocks item use here.
pub(crate) fn is_control_blocked(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::BLOCK_CONTROL != 0
}

/// Java `Creature.isHpBlocked()` — incoming HP damage is refused outright
/// (Celestial Shield, Flames of Invincibility, …). (Java also ORs
/// `isInvul()`; GM invulnerability is checked separately at the one real
/// consumer, `game_loop::combat::player_receive_damage`, which already has
/// its own `AdminFlags.invul` gate right next to this one.)
pub(crate) fn is_hp_blocked(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & effect_flag::HP_BLOCK != 0
}

/// Java `Creature.isMovementDisabled()`, effect-driven terms only: a stun
/// blocks movement, and so does a root (which leaves attacking and casting
/// alone).
pub(crate) fn is_movement_disabled(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & (effect_flag::BLOCK_ACTIONS | effect_flag::ROOTED | effect_flag::IMMOBILIZED) != 0
}
