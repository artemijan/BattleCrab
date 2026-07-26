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

// ---------------------------------------------------------------------------
// The Clan Hall Door Manager (owner opens/closes doors)
// ---------------------------------------------------------------------------

use crate::game_loop::clan_hall_auction::{hall_by_npc_id, open_close_hall_doors};

/// An agent NPC resolves to its hall (Java `Npc.getClanHall`). Onyx Hall's
/// `<npcs>` names 35395 (its door manager).
#[test]
fn a_managers_hall_is_found_by_its_npc() {
    let (mut world, _db, _l) = combat_test_world();
    world.clan_halls = load_clan_halls(DIST);
    assert_eq!(hall_by_npc_id(&world, 35395), Some(ONYX));
    assert_eq!(hall_by_npc_id(&world, 999_999), None, "not an agent");
}

/// **Opening the hall's doors opens them; closing closes them.**
#[test]
fn opening_a_halls_doors_toggles_them() {
    use crate::data::door_data::DoorOpenMethod;
    let (mut world, _db, _l) = combat_test_world();
    world.clan_halls = load_clan_halls(DIST);
    let door_id = 24190001;
    crate::model::door::spawn_door_for_test(&mut world, test_door(door_id, DoorOpenMethod::None));
    // Pretend Onyx Hall's door is this test door.
    world.clan_halls.get_mut(&ONYX).unwrap().doors = vec![door_id];

    open_close_hall_doors(&mut world, ONYX, true);
    assert!(world.geo.doors.is_open(door_id), "the door opened");

    open_close_hall_doors(&mut world, ONYX, false);
    assert!(!world.geo.doors.is_open(door_id), "the door closed");
}

// ---------------------------------------------------------------------------
// Function upgrades (buy / expire / remove)
// ---------------------------------------------------------------------------

use crate::data::residence_function_data::ResidenceFunctionData;
use crate::game_loop::clan_hall_function::{
    buy_function, function_level, handle_function_expiry, remove_function, FunctionOutcome,
};
use crate::model::inventory::Inventory;

const HP_REGEN: i32 = 1; // ResidenceFunctions.xml function id
const FUNC_PLAYER: i32 = 8300;

fn function_world() -> (World, db::CmdRx) {
    let (mut world, db, _l) = combat_test_world();
    world.id_pool = 0x3200_0000..0x3200_1000;
    world.clan_halls = load_clan_halls(DIST);
    world.data.residence_functions = ResidenceFunctionData::load_from(DIST);
    (world, db)
}

fn fund_inventory(world: &mut World, oid: i32, adena: i64) {
    let item_oid = world.alloc_object_id().unwrap();
    let catalog = &world.data.item_data;
    world
        .objects
        .get_component_mut::<Inventory>(&oid)
        .unwrap()
        .add_item(catalog, item_oid, ADENA_ID, adena);
}

fn inv_adena(world: &World, oid: i32) -> i64 {
    world
        .objects
        .get_component::<Inventory>(&oid)
        .unwrap()
        .count_of(ADENA_ID)
}

/// **The function catalogue loads** — HP_REGEN level 1 costs 700 adena for a day.
#[test]
fn the_function_catalogue_loads() {
    let data = ResidenceFunctionData::load_from(DIST);
    assert!(!data.is_empty(), "functions parsed");
    assert_eq!(data.type_of(HP_REGEN), Some("HP_REGEN"));
    let lv1 = data.level(HP_REGEN, 1).expect("HP_REGEN lv1");
    assert_eq!(lv1.cost_id, 57);
    assert_eq!(lv1.cost_count, 700);
    assert_eq!(lv1.duration_ms, 86_400_000, "1 day");
    assert!((lv1.value - 1.2).abs() < 1e-9);
}

