//! Clan-hall auctions (Java `model/residences/ClanHallAuction` +
//! `ClanHallAuctionManager`) — the bid / outbid / cancel / finalize logic.
//!
//! **Escrow model** (from `ClanHallAuctioneer.processBidBypass`): only the
//! *current highest* bidder's adena is ever held. A new bid takes the bidder's
//! adena from the clan warehouse and **refunds the previous highest bidder**; so
//! at any moment exactly one clan's adena is escrowed per hall. Cancelling only
//! removes the map entry (Java `removeBid` — no refund; a non-highest clan was
//! already refunded when it was outbid, and the highest forfeits by cancelling).
//! At finalize the highest bidder's held adena is consumed and it wins the hall.
//!
//! Reachable through the Clan Hall Auctioneer NPC (bid/cancel) and the weekly
//! [`ScheduledTask::ClanHallAuctionEnd`] close (finalize).

use crate::data::item_data::ADENA_ID;
use crate::db::DbCommand;
use crate::game_loop::clans::clan_of_or_zero;
use crate::model::clan_hall::{ClanHallBid, ClanHallType};
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `Inventory.MAX_ADENA` — 999.9 billion.
const MAX_ADENA: i64 = 99_900_000_000;
/// Java: only a clan of level 2+ may bid.
const MIN_CLAN_LEVEL: i32 = 2;
/// The auction cycle length — one week (Java `ClanHallAuctionManager`'s
/// `604800000` ms). The port re-arms this from boot rather than aligning to a
/// fixed wall-clock instant (documented divergence, like the siege schedule's).
const WEEK_TICKS: u64 = 7 * 24 * 60 * 60 * 10;

/// What the auctioneer decided about a bid (each is one Java refusal branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidOutcome {
    /// The bid was placed (and any previous highest bidder refunded).
    Accepted,
    /// The hall isn't up for auction (unknown, not auctionable, already owned).
    HallUnavailable,
    /// No clan, or a clan below level 2.
    ClanTooLow,
    /// The clan already owns a hall.
    AlreadyOwnsHall,
    /// The clan is bidding on a different hall.
    BiddingElsewhere,
    /// Over the 999.9-billion adena cap.
    BidTooHigh,
    /// Not above the current highest bid (or the minimum).
    BidTooLow,
    /// Not enough adena in the clan warehouse.
    NotEnoughAdena,
}

/// `ClanHallAuction.getHighestBid` — the top bid, or the hall's minimum when
/// there are none.
pub(crate) fn highest_bid(world: &World, hall_id: i32) -> i64 {
    let floor = world
        .clan_halls
        .get(&hall_id)
        .map(|h| h.min_bid)
        .unwrap_or(0);
    world
        .clan_hall_bids
        .get(&hall_id)
        .and_then(|b| b.values().map(|x| x.amount).max())
        .map(|top| top.max(floor))
        .unwrap_or(floor)
}

/// `ClanHallAuction.getHighestBidder` — the top bidding clan and its amount.
pub(crate) fn highest_bidder(world: &World, hall_id: i32) -> Option<(i32, i64)> {
    world
        .clan_hall_bids
        .get(&hall_id)?
        .iter()
        .max_by_key(|(_, b)| b.amount)
        .map(|(&clan_id, b)| (clan_id, b.amount))
}

/// `ClanHallAuctionManager.checkForClanBid` — is the clan bidding on some
/// *other* hall?
pub(crate) fn is_bidding_elsewhere(world: &World, hall_id: i32, clan_id: i32) -> bool {
    world
        .clan_hall_bids
        .iter()
        .any(|(&id, bids)| id != hall_id && bids.contains_key(&clan_id))
}

/// Whether this clan already owns any hall (Java `getClanHallByClan != null`).
fn owns_a_hall(world: &World, clan_id: i32) -> bool {
    world.clan_halls.values().any(|h| h.owner_id == clan_id)
}

/// The hall this clan is bidding on, if any (Java `getClanHallAuctionByClan`).
pub(crate) fn clan_bid_hall(world: &World, clan_id: i32) -> Option<i32> {
    world
        .clan_hall_bids
        .iter()
        .find(|(_, bids)| bids.contains_key(&clan_id))
        .map(|(&id, _)| id)
}

