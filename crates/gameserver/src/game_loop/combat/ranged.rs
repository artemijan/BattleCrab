//! Bow and crossbow attacks — ammunition, MP upkeep and the reload gauge.
//!
//! Java keeps this inside `Creature.doAttack`'s `WeaponType.BOW`/`CROSSBOW`
//! branches plus `Player.checkAndEquipAmmunition` and
//! `Inventory.findArrowForBow`/`reduceArrowCount`; it lives here so
//! `combat::do_auto_attack` keeps one short ranged branch instead of the whole
//! thing inline.
//!
//! A ranged swing differs from a melee one in four ways:
//!
//! 1. it needs **ammunition** of the weapon's own grade, auto-equipped into the
//!    left hand,
//! 2. it costs **MP** per shot,
//! 3. it is followed by a **reload delay** (`_disableRangedAttackEndTime`)
//!    during which no further shot is allowed, shown to the player as a red
//!    `SetupGauge`, and
//! 4. it consumes one arrow when it fires.
//!
//! The attack *range* already works — bows declare `pAtkRange` 500, which the
//! item-stat path has applied to `CombatStats.atk_range` since G14.

use crate::data::item_data::{EtcItemType, WeaponType};
use crate::game_loop::helpers::{send_action_failed, send_inventory_update, spend_mp};
use crate::model::inventory::{Inventory, PaperdollSlot};
use crate::network::server_packets::{self, sm_ids};
use crate::world::World;

use crate::game_loop::helpers::client_for_player;

/// Java `SetupGauge.RED` — the bar colour a reloading bow uses.
const GAUGE_RED: i32 = 0;

/// The equipped weapon's type, or `None` when unarmed.
pub(crate) fn equipped_weapon_type(world: &World, object_id: i32) -> Option<WeaponType> {
    let inv = world.objects.get_component::<Inventory>(&object_id)?;
    let rhand = inv.paperdoll_item_id(PaperdollSlot::RHand);
    if rhand == 0 {
        return None;
    }
    Some(world.data.item_data.weapon_type(rhand))
}

/// Java `WeaponType.isRanged()` — bows and crossbows in all their variants.
pub(crate) fn is_ranged(t: WeaponType) -> bool {
    matches!(
        t,
        WeaponType::Bow | WeaponType::Crossbow | WeaponType::TwoHandCrossbow
    )
}

fn is_crossbow(t: WeaponType) -> bool {
    matches!(t, WeaponType::Crossbow | WeaponType::TwoHandCrossbow)
}

/// Why a ranged swing was refused, so the caller can send the right reply.
pub(crate) enum RangedRefusal {
    /// The reload delay has not elapsed — Java sends a bare `ActionFailed`.
    Reloading,
    /// No matching ammunition; the attack intention is dropped.
    OutOfAmmo,
    /// Not enough MP for the shot.
    NotEnoughMp,
}

