//! Item-auction house (G30.5) — port of Java `ItemAuctionManager` +
//! `ItemAuctionInstance` + `ItemAuctionLink`. Boot load (slice 1), the auction
//! lifecycle + scheduling (slice 2), and bidding — adena escrow, outbid, the
//! ending-extend state machine, and cancel/refund — plus the auctioneer NPC
//! dialog + the client packets (slice 3). Winner→warehouse delivery is slice 4.

use rand::Rng;
use tracing::info;

use crate::db::DbCommand;
use crate::model::inventory::Inventory;
use crate::model::item_auction::{AuctionState, ExtendState, ItemAuction};
use crate::network::server_packets::{self as sp, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::world::World;

const TICKS_PER_SECOND: u64 = 10;
const MINUTE_MILLIS: i64 = 60_000;
/// Java `START_TIME_SPACE` (1 min) / `FINISH_TIME_SPACE` (10 min).
const START_TIME_SPACE: i64 = MINUTE_MILLIS;
const FINISH_TIME_SPACE: i64 = 10 * MINUTE_MILLIS;
const ADENA_ID: i32 = 57;
/// Java's hard cap on a bid (999.9 bn).
const MAX_BID: i64 = 100_000_000_000;
/// The last-10-minutes window in which a bid extends the auction.
const EXTEND_WINDOW_MILLIS: i64 = 10 * MINUTE_MILLIS;
const EXTEND_5_MILLIS: i64 = 5 * MINUTE_MILLIS;
const EXTEND_3_MILLIS: i64 = 3 * MINUTE_MILLIS;

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

// ---------------------------------------------------------------------------
// Bidding (Java `ItemAuction.registerBid` / `cancelBid`)
// ---------------------------------------------------------------------------

/// Java `ItemAuction.registerBid`: place `bid` adena on the given auctioneer's
/// current auction. Escrows the adena (full for a new bid, the delta when
/// raising your own, full again after a cancel), tracks the highest bid, and
/// extends the ending time when bid in the last 10 minutes.
pub(crate) fn register_bid(
    world: &mut World,
    instance_id: i32,
    client_id: u32,
    player: i32,
    bid: i64,
) {
    let Some(auction_id) = world
        .item_auctions
        .instances
        .get(&instance_id)
        .and_then(|rt| rt.current)
    else {
        return;
    };
    let Some(a) = world.item_auctions.auctions.get(&auction_id) else {
        return;
    };
    let init_bid = init_bid_of(world, auction_id);
    if bid < init_bid {
        send_sm(
            world,
            client_id,
            sm_ids::YOUR_BID_PRICE_MUST_BE_HIGHER_THAN_THE_MINIMUM_PRICE,
        );
        return;
    }
    if bid > MAX_BID {
        send_sm(
            world,
            client_id,
            sm_ids::THE_HIGHEST_BID_IS_OVER_999_9_BILLION,
        );
        return;
    }
    if a.state != AuctionState::Started {
        return;
    }
    let highest = a.highest_bid().map(|b| b.last_bid);
    if highest.is_some_and(|h| bid < h) {
        send_sm(
            world,
            client_id,
            sm_ids::YOUR_BID_MUST_BE_HIGHER_THAN_THE_CURRENT_HIGHEST_BID,
        );
        return;
    }

    // Escrow (Java `reduceItemCount`): the amount depends on the player's
    // existing bid on this auction.
    let existing = a.bid_of(player).copied();
    let charge = match existing {
        None => bid,
        Some(b) if b.is_canceled() => bid,
        Some(b) => {
            if bid < b.last_bid {
                send_sm(
                    world,
                    client_id,
                    sm_ids::YOUR_BID_MUST_BE_HIGHER_THAN_THE_CURRENT_HIGHEST_BID,
                );
                return;
            }
            bid - b.last_bid
        }
    };
    if !reduce_adena(world, player, charge) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA_FOR_THIS_BID,
        );
        return;
    }

    // Record the bid.
    let prev_highest_player = world
        .item_auctions
        .auctions
        .get(&auction_id)
        .and_then(|a| a.highest_bid())
        .map(|b| b.player_obj_id);
    {
        let a = world.item_auctions.auctions.get_mut(&auction_id).unwrap();
        match a.bids.iter_mut().find(|b| b.player_obj_id == player) {
            Some(b) => b.last_bid = bid,
            None => a.bids.push(crate::model::item_auction::ItemAuctionBid {
                player_obj_id: player,
                last_bid: bid,
            }),
        }
    }
    let _ = world.db.send(DbCommand::StoreItemAuctionBid {
        auction_id,
        player_obj_id: player,
        bid,
    });

    // Notify a displaced previous highest bidder (Java `onPlayerBid`).
    if let Some(prev) = prev_highest_player {
        if prev != player {
            if let Some(cid) = crate::game_loop::helpers::client_for_player(world, prev) {
                send_pkt(
                    world,
                    cid,
                    sp::system_message_with(
                        sm_ids::YOU_WERE_OUTBID_THE_NEW_HIGHEST_BID_IS_S1_ADENA,
                        &[SmParam::Long(bid)],
                    ),
                );
            }
        }
    }

    apply_ending_extend(world, auction_id, player);

    send_pkt(
        world,
        client_id,
        sp::system_message_with(
            sm_ids::YOU_HAVE_SUBMITTED_A_BID_FOR_THE_AUCTION_OF_S1,
            &[SmParam::Long(bid)],
        ),
    );
}

