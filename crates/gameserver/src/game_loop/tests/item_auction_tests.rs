//! Item auction (G30.5) — the schedule math, the config gate, the boot load
//! (slice 1), and the auction lifecycle + scheduling state machine (slice 2).

use super::*;

use crate::data::item_auction_data::{AuctionInstanceCfg, AuctionItem, AuctionSchedule};
use crate::game_loop::item_auction;
use crate::model::item_auction::{next_date, AuctionState, ItemAuction};
use crate::scheduler::ScheduledTask;

const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;

/// An enabled world with one synthetic auctioneer instance (a daily interval).
fn world_with_instance(instance_id: i32) -> World {
    let (mut world, _tx, _rx, _link) = test_world();
    world.cfg.general.alt_item_auction_enabled = true;
    world
        .data
        .item_auctions
        .insert_for_test(AuctionInstanceCfg {
            instance_id,
            schedule: AuctionSchedule {
                interval_days: Some(1),
                weekday: None,
                hour: 12,
                minute: 0,
            },
            items: vec![AuctionItem {
                auction_item_id: 1,
                auction_length_min: 300,
                auction_init_bid: 100_000,
                item_id: 9901,
                item_count: 1,
                enchant_level: 0,
            }],
        });
    world
}

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

// --- Lifecycle + scheduling (slice 2) ---

#[test]
fn boot_with_no_auctions_creates_a_next_auction_and_arms_it() {
    let mut world = world_with_instance(31113);
    item_auction::on_loaded(&mut world, 1, vec![]);

    // One CREATED auction, tracked as the instance's `next`, with a state task.
    assert_eq!(world.item_auctions.auctions.len(), 1);
    let (&id, a) = world.item_auctions.auctions.iter().next().unwrap();
    assert_eq!(a.state, AuctionState::Created);
    assert_eq!(a.instance_id, 31113);
    let rt = world.item_auctions.instances[&31113];
    assert_eq!(rt.next, Some(id));
    assert!(rt.current.is_none());
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::ItemAuctionState { auction_id: id }));
}

#[test]
fn state_task_runs_created_to_started_to_finished() {
    let mut world = world_with_instance(31113);
    item_auction::on_loaded(&mut world, 1, vec![]);
    let id = *world.item_auctions.auctions.keys().next().unwrap();

    // CREATED → STARTED, and a fresh next auction is created + tracked.
    item_auction::run_state_task(&mut world, id);
    assert_eq!(
        world.item_auctions.auctions[&id].state,
        AuctionState::Started
    );
    assert_eq!(world.item_auctions.instances[&31113].current, Some(id));
    assert_eq!(
        world.item_auctions.auctions.len(),
        2,
        "a next auction was created"
    );

    // STARTED → FINISHED (no bids → no extension), and re-pick current/next.
    item_auction::run_state_task(&mut world, id);
    assert_eq!(
        world.item_auctions.auctions[&id].state,
        AuctionState::Finished
    );
}

#[test]
fn a_started_auction_at_boot_becomes_current() {
    let mut world = world_with_instance(31113);
    let now = commons::util::now_millis();
    let started = ItemAuction::new(7, 31113, 1, now - HOUR, now + HOUR, AuctionState::Started);
    item_auction::on_loaded(&mut world, 8, vec![started]);

    assert_eq!(world.item_auctions.instances[&31113].current, Some(7));
    // A next auction was created alongside the running one.
    assert!(world.item_auctions.instances[&31113].next.is_some());
    assert!(world.item_auctions.auctions.len() >= 2);
}
