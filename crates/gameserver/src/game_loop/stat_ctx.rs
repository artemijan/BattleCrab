//! The seven-component borrow every buff change needs, resolved once.
//!
//! [`crate::model::Player::apply_buff`] and `remove_buff` rebuild the modifier
//! maps from scratch, so they need the player, their base stats, the modifier
//! maps, the inventory, the buff list, and the speed and combat blocks — seven
//! components plus [`GameData`]. Eight call sites across the game loop used to
//! spell that `get_many_mut` tuple out in full, twenty-odd lines apiece, purely
//! to reach a one-line buff call.
//!
//! The borrow is deliberately scoped by a closure rather than handed out: half
//! the call sites make *several* buff calls against one lookup (a loop over
//! removed skills, or a remove-then-reapply at a new level), and re-resolving
//! seven components per call would turn one lookup into N.

use crate::data::GameData;
use crate::model::Player;
use crate::model::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers};
use crate::model::inventory::Inventory;
use crate::model::skill::ActiveBuff;
use crate::world::World;

/// A resolved buff-editing borrow. Obtain one with [`with_stat_ctx`].
pub(crate) struct StatCtx<'a> {
    data: &'a GameData,
    player: &'a Player,
    base: &'a BaseStats,
    mods: &'a mut StatModifiers,
    inventory: &'a Inventory,
    buffs: &'a mut Buffs,
    speeds: &'a mut Speeds,
    combat: &'a mut CombatStats,
}

impl StatCtx<'_> {
    /// [`Player::apply_buff`] — `false` when the buff was refused (a same-type
    /// buff of equal or higher level is already up).
    pub(crate) fn apply(&mut self, buff: ActiveBuff) -> bool {
        self.player.apply_buff(
            self.data,
            self.base,
            self.mods,
            self.inventory,
            self.buffs,
            self.speeds,
            self.combat,
            buff,
        )
    }

    /// [`Player::remove_buff`] — a no-op when the skill isn't up.
    pub(crate) fn remove(&mut self, skill_id: i32) {
        self.player.remove_buff(
            self.data,
            self.base,
            self.mods,
            self.inventory,
            self.buffs,
            self.speeds,
            self.combat,
            skill_id,
        );
    }
}

/// Resolve the buff-editing components for `object_id` and run `f` against
/// them. `None` — and `f` never runs — when the object is gone or is missing
/// one of the seven, which is the same guard every call site used to write by
/// hand.
///
/// `world` stays borrowed for the closure's duration, exactly as it did while
/// the `get_many_mut` tuple was alive; reach anything else the body needs
/// (`world.tick`, packet sends) before or after the call.
pub(crate) fn with_stat_ctx<R>(
    world: &mut World,
    object_id: i32,
    f: impl FnOnce(&mut StatCtx<'_>) -> R,
) -> Option<R> {
    let World { objects, data, .. } = world;
    let (player, base, mut mods, inventory, mut buffs, mut speeds, mut combat) = objects
        .get_many_mut::<(
            &Player,
            &BaseStats,
            &mut StatModifiers,
            &Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&object_id)?;
    let mut ctx = StatCtx {
        data,
        player,
        base,
        mods: &mut mods,
        inventory,
        buffs: &mut buffs,
        speeds: &mut speeds,
        combat: &mut combat,
    };
    Some(f(&mut ctx))
}
