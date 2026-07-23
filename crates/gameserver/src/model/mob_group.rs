//! Port of `MobGroup` / `MobGroupTable` / `ControllableMob` — GM-spawned mob
//! groups steered by `//mobgroup_*` (`AdminMobGroup`). Each group is a set of
//! ordinary NPC entities tagged with the [`Controllable`] component; the group's
//! [`MobGroupState`] drives their per-tick behavior (see
//! `game_loop::npc_ai::controllable_think`), reusing the normal NPC
//! movement/combat primitives rather than a separate AI tree.

use bevy_ecs::component::Component;

/// Marks an NPC as a member of a GM-controlled mob group (Java
/// `ControllableMob`). The group's state lives in [`crate::world::World::mob_groups`].
#[derive(Component, Debug, Clone, Copy)]
pub struct Controllable {
    pub group_id: i32,
}

/// The group's AI mode (Java `MobGroup` set-mode calls → `ControllableMobAI`
/// state). Combat states carry the target object id resolved when the mode was
/// set; movement states carry the commanding player's object id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobGroupState {
    /// `setIdleMode` — passive, stationary, no aggro.
    Idle,
    /// `setNoMoveMode` — hold position (no movement).
    NoMove,
    /// `setAttackRandom` — the default aggressive AI (aggro nearby players).
    Random,
    /// `setAttackTarget(target)` — every member attacks the creature.
    Attack(i32),
    /// `setAttackGroup(otherGroup)` — attack another group's members.
    AttackGroup(i32),
    /// `setFollowMode(commander)` — follow the commanding player.
    Follow(i32),
    /// `returnGroup(commander)` — walk back to the commanding player.
    Return(i32),
    /// `setCastMode(target)` — cast at the target (simplified to attack here).
    Cast(i32),
}

/// Port of `MobGroup`: the group id, the template every member is spawned from,
/// the requested size, the live member object ids, the current AI state, and
/// the invulnerability toggle (`//mobgroup_invul`).
#[derive(Debug, Clone)]
pub struct MobGroup {
    pub id: i32,
    pub npc_id: i32,
    pub max_count: i32,
    pub members: Vec<i32>,
    pub state: MobGroupState,
    pub invul: bool,
}

impl MobGroup {
    pub fn new(id: i32, npc_id: i32, max_count: i32) -> Self {
        Self {
            id,
            npc_id,
            max_count,
            members: Vec::new(),
            state: MobGroupState::Idle,
            invul: false,
        }
    }

    /// Live member count (Java `getActiveMobCount`).
    pub fn alive_count(&self) -> usize {
        self.members.len()
    }
}