/// The hall an agent NPC belongs to (Java `Npc.getClanHall()`) — the hall whose
/// `<npcs>` list names this template id.
pub(crate) fn hall_by_npc_id(world: &World, npc_id: i32) -> Option<i32> {
    world
        .clan_halls
        .values()
        .find(|h| h.npcs.contains(&npc_id))
        .map(|h| h.id)
}

/// `(owner clan id, hall id)` for the hall an agent NPC belongs to.
///
/// `None` when the NPC is not a hall agent, which for a registered manager or
/// door manager should not happen — the scripts treat it as "refuse", not as
/// "unowned", because an unowned hall is `owner_id == 0` and that is a
/// different answer.
pub(crate) fn hall_ownership(world: &World, npc_id: i32) -> Option<(i32, i32)> {
    let hall_id = hall_by_npc_id(world, npc_id)?;
    let owner_id = world.clan_halls.get(&hall_id).map(|h| h.owner_id)?;
    Some((owner_id, hall_id))
}

/// `ClanHall.banishOthers` — eject every player standing inside the hall who
/// isn't in the owning clan, to the hall's banish point. Uses the `ClanHallZone`
/// (`clan_hall_at`) to decide who is inside.
pub(crate) fn banish_others(world: &mut World, hall_id: i32) {
    let Some((owner_id, banish)) = world
        .clan_halls
        .get(&hall_id)
        .map(|h| (h.owner_id, h.banish))
    else {
        return;
    };
    let player_oids: Vec<i32> = world.in_game_player_oids().collect();
    let mut targets = Vec::new();
    for oid in player_oids {
        let Some(pos) = world
            .objects
            .get_component::<crate::model::components::space::Position>(&oid)
        else {
            continue;
        };
        if world.data.zone_data.clan_hall_at(pos.x, pos.y, pos.z) != Some(hall_id) {
            continue;
        }
        let clan_id = clan_of_or_zero(world, oid);
        if clan_id != owner_id {
            targets.push(oid);
        }
    }
    for oid in targets {
        crate::game_loop::death::teleport_player(world, oid, banish.0, banish.1, banish.2);
    }
}

/// `ClanHall.openCloseDoors` — open or close every door of a hall.
pub(crate) fn open_close_hall_doors(world: &mut World, hall_id: i32, open: bool) {
    let doors = world
        .clan_halls
        .get(&hall_id)
        .map(|h| h.doors.clone())
        .unwrap_or_default();
    for door_id in doors {
        crate::game_loop::npc::doors::set_door_by_id(world, door_id, open);
    }
}

/// `processBidBypass` minus the leadership check (a per-player concern the NPC
/// handler does): validate, take the bidder's adena, refund the previous
/// highest, and record the bid.
pub(crate) fn place_bid(
    world: &mut World,
    hall_id: i32,
    clan_id: i32,
    amount: i64,
    now: i64,
) -> BidOutcome {
    // The hall must be a free, auctionable residence.
    match world.clan_halls.get(&hall_id) {
        Some(h) if h.hall_type == ClanHallType::Auctionable && h.owner_id == 0 => {}
        _ => return BidOutcome::HallUnavailable,
    }
    if world
        .clans
        .get(&clan_id)
        .is_none_or(|c| c.level < MIN_CLAN_LEVEL)
    {
        return BidOutcome::ClanTooLow;
    }
    if owns_a_hall(world, clan_id) {
        return BidOutcome::AlreadyOwnsHall;
    }
    if is_bidding_elsewhere(world, hall_id, clan_id) {
        return BidOutcome::BiddingElsewhere;
    }
    if amount > MAX_ADENA {
        return BidOutcome::BidTooHigh;
    }
    if amount < highest_bid(world, hall_id) {
        return BidOutcome::BidTooLow;
    }
    // Take the bid from the clan warehouse (Java `destroyItemByItemId`).
    if !take_clan_adena(world, clan_id, amount) {
        return BidOutcome::NotEnoughAdena;
    }
    // Refund the *previous* highest bidder, then record ours as the new highest.
    if let Some((prev_clan, prev_amount)) = highest_bidder(world, hall_id) {
        give_clan_adena(world, prev_clan, prev_amount);
        if prev_clan != clan_id {
            crate::game_loop::commerce::warehouse::persist_clan_warehouse(world, prev_clan);
        }
    }
    world.clan_hall_bids.entry(hall_id).or_default().insert(
        clan_id,
        ClanHallBid {
            amount,
            bid_time: now,
        },
    );
    // Persist the bidder's warehouse (adena moved) and the bid row.
    crate::game_loop::commerce::warehouse::persist_clan_warehouse(world, clan_id);
    let _ = world.db.send(DbCommand::SaveClanHallBid {
        hall_id,
        clan_id,
        bid: amount,
        bid_time: now,
    });
    BidOutcome::Accepted
}

