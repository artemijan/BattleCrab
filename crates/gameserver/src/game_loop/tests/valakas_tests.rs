//! Valakas — the attack-side rules.

use super::*;

use crate::game_loop::valakas::{AttackVerdict, DEAD, FIGHTING, VALAKAS, WAITING};

const VALAKAS_OID: i32 = NPC_OID + 100;
const PLAYER: i32 = 9990;
const CID: u32 = 1;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
/// A point inside zone 12010 ("Valakas Boss"), taken from the boss's own lair.
const IN_LAIR: (i32, i32, i32) = (212_852, -114_842, -1_632);

fn valakas_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    // The real zone data — the whole mechanic is "is the attacker inside it".
    world.data.zone_data = crate::data::zone_data::ZoneData::load_from(DIST);
    let mut t = crate::data::npc_data::default_template(VALAKAS);
    t.type_name = "GrandBoss".into();
    t.level = 85;
    t.base_hp_max = 1_000_000.0;
    world.data.npc_data.insert_for_test(t);
    world.grand_bosses.insert(
        VALAKAS,
        crate::model::grand_boss::GrandBoss {
            boss_id: VALAKAS,
            loc_x: IN_LAIR.0,
            loc_y: IN_LAIR.1,
            loc_z: IN_LAIR.2,
            heading: 0,
            respawn_time: 0,
            current_hp: 0.0,
            current_mp: 0.0,
            status: FIGHTING,
        },
    );
    (world, db, l)
}

fn put_player_at(world: &mut World, x: i32, y: i32, z: i32) {
    let p = world.objects.get_component_mut::<Position>(&PLAYER).unwrap();
    p.x = x;
    p.y = y;
    p.z = z;
}

/// The lair point the fixture uses really is inside zone 12010 — otherwise
/// every "inside" test below would be testing the outside path by accident.
#[test]
fn the_fixtures_lair_point_is_actually_inside_the_zone() {
    let zones = crate::data::zone_data::ZoneData::load_from(DIST);
    let zone = zones.by_id(12010).expect("Valakas Boss zone");
    assert!(zone.contains(IN_LAIR.0, IN_LAIR.1, IN_LAIR.2), "the fixture's point is inside the lair");
}

/// **Attacking from outside the lair kills you.** Java's `attacker.doDie()` —
/// a hard anti-exploit against plinking at Valakas from safety.
#[test]
fn attacking_from_outside_the_lair_kills_the_attacker() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    put_player_at(&mut world, 0, 0, 0); // far outside

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::KilledForAttackingFromOutside);
    assert!(world.objects.get_component::<Vitals>(&PLAYER).unwrap().dead, "the attacker died");
}

/// **The zone check comes first.** With Valakas dead, an out-of-zone attacker
/// still dies rather than merely being teleported — Java's ordering, and the
/// half a reordering would silently lose.
#[test]
fn the_zone_check_precedes_the_status_check() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = DEAD;
    put_player_at(&mut world, 0, 0, 0);

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::KilledForAttackingFromOutside, "death, not a teleport");
}

/// Inside the lair but before the fight has begun: bounced out, not killed.
#[test]
fn attacking_before_the_fight_removes_the_attacker() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = WAITING;
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::RemovedNotFighting);
    assert!(!world.objects.get_component::<Vitals>(&PLAYER).unwrap().dead, "removed, not killed");
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    assert_eq!((p.x, p.y), (150_037, -57_255), "dumped at ATTACKER_REMOVE");
}

/// Inside the lair, fight underway: an ordinary hit is allowed through.
#[test]
fn a_legitimate_attacker_is_allowed() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::Allowed);
    assert!(!world.objects.get_component::<Vitals>(&PLAYER).unwrap().dead);
}
