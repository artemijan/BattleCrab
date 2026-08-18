//! The castle treasury — port of `Castle.addToTreasury` /
//! `Castle.addToTreasuryNoTax` / `Castle.getTaxPercent` (Java
//! `model/siege/Castle`).
//!
//! The treasury (`castle.treasury`) is the castle owner's vault. Money reaches
//! it three ways, all of them routed through this module:
//!
//! - **tax** on purchases made inside the castle's `TaxZone`
//!   ([`add_to_treasury`], which pays the liege castle its cut first),
//! - **manor** seed sales at the Manor Manager ([`add_to_treasury_no_tax`]),
//! - the owner's **chamberlain deposits** (also no-tax).
//!
//! It drains through chamberlain withdrawals and (once the manor settlement
//! lands) the period's manor cost. Java persists on *every* change with a
//! single-row `UPDATE castle SET treasury = ?`, before which nothing is
//! cached — the port does the same through [`DbCommand::UpdateCastleTreasury`],
//! so a crash can lose at most the in-flight command.
//!
//! **An unowned castle has no treasury**: Java's `_ownerId <= 0` guard makes
//! every add a no-op returning `false`, so seed money spent at a castle nobody
//! holds simply vanishes (and the tax cascade skips a liege with no owner).

use crate::db::DbCommand;
pub(crate) use crate::model::castle::{CastleSide, TaxType};
use crate::world::World;

/// Java `Castle.getTaxPercent(type)` — the percent for the castle's current
/// side, off `Feature.ini`. An unknown castle taxes nothing.
pub(crate) fn tax_percent(world: &World, castle_id: i32, tax_type: TaxType) -> i32 {
    let Some(castle) = world.castle(castle_id) else {
        return 0;
    };
    let f = &world.cfg.feature;
    match (castle.side, tax_type) {
        (CastleSide::Light, TaxType::Buy) => f.castle_buy_tax_light,
        (CastleSide::Light, TaxType::Sell) => f.castle_sell_tax_light,
        (CastleSide::Dark, TaxType::Buy) => f.castle_buy_tax_dark,
        (CastleSide::Dark, TaxType::Sell) => f.castle_sell_tax_dark,
        (CastleSide::Neutral, TaxType::Buy) => f.castle_buy_tax_neutral,
        (CastleSide::Neutral, TaxType::Sell) => f.castle_sell_tax_neutral,
    }
}

/// Java `Castle.getTaxRate(type)` — [`tax_percent`] as a fraction. The buy rate
/// is what a merchant standing in the castle's tax zone adds to its prices.
pub(crate) fn tax_rate(world: &World, castle_id: i32, tax_type: TaxType) -> f64 {
    f64::from(tax_percent(world, castle_id, tax_type)) / 100.0
}

/// Java `Npc.getTaxCastle()` — the castle whose `TaxZone` this NPC stands in,
/// or `None` when it stands in none (most of the map). Java latches the zone
/// onto the NPC when it spawns/enters (`setTaxZone`) and never recomputes it;
/// NPCs don't wander between tax zones, so resolving it by geometry at sale
/// time gives the same answer. An NPC inside an instance has no tax castle in
/// Java (`setTaxZone` ignores the zone) — the same gate applies here.
pub(crate) fn npc_tax_castle(world: &World, npc_oid: i32) -> Option<i32> {
    if super::helpers::instance_of(world, npc_oid) != 0 {
        return None;
    }
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&npc_oid)?;
    world
        .data
        .zone_data
        .tax_castle_at(pos.x, pos.y, pos.z)
        .filter(|&id| id > 0)
}

/// Java `Npc.getCastleTaxRate(TaxType.BUY)` — the buy-tax fraction a merchant
/// adds to its prices, or 0 when it stands outside every tax zone.
pub(crate) fn npc_tax_rate(world: &World, npc_oid: i32) -> f64 {
    npc_tax_castle(world, npc_oid).map_or(0.0, |id| tax_rate(world, id, TaxType::Buy))
}

/// Java `Npc.handleTaxPayment(amount)` — pay this NPC's castle its tax, through
/// the liege cascade. A non-positive amount, an NPC outside every tax zone, or
/// an unowned castle all make it a no-op.
pub(crate) fn handle_tax_payment(world: &mut World, npc_oid: i32, amount: i64) {
    if amount <= 0 {
        return;
    }
    if let Some(castle_id) = npc_tax_castle(world, npc_oid) {
        add_to_treasury(world, castle_id, amount);
    }
}

