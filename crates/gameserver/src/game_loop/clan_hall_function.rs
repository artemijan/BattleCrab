//! Clan-hall function upgrades (Java `ClanHall.addFunction`/`removeFunction` +
//! `ResidenceFunction`) — the buy / expire / remove economics.
//!
//! Two payers, matching Java: the **initial purchase** is paid from the buying
//! player's own inventory (`ClanHallManager.setFunction` → `takeItems(player)`);
//! the weekly **renewal** on expiry (`ResidenceFunction.reactivate`) is paid from
//! the owning clan's warehouse. A function that can't renew is removed.
//!
//! Staged: the buy/remove entry points are wired by the Clan Hall Manager NPC's
//! function menu; the per-type benefits (HP/MP regen, teleport, buffs) are their
//! own slices.

use crate::data::item_data::ADENA_ID;
use crate::db::DbCommand;
use crate::model::clan_hall::ActiveFunction;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// What `setFunction` decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionOutcome {
    /// Bought (and scheduled to expire after its rental period).
    Bought,
    /// No such `(func_id, level)` in the catalogue.
    NoSuchFunction,
    /// The hall already has this exact function level.
    AlreadyActive,
    /// The buyer's inventory can't cover the cost.
    NotEnough,
}

/// The level of an active function on a hall (0 = none).
pub(crate) fn function_level(world: &World, hall_id: i32, func_id: i32) -> i32 {
    world
        .clan_hall_functions
        .get(&hall_id)
        .and_then(|f| f.get(&func_id))
        .map(|f| f.level)
        .unwrap_or(0)
}

/// The benefit `value` of a hall's active function of a given type (Java
/// `ResidenceFunction.getValue`), or `None` when the hall hasn't bought it. For
/// the regen types this is the multiplier applied to a member's HP/MP regen.
pub(crate) fn active_function_value(world: &World, hall_id: i32, type_name: &str) -> Option<f64> {
    let func_id = world.data.residence_functions.id_of_type(type_name)?;
    let level = function_level(world, hall_id, func_id);
    if level == 0 {
        return None;
    }
    world
        .data
        .residence_functions
        .level(func_id, level)
        .map(|l| l.value)
}

/// `ClanHallManager.setFunction`: the owning-clan buyer purchases a function
/// level, paying its cost from **their own inventory**. The function then rents
/// for its `duration` before the expiry check.
pub(crate) fn buy_function(
    world: &mut World,
    hall_id: i32,
    buyer_oid: i32,
    func_id: i32,
    level: i32,
    now: i64,
) -> FunctionOutcome {
    let Some(tpl) = world
        .data
        .residence_functions
        .level(func_id, level)
        .copied()
    else {
        return FunctionOutcome::NoSuchFunction;
    };
    // Already at exactly this level? Java refuses (`getFunction(id, lv) != null`).
    if function_level(world, hall_id, func_id) == level {
        return FunctionOutcome::AlreadyActive;
    }
    // Pay from the buyer's inventory.
    let has = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&buyer_oid)
        .is_some_and(|inv| inv.count_of(tpl.cost_id) >= tpl.cost_count);
    if !has {
        return FunctionOutcome::NotEnough;
    }
    if let Some(inv) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&buyer_oid)
    {
        inv.remove_item(tpl.cost_id, tpl.cost_count);
    }

    let expiration = now + tpl.duration_ms;
    set_function(world, hall_id, func_id, level, expiration);
    FunctionOutcome::Bought
}

/// Record an active function, persist it, and arm its expiry (shared by a fresh
/// purchase and a boot restore).
pub(crate) fn set_function(
    world: &mut World,
    hall_id: i32,
    func_id: i32,
    level: i32,
    expiration: i64,
) {
    world
        .clan_hall_functions
        .entry(hall_id)
        .or_default()
        .insert(func_id, ActiveFunction { level, expiration });
    let _ = world.db.send(DbCommand::SaveResidenceFunction {
        residence_id: hall_id,
        func_id,
        level,
        expiration,
    });
    arm_function_expiry(world, hall_id, func_id);
}

/// `ClanHall.removeFunction` — drop a function. Returns whether one was present.
pub(crate) fn remove_function(world: &mut World, hall_id: i32, func_id: i32) -> bool {
    let removed = world
        .clan_hall_functions
        .get_mut(&hall_id)
        .is_some_and(|f| f.remove(&func_id).is_some());
    if removed {
        let _ = world.db.send(DbCommand::RemoveResidenceFunction {
            residence_id: hall_id,
            func_id,
        });
    }
    removed
}

/// Arm the function's expiry check at its `expiration` (immediately if past).
pub(crate) fn arm_function_expiry(world: &mut World, hall_id: i32, func_id: i32) {
    let now = commons::util::now_millis();
    let Some(expiration) = world
        .clan_hall_functions
        .get(&hall_id)
        .and_then(|f| f.get(&func_id))
        .map(|f| f.expiration)
    else {
        return;
    };
    let delay_ms = (expiration - now).max(0).min(i32::MAX as i64) as i32;
    world.scheduler.schedule(
        world.tick + super::helpers::ms_to_ticks(delay_ms),
        ScheduledTask::ClanHallFunctionExpire { hall_id, func_id },
    );
}

/// `ResidenceFunction.onFunctionExpiration` → `reactivate`: charge the rental
/// again from the owning clan's warehouse and extend it; if the warehouse can't
/// pay, the function is removed.
pub(crate) fn handle_function_expiry(world: &mut World, hall_id: i32, func_id: i32) {
    let now = commons::util::now_millis();
    // Still the active function? (A newer purchase may have replaced it.)
    let Some(level) = world
        .clan_hall_functions
        .get(&hall_id)
        .and_then(|f| f.get(&func_id))
        .filter(|f| f.expiration <= now)
        .map(|f| f.level)
    else {
        return;
    };
    let Some(tpl) = world
        .data
        .residence_functions
        .level(func_id, level)
        .copied()
    else {
        remove_function(world, hall_id, func_id);
        return;
    };
    let owner_id = world
        .clan_halls
        .get(&hall_id)
        .map(|h| h.owner_id)
        .unwrap_or(0);
    let can_pay = owner_id != 0
        && world
            .clans
            .get(&owner_id)
            .is_some_and(|c| c.warehouse.0.count_of(ADENA_ID) >= tpl.cost_count);

    if !can_pay {
        remove_function(world, hall_id, func_id);
        return;
    }
    if let Some(clan) = world.clans.get_mut(&owner_id) {
        clan.warehouse.0.remove_item(tpl.cost_id, tpl.cost_count);
    }
    super::warehouse::persist_clan_warehouse(world, owner_id);
    set_function(world, hall_id, func_id, level, now + tpl.duration_ms);
}