/// **Buying a function takes the price from the buyer's inventory and arms its
/// expiry.**
#[test]
fn buying_a_function_charges_and_activates() {
    let (mut world, _db) = function_world();
    let _rx = ingame_player(&mut world, 9, FUNC_PLAYER, 0, 0, 0);
    fund_inventory(&mut world, FUNC_PLAYER, 5_000);
    let before = world.scheduler.len();
    let now = commons::util::now_millis();

    let outcome = buy_function(&mut world, ONYX, FUNC_PLAYER, HP_REGEN, 1, now);

    assert_eq!(outcome, FunctionOutcome::Bought);
    assert_eq!(
        function_level(&world, ONYX, HP_REGEN),
        1,
        "active at level 1"
    );
    assert_eq!(inv_adena(&world, FUNC_PLAYER), 5_000 - 700, "price charged");
    assert!(world.scheduler.len() > before, "expiry armed");
}

/// A buyer who can't pay is refused and nothing is charged.
#[test]
fn buying_without_the_price_is_refused() {
    let (mut world, _db) = function_world();
    let _rx = ingame_player(&mut world, 9, FUNC_PLAYER, 0, 0, 0);
    fund_inventory(&mut world, FUNC_PLAYER, 100);
    let now = commons::util::now_millis();

    assert_eq!(
        buy_function(&mut world, ONYX, FUNC_PLAYER, HP_REGEN, 1, now),
        FunctionOutcome::NotEnough
    );
    assert_eq!(function_level(&world, ONYX, HP_REGEN), 0, "not activated");
    assert_eq!(inv_adena(&world, FUNC_PLAYER), 100, "untouched");
}

/// **An expired function renews from the clan warehouse — or is dropped if the
/// warehouse can't pay.**
#[test]
fn an_expired_function_renews_or_drops() {
    let (mut world, _db) = function_world();
    let now = commons::util::now_millis();
    world.clan_halls.get_mut(&ONYX).unwrap().owner_id = 10;

    // Solvent clan → the function renews.
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 5_000);
    crate::game_loop::clan_hall_function::set_function(&mut world, ONYX, HP_REGEN, 1, now - 1000);
    handle_function_expiry(&mut world, ONYX, HP_REGEN);
    assert_eq!(function_level(&world, ONYX, HP_REGEN), 1, "renewed");
    assert!(
        world.clan_halls[&ONYX].owner_id == 10,
        "still owned by the paying clan"
    );

    // Now broke → the next expiry drops it.
    world
        .clans
        .get_mut(&10)
        .unwrap()
        .warehouse
        .0
        .remove_item(ADENA_ID, 100_000);
    world
        .clan_hall_functions
        .get_mut(&ONYX)
        .unwrap()
        .get_mut(&HP_REGEN)
        .unwrap()
        .expiration = now - 1000;
    handle_function_expiry(&mut world, ONYX, HP_REGEN);
    assert_eq!(
        function_level(&world, ONYX, HP_REGEN),
        0,
        "dropped — couldn't pay"
    );
}

/// Removing a function clears it.
#[test]
fn removing_a_function_clears_it() {
    let (mut world, _db) = function_world();
    let now = commons::util::now_millis();
    crate::game_loop::clan_hall_function::set_function(&mut world, ONYX, HP_REGEN, 1, now + 1000);
    assert_eq!(function_level(&world, ONYX, HP_REGEN), 1);

    assert!(remove_function(&mut world, ONYX, HP_REGEN));
    assert_eq!(function_level(&world, ONYX, HP_REGEN), 0, "gone");
}

/// `removeFunction` names a type, not an id — the reverse lookup the Clan Hall
/// Manager uses to remove a function.
#[test]
fn a_function_id_is_found_by_its_type_name() {
    let data = ResidenceFunctionData::load_from(DIST);
    assert_eq!(data.id_of_type("HP_REGEN"), Some(HP_REGEN));
    assert_eq!(data.id_of_type("NOT_A_TYPE"), None);
}

// ---------------------------------------------------------------------------
// ClanHallZone groundwork
// ---------------------------------------------------------------------------

use crate::data::zone_data::ZoneData;

