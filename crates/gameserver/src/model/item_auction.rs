//! Item-auction runtime state (G30.5) — the World-side counterpart of Java's
//! `ItemAuction*` model. This slice is the data model, the manager runtime, and
//! the pure schedule math; the lifecycle state machine, bidding, and delivery
//! come in later slices.

use std::collections::HashMap;

const MILLIS_PER_MINUTE: i64 = 60_000;
const MILLIS_PER_HOUR: i64 = 3_600_000;
const MILLIS_PER_DAY: i64 = 86_400_000;
const MILLIS_PER_WEEK: i64 = 7 * MILLIS_PER_DAY;

/// An auction's lifecycle state (Java `ItemAuctionState`; the byte ids persist
/// in `item_auction.auctionStateId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionState {
    Created,
    Started,
    Finished,
}

impl AuctionState {
    pub fn state_id(self) -> i8 {
        match self {
            AuctionState::Created => 0,
            AuctionState::Started => 1,
            AuctionState::Finished => 2,
        }
    }

    pub fn from_state_id(id: i8) -> Option<Self> {
        match id {
            0 => Some(AuctionState::Created),
            1 => Some(AuctionState::Started),
            2 => Some(AuctionState::Finished),
            _ => None,
        }
    }
}

/// The ending-extend phase of a running auction (Java `ItemAuctionExtendState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtendState {
    #[default]
    Initial,
    ExtendBy5Min,
    ExtendBy3Min,
    ExtendByConfigPhaseA,
    ExtendByConfigPhaseB,
}

/// One player's standing bid on an auction (Java `ItemAuctionBid`). A canceled
/// bid keeps a row with `last_bid <= 0` until cleared.
#[derive(Debug, Clone, Copy)]
pub struct ItemAuctionBid {
    pub player_obj_id: i32,
    pub last_bid: i64,
}

impl ItemAuctionBid {
    pub fn is_canceled(&self) -> bool {
        self.last_bid <= 0
    }
}

/// One auction (Java `ItemAuction`). `auction_item_id` points into the owning
/// instance's catalogue (resolved to the reward item at delivery).
#[derive(Debug, Clone)]
pub struct ItemAuction {
    pub auction_id: i32,
    pub instance_id: i32,
    pub auction_item_id: i32,
    pub starting_time: i64,
    pub ending_time: i64,
    pub state: AuctionState,
    /// The live ending-extend phase, advanced by bids (Java
    /// `_auctionEndingExtendState`).
    pub extend_state: ExtendState,
    /// The extend phase the state task last scheduled for, chasing `extend_state`
    /// (Java `_scheduledAuctionEndingExtendState`).
    pub scheduled_extend_state: ExtendState,
    pub bids: Vec<ItemAuctionBid>,
    /// The last player to bid, for the extend-only-on-a-different-bidder gate
    /// (Java `_lastBidPlayerObjId`).
    pub last_bid_player: i32,
}

impl ItemAuction {
    pub fn new(
        auction_id: i32,
        instance_id: i32,
        auction_item_id: i32,
        starting_time: i64,
        ending_time: i64,
        state: AuctionState,
    ) -> Self {
        Self {
            auction_id,
            instance_id,
            auction_item_id,
            starting_time,
            ending_time,
            state,
            extend_state: ExtendState::Initial,
            scheduled_extend_state: ExtendState::Initial,
            bids: Vec::new(),
            last_bid_player: 0,
        }
    }

    /// The current highest bid (Java `_highestBid`), ignoring canceled ones.
    pub fn highest_bid(&self) -> Option<&ItemAuctionBid> {
        self.bids
            .iter()
            .filter(|b| !b.is_canceled())
            .max_by_key(|b| b.last_bid)
    }

    pub fn bid_of(&self, player_obj_id: i32) -> Option<&ItemAuctionBid> {
        self.bids.iter().find(|b| b.player_obj_id == player_obj_id)
    }
}

/// One auctioneer instance's live pointers (Java `ItemAuctionInstance`'s
/// `_currentAuction`/`_nextAuction`) — the current (running/finished) and next
/// (created) auction ids.
#[derive(Debug, Clone, Copy, Default)]
pub struct InstanceRuntime {
    pub current: Option<i32>,
    pub next: Option<i32>,
}

/// The item-auction manager runtime (Java `ItemAuctionManager` + the live
/// per-instance auctions). Inert unless `AltItemAuctionEnabled`.
#[derive(Debug, Default)]
pub struct ItemAuctionManager {
    /// Whether the engine is running (config `AltItemAuctionEnabled`).
    pub enabled: bool,
    /// The next auction id to allocate (Java `_auctionIds`, resumed from
    /// `MAX(auctionId)+1` at boot).
    pub next_auction_id: i32,
    /// Live auctions by id (across all instances), loaded from `item_auction`.
    pub auctions: HashMap<i32, ItemAuction>,
    /// Per-auctioneer-NPC current/next pointers (Java the per-instance fields).
    pub instances: HashMap<i32, InstanceRuntime>,
}

impl ItemAuctionManager {
    /// Allocate the next auction id (Java `getNextAuctionId`).
    pub fn alloc_auction_id(&mut self) -> i32 {
        let id = self.next_auction_id.max(1);
        self.next_auction_id = id + 1;
        id
    }

    /// Every live auction belonging to one auctioneer instance.
    pub fn auctions_for(&self, instance_id: i32) -> impl Iterator<Item = &ItemAuction> {
        self.auctions
            .values()
            .filter(move |a| a.instance_id == instance_id)
    }
}

/// The next occurrence of a schedule at or after `now_millis` (Java
/// `AuctionDateGenerator.nextDate`), computed in UTC like the siege schedule.
/// `weekday` is `Mon=0..=Sun=6`; exactly one of `weekday`/`interval_days` is set.
pub fn next_date(
    now_millis: i64,
    weekday: Option<u32>,
    interval_days: Option<i32>,
    hour: u32,
    minute: u32,
) -> i64 {
    let now_day = now_millis.div_euclid(MILLIS_PER_DAY);
    let time_of_day = hour as i64 * MILLIS_PER_HOUR + minute as i64 * MILLIS_PER_MINUTE;
    if let Some(target) = weekday {
        // This week's target weekday at hh:mm, then roll forward by whole weeks.
        let now_weekday = (now_day + 3).rem_euclid(7) as u32; // 1970-01-01 was Thu
        let delta = target as i64 - now_weekday as i64;
        let candidate = (now_day + delta) * MILLIS_PER_DAY + time_of_day;
        calc_dest_time(candidate, now_millis, MILLIS_PER_WEEK)
    } else {
        // Today at hh:mm, then roll forward by whole intervals.
        let interval = interval_days.unwrap_or(1).max(1) as i64 * MILLIS_PER_DAY;
        let candidate = now_day * MILLIS_PER_DAY + time_of_day;
        calc_dest_time(candidate, now_millis, interval)
    }
}

/// Java `AuctionDateGenerator.calcDestTime`: roll `time` forward by whole `add`
/// steps until it is not before `date`.
fn calc_dest_time(time: i64, date: i64, add: i64) -> i64 {
    let mut time = time;
    if time < date {
        time += ((date - time) / add) * add;
        if time < date {
            time += add;
        }
    }
    time
}
