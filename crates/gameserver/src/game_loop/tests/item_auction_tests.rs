//! Item auction (G30.5) — slice 1: the schedule math, the config gate, and the
//! boot load of persisted auctions + the auction-id allocator.

use super::*;

use crate::game_loop::item_auction;
use crate::model::item_auction::{next_date, AuctionState, ItemAuction};

const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;

#[test]
fn next_date_interval_rolls_forward_to_the_next_occurrence() {
    // "Every day at 20:00." From 2024-01-01 10:00 UTC → the same day 20:00.
    let base = 19723 * DAY; // 2024-01-01 00:00 UTC (epoch day 19723)
    let now = base + 10 * HOUR;
    let got = next_date(now, None, Some(1), 20, 0);
    assert_eq!(got, base + 20 * HOUR);
    // From 21:00 (past today's 20:00) → tomorrow 20:00.
    let got2 = next_date(base + 21 * HOUR, None, Some(1), 20, 0);
    assert_eq!(got2, base + DAY + 20 * HOUR);
}

#[test]
fn next_date_weekday_lands_on_the_target_weekday() {
    // 1970-01-01 was Thursday (weekday 3 in Mon=0..Sun=6). Ask for Monday (0).
    let now = 0; // epoch
    let got = next_date(now, Some(0), None, 12, 0);
    // The next Monday at 12:00 is epoch day 4 (1970-01-05).
    assert_eq!(got, 4 * DAY + 12 * HOUR);
    assert_eq!((got.rem_euclid(DAY)) / HOUR, 12);
    // And it is indeed a Monday: (day + 3) % 7 == 0.
    assert_eq!((got / DAY + 3).rem_euclid(7), 0);
}

#[test]
fn a_disabled_auction_house_stays_inert() {
    let (mut world, _tx, _rx, _link) = test_world();
    world.cfg.general.alt_item_auction_enabled = false;
    item_auction::on_loaded(&mut world, 5, vec![]);
    assert!(!world.item_auctions.enabled);
    assert_eq!(world.item_auctions.next_auction_id, 0); // untouched
}

#[test]
fn boot_load_seeds_the_allocator_and_the_live_auctions() {
    let (mut world, _tx, _rx, _link) = test_world();
    world.cfg.general.alt_item_auction_enabled = true;
    let auction = ItemAuction::new(7, 31113, 1, 1000, 2000, AuctionState::Started);
    item_auction::on_loaded(&mut world, 8, vec![auction]);

    assert!(world.item_auctions.enabled);
    assert_eq!(world.item_auctions.next_auction_id, 8);
    assert!(world.item_auctions.auctions.contains_key(&7));
    // The allocator hands out the loaded next id, then advances.
    assert_eq!(world.item_auctions.alloc_auction_id(), 8);
    assert_eq!(world.item_auctions.alloc_auction_id(), 9);
}

#[test]
fn highest_bid_ignores_canceled_bids() {
    use crate::model::item_auction::ItemAuctionBid;
    let mut a = ItemAuction::new(1, 1, 1, 0, 0, AuctionState::Started);
    a.bids.push(ItemAuctionBid {
        player_obj_id: 10,
        last_bid: 500,
    });
    a.bids.push(ItemAuctionBid {
        player_obj_id: 20,
        last_bid: -1,
    }); // canceled
    a.bids.push(ItemAuctionBid {
        player_obj_id: 30,
        last_bid: 300,
    });
    assert_eq!(a.highest_bid().unwrap().player_obj_id, 10);
}
