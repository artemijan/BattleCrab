//! Clan halls (G24) — the residence definitions load, and persisted ownership
//! overlays onto them at boot.

use super::*;

use crate::data::clan_hall_data::load_clan_halls;
use crate::model::clan_hall::{ClanHallGrade, ClanHallType};

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// **All 48 clan halls load, with their auction terms, agents and doors.**
/// Onyx Hall (id 27) is the fixture — a Gludin GRADE_B auctionable hall.
#[test]
fn all_forty_eight_clan_halls_load() {
    let halls = load_clan_halls(DIST);
    assert_eq!(halls.len(), 48, "every clan hall parsed");

    let onyx = halls.get(&27).expect("Onyx Hall (id 27)");
    assert_eq!(onyx.name, "Onyx Hall");
    assert_eq!(onyx.grade, ClanHallGrade::B);
    assert_eq!(onyx.hall_type, ClanHallType::Auctionable);
    assert_eq!(onyx.min_bid, 5_000_000);
    assert_eq!(onyx.lease, 500_000);
    assert_eq!(onyx.deposit, 500_000);
    assert_eq!(onyx.npcs, vec![35395, 35394], "its two agent NPCs");
    assert_eq!(onyx.doors.len(), 4, "its four doors");
    assert_eq!(onyx.owner_restart, (-84171, 153385, -3159));
    assert_eq!(onyx.banish, (-83860, 153744, -3176));
    // Fresh from XML, no owner yet.
    assert_eq!(onyx.owner_id, 0);
}

/// **Persisted ownership overlays onto the static defs at boot.** The
/// `ClanHallsLoaded` event carries `clanhall` rows; draining it sets the owner
/// and lease on the matching hall.
#[test]
fn ownership_overlays_at_boot() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.clan_halls = load_clan_halls(DIST);
    assert!(world.clan_halls.is_empty(), "nothing until the rows arrive");

    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(crate::db::DbEvent::ClanHallsLoaded {
        rows: vec![crate::db::ClanHallRow {
            id: 27,
            owner_id: 500,
            paid_until: 1_700_000_000_000,
        }],
    })
    .unwrap();
    drop(tx);
    crate::game_loop::net::drain_db(&mut world, &rx);

    assert_eq!(world.clan_halls.len(), 48, "the defs are in the world");
    let onyx = world.clan_halls.get(&27).unwrap();
    assert_eq!(onyx.owner_id, 500, "clan 500 owns Onyx Hall");
    assert_eq!(onyx.paid_until, 1_700_000_000_000);
    // A hall with no row stays free.
    assert_eq!(world.clan_halls.get(&22).unwrap().owner_id, 0);
}

/// The reverse lookup — "which hall does this clan own" — finds the overlaid
/// owner (the read the admin `//claninfo` panel uses).
#[test]
fn a_clan_can_be_found_by_the_hall_it_owns() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.clan_halls = load_clan_halls(DIST);
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(crate::db::DbEvent::ClanHallsLoaded {
        rows: vec![crate::db::ClanHallRow {
            id: 27,
            owner_id: 777,
            paid_until: 0,
        }],
    })
    .unwrap();
    drop(tx);
    crate::game_loop::net::drain_db(&mut world, &rx);

    let owned = world
        .clan_halls
        .values()
        .find(|h| h.owner_id == 777)
        .map(|h| h.name.as_str());
    assert_eq!(owned, Some("Onyx Hall"));
}

// ---------------------------------------------------------------------------
// Auctions (bid / outbid / cancel / finalize)
// ---------------------------------------------------------------------------

use crate::data::item_data::ADENA_ID;
use crate::game_loop::clan_hall_auction::{
    cancel_bid, finalize_auction, highest_bidder, place_bid, BidOutcome,
};
use crate::model::clan::Clan;

/// Onyx Hall (id 27) — minimum bid 5,000,000.
const ONYX: i32 = 27;
const OTHER_HALL: i32 = 22;

fn mk_clan(id: i32, level: i32) -> Clan {
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: id * 10,
        level,
        reputation_score: 0,
        castle_id: 0,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
    }
}

fn auction_world() -> World {
    let (mut world, _db, _l) = combat_test_world();
    world.id_pool = 0x3000_0000..0x3000_1000;
    world.clan_halls = load_clan_halls(DIST);
    world
}

fn fund_clan(world: &mut World, clan_id: i32, adena: i64) {
    let oid = world.alloc_object_id().unwrap();
    let catalog = &world.data.item_data;
    world
        .clans
        .get_mut(&clan_id)
        .unwrap()
        .warehouse
        .0
        .add_item(catalog, oid, ADENA_ID, adena);
}

fn clan_adena(world: &World, clan_id: i32) -> i64 {
    world.clans[&clan_id].warehouse.0.count_of(ADENA_ID)
}

/// A qualified clan places the opening bid; the adena leaves its warehouse.
#[test]
fn a_clan_places_the_first_bid() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 10_000_000);

    assert_eq!(
        place_bid(&mut world, ONYX, 10, 5_000_000, 0),
        BidOutcome::Accepted
    );
    assert_eq!(highest_bidder(&world, ONYX), Some((10, 5_000_000)));
    assert_eq!(clan_adena(&world, 10), 5_000_000, "the bid was escrowed");
}

