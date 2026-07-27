//! Item-auction house (G30.5) — port of Java `ItemAuctionManager` +
//! `ItemAuctionInstance`. Boot load (slice 1), plus the auction lifecycle +
//! scheduling (slice 2): each auctioneer instance keeps a current + next
//! auction and drives CREATED→STARTED→FINISHED on a per-auction timer. Bidding
//! and the winner→warehouse delivery come in slices 3–4.

use rand::Rng;
use tracing::info;

use crate::db::DbCommand;
use crate::model::item_auction::{AuctionState, ItemAuction};
use crate::scheduler::ScheduledTask;
use crate::world::World;

const TICKS_PER_SECOND: u64 = 10;
const MINUTE_MILLIS: i64 = 60_000;
/// Java `START_TIME_SPACE` (1 min) / `FINISH_TIME_SPACE` (10 min).
const START_TIME_SPACE: i64 = MINUTE_MILLIS;
const FINISH_TIME_SPACE: i64 = 10 * MINUTE_MILLIS;

/// Boot restore (Java `ItemAuctionManager` constructor), driven by
/// `DbEvent::ItemAuctionsLoaded`: gate on `AltItemAuctionEnabled`, seed the
/// auction-id allocator, load the persisted in-flight auctions, then pick the
/// current/next auction for each configured instance and arm its state task.
pub(crate) fn on_loaded(world: &mut World, next_auction_id: i32, auctions: Vec<ItemAuction>) {
    if !world.cfg.general.alt_item_auction_enabled {
        return;
    }
    world.item_auctions.enabled = true;
    world.item_auctions.next_auction_id = next_auction_id.max(1);
    world.item_auctions.auctions = auctions.into_iter().map(|a| (a.auction_id, a)).collect();

    // Each auctioneer instance (Java `ItemAuctionInstance`'s constructor tail).
    let instance_ids: Vec<i32> = world
        .data
        .item_auctions
        .iter()
        .map(|c| c.instance_id)
        .collect();
    for instance_id in instance_ids {
        check_and_set_current_and_next(world, instance_id);
    }
}

/// Java `ItemAuctionInstance.checkAndSetCurrentAndNextAuction`: from this
/// instance's live auctions, pick the current (running/finished) and next
/// (created) auction — creating a fresh one when needed — and arm the state
/// task for whichever transitions first.
pub(crate) fn check_and_set_current_and_next(world: &mut World, instance_id: i32) {
    let now = commons::util::now_millis();

    // The instance's live auctions, newest start first (Java sorts reversed).
    let mut ids: Vec<i32> = world
        .item_auctions
        .auctions_for(instance_id)
        .map(|a| a.auction_id)
        .collect();
    ids.sort_by_key(|&id| std::cmp::Reverse(start_of(world, id)));

    let mut current: Option<i32> = None;
    let mut next: Option<i32> = None;

    match ids.len() {
        0 => {
            next = Some(create_auction(world, instance_id, now + START_TIME_SPACE));
        }
        1 => {
            let id = ids[0];
            match state_of(world, id) {
                AuctionState::Created => {
                    if start_of(world, id) < now + START_TIME_SPACE {
                        current = Some(id);
                        next = Some(create_auction(world, instance_id, now + START_TIME_SPACE));
                    } else {
                        next = Some(id);
                    }
                }
                AuctionState::Started => {
                    current = Some(id);
                    let after = (end_of(world, id) + FINISH_TIME_SPACE).max(now + START_TIME_SPACE);
                    next = Some(create_auction(world, instance_id, after));
                }
                AuctionState::Finished => {
                    current = Some(id);
                    next = Some(create_auction(world, instance_id, now + START_TIME_SPACE));
                }
            }
        }
        _ => {
            // Highest-priority current: a STARTED one, else the first already
            // started-by-time (ids are newest-start-first).
            for &id in &ids {
                if state_of(world, id) == AuctionState::Started {
                    current = Some(id);
                    break;
                }
                if start_of(world, id) <= now {
                    current = Some(id);
                    break;
                }
            }
            for &id in &ids {
                if start_of(world, id) > now && current != Some(id) {
                    next = Some(id);
                    break;
                }
            }
            if next.is_none() {
                next = Some(create_auction(world, instance_id, now + START_TIME_SPACE));
            }
        }
    }

    let rt = world
        .item_auctions
        .instances
        .entry(instance_id)
        .or_default();
    rt.current = current;
    rt.next = next;

    // Arm the state task (Java's tail): the current auction's end/start when it
    // is not finished, else the next auction's start.
    let (task_id, at) = match current {
        Some(id) if state_of(world, id) != AuctionState::Finished => {
            let at = if state_of(world, id) == AuctionState::Started {
                end_of(world, id)
            } else {
                start_of(world, id)
            };
            (id, at)
        }
        _ => {
            let id = next.expect("next is always set");
            (id, start_of(world, id))
        }
    };
    schedule_at(world, task_id, at);
}

