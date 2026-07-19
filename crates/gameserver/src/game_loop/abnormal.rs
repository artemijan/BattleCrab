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

/// Java `Creature.isMovementDisabled()`, effect-driven terms only: a stun
/// blocks movement, and so does a root (which leaves attacking and casting
/// alone).
pub(crate) fn is_movement_disabled(world: &World, object_id: i32) -> bool {
    flags_of(world, object_id) & (effect_flag::BLOCK_ACTIONS | effect_flag::ROOTED) != 0
}