/// Java `onPlayerBid`'s tail: a bid in the last 10 minutes extends the ending
/// time through the state ladder (INITIAL → +5min → +3min → config phases),
/// each step past the first gated on a *different* player bidding.
fn apply_ending_extend(world: &mut World, auction_id: i32, player: i32) {
    let cfg_extend = world.cfg.general.alt_item_auction_time_extends_on_bid;
    let now = commons::util::now_millis();
    let Some(a) = world.item_auctions.auctions.get_mut(&auction_id) else {
        return;
    };
    if a.ending_time - now > EXTEND_WINDOW_MILLIS {
        return;
    }
    let mut extended_5 = false;
    let mut extended_3 = false;
    match a.extend_state {
        ExtendState::Initial => {
            a.extend_state = ExtendState::ExtendBy5Min;
            a.ending_time += EXTEND_5_MILLIS;
            extended_5 = true;
        }
        ExtendState::ExtendBy5Min => {
            if a.last_bid_player != player {
                a.extend_state = ExtendState::ExtendBy3Min;
                a.ending_time += EXTEND_3_MILLIS;
                extended_3 = true;
            }
        }
        ExtendState::ExtendBy3Min => {
            if cfg_extend > 0 && a.last_bid_player != player {
                a.extend_state = ExtendState::ExtendByConfigPhaseA;
                a.ending_time += cfg_extend;
            }
        }
        ExtendState::ExtendByConfigPhaseA => {
            if a.last_bid_player != player
                && a.scheduled_extend_state == ExtendState::ExtendByConfigPhaseB
            {
                a.extend_state = ExtendState::ExtendByConfigPhaseB;
                a.ending_time += cfg_extend;
            }
        }
        ExtendState::ExtendByConfigPhaseB => {
            if a.last_bid_player != player
                && a.scheduled_extend_state == ExtendState::ExtendByConfigPhaseA
            {
                a.ending_time += cfg_extend;
                a.extend_state = ExtendState::ExtendByConfigPhaseA;
            }
        }
    }
    a.last_bid_player = player;
    if extended_5 {
        broadcast_to_bidders(
            world,
            auction_id,
            sm_ids::BIDDER_EXISTS_THE_AUCTION_TIME_HAS_BEEN_EXTENDED_BY_5_MINUTES,
        );
    } else if extended_3 {
        broadcast_to_bidders(
            world,
            auction_id,
            sm_ids::BIDDER_EXISTS_AUCTION_TIME_HAS_BEEN_EXTENDED_BY_3_MINUTES,
        );
    }
}