/// Java `ItemAuctionInstance.ScheduleAuctionTask.runImpl`: advance one auction's
/// state. CREATED→STARTED, or STARTED→FINISHED (honoring a bid-driven ending
/// extension by re-arming), then re-pick current/next.
pub(crate) fn run_state_task(world: &mut World, auction_id: i32) {
    let Some(instance_id) = world
        .item_auctions
        .auctions
        .get(&auction_id)
        .map(|a| a.instance_id)
    else {
        return;
    };
    match state_of(world, auction_id) {
        AuctionState::Created => {
            set_state(world, auction_id, AuctionState::Started);
            info!("ItemAuction: auction {auction_id} started (instance {instance_id}).");
            check_and_set_current_and_next(world, instance_id);
        }
        AuctionState::Started => {
            // A bid in the last 10 min may have extended the end time; the
            // scheduled-vs-actual extend-state gate re-arms until they agree
            // (Java's `ScheduleAuctionTask` STARTED switch). Until bidding lands
            // (slice 3) the extend state is always Initial, so this falls
            // straight through to FINISHED.
            if reschedule_for_extend(world, auction_id) {
                return;
            }
            set_state(world, auction_id, AuctionState::Finished);
            on_auction_finished(world, auction_id);
            check_and_set_current_and_next(world, instance_id);
        }
        AuctionState::Finished => {}
    }
}

/// Java's STARTED-case extend re-check: if the auction was extended past what
/// this task was scheduled for, advance the scheduled-extend state and re-arm
/// at the new end time, returning `true`. Inert until bidding (slice 3) sets a
/// non-`Initial` extend state.
fn reschedule_for_extend(world: &mut World, auction_id: i32) -> bool {
    use crate::model::item_auction::ExtendState::*;
    let Some(a) = world.item_auctions.auctions.get_mut(&auction_id) else {
        return false;
    };
    let advance = match a.extend_state {
        Initial => false,
        ExtendBy5Min => a.scheduled_extend_state == Initial,
        ExtendBy3Min => a.scheduled_extend_state != ExtendBy3Min,
        ExtendByConfigPhaseA => a.scheduled_extend_state != ExtendByConfigPhaseB,
        ExtendByConfigPhaseB => a.scheduled_extend_state != ExtendByConfigPhaseA,
    };
    if !advance {
        return false;
    }
    // Mirror Java: the scheduled state chases the actual one, then re-arm.
    a.scheduled_extend_state = match a.extend_state {
        ExtendByConfigPhaseA => ExtendByConfigPhaseB,
        ExtendByConfigPhaseB => ExtendByConfigPhaseA,
        other => other,
    };
    let end = a.ending_time;
    schedule_at(world, auction_id, end);
    true
}

/// Java `onAuctionFinished` — the winner→warehouse delivery. Slice 4; for now
/// just announce (no bids on this dist yet) so the lifecycle is observable.
fn on_auction_finished(world: &mut World, auction_id: i32) {
    // TODO(G30.5) slice 4: deliver the item to the highest bidder's warehouse
    //   (offline: set owner + WAREHOUSE location), clear canceled bids. With no
    //   bids the auction simply closes.
    info!("ItemAuction: auction {auction_id} finished.");
    let _ = world;
}

/// Java `createAuction(after)`: a random catalogue item, the next scheduled
/// start (≥ `after`), a fresh id + row, inserted live.
fn create_auction(world: &mut World, instance_id: i32, after: i64) -> i32 {
    let Some(cfg) = world.data.item_auctions.get(instance_id) else {
        // Shouldn't happen (callers iterate configured instances), but stay safe.
        return world.item_auctions.alloc_auction_id();
    };
    let sched = cfg.schedule;
    let item = &cfg.items[world.rng.gen_range(0..cfg.items.len())];
    let (auction_item_id, length_min) = (item.auction_item_id, item.auction_length_min);

    let starting_time = crate::model::item_auction::next_date(
        after,
        sched.weekday,
        sched.interval_days,
        sched.hour,
        sched.minute,
    );
    let ending_time = starting_time + length_min as i64 * MINUTE_MILLIS;

    let auction_id = world.item_auctions.alloc_auction_id();
    let auction = ItemAuction::new(
        auction_id,
        instance_id,
        auction_item_id,
        starting_time,
        ending_time,
        AuctionState::Created,
    );
    persist(world, &auction);
    world.item_auctions.auctions.insert(auction_id, auction);
    auction_id
}

// --- helpers ---

fn state_of(world: &World, auction_id: i32) -> AuctionState {
    world
        .item_auctions
        .auctions
        .get(&auction_id)
        .map_or(AuctionState::Finished, |a| a.state)
}

fn start_of(world: &World, auction_id: i32) -> i64 {
    world
        .item_auctions
        .auctions
        .get(&auction_id)
        .map_or(0, |a| a.starting_time)
}

fn end_of(world: &World, auction_id: i32) -> i64 {
    world
        .item_auctions
        .auctions
        .get(&auction_id)
        .map_or(0, |a| a.ending_time)
}

fn set_state(world: &mut World, auction_id: i32, state: AuctionState) {
    if let Some(a) = world.item_auctions.auctions.get_mut(&auction_id) {
        a.state = state;
    }
    // Java `storeMe` on every state change.
    if let Some(a) = world.item_auctions.auctions.get(&auction_id).cloned() {
        persist(world, &a);
    }
}

fn persist(world: &World, a: &ItemAuction) {
    let _ = world.db.send(DbCommand::StoreItemAuction {
        auction_id: a.auction_id,
        instance_id: a.instance_id,
        auction_item_id: a.auction_item_id,
        starting_time: a.starting_time,
        ending_time: a.ending_time,
        state_id: a.state.state_id(),
    });
}

/// Arm the auction's state task at wall-clock `at_millis` (Java `ThreadPool
/// .schedule(task, max(target - now, 0))`).
fn schedule_at(world: &mut World, auction_id: i32, at_millis: i64) {
    let now = commons::util::now_millis();
    let delay_ticks = ((at_millis - now).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::ItemAuctionState { auction_id },
    );
}