/// **Outbidding refunds the previous highest bidder.** Only the current top
/// bid's adena is ever held.
#[test]
fn outbidding_refunds_the_previous_highest() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    fund_clan(&mut world, 10, 10_000_000);
    fund_clan(&mut world, 20, 20_000_000);

    place_bid(&mut world, ONYX, 10, 5_000_000, 0);
    assert_eq!(
        place_bid(&mut world, ONYX, 20, 6_000_000, 1),
        BidOutcome::Accepted
    );

    assert_eq!(highest_bidder(&world, ONYX), Some((20, 6_000_000)));
    assert_eq!(
        clan_adena(&world, 10),
        10_000_000,
        "clan 10 was refunded in full"
    );
    assert_eq!(
        clan_adena(&world, 20),
        14_000_000,
        "clan 20's bid is escrowed"
    );
}

/// A bid at or below the current highest is refused.
#[test]
fn a_bid_not_above_the_highest_is_refused() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    fund_clan(&mut world, 10, 10_000_000);
    fund_clan(&mut world, 20, 10_000_000);
    place_bid(&mut world, ONYX, 10, 6_000_000, 0);

    assert_eq!(
        place_bid(&mut world, ONYX, 20, 5_000_000, 1),
        BidOutcome::BidTooLow
    );
    assert_eq!(
        clan_adena(&world, 20),
        10_000_000,
        "no adena taken on a refusal"
    );
}

/// The opening bid must meet the hall's minimum.
#[test]
fn the_first_bid_must_meet_the_minimum() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 10_000_000);

    assert_eq!(
        place_bid(&mut world, ONYX, 10, 4_000_000, 0),
        BidOutcome::BidTooLow
    );
}

/// Not enough adena in the clan warehouse → refused, nothing taken.
#[test]
fn not_enough_adena_is_refused() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 3_000_000);

    assert_eq!(
        place_bid(&mut world, ONYX, 10, 5_000_000, 0),
        BidOutcome::NotEnoughAdena
    );
    assert!(highest_bidder(&world, ONYX).is_none());
    assert_eq!(clan_adena(&world, 10), 3_000_000, "untouched");
}

/// A clan can't have live bids on two different halls.
#[test]
fn a_clan_cannot_bid_on_two_halls() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 50_000_000);
    place_bid(&mut world, ONYX, 10, 5_000_000, 0);

    assert_eq!(
        place_bid(&mut world, OTHER_HALL, 10, 5_000_000, 1),
        BidOutcome::BiddingElsewhere
    );
}

/// A clan that already owns a hall can't bid on another.
#[test]
fn a_hall_owner_cannot_bid() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 50_000_000);
    world.clan_halls.get_mut(&OTHER_HALL).unwrap().owner_id = 10;

    assert_eq!(
        place_bid(&mut world, ONYX, 10, 5_000_000, 0),
        BidOutcome::AlreadyOwnsHall
    );
}

/// **Finalize awards the hall to the highest bidder** and clears the bids.
#[test]
fn finalize_awards_to_the_highest() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    fund_clan(&mut world, 10, 10_000_000);
    fund_clan(&mut world, 20, 20_000_000);
    place_bid(&mut world, ONYX, 10, 5_000_000, 0);
    place_bid(&mut world, ONYX, 20, 6_000_000, 1);

    finalize_auction(&mut world, ONYX);

    assert_eq!(world.clan_halls[&ONYX].owner_id, 20, "clan 20 won the hall");
    assert!(
        world.clan_hall_bids.get(&ONYX).is_none(),
        "the bids are cleared"
    );
}

/// Cancelling removes the bid but does not refund (Java `removeBid`).
#[test]
fn cancelling_removes_the_bid_without_refund() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 10_000_000);
    place_bid(&mut world, ONYX, 10, 5_000_000, 0);
    assert_eq!(clan_adena(&world, 10), 5_000_000);

    assert!(cancel_bid(&mut world, ONYX, 10));
    assert!(highest_bidder(&world, ONYX).is_none(), "bid gone");
    assert_eq!(clan_adena(&world, 10), 5_000_000, "no refund on cancel");
}

// ---------------------------------------------------------------------------
// Reachability — weekly close, persistence, boot load
// ---------------------------------------------------------------------------

/// **The weekly close awards every hall's auction and re-arms itself.**
#[test]
fn the_weekly_close_finalizes_and_rearms() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    fund_clan(&mut world, 10, 10_000_000);
    fund_clan(&mut world, 20, 20_000_000);
    place_bid(&mut world, ONYX, 10, 5_000_000, 0);
    place_bid(&mut world, ONYX, 20, 6_000_000, 1);
    let before = world.scheduler.len();

    crate::game_loop::clan_hall_auction::handle_auction_end(&mut world);

    assert_eq!(world.clan_halls[&ONYX].owner_id, 20, "the top bidder won");
    assert!(world.clan_hall_bids.get(&ONYX).is_none(), "bids cleared");
    assert!(world.scheduler.len() > before, "next week's close is armed");
}