/// **Clan-hall zones load and resolve a position to their `clanHallId`** — the
/// groundwork banish and the regen benefits need. The Gludio Onyx Hall zone
/// (clanHallId 23) is a rectangle around (-14943, 125584).
#[test]
fn a_point_inside_a_hall_resolves_to_its_id() {
    let zones = ZoneData::load_from(DIST);
    assert_eq!(
        zones.clan_hall_at(-14943, 125584, -3000),
        Some(23),
        "inside Gludio Onyx Hall"
    );
    assert_eq!(
        zones.clan_hall_at(0, 0, 0),
        None,
        "the open world is not a clan hall"
    );
}

/// **Banish ejects outsiders standing in the hall but leaves the owning clan's
/// members** (Java `banishOthers`, scoped by the ClanHallZone).
#[test]
fn banish_ejects_outsiders_but_not_members() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.zone_data = ZoneData::load_from(DIST);
    world.clan_halls = load_clan_halls(DIST);
    world.clan_halls.get_mut(&23).unwrap().owner_id = 100;
    let inside = (-14943, 125584, -3000);
    let (outsider, member) = (8400, 8401);
    let _r1 = ingame_player(&mut world, 11, outsider, inside.0, inside.1, inside.2);
    let _r2 = ingame_player(&mut world, 12, member, inside.0, inside.1, inside.2);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&outsider)
        .unwrap()
        .clan_id = 999;
    world
        .objects
        .get_component_mut::<crate::model::Player>(&member)
        .unwrap()
        .clan_id = 100;

    crate::game_loop::clan_hall_auction::banish_others(&mut world, 23);

    let op = world
        .objects
        .get_component::<crate::model::components::Position>(&outsider)
        .unwrap();
    assert_eq!((op.x, op.y), (-14684, 125655), "the outsider was banished");
    let mp = world
        .objects
        .get_component::<crate::model::components::Position>(&member)
        .unwrap();
    assert_eq!((mp.x, mp.y), (inside.0, inside.1), "the member stayed put");
}

// ---------------------------------------------------------------------------
// HP/MP regen benefit
// ---------------------------------------------------------------------------

use crate::game_loop::clan_hall_function::{active_function_value, set_function};
use crate::game_loop::regen::clan_hall_regen_mult;

fn regen_world_with_hall(owner_clan: i32) -> World {
    let (mut world, _db, _l) = combat_test_world();
    world.data.zone_data = ZoneData::load_from(DIST);
    world.clan_halls = load_clan_halls(DIST);
    world.data.residence_functions = ResidenceFunctionData::load_from(DIST);
    world.clan_halls.get_mut(&23).unwrap().owner_id = owner_clan;
    world
}

/// A bought function's `value` is what the benefit reads.
#[test]
fn an_active_functions_value_is_reported() {
    let mut world = regen_world_with_hall(100);
    let now = commons::util::now_millis();
    set_function(&mut world, 23, HP_REGEN, 1, now + 1_000_000);

    assert_eq!(active_function_value(&world, 23, "HP_REGEN"), Some(1.2));
    assert_eq!(
        active_function_value(&world, 23, "MP_REGEN"),
        None,
        "not bought"
    );
}

/// **A member standing in their own hall gets the HP/MP regen boost.**
#[test]
fn a_member_in_their_own_hall_is_boosted() {
    let mut world = regen_world_with_hall(100);
    let now = commons::util::now_millis();
    set_function(&mut world, 23, HP_REGEN, 1, now + 1_000_000); // value 1.2
    let inside = (-14943, 125584, -3000);
    let member = 8500;
    let _r = ingame_player(&mut world, 13, member, inside.0, inside.1, inside.2);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&member)
        .unwrap()
        .clan_id = 100;

    let (hp, mp) = clan_hall_regen_mult(&world, member);
    assert!((hp - 1.2).abs() < 1e-9, "HP boosted by the function value");
    assert_eq!(mp, 1.0, "no MP function → no MP boost");
}