/// Java `Castle.getTreasury()`; 0 for an unknown castle.
pub(crate) fn treasury(world: &World, castle_id: i32) -> i64 {
    world
        .castles
        .iter()
        .find(|c| c.id == castle_id)
        .map_or(0, |c| c.treasury)
}

/// Java `Castle.addToTreasury(amount)` — credit tax income, paying the castle's
/// **liege** its own buy-tax cut off the top first.
///
/// The liege map is Java's `switch (getName().toLowerCase())`, verbatim: the two
/// north-eastern castles feed Rune, the five original ones feed Aden, and Aden /
/// Rune themselves (and Schuttgart's own liege chain) keep everything. Note the
/// asymmetry Java has and the port keeps: the cut is **subtracted from the
/// payer regardless** of whether the liege is owned — an unowned Aden makes that
/// tax evaporate rather than staying with the vassal.
pub(crate) fn add_to_treasury(world: &mut World, castle_id: i32, amount: i64) {
    // Java's first check: an unowned castle takes nothing (and pays no liege).
    if !is_owned(world, castle_id) {
        return;
    }
    let mut amount = amount;
    if let Some(liege_id) = liege_castle_id(world, castle_id) {
        // Java uses the *liege's* BUY rate, whatever the payer's side is.
        let liege_tax = (amount as f64 * tax_rate(world, liege_id, TaxType::Buy)) as i64;
        if is_owned(world, liege_id) {
            add_to_treasury(world, liege_id, liege_tax);
        }
        amount -= liege_tax;
    }
    add_to_treasury_no_tax(world, castle_id, amount);
}

/// Java `Castle.addToTreasuryNoTax(amount)`. Returns `false` (changing nothing)
/// when the castle is unowned, or when a withdrawal (negative `amount`) is
/// larger than the balance. A credit that would overflow is **clamped to
/// `MaxAdena`**, not rejected. Persists on every change, like Java.
pub(crate) fn add_to_treasury_no_tax(world: &mut World, castle_id: i32, amount: i64) -> bool {
    if !is_owned(world, castle_id) {
        return false;
    }
    let max_adena = world.cfg.character.max_adena;
    let Some(castle) = world.castle_mut(castle_id) else {
        return false;
    };
    if amount < 0 {
        let debit = -amount;
        if castle.treasury < debit {
            return false;
        }
        castle.treasury -= debit;
    } else if castle.treasury.saturating_add(amount) > max_adena {
        castle.treasury = max_adena;
    } else {
        castle.treasury += amount;
    }
    let treasury = castle.treasury;
    let _ = world.db.send(DbCommand::UpdateCastleTreasury {
        castle_id,
        treasury,
    });
    true
}

/// Java `_ownerId > 0` — a clan holds this castle.
fn is_owned(world: &World, castle_id: i32) -> bool {
    super::siege::owner_clan_id_opt(world, castle_id).is_some()
}

/// The castle whose buy tax is skimmed off this castle's tax income, from
/// Java's name `switch` in `addToTreasury`. Names are matched case-insensitively
/// against the `castle` table, exactly as Java does.
fn liege_castle_id(world: &World, castle_id: i32) -> Option<i32> {
    let name = world
        .castles
        .iter()
        .find(|c| c.id == castle_id)?
        .name
        .to_lowercase();
    let liege = match name.as_str() {
        "schuttgart" | "goddard" => "rune",
        "dion" | "giran" | "gludio" | "innadril" | "oren" => "aden",
        _ => return None,
    };
    world
        .castles
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(liege))
        .map(|c| c.id)
}

/// Java `CastleManager._castleCirclets` — castle id → its circlet item id.
/// Index 0 is unused (castle ids are 1..=9), exactly as Java's array is.
const CASTLE_CIRCLETS: [i32; 10] = [0, 6838, 6835, 6839, 6837, 6840, 6834, 6836, 8182, 8183];

/// Java `CastleManager.getCircletByCastleId` — `0` for anything outside 1..=9.
pub(crate) fn circlet_of(castle_id: i32) -> i32 {
    if (1..10).contains(&castle_id) {
        CASTLE_CIRCLETS[castle_id as usize]
    } else {
        0
    }
}

