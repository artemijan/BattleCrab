//! Item auction (G30.5) — the schedule math, the config gate, the boot load
//! (slice 1), and the auction lifecycle + scheduling state machine (slice 2).

use super::*;

use crate::data::item_auction_data::{AuctionInstanceCfg, AuctionItem, AuctionSchedule};
use crate::game_loop::item_auction;
use crate::model::item_auction::{AuctionState, ExtendState, ItemAuction, next_date};
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
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .contains(&ScheduledTask::ItemAuctionState { auction_id: id })
    );
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

// --- Bidding + cancel/refund (slice 3) ---

use crate::model::inventory::Inventory;

/// An enabled world with one instance, a live STARTED auction (id 1), and an
/// in-game bidder (oid `player`) holding `adena`.
fn bidding_world(player: i32, adena: i64) -> World {
    let mut world = world_with_instance(31113);
    world.id_pool = 0x9000_0000..0x9000_0100;
    insert_adena_template(&mut world);
    // A live auction of catalogue item 1 (init bid 100000), far from ending.
    let now = commons::util::now_millis();
    let a = ItemAuction::new(
        1,
        31113,
        1,
        now - HOUR,
        now + 5 * HOUR,
        AuctionState::Started,
    );
    world.item_auctions.enabled = true;
    world.item_auctions.next_auction_id = 2;
    world.item_auctions.auctions.insert(1, a);
    world.item_auctions.instances.insert(
        31113,
        model::item_auction::InstanceRuntime {
            current: Some(1),
            next: None,
        },
    );
    ingame_player(&mut world, 1, player, 0, 0, 0);
    items::add_inventory_item(&mut world, player, 57, adena);
    world
}

fn ia_adena(world: &World, oid: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .map_or(0, |i| i.adena())
}

#[test]
fn a_bid_escrows_adena_and_becomes_highest() {
    let mut world = bidding_world(100, 500_000);
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    assert_eq!(ia_adena(&world, 100), 350_000); // 500k - 150k escrowed
    let a = &world.item_auctions.auctions[&1];
    assert_eq!(a.highest_bid().unwrap().last_bid, 150_000);
}

#[test]
fn a_bid_below_the_init_bid_is_rejected() {
    let mut world = bidding_world(100, 500_000);
    item_auction::register_bid(&mut world, 31113, 1, 100, 50_000); // < 100000 init
    assert_eq!(ia_adena(&world, 100), 500_000); // untouched
    assert!(world.item_auctions.auctions[&1].highest_bid().is_none());
}

#[test]
fn raising_your_own_bid_charges_only_the_delta() {
    let mut world = bidding_world(100, 500_000);
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    item_auction::register_bid(&mut world, 31113, 1, 100, 200_000);
    assert_eq!(ia_adena(&world, 100), 300_000); // 500k - 150k - 50k delta
    assert_eq!(
        world.item_auctions.auctions[&1]
            .highest_bid()
            .unwrap()
            .last_bid,
        200_000
    );
}

#[test]
fn canceling_a_losing_bid_refunds_the_adena() {
    let mut world = bidding_world(100, 500_000);
    ingame_player(&mut world, 2, 200, 0, 0, 0);
    items::add_inventory_item(&mut world, 200, 57, 500_000);
    // 100 bids 150k, then 200 outbids with 200k → 100 is a loser.
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    item_auction::register_bid(&mut world, 31113, 1, 200, 200_000);
    assert_eq!(ia_adena(&world, 100), 350_000);

    assert!(item_auction::cancel_bid(&mut world, 1, 1, 100));
    assert_eq!(ia_adena(&world, 100), 500_000); // fully refunded
    assert!(
        world.item_auctions.auctions[&1]
            .bid_of(100)
            .unwrap()
            .is_canceled()
    );
}