/// Java `ItemAuction.cancelBid`: reclaim a losing bid's escrowed adena and mark
/// it canceled. Returns `true` when the player had a cancelable/held bid (Java's
/// return, which also covers the "you have the highest bid" reserve case).
pub(crate) fn cancel_bid(world: &mut World, auction_id: i32, client_id: u32, player: i32) -> bool {
    let Some(a) = world.item_auctions.auctions.get(&auction_id) else {
        return false;
    };
    match a.state {
        AuctionState::Created => return false,
        AuctionState::Finished => {
            let expired_after = world.cfg.general.alt_item_auction_expired_after_days as i64;
            let now = commons::util::now_millis();
            if a.starting_time < now - expired_after * 86_400_000 {
                return false;
            }
        }
        AuctionState::Started => {}
    }
    let Some(highest_player) = a.highest_bid().map(|b| b.player_obj_id) else {
        return false;
    };
    let bid = match a.bid_of(player) {
        Some(b) => *b,
        None => return false,
    };
    // Can't return the winning bid.
    if bid.player_obj_id == highest_player {
        if a.state == AuctionState::Finished {
            return false;
        }
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CURRENTLY_HAVE_THE_HIGHEST_BID_BUT_THE_RESERVE_HAS_NOT_BEEN_MET,
        );
        return true;
    }
    if bid.is_canceled() {
        return false;
    }

    add_adena(world, player, bid.last_bid);
    let finished = matches!(state_of(world, auction_id), AuctionState::Finished);
    if let Some(a) = world.item_auctions.auctions.get_mut(&auction_id) {
        if let Some(b) = a.bids.iter_mut().find(|b| b.player_obj_id == player) {
            b.last_bid = -1; // cancelBid()
        }
    }
    if finished {
        let _ = world.db.send(DbCommand::DeleteItemAuctionBid {
            auction_id,
            player_obj_id: player,
        });
    } else {
        let _ = world.db.send(DbCommand::StoreItemAuctionBid {
            auction_id,
            player_obj_id: player,
            bid: -1,
        });
    }
    send_sm(world, client_id, sm_ids::YOU_HAVE_CANCELED_YOUR_BID);
    true
}

// ---------------------------------------------------------------------------
// The auctioneer NPC dialog + client packets (Java `ItemAuctionLink`,
// `RequestBidItemAuction`, `RequestInfoItemAuction`)
// ---------------------------------------------------------------------------

/// `bypass -h ItemAuction <show|cancel>` from an auctioneer NPC (the NPC id is
/// the auction instance id).
pub(crate) fn link_bypass(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    command: &str,
) {
    let instance_id = npc_id_of(world, npc_oid);
    if world.item_auctions.instances.get(&instance_id).is_none() {
        return;
    }
    match command.split_whitespace().nth(1) {
        Some("show") => match world.item_auctions.instances[&instance_id].current {
            Some(cur) => {
                let pkt = build_info_packet(
                    world,
                    false,
                    cur,
                    world.item_auctions.instances[&instance_id].next,
                );
                send_pkt(world, client_id, pkt);
            }
            None => send_sm(world, client_id, sm_ids::IT_IS_NOT_AN_AUCTION_PERIOD),
        },
        Some("cancel") => {
            // Cancel every held bid across this instance's auctions (Java
            // `getAuctionsByBidder`).
            let ids: Vec<i32> = world
                .item_auctions
                .auctions_for(instance_id)
                .filter(|a| a.state != AuctionState::Created && a.bid_of(player).is_some())
                .map(|a| a.auction_id)
                .collect();
            let mut returned = false;
            for id in ids {
                if cancel_bid(world, id, client_id, player) {
                    returned = true;
                }
            }
            if !returned {
                send_sm(
                    world,
                    client_id,
                    sm_ids::THERE_ARE_NO_OFFERINGS_I_OWN_OR_I_MADE_A_BID_FOR,
                );
            }
        }
        _ => {}
    }
}

/// `RequestBidItemAuction` (Ex 0x36): read `(instanceId, bid)` and bid on the
/// instance's current auction (Java `RequestBidItemAuction.runImpl`;
/// `Inventory.MAX_ADENA` is the same cap as `MAX_BID`).
pub(crate) fn on_request_bid(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(player) = player_of(world, client_id) else {
        return;
    };
    let mut r = commons::network::PacketReader::new(body);
    let (Some(instance_id), Some(bid)) = (r.read_i32(), r.read_i64()) else {
        return;
    };
    if !(0..=MAX_BID).contains(&bid) {
        return;
    }
    register_bid(world, instance_id, client_id, player, bid);
}

/// `RequestInfoItemAuction` (Ex 0x37): read `instanceId` and send its current
/// auction window (Java `RequestInfoItemAuction.runImpl`).
pub(crate) fn on_request_info(world: &mut World, client_id: u32, body: &[u8]) {
    let mut r = commons::network::PacketReader::new(body);
    let Some(instance_id) = r.read_i32() else {
        return;
    };
    let Some(rt) = world.item_auctions.instances.get(&instance_id).copied() else {
        return;
    };
    if let Some(cur) = rt.current {
        let pkt = build_info_packet(world, true, cur, rt.next);
        send_pkt(world, client_id, pkt);
    }
}

