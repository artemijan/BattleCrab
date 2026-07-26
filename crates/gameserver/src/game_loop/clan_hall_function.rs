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
use crate::model::components::{Reuses, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// Java `ALLOWED_BUFFS` — the support-magic skills the BUFF function may cast
/// (the standard NPC-buff line, skills 4342–4360).
const ALLOWED_BUFFS: &[i32] = &[
    4342, 4343, 4344, 4346, 4345, 4347, 4349, 4350, 4348, 4351, 4352, 4353, 4358, 4354, 4355, 4356,
    4357, 4359, 4360,
];

/// What a buff-function cast decided (Java `ClanHallManager` `buffs` branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffCastOutcome {
    /// Buff cast on the caller.
    Cast,
    /// The token wasn't a `<id>_<lvl>` in `ALLOWED_BUFFS` (Java ignores it).
    NotAllowed,
    /// The manager NPC lacks the MP to cast it.
    NotEnoughMp,
    /// The skill is still on the NPC's reuse timer.
    OnReuse,
}

/// `ClanHallManager` `useFunctions buffs <id>_<lvl>`: the manager NPC casts a
/// support buff on the caller, gated on being an allowed buff, the NPC's MP,
/// and the NPC's reuse timer (Java `castSkill` → `npc.doCast`).
pub(crate) fn cast_hall_buff(
    world: &mut World,
    npc_oid: i32,
    player_oid: i32,
    token: &str,
) -> BuffCastOutcome {
    let Some((skill_id, skill_lvl)) = token
        .split_once('_')
        .and_then(|(id, lvl)| Some((id.parse().ok()?, lvl.parse::<i32>().ok()?)))
    else {
        return BuffCastOutcome::NotAllowed;
    };
    if !ALLOWED_BUFFS.contains(&skill_id) {
        return BuffCastOutcome::NotAllowed;
    }
    let Some(skill) = world.data.skill_data.get(skill_id, skill_lvl).cloned() else {
        return BuffCastOutcome::NotAllowed;
    };
    // Java: `npc.getCurrentMp() < (mpConsume + mpInitialConsume)`.
    let cost = (skill.mp_consume + skill.mp_initial_consume) as f64;
    let npc_mp = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .map(|v| v.cur_mp)
        .unwrap_or(0.0);
    if npc_mp < cost {
        return BuffCastOutcome::NotEnoughMp;
    }
    // Java `npc.isSkillDisabled` — the reuse timer.
    let on_reuse = world
        .objects
        .get_component::<Reuses>(&npc_oid)
        .and_then(|r| r.0.get(&skill.reuse_key()))
        .is_some_and(|r| r.until_tick > world.tick);
    if on_reuse {
        return BuffCastOutcome::OnReuse;
    }
    // Trigger-cast (animation + effects), then charge the NPC's MP and arm its
    // reuse — Java `npc.doCast` does both as part of the cast.
    super::support_magic::cast_from_npc(world, npc_oid, player_oid, (skill_id, skill_lvl));
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&npc_oid) {
        v.cur_mp = (v.cur_mp - cost).max(0.0);
    }
    super::skills::cast::set_skill_reuse(world, npc_oid, &skill);
    BuffCastOutcome::Cast
}

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