/// A non-member (or a member in someone else's hall) gets no boost.
#[test]
fn an_outsider_in_the_hall_is_not_boosted() {
    let mut world = regen_world_with_hall(100);
    let now = commons::util::now_millis();
    set_function(&mut world, 23, HP_REGEN, 1, now + 1_000_000);
    let inside = (-14943, 125584, -3000);
    let outsider = 8501;
    let _r = ingame_player(&mut world, 14, outsider, inside.0, inside.1, inside.2);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&outsider)
        .unwrap()
        .clan_id = 999; // not the owner

    assert_eq!(
        clan_hall_regen_mult(&world, outsider),
        (1.0, 1.0),
        "not your hall — no boost"
    );
}

// ---------------------------------------------------------------------------
// Teleport function benefit
// ---------------------------------------------------------------------------

/// **The teleport function and its lists load** — the hall's TELEPORT level
/// picks a `tel<level>` list from the manager NPC's teleporter data.
#[test]
fn the_teleport_function_and_lists_load() {
    let data = ResidenceFunctionData::load_from(DIST);
    let tp = data.id_of_type("TELEPORT").expect("TELEPORT function");
    assert!(data.level(tp, 1).is_some(), "TELEPORT has a level 1");

    // The manager NPC's tel1/tel2 lists exist (Aden's Korgen, 35447).
    let tele = crate::data::teleporter_data::TeleporterData::load_from(DIST);
    assert!(tele.holder(35447, "tel1").is_some(), "tel1 list loads");
    assert!(tele.holder(35447, "tel2").is_some(), "tel2 list loads");
}

/// **A hall teleport moves the player** — the manager's `tel1` list (loaded
/// from the manager NPC's teleporter data) sends them to its first location.
#[test]
fn a_hall_teleport_moves_the_player() {
    let (mut world, _db, _l) = combat_test_world();
    world.data.teleporters = crate::data::teleporter_data::TeleporterData::load_from(DIST);
    let npc = NPC_OID + 300;
    add_test_npc(&mut world, npc, 35447, "Folk", 70, 100, 100, 0); // Korgen
    let player = 8600;
    let _rx = ingame_player(&mut world, 15, player, 100, 100, 0);

    crate::game_loop::teleporter::do_teleport(&mut world, 15, player, npc, "tel1", Some(0));

    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&player)
        .unwrap();
    // x/y land exactly; z snaps to ground geometry.
    assert_eq!(
        (pos.x, pos.y),
        (147450, 28081),
        "teleported to tel1's Village Square"
    );
}

// ---------------------------------------------------------------------------
// Support-buff benefit (BUFF function)
// ---------------------------------------------------------------------------

use crate::game_loop::clan_hall_function::{cast_hall_buff, BuffCastOutcome};
use crate::model::components::{Buffs, Vitals};

/// Build a world with a clan-hall manager NPC and a nearby player, plus a
/// synthetic "Wind Walk" (4342_2) branded onto the Might buff template so it
/// lands in the player's `Buffs`. `mp` is the manager's starting MP.
fn buff_world(
    mp: f64,
) -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
    i32,
    i32,
) {
    let (mut world, db, l) = combat_test_world();
    // Re-brand the Might buff (1068 → an ActiveBuff producer) as an allowed
    // hall buff, costing 100 MP with a 5s reuse.
    let mut buff = world.data.skill_data.get(1068, 1).unwrap().clone();
    buff.id = 4342;
    buff.level = 2;
    buff.name = "Wind Walk".into();
    buff.mp_consume = 100;
    buff.mp_initial_consume = 0;
    buff.reuse_delay = 5000;
    buff.abnormal_type = "SPEED_UP".into();
    world.data.skill_data.insert_for_test(buff);

    let npc = NPC_OID + 400;
    add_test_npc(&mut world, npc, 35447, "Merchant", 70, 100, 100, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&npc)
        .unwrap()
        .cur_mp = mp;
    let player = 8700;
    let _rx = ingame_player(&mut world, 16, player, 120, 100, 0);
    (world, db, l, npc, player)
}

