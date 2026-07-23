//! `ScriptZone` support — the groundwork every `ai/bosses` script needs.

use super::*;

const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
/// Queen Ant's lair, the zone `QueenAnt.java` opens with
/// `ZoneManager.getZoneById(12012)`.
const QUEEN_ANT_ZONE: i32 = 12012;

/// The dist's `ScriptZone`s load, and the one the boss scripts address by id
/// is findable. Read from the real data: a fixture would prove nothing about
/// whether `type="ScriptZone"` maps or `id=` is kept.
#[test]
fn the_real_script_zones_load_and_are_addressable() {
    let zones = crate::data::zone_data::ZoneData::load_from(DIST);
    let queen = zones
        .by_id(QUEEN_ANT_ZONE)
        .expect("zone 12012 (Queen Ant Boss) is addressable by id");
    assert_eq!(queen.name, "Queen Ant Boss");
    assert_eq!(queen.kind, crate::data::zone_data::ZoneKind::Script);
}

/// `isInsideZone` — the containment test the scripts use to decide whether a
/// stored boss location is still valid. Uses the zone's own bounds, so it
/// cannot drift from the polygon the data defines.
#[test]
fn script_zone_containment_matches_the_polygon() {
    let zones = crate::data::zone_data::ZoneData::load_from(DIST);
    let queen = zones.by_id(QUEEN_ANT_ZONE).unwrap();

    // Queen Ant's own spawn point, from QueenAnt.java's QUEEN_X/Y/Z.
    assert!(
        queen.contains(-21610, 181594, -5734),
        "the boss's own spawn point is inside its zone"
    );
    // Far outside, and outside the Z band.
    assert!(!queen.contains(0, 0, -5734), "a distant point is outside");
    assert!(
        !queen.contains(-21610, 181594, 5000),
        "above the zone's maxZ is outside"
    );
}

/// A `ScriptZone` claims **no membership bit**: Java gives it no `ZoneId`, so
/// standing in one must not alter a player's zone flags.
#[test]
fn a_script_zone_claims_no_membership_bit() {
    assert_eq!(crate::data::zone_data::ZoneKind::Script.bit(), 0);
}

/// An unknown id finds nothing rather than silently returning the first zone.
#[test]
fn an_unknown_zone_id_finds_nothing() {
    let zones = crate::data::zone_data::ZoneData::load_from(DIST);
    assert!(zones.by_id(-1).is_none());
}