/// The whole pre-shot gate for a ranged weapon (Java's `BOW`/`CROSSBOW` block
/// in `doAttack`): reload delay, ammunition, MP. On success the arrow is
/// consumed, the MP spent, the gauge sent and the next-shot time armed, and the
/// caller proceeds with an ordinary swing.
pub(crate) fn prepare_ranged_shot(
    world: &mut World,
    attacker_oid: i32,
    weapon_type: WeaponType,
) -> Result<(), RangedRefusal> {
    // 1. Reload delay (`_disableRangedAttackEndTime > now`).
    if world
        .objects
        .get_component::<crate::model::components::RangedReload>(&attacker_oid)
        .is_some_and(|r| r.ready_at_tick > world.tick)
    {
        return Err(RangedRefusal::Reloading);
    }

    let ammo_type = if is_crossbow(weapon_type) {
        EtcItemType::Bolt
    } else {
        EtcItemType::Arrow
    };

    // 2. Ammunition: already equipped, or find + equip a matching stack.
    let equipped = equipped_ammo(world, attacker_oid, ammo_type);
    let ammo_object_id = match equipped {
        Some(oid) => oid,
        None => match equip_ammunition(world, attacker_oid, ammo_type) {
            Some(oid) => oid,
            None => return Err(RangedRefusal::OutOfAmmo),
        },
    };

    // 3. MP for the shot.
    let mp_cost = shot_mp_cost(world, attacker_oid);
    if mp_cost > 0.0 {
        let cur_mp = world
            .objects
            .get_component::<crate::model::components::Vitals>(&attacker_oid)
            .map(|v| v.cur_mp)
            .unwrap_or(0.0);
        if cur_mp < mp_cost {
            return Err(RangedRefusal::NotEnoughMp);
        }
        spend_mp(world, attacker_oid, mp_cost);
    }

    // 4. Fire: spend the arrow, arm the reload, show the gauge.
    consume_one(world, attacker_oid, ammo_object_id);
    let reuse_ms = reuse_time_ms(world, attacker_oid);
    world.objects.add_components(
        &attacker_oid,
        crate::model::components::RangedReload {
            ready_at_tick: world.tick + crate::scheduler::ms_to_ticks(reuse_ms),
        },
    );
    if is_crossbow(weapon_type) {
        crate::game_loop::helpers::send_sm_bare_to_player(
            world,
            attacker_oid,
            sm_ids::YOUR_CROSSBOW_IS_PREPARING_TO_FIRE,
        );
    }
    crate::game_loop::helpers::send_to_player(
        world,
        attacker_oid,
        server_packets::setup_gauge(attacker_oid, GAUGE_RED, reuse_ms),
    );
    Ok(())
}

/// Tell the player why the shot didn't happen, with Java's packet for each case.
pub(crate) fn report_refusal(world: &mut World, attacker_oid: i32, why: RangedRefusal) {
    let Some(client_id) = client_for_player(world, attacker_oid) else {
        return;
    };
    match why {
        // Java only schedules an AI re-think and sends ActionFailed; the swing
        // is retried on the next combat tick.
        RangedRefusal::Reloading => {
            send_action_failed(world, client_id);
        }
        RangedRefusal::OutOfAmmo => {
            world
                .objects
                .remove_component::<crate::model::components::Intent>(&attacker_oid);
            crate::game_loop::helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::YOU_HAVE_RUN_OUT_OF_ARROWS,
                &[],
            );
        }
        RangedRefusal::NotEnoughMp => {
            crate::game_loop::helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::NOT_ENOUGH_MP,
                &[],
            );
        }
    }
}

/// The object id of the currently equipped ammunition, when it is of `kind`.
fn equipped_ammo(world: &World, object_id: i32, kind: EtcItemType) -> Option<i32> {
    let inv = world.objects.get_component::<Inventory>(&object_id)?;
    let lhand_id = inv.paperdoll_item_id(PaperdollSlot::LHand);
    if lhand_id == 0 {
        return None;
    }
    if world.data.item_data.get(lhand_id).map(|t| t.etc_item_type) != Some(kind) {
        return None;
    }
    inv.first_of_item(lhand_id).map(|i| i.object_id)
}

/// Java `Inventory.findArrowForBow` / `findBoltForCrossBow` + the equip half of
/// `checkAndEquipAmmunition`: the first stack of the right kind whose **grade
/// matches the bow's**, moved into the left hand.
fn equip_ammunition(world: &mut World, object_id: i32, kind: EtcItemType) -> Option<i32> {
    let bow_grade = {
        let inv = world.objects.get_component::<Inventory>(&object_id)?;
        let rhand = inv.paperdoll_item_id(PaperdollSlot::RHand);
        world.data.item_data.get(rhand)?.crystal_type
    };
    // Java matches on `getCrystalTypePlus()`, which collapses the S-grades; on
    // an Interlude dist nothing above S exists, so plain grade equality is the
    // same predicate.
    let found = {
        let inv = world.objects.get_component::<Inventory>(&object_id)?;
        inv.items()
            .iter()
            .find(|i| {
                world
                    .data
                    .item_data
                    .get(i.item_id)
                    .is_some_and(|t| t.etc_item_type == kind && t.crystal_type == bow_grade)
            })
            .map(|i| i.object_id)
    }?;
    // Ammunition goes straight into the left hand: the general equip path
    // refuses `Etc` items and would displace the two-handed bow.
    world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.equip_ammunition(found));
    Some(found)
}