fn player_has_buff(world: &World, player: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&player)
        .is_some_and(|b| b.0.iter().any(|a| a.skill_id == skill_id))
}

/// **A hall buff is cast and paid for from the manager's MP.**
#[test]
fn a_hall_buff_is_cast_and_costs_the_managers_mp() {
    let (mut world, _db, _l, npc, player) = buff_world(300.0);

    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "4342_2"),
        BuffCastOutcome::Cast
    );
    assert!(
        player_has_buff(&world, player, 4342),
        "the player got the buff"
    );
    assert_eq!(
        world.objects.get_component::<Vitals>(&npc).unwrap().cur_mp,
        200.0,
        "the manager's MP paid the 100 cost"
    );
}

/// **A buff the manager can't afford is refused, and casts nothing.**
#[test]
fn a_hall_buff_needs_the_managers_mp() {
    let (mut world, _db, _l, npc, player) = buff_world(50.0); // < 100 cost

    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "4342_2"),
        BuffCastOutcome::NotEnoughMp
    );
    assert!(!player_has_buff(&world, player, 4342), "no buff applied");
    assert_eq!(
        world.objects.get_component::<Vitals>(&npc).unwrap().cur_mp,
        50.0,
        "no MP spent on a refusal"
    );
}

/// **The reuse timer blocks a second cast** (Java `npc.isSkillDisabled`).
#[test]
fn a_hall_buff_respects_the_reuse_timer() {
    let (mut world, _db, _l, npc, player) = buff_world(1000.0);

    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "4342_2"),
        BuffCastOutcome::Cast
    );
    // Immediately again — the 5s reuse is still running.
    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "4342_2"),
        BuffCastOutcome::OnReuse
    );
}

/// **Only skills in `ALLOWED_BUFFS` can be cast** — a real skill outside the
/// list, and a malformed token, are both ignored.
#[test]
fn only_listed_buffs_are_castable() {
    let (mut world, _db, _l, npc, player) = buff_world(1000.0);

    // 1068 (Might) is a real buff but not in the clan-hall ALLOWED_BUFFS list.
    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "1068_1"),
        BuffCastOutcome::NotAllowed
    );
    assert_eq!(
        cast_hall_buff(&mut world, npc, player, "not-a-skill"),
        BuffCastOutcome::NotAllowed
    );
    assert!(!player_has_buff(&world, player, 1068));
}

// ---------------------------------------------------------------------------
// Item-creation benefit (ITEM function)
// ---------------------------------------------------------------------------

/// **The item function maps to the manager's buylists** — Java's
/// `showBuyWindow(player, npcId·"0"·(level-1))` is `npcId*100 + (level-1)`, and
/// each tier's buylist must exist and allow the manager NPC.
#[test]
fn the_item_functions_map_to_the_managers_buylists() {
    let items = crate::data::ItemData::load_from(DIST);
    let buy_lists = crate::data::BuyListData::load_from(DIST, &items);
    let npc_id = 35447; // Korgen (Aden)

    for level in 1..=3 {
        let list_id = npc_id * 100 + (level - 1);
        let list = buy_lists
            .get(list_id)
            .unwrap_or_else(|| panic!("buylist {list_id} for item level {level}"));
        assert!(
            list.is_npc_allowed(npc_id),
            "buylist {list_id} allows the manager"
        );
    }
    // A 4th tier has no buylist → the manager's noFunction fallback.
    assert!(buy_lists.get(npc_id * 100 + 3).is_none());
}