/// Placing a bid persists it (so escrowed adena stays accounted for).
#[test]
fn placing_a_bid_persists_it() {
    let (mut world, mut db, _l) = combat_test_world();
    world.id_pool = 0x3100_0000..0x3100_1000;
    world.clan_halls = load_clan_halls(DIST);
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 10_000_000);

    place_bid(&mut world, ONYX, 10, 5_000_000, 42);

    let saved = std::iter::from_fn(|| db.try_recv().ok()).any(|c| {
        matches!(
            c,
            crate::db::DbCommand::SaveClanHallBid {
                hall_id: 27,
                clan_id: 10,
                bid: 5_000_000,
                ..
            }
        )
    });
    assert!(saved, "the bid row was persisted");
}

/// **Bids are restored at boot** and the weekly close is armed.
#[test]
fn bids_are_restored_at_boot() {
    let (mut world, _db, _l) = combat_test_world();
    world.clan_halls = load_clan_halls(DIST);
    assert!(world.clan_hall_bids.is_empty());

    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(crate::db::DbEvent::ClanHallBiddersLoaded {
        rows: vec![crate::db::ClanHallBidRow {
            hall_id: ONYX,
            clan_id: 10,
            bid: 5_000_000,
            bid_time: 7,
        }],
    })
    .unwrap();
    drop(tx);
    let before = world.scheduler.len();
    crate::game_loop::net::drain_db(&mut world, &rx);

    assert_eq!(
        highest_bidder(&world, ONYX),
        Some((10, 5_000_000)),
        "bid restored"
    );
    assert!(
        world.scheduler.len() > before,
        "the weekly auction close is armed"
    );
}

// ---------------------------------------------------------------------------
// The lease / rental cycle
// ---------------------------------------------------------------------------

const DAY_MS: i64 = 86_400_000;

fn own_hall(world: &mut World, hall_id: i32, clan_id: i32, paid_until: i64) {
    let h = world.clan_halls.get_mut(&hall_id).unwrap();
    h.owner_id = clan_id;
    h.paid_until = paid_until;
}

/// **Winning a hall starts the lease clock** — the first rent is due in a week
/// and the payment check is armed.
#[test]
fn winning_a_hall_starts_the_lease_clock() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 10_000_000);
    place_bid(&mut world, ONYX, 10, 5_000_000, 0);
    let before = world.scheduler.len();
    let now = commons::util::now_millis();

    crate::game_loop::clan_hall_auction::finalize_auction(&mut world, ONYX);

    let hall = &world.clan_halls[&ONYX];
    assert_eq!(hall.owner_id, 10, "clan 10 owns it");
    assert!(hall.paid_until >= now + 6 * DAY_MS, "rent due in ~a week");
    assert!(world.scheduler.len() > before, "the lease check is armed");
}

/// A solvent owner pays the weekly rent and keeps the hall; the clock advances.
#[test]
fn a_paying_owner_keeps_the_hall() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 2_000_000);
    let now = commons::util::now_millis();
    own_hall(&mut world, ONYX, 10, now); // rent due now

    crate::game_loop::clan_hall_auction::handle_lease_check(&mut world, ONYX);

    assert_eq!(world.clan_halls[&ONYX].owner_id, 10, "still owned");
    assert_eq!(
        clan_adena(&world, 10),
        2_000_000 - 500_000,
        "one lease (500k) was charged"
    );
    assert!(
        world.clan_halls[&ONYX].paid_until >= now + 6 * DAY_MS,
        "the clock advanced a week"
    );
}

/// A delinquent owner who is only a little overdue gets a reminder (a retry),
/// not eviction.
#[test]
fn a_delinquent_owner_gets_a_retry() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5)); // no adena funded
    let now = commons::util::now_millis();
    own_hall(&mut world, ONYX, 10, now - 2 * DAY_MS); // 2 days overdue
    let before = world.scheduler.len();

    crate::game_loop::clan_hall_auction::handle_lease_check(&mut world, ONYX);

    assert_eq!(
        world.clan_halls[&ONYX].owner_id, 10,
        "still owned — just a retry"
    );
    assert!(world.scheduler.len() > before, "a retry check was armed");
}

/// **An owner more than a week overdue loses the hall** — it returns to the free
/// pool.
#[test]
fn a_week_overdue_owner_is_evicted() {
    let mut world = auction_world();
    world.clans.insert(10, mk_clan(10, 5)); // can't pay
    let now = commons::util::now_millis();
    own_hall(&mut world, ONYX, 10, now - 10 * DAY_MS); // 10 days overdue

    crate::game_loop::clan_hall_auction::handle_lease_check(&mut world, ONYX);

    assert_eq!(world.clan_halls[&ONYX].owner_id, 0, "the hall was revoked");
    assert_eq!(
        world.clan_halls[&ONYX].paid_until, 0,
        "the lease clock cleared"
    );
}