/// Java `CastleManager.removeCirclet(ClanMember, castleId)` — take this
/// castle's circlet off one character, unequipping it first if worn.
///
/// Java has an online and an offline branch (the latter edits the `items` rows
/// directly). This port is memory-first and keeps every logged-in character's
/// inventory in the ECS, so the online branch covers everyone it can reach;
/// a member who is offline keeps the circlet until they log in, where Java
/// would have deleted the row. Recorded rather than silently equivalent.
pub(crate) fn remove_circlet(world: &mut World, member_oid: i32, castle_id: i32) {
    let circlet_id = circlet_of(castle_id);
    if circlet_id == 0 {
        return;
    }
    // `if (circlet.isEquipped()) unEquipItemInSlot(...)` — a worn circlet is
    // taken off before it is destroyed, so the paperdoll does not keep
    // referencing a deleted object.
    let equipped_oid = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&member_oid)
        .and_then(|inv| {
            inv.equipped_items()
                .into_iter()
                .find(|i| i.item_id == circlet_id)
                .map(|i| i.object_id)
        });
    if let Some(oid) = equipped_oid
        && let Some(inv) = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&member_oid)
    {
        inv.unequip_item(oid);
    }
    let changes = crate::game_loop::items::destroy_item_by_id(world, member_oid, circlet_id, 1);
    if changes.is_empty() {
        return;
    }
    crate::game_loop::helpers::send_inventory_update(world, member_oid, changes);
    // Java's `broadcastUserInfo()` after removal — the circlet is a
    // head-slot item, so onlookers must stop drawing it.
    crate::game_loop::player_info::broadcast_user_info(world, member_oid);
}

/// Java `CastleManager.removeCirclet(Clan, castleId)` — every member of the
/// clan, gated by the caller on `RemoveCastleCirclets`.
pub(crate) fn remove_circlets_from_clan(world: &mut World, clan_id: i32, castle_id: i32) {
    if !world.cfg.character.remove_castle_circlets {
        return;
    }
    let members: Vec<i32> = world
        .clans
        .get(&clan_id)
        .map(|c| c.members.iter().map(|m| m.char_id).collect())
        .unwrap_or_default();
    for oid in members {
        remove_circlet(world, oid, castle_id);
    }
}

// ---------------------------------------------------------------------------
// Castle functions (Java `Castle.CastleFunction` + `updateFunctions`) — the
// rentable teleport / support / regen services the chamberlain console sells.
// ---------------------------------------------------------------------------

use crate::model::castle::CastleFunc;
use crate::scheduler::ScheduledTask;

/// The active function of `func_type` on a castle, if rented.
pub(crate) fn castle_function(world: &World, castle_id: i32, func_type: i32) -> Option<CastleFunc> {
    world.castle_functions.get(&(castle_id, func_type)).copied()
}

/// Java `Castle.removeFunction`.
pub(crate) fn remove_castle_function(world: &mut World, castle_id: i32, func_type: i32) {
    world.castle_functions.remove(&(castle_id, func_type));
}

/// Java `Castle.updateFunctions`' bookkeeping half (the caller has already
/// taken the lease from the buyer — `QuestCtx::take_items` sends the
/// inventory packets). `lvl == 0` deactivates. A *fresh* purchase schedules
/// its first renewal immediately with `charge_warehouse = false` (Java's
/// `endDate = 0` task), which stamps the real end time without charging
/// twice; a cheaper/equal level change rides the already-paid period (Java's
/// `diffLease <= 0` arm), a costlier one restarts the cycle.
pub(crate) fn update_castle_function(
    world: &mut World,
    castle_id: i32,
    func_type: i32,
    level: i32,
    lease: i64,
    rate_ms: i64,
) {
    if level == 0 && lease == 0 {
        remove_castle_function(world, castle_id, func_type);
        return;
    }
    let existing = castle_function(world, castle_id, func_type);
    match existing {
        // Fresh purchase, or a raise to a costlier level: (re)start the cycle.
        None => {
            world.castle_functions.insert(
                (castle_id, func_type),
                CastleFunc {
                    level,
                    lease,
                    rate_ms,
                    end_time: 0,
                },
            );
            world.scheduler.schedule(
                world.tick + 1,
                ScheduledTask::CastleFunctionRenew {
                    castle_id,
                    func_type,
                    charge_warehouse: false,
                },
            );
        }
        Some(old) if lease > old.lease => {
            world.castle_functions.insert(
                (castle_id, func_type),
                CastleFunc {
                    level,
                    lease,
                    rate_ms,
                    end_time: -1,
                },
            );
            world.scheduler.schedule(
                world.tick + 1,
                ScheduledTask::CastleFunctionRenew {
                    castle_id,
                    func_type,
                    charge_warehouse: false,
                },
            );
        }
        // A cheaper/equal change rides the already-paid period.
        Some(old) => {
            world.castle_functions.insert(
                (castle_id, func_type),
                CastleFunc {
                    level,
                    lease,
                    rate_ms: old.rate_ms,
                    end_time: old.end_time,
                },
            );
        }
    }
}