/// **A manager serves a clan-hall buylist** — the item benefit's buy window
/// reaches the player through the shop path.
#[test]
fn a_manager_serves_its_item_buylist() {
    let (mut world, _db, _l) = combat_test_world();
    let npc = NPC_OID + 500;
    add_test_npc(&mut world, npc, 35447, "Merchant", 70, 100, 100, 0);
    world
        .data
        .buy_lists
        .insert_for_test(crate::data::buy_list_data::BuyList {
            list_id: 3544700, // 35447 * 100 + 0 (item level 1)
            npcs: vec![35447],
            products: vec![crate::data::buy_list_data::Product {
                item_id: 6902,
                price: 1000,
                base_tax: 0,
            }],
        });
    let player = 8800;
    let mut rx = ingame_player(&mut world, 17, player, 120, 100, 0);

    crate::game_loop::shop::show_buy_window(&mut world, 17, player, npc, 3544700);

    // The ExBuySellList packet (0xFE 0xB8) reaches the client.
    let sent = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        sent.iter()
            .any(|p| p.first() == Some(&0xFE) && p.get(1) == Some(&0xB8)),
        "the buy window was sent"
    );
}

// ---------------------------------------------------------------------------
// EXP_RESTORE benefit (respawn at the clan hall)
// ---------------------------------------------------------------------------

/// Set up a dead player (clan 42) whose clan owns Onyx Hall, respawn them "to
/// clanhall", and return their exp/lost-exp/position afterward. `restore_level`
/// = the EXP_RESTORE function level to buy (0 = none).
fn respawn_at_hall(restore_level: i32, exp: i64, lost: i64) -> (i64, i64, (i32, i32, i32)) {
    let (mut world, _db, _l) = combat_test_world();
    world.clan_halls = load_clan_halls(DIST);
    world.data.residence_functions = ResidenceFunctionData::load_from(DIST);
    world.clan_halls.get_mut(&ONYX).unwrap().owner_id = 42;
    if restore_level > 0 {
        let exp_func = world
            .data
            .residence_functions
            .id_of_type("EXP_RESTORE")
            .unwrap();
        set_function(&mut world, ONYX, exp_func, restore_level, i64::MAX);
    }

    let player = 8900;
    let _rx = ingame_player(&mut world, 18, player, 100, 100, 0);
    {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&player)
            .unwrap();
        p.clan_id = 42;
        p.exp = exp;
        p.lost_exp_on_death = lost;
    }
    world
        .objects
        .get_component_mut::<Vitals>(&player)
        .unwrap()
        .dead = true;

    handle_request_restart_point(&mut world, 18, &1i32.to_le_bytes());

    let p = world
        .objects
        .get_component::<crate::model::Player>(&player)
        .unwrap();
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&player)
        .unwrap();
    (p.exp, p.lost_exp_on_death, (pos.x, pos.y, pos.z))
}

/// **Respawning at the hall restores lost exp** via the EXP_RESTORE function
/// (level 1 = 5%), and lands the player at the hall's owner-restart point.
#[test]
fn respawning_at_the_hall_restores_exp() {
    let restart = load_clan_halls(DIST)[&ONYX].owner_restart;
    let (exp, lost, pos) = respawn_at_hall(1, 1000, 400);
    assert_eq!(exp, 1020, "5% of the 400 lost exp restored");
    assert_eq!(lost, 0, "the restore is consumed");
    // x/y land exactly; z snaps to ground geometry.
    assert_eq!(
        (pos.0, pos.1),
        (restart.0, restart.1),
        "respawned at the hall"
    );
}

/// **Without the EXP_RESTORE function, no exp is restored** — the player still
/// respawns at the hall, but keeps their post-death exp.
#[test]
fn a_hall_without_the_function_restores_nothing() {
    let restart = load_clan_halls(DIST)[&ONYX].owner_restart;
    let (exp, lost, pos) = respawn_at_hall(0, 1000, 400);
    assert_eq!(exp, 1000, "no exp restored");
    assert_eq!(lost, 400, "the lost-exp record is untouched");
    assert_eq!(
        (pos.0, pos.1),
        (restart.0, restart.1),
        "still respawns at the hall"
    );
}

// ---------------------------------------------------------------------------
// Auctioneer dynamic-page templating
// ---------------------------------------------------------------------------

use crate::model::clan_hall::ClanHallBid;
use crate::scripts::clan_hall_auctioneer::{
    render_bid_form, render_bidder_list, render_hall_info, render_hall_list,
};

