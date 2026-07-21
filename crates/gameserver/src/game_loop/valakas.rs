//! Valakas (`ai/bosses/Valakas`) — the attack-side rules.
//!
//! Valakas uses the **four-state** status ladder rather than the two-state one
//! the simpler bosses share:
//!
//! | status | meaning |
//! |---|---|
//! | 0 `DORMANT` | spawned, nobody has entered; entry unlocked |
//! | 1 `WAITING` | someone entered, 30-minute window for others; entry unlocked |
//! | 2 `FIGHTING` | engaged; entry **locked** |
//! | 3 `DEAD` | killed; entry locked |
//!
//! Only the `onAttack` half is ported here — the lair's entry/teleport flow and
//! the 30-minute window are their own slice.

use crate::model::components::Position;
use crate::world::World;

pub const VALAKAS: i32 = 29028;

/// `getZoneById(12010)` — "Valakas Boss", a `ScriptZone`.
const BOSS_ZONE_ID: i32 = 12010;

/// `ATTACKER_REMOVE` — where a player attacking outside the fight is dumped.
const ATTACKER_REMOVE: (i32, i32, i32) = (150_037, -57_255, -2_976);

pub const DORMANT: i32 = 0;
pub const WAITING: i32 = 1;
pub const FIGHTING: i32 = 2;
pub const DEAD: i32 = 3;

/// Strider riders are debuffed on sight (skill 4258), once.
const STRIDER_DEBUFF: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// What `Valakas.onAttack` decided to do about an attacker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackVerdict {
    /// Attacking from **outside the lair** — Java calls `attacker.doDie()`.
    /// A hard anti-exploit: you cannot plink at Valakas from safety.
    KilledForAttackingFromOutside,
    /// Attacking before the fight has started — bounced to `ATTACKER_REMOVE`.
    RemovedNotFighting,
    /// A normal hit.
    Allowed,
}

/// `Valakas.onAttack`, minus the timer bookkeeping.
///
/// The order is Java's and is load-bearing: the **zone check comes first**, so
/// an out-of-zone attacker dies whatever the boss's status — including while
/// Valakas is dead, when the status check would otherwise have merely teleported
/// them.
pub(crate) fn on_valakas_attacked(world: &mut World, valakas_oid: i32, attacker_oid: i32) -> AttackVerdict {
    if world.objects.get_component::<crate::model::Player>(&attacker_oid).is_none() {
        return AttackVerdict::Allowed;
    }

    if !attacker_in_lair(world, attacker_oid) {
        // `attacker.doDie(attacker)` — self-inflicted, so it carries no PvP or
        // karma consequence for anyone.
        crate::game_loop::death::player_do_die(world, attacker_oid, attacker_oid);
        return AttackVerdict::KilledForAttackingFromOutside;
    }

    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(FIGHTING) {
        let (x, y, z) = ATTACKER_REMOVE;
        crate::game_loop::death::teleport_player(world, attacker_oid, x, y, z);
        return AttackVerdict::RemovedNotFighting;
    }

    // A strider-mounted attacker is debuffed, once — Java checks
    // `!isAffectedBySkill(4258)` so it isn't recast every swing.
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    if on_strider && !already_debuffed(world, attacker_oid) {
        cast_debuff(world, valakas_oid, attacker_oid);
    }

    AttackVerdict::Allowed
}

fn attacker_in_lair(world: &World, attacker_oid: i32) -> bool {
    let Some(pos) = world.objects.get_component::<Position>(&attacker_oid) else { return false };
    world.data.zone_data.by_id(BOSS_ZONE_ID).is_some_and(|z| z.contains(pos.x, pos.y, pos.z))
}

fn already_debuffed(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == STRIDER_DEBUFF))
}

fn cast_debuff(world: &mut World, caster_oid: i32, target_oid: i32) {
    let Some(skill) = world.data.skill_data.get(STRIDER_DEBUFF, 1).cloned() else { return };
    crate::game_loop::skills::effects::apply_continuous_effects(world, caster_oid, target_oid, &skill, None);
}
