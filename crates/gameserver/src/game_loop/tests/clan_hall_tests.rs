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