fn put_bid(world: &mut World, hall: i32, clan: i32, amount: i64, bid_time: i64) {
    world
        .clan_hall_bids
        .entry(hall)
        .or_default()
        .insert(clan, ClanHallBid { amount, bid_time });
}

/// An auction world whose `root` points at the dist tree so the auctioneer's
/// html templates load.
fn auctioneer_world() -> World {
    let mut world = auction_world();
    world.data.root = DIST.to_string();
    world
}

/// **The hall list templates every free hall with its highest bid**, and omits
/// owned halls (Java `getFreeAuctionableHall`).
#[test]
fn the_hall_list_lists_free_halls_with_bids() {
    let mut world = auctioneer_world();
    put_bid(&mut world, ONYX, 10, 6_000_000, 100);
    world.clan_halls.get_mut(&OTHER_HALL).unwrap().owner_id = 99; // owned → excluded

    let html = render_hall_list(&world);
    assert!(!html.contains("%agitList%"), "the row template is filled");
    assert!(!html.contains("%pages%"), "the pager placeholder is filled");
    assert!(
        html.contains("auctionList id=27\""),
        "Onyx (free) is listed"
    );
    assert!(html.contains("6000000"), "with its highest bid");
    assert!(
        !html.contains("auctionList id=22\""),
        "the owned hall is not listed"
    );
}

/// **A hall's info page fills in its auction fields** — rent, grade, current
/// minimum bid, and the bid count.
#[test]
fn the_info_page_fills_the_auction_fields() {
    let mut world = auctioneer_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    put_bid(&mut world, ONYX, 10, 5_000_000, 100);
    put_bid(&mut world, ONYX, 20, 6_000_000, 200);

    let html = render_hall_info(&world, ONYX).expect("info page");
    for ph in [
        "%id%",
        "%owner%",
        "%rent%",
        "%grade%",
        "%minBid%",
        "%bidNumber%",
    ] {
        assert!(!html.contains(ph), "placeholder {ph} was filled");
    }
    assert!(html.contains("6000000"), "the highest bid is the minimum");
    assert!(html.contains("500000"), "Onyx's rent (lease)");
    assert!(html.contains("30 square feet"), "grade B = 30");
    assert!(html.contains(">2<"), "two bids counted");
}

/// **The bidder list shows every bidding clan, newest first.**
#[test]
fn the_bidder_list_orders_by_most_recent() {
    let mut world = auctioneer_world();
    world.clans.insert(10, mk_clan(10, 5));
    world.clans.insert(20, mk_clan(20, 5));
    put_bid(&mut world, ONYX, 10, 5_000_000, 100); // older
    put_bid(&mut world, ONYX, 20, 6_000_000, 200); // newer

    let html = render_bidder_list(&world, ONYX).expect("bidder list");
    assert!(!html.contains("%bidderList%"), "the rows are filled");
    let i20 = html.find("Clan20").expect("clan 20 listed");
    let i10 = html.find("Clan10").expect("clan 10 listed");
    assert!(i20 < i10, "the most recent bidder is listed first");
    assert!(html.contains("6000000") && html.contains("5000000"));
}

/// **The bid form shows the clan's warehouse adena and the current minimum.**
#[test]
fn the_bid_form_shows_clan_adena() {
    let mut world = auctioneer_world();
    world.clans.insert(10, mk_clan(10, 5));
    fund_clan(&mut world, 10, 12_345_678);
    put_bid(&mut world, ONYX, 20, 6_000_000, 100);

    let html = render_bid_form(&world, 10, ONYX).expect("bid form");
    assert!(!html.contains("%clanAdena%") && !html.contains("%minBid%"));
    assert!(html.contains("12345678"), "the clan's adena");
    assert!(html.contains("6000000"), "the current minimum bid");
    assert!(
        html.contains("id=27"),
        "the hall id is baked into the bypass"
    );
}