/// Java `CastleFunction.FunctionTask.run`: at the end of a rental period,
/// charge the owning clan's warehouse the lease and extend; a warehouse that
/// can't pay loses the function. The immediate post-purchase run
/// (`charge_warehouse == false`) only stamps the end time and re-arms.
pub(crate) fn handle_function_renew(
    world: &mut World,
    castle_id: i32,
    func_type: i32,
    charge_warehouse: bool,
) {
    let Some(func) = castle_function(world, castle_id, func_type) else {
        return;
    };
    let Some(owner_id) = super::siege::owner_clan_id_opt(world, castle_id) else {
        return; // Java: `_ownerId <= 0` → the task dies silently
    };
    let can_pay = world
        .clans
        .get(&owner_id)
        .is_some_and(|c| c.warehouse.0.count_of(57) >= func.lease);
    if charge_warehouse && !can_pay {
        remove_castle_function(world, castle_id, func_type);
        return;
    }
    if charge_warehouse {
        if let Some(clan) = world.clans.get_mut(&owner_id) {
            clan.warehouse.0.remove_item(57, func.lease);
        }
        super::warehouse::persist_clan_warehouse(world, owner_id);
    }
    let rate = func.rate_ms;
    if let Some(f) = world.castle_functions.get_mut(&(castle_id, func_type)) {
        f.end_time = commons::util::now_millis() + rate;
    }
    world.scheduler.schedule(
        world.tick + crate::scheduler::ms_to_ticks(rate),
        ScheduledTask::CastleFunctionRenew {
            castle_id,
            func_type,
            charge_warehouse: true,
        },
    );
}

// ---------------------------------------------------------------------------
// Door + trap (flame-tower) upgrades — persisted through `global_vars` (the
// behaviourally-equivalent home for Java's `castle_doorupgrade` /
// `castle_trapupgrade` rows; the storage layout differs, the round trip
// doesn't).
// ---------------------------------------------------------------------------

fn door_upgrade_key(door_id: i32) -> String {
    format!("CastleDoorUpgrade_{door_id}")
}

fn trap_upgrade_key(castle_id: i32, tower_index: i32) -> String {
    format!("CastleTrapUpgrade_{castle_id}_{tower_index}")
}

/// The door's upgrade HP ratio (Java `DoorStat.getUpgradeHpRatio`, 1 = base).
pub(crate) fn door_upgrade_ratio(world: &World, door_id: i32) -> i32 {
    super::global_vars::get_i64(world, &door_upgrade_key(door_id), 1).max(1) as i32
}

/// The door object carrying `door_id`, for the HP-upgrade stamp.
///
/// Unlike [`crate::game_loop::doors::find_shared_door`] this does **not** skip
/// instance door copies. That is preserved from the code this replaced rather
/// than chosen: whether a castle door id can ever appear in an instance
/// template's doorlist is untested, and quietly changing it here would be a
/// behaviour change hidden inside a deduplication.
fn find_upgradable_door(world: &World, door_id: i32) -> Option<i32> {
    world.door_regions.values().flatten().copied().find(|oid| {
        world
            .objects
            .get_component::<crate::model::door::Door>(oid)
            .is_some_and(|d| d.door_id == door_id)
    })
}

