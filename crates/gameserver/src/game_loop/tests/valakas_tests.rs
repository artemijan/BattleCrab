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

// ---------------------------------------------------------------------------
// The entry cinematic (slice 15)
// ---------------------------------------------------------------------------

/// The cinematic arms all ten beats up front and ends by starting the fight.
#[test]
fn the_cinematic_arms_every_beat_and_ends_in_fighting() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, 0, 0, 0);
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = WAITING;

    let before = world.scheduler.len();
    crate::game_loop::valakas::begin_cinematic(&mut world, VALAKAS_OID);
    assert_eq!(world.scheduler.len() - before, 10, "ten beats scheduled at once");

    // The last beat is what starts the fight.
    crate::game_loop::valakas::handle_cinematic_step(&mut world, VALAKAS_OID, 9);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, VALAKAS),
        Some(FIGHTING),
        "the final beat locks entry and starts the fight"
    );
}

/// Beginning the cinematic teleports Valakas into his lair first — the camera
/// shots are framed on him, so a boss still at its spawn point would show the
/// wrong scene.
#[test]
fn the_cinematic_moves_valakas_into_the_lair() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, 0, 0, 0);

    crate::game_loop::valakas::begin_cinematic(&mut world, VALAKAS_OID);
    let p = world.objects.get_component::<Position>(&VALAKAS_OID).unwrap();
    assert_eq!((p.x, p.y, p.z), IN_LAIR, "moved to the lair");
}

/// **The cinematic plays for the lair, not the neighbourhood.** A player
/// inside sees it; one outside sees nothing — which is why Java broadcasts on
/// the zone rather than the boss's region.
#[test]
fn only_players_inside_the_lair_see_the_cinematic() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);

    let mut inside_rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let outsider = PLAYER + 1;
    let mut outside_rx = ingame_caster(&mut world, CID + 1, outsider, 0, 0);
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    {
        let p = world.objects.get_component_mut::<Position>(&outsider).unwrap();
        p.x = 0;
        p.y = 0;
        p.z = 0;
    }
    while inside_rx.try_recv().is_ok() {}
    while outside_rx.try_recv().is_ok() {}

    crate::game_loop::valakas::handle_cinematic_step(&mut world, VALAKAS_OID, 0);

    let count = |rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>| {
        let mut n = 0;
        while let Ok(p) = rx.try_recv() {
            if p.first() == Some(&0xD6) {
                n += 1;
            }
        }
        n
    };
    assert_eq!(count(&mut inside_rx), 1, "the player in the lair saw the shot");
    assert_eq!(count(&mut outside_rx), 0, "the player outside saw nothing");
}

/// The beats are **not evenly spaced** — 330 ms between two of them and 6.7 s
/// between two others. Scheduling them as a chain of equal steps would look
/// right and play wrong, so the spacing is pinned.
#[test]
fn the_beats_keep_their_uneven_spacing() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, 0, 0, 0);
    let base = world.tick;
    crate::game_loop::valakas::begin_cinematic(&mut world, VALAKAS_OID);

    let mut ticks: Vec<u64> = world.scheduler.pending_ticks_for_test();
    ticks.sort_unstable();
    ticks.dedup();
    // Steps 5 and 6 are 330 ms apart — under a tick, so they land together;
    // steps 8 and 9 are 6.7 s apart and must not.
    assert!(ticks.len() >= 8, "the beats are spread across distinct ticks: {ticks:?}");
    let span = ticks.last().unwrap() - base;
    assert_eq!(span, 260, "the sequence runs 26 s end to end");
}
