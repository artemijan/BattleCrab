//! Settlement: closing-period payout, the treasury gate, next-period
//! charge and the clan-warehouse deposit.

use super::*;
/// The manor castles that currently have an owning clan — Java skips the rest
/// (`if (owner == null) continue`).
pub(super) fn owned_manor_castles(world: &World) -> Vec<i32> {
    world
        .data
        .manor
        .manor_castle_ids()
        .into_iter()
        .filter(|&id| castle_owner_clan_id(world, id).is_some())
        .collect()
}

/// The settlement half of Java's `APPROVED` case, run **before** the rollover so
/// it reads the closing period's procure list:
///
/// - crops players actually sold (`startAmount - amount`) are paid to the owner
///   clan's warehouse as **mature** crops at 90 %, with Java's consolation
///   rounding: a payout that rounds to 0 becomes 1 item 90 % of the time.
/// - the adena still reserved for crops nobody sold (`amount × price`) goes back
///   into the castle treasury.
///
/// A line whose `startAmount` is 0 (nothing was ever set up) is skipped whole.
pub(super) fn settle_closing_period(world: &mut World, castle_id: i32) {
    let Some(clan_id) = castle_owner_clan_id(world, castle_id) else {
        return;
    };
    let closing: Vec<CropProcure> = world.manor.crop_procure(castle_id, false).to_vec();
    for crop in closing {
        if crop.start_amount <= 0 {
            continue;
        }
        if crop.start_amount != crop.amount {
            let sold = crop.start_amount - crop.amount;
            let mut count = (sold as f64 * 0.9) as i64;
            // Java: `if ((count < 1) && (Rnd.get(99) < 90)) count = 1;`
            if count < 1 && world.roll(99) < 90 {
                count = 1;
            }
            if count > 0
                && let Some(mature_id) = world
                    .data
                    .manor
                    .seed_by_crop(crop.crop_id)
                    .map(|s| s.mature_id)
            {
                add_to_clan_warehouse(world, clan_id, mature_id, count);
            }
        }
        // Reserved-but-unused money goes back to the vault, untaxed.
        if crop.amount > 0 {
            crate::game_loop::castle::add_to_treasury_no_tax(
                world,
                castle_id,
                crop.amount * crop.price,
            );
        }
    }
}

/// Java's post-rollover check: if the treasury can't cover the period that was
/// just promoted to *current*, the castle's **next** setup is wiped, so the
/// manor closes after this one. (Nothing is charged here — that happens at the
/// MODIFIABLE → APPROVED step.)
pub(super) fn gate_next_period_on_treasury(world: &mut World, castle_id: i32) {
    if crate::game_loop::castle::treasury(world, castle_id) < manor_cost(world, castle_id, false) {
        world.manor.set_seed_production(castle_id, true, Vec::new());
        world.manor.set_crop_procure(castle_id, true, Vec::new());
    }
}

/// Java's `MODIFIABLE` case: charge the next period's cost to the treasury, or —
/// when the warehouse has no room for the crops **and** the treasury can't pay —
/// clear the setup and warn the leader.
///
/// Note Java's `&&`: a castle with warehouse room is charged even if the vault
/// is short, and `addToTreasuryNoTax` then refuses the debit, so that period
/// runs free. Kept verbatim.
pub(super) fn charge_next_period(world: &mut World, castle_id: i32) {
    let Some(clan_id) = castle_owner_clan_id(world, castle_id) else {
        return;
    };
    // Slots the next period's crops would need: one per crop line that is set up
    // and has no mature stack in the warehouse already.
    let slots = world
        .manor
        .crop_procure(castle_id, true)
        .iter()
        .filter(|c| c.start_amount > 0)
        .filter_map(|c| {
            world
                .data
                .manor
                .seed_by_crop(c.crop_id)
                .map(|s| s.mature_id)
        })
        .filter(|&mature_id| {
            world
                .clans
                .get(&clan_id)
                .is_none_or(|clan| clan.warehouse.0.count_of(mature_id) == 0)
        })
        .count() as i32;
    let fits = world.clans.get(&clan_id).is_some_and(|clan| {
        (clan.warehouse.size() as i32 + slots) <= world.cfg.character.warehouse_slots_clan
    });
    let cost = manor_cost(world, castle_id, true);
    if !fits && crate::game_loop::castle::treasury(world, castle_id) < cost {
        world.manor.set_seed_production(castle_id, true, Vec::new());
        world.manor.set_crop_procure(castle_id, true, Vec::new());
        notify_leader(
            world,
            castle_id,
            sm_ids::NOT_ENOUGH_FUNDS_IN_CLAN_WAREHOUSE_FOR_MANOR,
        );
    } else {
        crate::game_loop::castle::add_to_treasury_no_tax(world, castle_id, -cost);
    }
}

/// Java `getManorCost(castleId, nextPeriod)` — what a period costs its castle:
/// each seed line at its reference price × start amount (an unknown seed counts
/// as 1), plus each crop line's reserved buy-back money (price × start amount).
pub(crate) fn manor_cost(world: &World, castle_id: i32, next_period: bool) -> i64 {
    let seeds: i64 = world
        .manor
        .seed_production(castle_id, next_period)
        .iter()
        .map(|sp| match world.data.manor.seed_by_id(sp.seed_id) {
            None => 1,
            Some(_) => i64::from(reference_price(world, sp.seed_id)) * sp.start_amount,
        })
        .sum();
    let crops: i64 = world
        .manor
        .crop_procure(castle_id, next_period)
        .iter()
        .map(|cp| cp.price * cp.start_amount)
        .sum();
    seeds + crops
}

/// `cwh.addItem("Manor", matureId, count, null, null)` — drop the payout into
/// the clan warehouse (merging into an existing stack) and persist it.
fn add_to_clan_warehouse(world: &mut World, clan_id: i32, item_id: i32, count: i64) {
    let stackable = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.is_stackable);
    let has_stack = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.warehouse.0.count_of(item_id) > 0);
    // A new stack/instance needs an object id; a merge doesn't use one.
    let object_id = if stackable && has_stack {
        0
    } else {
        match world.alloc_object_id() {
            Some(id) => id,
            None => return,
        }
    };
    let World { clans, data, .. } = world;
    if let Some(clan) = clans.get_mut(&clan_id) {
        clan.warehouse
            .0
            .add_item(&data.item_data, object_id, item_id, count);
    }
    crate::game_loop::warehouse::persist_clan_warehouse(world, clan_id);
}