/// `ClanHallAuction.removeBid` — cancel a clan's bid. No refund (Java), so the
/// current highest forfeits by cancelling. Returns whether a bid was removed.
pub(crate) fn cancel_bid(world: &mut World, hall_id: i32, clan_id: i32) -> bool {
    let removed = world
        .clan_hall_bids
        .get_mut(&hall_id)
        .is_some_and(|bids| bids.remove(&clan_id).is_some());
    if removed {
        let _ = world
            .db
            .send(DbCommand::RemoveClanHallBid { hall_id, clan_id });
    }
    removed
}

/// `ClanHallAuction.finalizeAuctions` — the weekly close: the highest bidder
/// wins the hall (its held adena is consumed), and all bids are cleared.
pub(crate) fn finalize_auction(world: &mut World, hall_id: i32) {
    let Some((winner, _)) = highest_bidder(world, hall_id) else {
        return; // no bids — the hall stays free
    };
    world.clan_hall_bids.remove(&hall_id);
    let _ = world.db.send(DbCommand::ClearClanHallBids { hall_id });
    // `ClanHall.setOwner(clan)` — hand the hall over and start the lease clock.
    set_hall_owner(world, hall_id, winner);
}

// ---------------------------------------------------------------------------
// The lease / rental cycle (Java `ClanHall.setOwner` + `CheckPaymentTask`)
// ---------------------------------------------------------------------------

const DAY_MS: i64 = 86_400_000;
/// One rental period (Java `Duration.ofDays(7)`).
const LEASE_PERIOD_MS: i64 = 7 * DAY_MS;
/// A week overdue (Java `getCostFailDay() > 8`) and the hall is revoked.
const FAIL_LIMIT_DAYS: i64 = 8;

/// `ClanHall.setOwner(clan)`: give the hall to a clan and (re)start its lease
/// clock. A fresh owner's first rent is due in a week.
pub(crate) fn set_hall_owner(world: &mut World, hall_id: i32, clan_id: i32) {
    let now = commons::util::now_millis();
    let paid_until = {
        let Some(hall) = world.clan_halls.get_mut(&hall_id) else {
            return;
        };
        hall.owner_id = clan_id;
        if hall.paid_until == 0 {
            hall.paid_until = now + LEASE_PERIOD_MS;
        }
        hall.paid_until
    };
    let _ = world.db.send(DbCommand::SaveClanHall {
        id: hall_id,
        owner_id: clan_id,
        paid_until,
    });
    arm_lease_check(world, hall_id);
}

/// `ClanHall.setOwner(null)`: revoke a hall — it returns to the free pool and its
/// lease clock stops. (The pending `ClanHallLeaseCheck` finds no owner and no-ops.)
pub(crate) fn revoke_hall(world: &mut World, hall_id: i32) {
    if let Some(hall) = world.clan_halls.get_mut(&hall_id) {
        hall.owner_id = 0;
        hall.paid_until = 0;
    }
    let _ = world.db.send(DbCommand::SaveClanHall {
        id: hall_id,
        owner_id: 0,
        paid_until: 0,
    });
}

/// Java `getCostFailDay` — whole days the rent is overdue (0 if not yet due).
fn cost_fail_days(paid_until: i64, now: i64) -> i64 {
    if now > paid_until {
        (now - paid_until) / DAY_MS
    } else {
        0
    }
}

/// Arm the next lease check at the hall's `paidUntil` (immediately if overdue).
pub(crate) fn arm_lease_check(world: &mut World, hall_id: i32) {
    let now = commons::util::now_millis();
    let Some(paid_until) = world.clan_halls.get(&hall_id).map(|h| h.paid_until) else {
        return;
    };
    let delay_ms = (paid_until - now).max(0).min(i32::MAX as i64) as i32;
    world.scheduler.schedule(
        world.tick + crate::scheduler::ms_to_ticks(delay_ms),
        ScheduledTask::ClanHallLeaseCheck { hall_id },
    );
}