/// Java `Castle.setDoorUpgrade(doorId, ratio, save)`: record the ratio and
/// re-derive the door's max HP (healing it to full, as Java's
/// `setCurrentHp(getMaxHp())` does on upgrade).
pub(crate) fn set_door_upgrade(world: &mut World, door_id: i32, ratio: i32) {
    super::global_vars::set(world, &door_upgrade_key(door_id), ratio);
    apply_door_upgrade(world, door_id, ratio);
}

/// Stamp an upgrade ratio onto the live door: HP becomes `base × ratio`, full.
/// A ratio below 1 would *shrink* the door, so it clamps — Java's upgrade path
/// can only ever raise it.
fn apply_door_upgrade(world: &mut World, door_id: i32, ratio: i32) {
    let base = world
        .data
        .door_data
        .get(door_id)
        .map(|t| t.hp_max)
        .unwrap_or(0);
    let oid = find_upgradable_door(world, door_id);
    if let Some(oid) = oid
        && let Some(d) = world
            .objects
            .get_component_mut::<crate::model::door::Door>(&oid)
    {
        d.current_hp = base * ratio.max(1);
    }
}

/// Boot re-apply (Java `loadDoorUpgrade`): the doors spawned with base HP
/// before the `global_variables` table landed; stamp every upgraded door's
/// max back on. Runs once, from the boot load event.
pub(crate) fn apply_door_upgrades_at_boot(world: &mut World) {
    let upgraded: Vec<(i32, i32)> = world
        .global_vars
        .iter()
        .filter_map(|(k, v)| {
            let door_id = k.strip_prefix("CastleDoorUpgrade_")?.parse::<i32>().ok()?;
            let ratio = v.parse::<i32>().ok()?;
            (ratio > 1).then_some((door_id, ratio))
        })
        .collect();
    for (door_id, ratio) in upgraded {
        apply_door_upgrade(world, door_id, ratio);
    }
}

/// The trap (flame-tower damage zone) upgrade level of one tower slot.
pub(crate) fn trap_upgrade_level(world: &World, castle_id: i32, tower_index: i32) -> i32 {
    super::global_vars::get_i64(world, &trap_upgrade_key(castle_id, tower_index), 0) as i32
}

/// Java `Castle.setTrapUpgrade`. The level is stored and reported by the
/// chamberlain console; the flame tower's per-level damage-zone activation is
/// part of the (unported) tower zone machinery, same as before this console.
pub(crate) fn set_trap_upgrade(world: &mut World, castle_id: i32, tower_index: i32, level: i32) {
    super::global_vars::set(world, &trap_upgrade_key(castle_id, tower_index), level);
}

/// Java `Castle.banishForeigners` → `CastleZone.banishForeigners(ownerId)`:
/// everyone inside the castle residence zone who is *not* in the owning clan
/// is ported out to the zone's banish spawns — the same eviction shape as
/// [`super::siege::oust_all_players`], filtered by clan.
pub(crate) fn banish_foreigners(world: &mut World, castle_id: i32) {
    let owner = super::siege::owner_clan_id_opt(world, castle_id).unwrap_or(0);
    let Some(zone) = world.data.zone_data.residence_teleport_zone(castle_id) else {
        return;
    };
    let spawns: Vec<(i32, i32, i32)> = world
        .data
        .zone_data
        .residence_teleport_spawns(castle_id)
        .to_vec();
    if spawns.is_empty() {
        return;
    }
    let (min_z, max_z) = (zone.territory.min_z, zone.territory.max_z);
    let inside: Vec<i32> = world
        .in_game_player_oids()
        .filter(|oid| {
            world
                .objects
                .get_component::<crate::model::Player>(oid)
                .is_some_and(|p| p.clan_id == 0 || p.clan_id != owner)
                && world
                    .objects
                    .get_component::<crate::model::components::Position>(oid)
                    .is_some_and(|p| {
                        p.z >= min_z
                            && p.z <= max_z
                            && world
                                .data
                                .zone_data
                                .residence_teleport_zone(castle_id)
                                .is_some_and(|zn| zn.territory.contains_2d(p.x, p.y))
                    })
        })
        .collect();
    for oid in inside {
        let idx = world.roll(spawns.len() as i32) as usize;
        let (x, y, z) = spawns[idx];
        super::death::teleport_player(world, oid, x, y, z);
    }
}