fn player_of(world: &World, client_id: u32) -> Option<i32> {
    match world.clients.get(&client_id) {
        Some(crate::session::ClientSession::InGame(s)) => Some(s.player_object_id()),
        _ => None,
    }
}

/// Build `ExItemAuctionInfoPacket` (Java) for the current + optional next auction.
fn build_info_packet(
    world: &World,
    refresh: bool,
    current_id: i32,
    next_id: Option<i32>,
) -> Vec<u8> {
    use commons::network::PacketWriter;
    let mut w = PacketWriter::new();
    w.write_u8(sp::opcodes::EX);
    w.write_i16(sp::opcodes::EX_ITEM_AUCTION_INFO);
    w.write_u8(if refresh { 0 } else { 1 }); // Java writes `!refresh`
    let cur = &world.item_auctions.auctions[&current_id];
    w.write_i32(cur.instance_id);
    let highest = cur.highest_bid().map(|b| b.last_bid);
    w.write_i64(highest.unwrap_or_else(|| init_bid_of(world, current_id)));
    let time_remaining = if cur.state == AuctionState::Started {
        ((cur.ending_time - commons::util::now_millis()).max(0) / 1000) as i32
    } else {
        0
    };
    w.write_i32(time_remaining);
    write_auction_item(world, &mut w, current_id);
    if let Some(next_id) = next_id {
        if let Some(next) = world.item_auctions.auctions.get(&next_id) {
            w.write_i64(init_bid_of(world, next_id));
            w.write_i32((next.starting_time / 1000) as i32);
            write_auction_item(world, &mut w, next_id);
        }
    }
    w.into_bytes()
}

/// Serialize an auction's reward as an item block (Java `writeItem(itemInfo)`),
/// via a synthetic [`Inventory`] item carrying the catalogue row's fields.
fn write_auction_item(world: &World, w: &mut commons::network::PacketWriter, auction_id: i32) {
    let a = &world.item_auctions.auctions[&auction_id];
    let (item_id, count, enchant) = catalogue_item(world, a.instance_id, a.auction_item_id)
        .map(|it| (it.item_id, it.item_count, it.enchant_level))
        .unwrap_or((0, 1, 0));
    let inst = crate::model::inventory::ItemInstance {
        object_id: 0,
        item_id,
        count,
        enchant_level: enchant,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    if let Some(t) = world.data.item_data.get(item_id) {
        crate::network::enter_world::write_item_entry(w, &inst, t, false);
    }
}

// --- bidding helpers ---

fn init_bid_of(world: &World, auction_id: i32) -> i64 {
    let Some(a) = world.item_auctions.auctions.get(&auction_id) else {
        return 0;
    };
    catalogue_item(world, a.instance_id, a.auction_item_id).map_or(0, |it| it.auction_init_bid)
}

fn catalogue_item(
    world: &World,
    instance_id: i32,
    auction_item_id: i32,
) -> Option<&crate::data::item_auction_data::AuctionItem> {
    world
        .data
        .item_auctions
        .get(instance_id)?
        .items
        .iter()
        .find(|it| it.auction_item_id == auction_item_id)
}

fn reduce_adena(world: &mut World, player: i32, count: i64) -> bool {
    let Some(inv) = world.objects.get_component::<Inventory>(&player) else {
        return false;
    };
    if inv.adena() < count {
        return false;
    }
    if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) {
        inv.remove_item(ADENA_ID, count);
    }
    true
}

fn add_adena(world: &mut World, player: i32, count: i64) {
    if count > 0 {
        super::items::add_inventory_item(world, player, ADENA_ID, count);
    }
}

fn broadcast_to_bidders(world: &World, auction_id: i32, sm_id: i16) {
    let Some(a) = world.item_auctions.auctions.get(&auction_id) else {
        return;
    };
    for b in &a.bids {
        if b.is_canceled() {
            continue;
        }
        if let Some(cid) = crate::game_loop::helpers::client_for_player(world, b.player_obj_id) {
            send_sm(world, cid, sm_id);
        }
    }
}

fn npc_id_of(world: &World, npc_oid: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map_or(0, |n| n.npc_id)
}

fn send_sm(world: &World, client_id: u32, sm_id: i16) {
    send_pkt(world, client_id, sp::system_message_with(sm_id, &[]));
}

fn send_pkt(world: &World, client_id: u32, pkt: Vec<u8>) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(pkt);
    }
}