/// `ClanHall.CheckPaymentTask`: charge the weekly rent from the owner's
/// warehouse. If it can't pay, retry tomorrow — unless the rent is more than a
/// week overdue, in which case the hall is revoked.
pub(crate) fn handle_lease_check(world: &mut World, hall_id: i32) {
    let now = commons::util::now_millis();
    let Some((owner_id, paid_until, lease)) = world
        .clan_halls
        .get(&hall_id)
        .filter(|h| h.owner_id != 0)
        .map(|h| (h.owner_id, h.paid_until, h.lease))
    else {
        return; // no owner — nothing to charge
    };

    let can_pay = world
        .clans
        .get(&owner_id)
        .is_some_and(|c| c.warehouse.0.count_of(ADENA_ID) >= lease);

    if !can_pay {
        if cost_fail_days(paid_until, now) > FAIL_LIMIT_DAYS {
            // A week overdue → ownership revoked, and Java tells the clan
            // before taking the hall (`broadcastToOnlineMembers` then
            // `setOwner(null)`), so the members learn why the hall vanished.
            crate::game_loop::clans::broadcast_to_clan(
                world,
                owner_id,
                &crate::network::server_packets::system_message_with(
                    crate::network::server_packets::sm_ids::THE_CLAN_HALL_FEE_IS_ONE_WEEK_OVERDUE,
                    &[],
                ),
            );
            revoke_hall(world, hall_id);
        } else {
            // Java's daily reminder, carrying the outstanding lease so the
            // clan knows how much to bank.
            crate::game_loop::clans::broadcast_to_clan(
                world,
                owner_id,
                &crate::network::server_packets::system_message_with(
                    crate::network::server_packets::sm_ids::PAYMENT_FOR_YOUR_CLAN_HALL_HAS_NOT_BEEN_MADE,
                    &[commons::system_messages::SmParam::Int(lease as i32)],
                ),
            );
            world.scheduler.schedule(
                world.tick + crate::scheduler::ms_to_ticks(DAY_MS),
                ScheduledTask::ClanHallLeaseCheck { hall_id },
            );
        }
        return;
    }

    // Pay the rent and advance the clock a week.
    if let Some(clan) = world.clans.get_mut(&owner_id) {
        clan.warehouse.0.remove_item(ADENA_ID, lease);
    }
    crate::game_loop::commerce::warehouse::persist_clan_warehouse(world, owner_id);
    let new_paid_until = if let Some(hall) = world.clan_halls.get_mut(&hall_id) {
        hall.paid_until += LEASE_PERIOD_MS;
        hall.paid_until
    } else {
        return;
    };
    let _ = world.db.send(DbCommand::SaveClanHall {
        id: hall_id,
        owner_id,
        paid_until: new_paid_until,
    });
    arm_lease_check(world, hall_id);
}

/// The weekly close (`ClanHallAuctionManager.onEnd`): finalize every hall that
/// has bids, then re-arm for next week.
pub(crate) fn handle_auction_end(world: &mut World) {
    let halls: Vec<i32> = world.clan_hall_bids.keys().copied().collect();
    for hall_id in halls {
        finalize_auction(world, hall_id);
    }
    schedule_weekly_close(world);
}

/// Arm the next weekly auction close.
pub(crate) fn schedule_weekly_close(world: &mut World) {
    world.auction_end_tick = world.tick + WEEK_TICKS;
    world
        .scheduler
        .schedule(world.auction_end_tick, ScheduledTask::ClanHallAuctionEnd);
}

/// How many clans have a standing bid on a hall (Java `getBidCount`).
pub(crate) fn bid_count(world: &World, hall_id: i32) -> usize {
    world
        .clan_hall_bids
        .get(&hall_id)
        .map(|b| b.len())
        .unwrap_or(0)
}

/// Take `amount` adena from a clan's warehouse; `false` if it hasn't enough.
fn take_clan_adena(world: &mut World, clan_id: i32, amount: i64) -> bool {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return false;
    };
    if clan.warehouse.0.count_of(ADENA_ID) < amount {
        return false;
    }
    clan.warehouse.0.remove_item(ADENA_ID, amount);
    true
}

/// Return `amount` adena to a clan's warehouse (an outbid refund).
fn give_clan_adena(world: &mut World, clan_id: i32, amount: i64) {
    let Some(oid) = world.alloc_object_id() else {
        return;
    };
    let catalog = &world.data.item_data;
    if let Some(clan) = world.clans.get_mut(&clan_id) {
        clan.warehouse.0.add_item(catalog, oid, ADENA_ID, amount);
    }
}