/// `PlayerInventory.reduceArrowCount` — one arrow per shot.
///
/// Java splits on what the stack has left (`updateItemCountNoDbUpdate`), and
/// the two halves cost very different things:
///
/// * **still arrows left** — decrement, refresh the weight, and send an
///   `InventoryUpdate` naming the modified stack. No unequip, no stat
///   recompute: the quiver stays in the left hand.
/// * **that was the last one** — `destroyItem`, whose `Inventory.removeItem`
///   unequips the empty quiver and runs the full paperdoll refresh. Once per
///   stack, not once per shot.
///
/// An *infinite* quiver bails before the decrement entirely.
fn consume_one(world: &mut World, object_id: i32, ammo_object_id: i32) {
    let Some((item_id, left)) = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| {
            inv.by_object_id(ammo_object_id)
                .map(|i| (i.item_id, i.count))
        })
    else {
        return;
    };
    // Java `arrows.getEtcItem().isInfinite()` — the quiver is never spent.
    if world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_infinite)
    {
        return;
    }

    if left > 1 {
        // The ordinary shot: decrement and tell the client, nothing more.
        let changes = world
            .objects
            .get_component_mut::<Inventory>(&object_id)
            .map(|inv| inv.remove_item(item_id, 1))
            .unwrap_or_default();
        send_inventory_update(world, object_id, changes);
        return;
    }
    // The last arrow: the quiver leaves the left hand, so this takes the
    // destroy protocol (paperdoll, options, stats) like any other worn item.
    let changes = crate::game_loop::items::destroy_item_by_id(world, object_id, item_id, 1);
    send_inventory_update(world, object_id, changes);
}

/// The MP a single shot costs — `weapon.getMpConsume()`, with Java's
/// `reducedMpConsume` roll when the weapon declares one.
fn shot_mp_cost(world: &mut World, object_id: i32) -> f64 {
    let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
        return 0.0;
    };
    let rhand = inv.paperdoll_item_id(PaperdollSlot::RHand);
    let Some(t) = world.data.item_data.get(rhand) else {
        return 0.0;
    };
    let (base, reduced, chance) = (
        t.mp_consume,
        t.reduced_mp_consume,
        t.reduced_mp_consume_chance,
    );
    if reduced > 0 && world.roll(100) < chance {
        reduced as f64
    } else {
        base as f64
    }
}

/// Java `Formulas.calculateReuseTime`: ranged weapons only, `900000 / pAtkSpd`.
fn reuse_time_ms(world: &World, object_id: i32) -> i32 {
    let p_atk_spd = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&object_id)
        .map(|c| c.p_atk_spd)
        .unwrap_or(1)
        .max(1);
    900_000 / p_atk_spd
}

/// Java `Creature.getAttackType().isRanged()` — the **attacker**-side ranged
/// flag that `Formulas.calcShldUse` (+30 % block rate against a bow) and
/// `calcAutoAttackDamage` (the 154 vs 77 weapon coefficient) both read.
///
/// Java falls back to `_template.getBaseAttackType()` when nothing is equipped,
/// which is how a *monster* can count as ranged. NPCs here carry no paperdoll
/// and no `baseAttackType` is modelled, so that branch is always melee — the
/// player-attacker case, which is the one the shield bonus is about, is exact.
pub(crate) fn attacker_is_ranged(world: &World, object_id: i32) -> bool {
    equipped_weapon_type(world, object_id).is_some_and(is_ranged)
}