#[test]
fn the_highest_bidder_cannot_cancel() {
    let mut world = bidding_world(100, 500_000);
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    // Returns true (Java's reserve-not-met branch) but does not refund.
    assert!(item_auction::cancel_bid(&mut world, 1, 1, 100));
    assert_eq!(ia_adena(&world, 100), 350_000); // still escrowed
    assert!(
        !world.item_auctions.auctions[&1]
            .bid_of(100)
            .unwrap()
            .is_canceled()
    );
}

#[test]
fn a_last_minute_bid_extends_the_ending_time() {
    let mut world = bidding_world(100, 500_000);
    // Move the end to 5 minutes out (inside the 10-min extend window).
    let now = commons::util::now_millis();
    world
        .item_auctions
        .auctions
        .get_mut(&1)
        .unwrap()
        .ending_time = now + 5 * 60_000;
    let before = world.item_auctions.auctions[&1].ending_time;

    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);

    let a = &world.item_auctions.auctions[&1];
    assert_eq!(a.extend_state, ExtendState::ExtendBy5Min);
    assert_eq!(a.ending_time, before + 5 * 60_000);
}

// --- Finish: delivery + expiry (slice 4) ---

fn warehouse_count(world: &World, oid: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<model::inventory::Warehouse>(&oid)
        .map_or(0, |wh| wh.0.count_of(item_id))
}

#[test]
fn a_winning_bidder_gets_the_item_in_their_warehouse() {
    let mut world = bidding_world(100, 500_000);
    // Register the auctioned item (9901) so the reward can be built.
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = 9901;
    t.name = "Reward".into();
    world.data.item_data.insert_for_test(t);
    // Bid, then finish the auction (end is far out → no extension).
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    item_auction::run_state_task(&mut world, 1); // STARTED → FINISHED + deliver

    assert_eq!(
        world.item_auctions.auctions[&1].state,
        AuctionState::Finished
    );
    assert_eq!(warehouse_count(&world, 100, 9901), 1, "reward in warehouse");
}

#[test]
fn a_finished_auction_with_no_bids_just_closes() {
    let mut world = bidding_world(100, 500_000);
    item_auction::run_state_task(&mut world, 1); // no bids
    assert_eq!(
        world.item_auctions.auctions[&1].state,
        AuctionState::Finished
    );
    assert_eq!(warehouse_count(&world, 100, 9901), 0);
}

#[test]
fn canceled_bids_are_cleared_when_the_auction_finishes() {
    let mut world = bidding_world(100, 500_000);
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = 9901;
    world.data.item_data.insert_for_test(t);
    ingame_player(&mut world, 2, 200, 0, 0, 0);
    items::add_inventory_item(&mut world, 200, 57, 500_000);
    // 100 bids, 200 outbids, 100 cancels (a canceled losing bid remains as a row).
    item_auction::register_bid(&mut world, 31113, 1, 100, 150_000);
    item_auction::register_bid(&mut world, 31113, 1, 200, 200_000);
    item_auction::cancel_bid(&mut world, 1, 1, 100);
    assert_eq!(world.item_auctions.auctions[&1].bids.len(), 2);

    item_auction::run_state_task(&mut world, 1); // finish → clear canceled
    // The canceled bid is gone; the winner's bid remains.
    let bids = &world.item_auctions.auctions[&1].bids;
    assert_eq!(bids.len(), 1);
    assert_eq!(bids[0].player_obj_id, 200);
}

#[test]
fn an_expired_finished_auction_is_dropped_at_boot() {
    let mut world = world_with_instance(31113);
    world.cfg.general.alt_item_auction_expired_after_days = 14;
    let now = commons::util::now_millis();
    let old = now - 30 * DAY; // 30 days ago, past the 14-day window
    let expired = ItemAuction::new(7, 31113, 1, old, old + HOUR, AuctionState::Finished);
    item_auction::on_loaded(&mut world, 8, vec![expired]);
    assert!(
        !world.item_auctions.auctions.contains_key(&7),
        "expired auction dropped"
    );
}
