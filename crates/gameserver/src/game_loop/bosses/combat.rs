//! Shared grand-boss combat bookkeeping — the pieces Antharas, Baium and
//! Valakas each carried a private copy of: the last-hit inactivity clock and
//! the anti-strider debuff.

use crate::world::World;

/// `ANTI_STRIDER` (4258, "Hinder Strider") — every lair boss debuffs a
/// strider-mounted attacker, once.
pub(crate) const ANTI_STRIDER: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// Java's static `_lastAttack`/`_timeTracker` (plus Valakas' `_actualVictim`):
/// the last tick a valid attacker struck the boss, kept on the boss so the
/// CHECK_ATTACK/regen beats can measure inactivity, and the skill AI's sticky
/// victim (`0` = none; only Valakas reads it).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct BossCombat {
    pub last_attack_tick: u64,
    pub actual_victim: i32,
}

/// `_lastAttack = now` — a valid hit resets the inactivity clock.
pub(crate) fn touch(world: &mut World, boss_oid: i32) {
    let now = world.tick;
    if let Some(c) = world.objects.get_component_mut::<BossCombat>(&boss_oid) {
        c.last_attack_tick = now;
    }
}

/// Ticks since the boss was last struck — `0` for a boss with no combat
/// clock (not in a fight), so an idle comparison never fires for it.
pub(crate) fn idle_ticks(world: &World, boss_oid: i32) -> u64 {
    world
        .objects
        .get_component::<BossCombat>(&boss_oid)
        .map(|c| world.tick.saturating_sub(c.last_attack_tick))
        .unwrap_or(0)
}

/// Which quarter of its health the boss is in — 0 (≥ 75 %) … 3 (< 25 %), the
/// band the escalating self-heal ladders key on.
pub(crate) fn hp_quarter(cur: f64, max: f64) -> usize {
    if cur < max * 0.25 {
        3
    } else if cur < max * 0.5 {
        2
    } else if cur < max * 0.75 {
        1
    } else {
        0
    }
}

/// Is the attacker a strider rider not yet hindered? (Java checks
/// `!isAffectedBySkill(4258)` so the debuff isn't recast every swing.)
pub(crate) fn strider_needs_debuff(world: &World, attacker_oid: i32) -> bool {
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    on_strider && !crate::game_loop::abnormal::has_buff(world, attacker_oid, ANTI_STRIDER)
}

/// Hinder a strider rider through the ordinary NPC cast path, once. (Valakas
/// applies the effect directly instead of casting — its script keeps that
/// variant on top of [`strider_needs_debuff`].)
pub(crate) fn anti_strider(world: &mut World, boss_oid: i32, attacker_oid: i32) {
    if strider_needs_debuff(world, attacker_oid) {
        crate::game_loop::npc::cast::cast_skill(world, boss_oid, attacker_oid, ANTI_STRIDER, 1);
    }
}
