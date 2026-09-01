//! Valakas — the attack-side rules.

use super::*;

use crate::game_loop::npc::bosses::combat::BossCombat;
use crate::game_loop::valakas::{AttackVerdict, DEAD, FIGHTING, VALAKAS, WAITING};

const VALAKAS_OID: i32 = NPC_OID + 100;
const PLAYER: i32 = 9990;
const CID: u32 = 1;
const DIST: &str = crate::data::DIST_GAME;
/// A point inside zone 12010 ("Valakas Boss"), taken from the boss's own lair.
const IN_LAIR: (i32, i32, i32) = (212_852, -114_842, -1_632);

fn valakas_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
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
        model::grand_boss::GrandBoss {
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
    let p = world
        .objects
        .get_component_mut::<Position>(&PLAYER)
        .unwrap();
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
    assert!(
        zone.contains(IN_LAIR.0, IN_LAIR.1, IN_LAIR.2),
        "the fixture's point is inside the lair"
    );
}

/// **Attacking from outside the lair kills you.** Java's `attacker.doDie()` —
/// a hard anti-exploit against plinking at Valakas from safety.
#[test]
fn attacking_from_outside_the_lair_kills_the_attacker() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    put_player_at(&mut world, 0, 0, 0); // far outside

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::KilledForAttackingFromOutside);
    assert!(
        world.objects.get_component::<Vitals>(&PLAYER).unwrap().dead,
        "the attacker died"
    );
}

/// **The zone check comes first.** With Valakas dead, an out-of-zone attacker
/// still dies rather than merely being teleported — Java's ordering, and the
/// half a reordering would silently lose.
#[test]
fn the_zone_check_precedes_the_status_check() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = DEAD;
    put_player_at(&mut world, 0, 0, 0);

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(
        verdict,
        AttackVerdict::KilledForAttackingFromOutside,
        "death, not a teleport"
    );
}

/// Inside the lair but before the fight has begun: bounced out, not killed.
#[test]
fn attacking_before_the_fight_removes_the_attacker() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = WAITING;
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);

    let verdict = crate::game_loop::valakas::on_valakas_attacked(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(verdict, AttackVerdict::RemovedNotFighting);
    assert!(
        !world.objects.get_component::<Vitals>(&PLAYER).unwrap().dead,
        "removed, not killed"
    );
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    assert_eq!((p.x, p.y), (150_037, -57_255), "dumped at ATTACKER_REMOVE");
}

/// Inside the lair, fight underway: an ordinary hit is allowed through.
#[test]
fn a_legitimate_attacker_is_allowed() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
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
    assert_eq!(
        world.scheduler.len() - before,
        10,
        "ten beats scheduled at once"
    );

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
    let p = world
        .objects
        .get_component::<Position>(&VALAKAS_OID)
        .unwrap();
    assert_eq!((p.x, p.y, p.z), IN_LAIR, "moved to the lair");
}

/// **The cinematic plays for the lair, not the neighbourhood.** A player
/// inside sees it; one outside sees nothing — which is why Java broadcasts on
/// the zone rather than the boss's region.
#[test]
fn only_players_inside_the_lair_see_the_cinematic() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );

    let mut inside_rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let outsider = PLAYER + 1;
    let mut outside_rx = ingame_caster(&mut world, CID + 1, outsider, 0, 0);
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    {
        let p = world
            .objects
            .get_component_mut::<Position>(&outsider)
            .unwrap();
        p.x = 0;
        p.y = 0;
        p.z = 0;
    }
    while inside_rx.try_recv().is_ok() {}
    while outside_rx.try_recv().is_ok() {}

    crate::game_loop::valakas::handle_cinematic_step(&mut world, VALAKAS_OID, 0);

    let count = |rx: &mut UnboundedReceiver<bytes::Bytes>| {
        let mut n = 0;
        while let Ok(p) = rx.try_recv() {
            if p.first() == Some(&0xD6) {
                n += 1;
            }
        }
        n
    };
    assert_eq!(
        count(&mut inside_rx),
        1,
        "the player in the lair saw the shot"
    );
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
    assert!(
        ticks.len() >= 8,
        "the beats are spread across distinct ticks: {ticks:?}"
    );
    let span = ticks.last().unwrap() - base;
    assert_eq!(span, 260, "the sequence runs 26 s end to end");
}

// ---------------------------------------------------------------------------
// Slice 21: the entry flow wired — Klein → Heart of Volcano → beginning.
// ---------------------------------------------------------------------------

use crate::game_loop::valakas::{DORMANT, MAX_PEOPLE, VACUALITE};
use crate::scheduler::ScheduledTask;

/// A world with a DORMANT, spawned Valakas and the item template registered.
fn entry_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = valakas_world();
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = DORMANT;
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = VACUALITE;
    t.name = "Vacualite Floating Stone".into();
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    (world, db, l)
}

fn give_stone(world: &mut World, oid: i32) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<Inventory>(&oid)
        .unwrap()
        .add_item(&data.item_data, 8_100_000 + oid, VACUALITE, 1);
}

/// Klein refuses a stoneless visitor (`31540-06`); with the stone, the player
/// is teleported to the Hall of Flames and gains the `allowEnter` flag.
#[test]
fn klein_gates_the_antechamber_on_the_vacualite_stone() {
    let (mut world, _db, _l) = entry_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert_eq!(
        crate::game_loop::valakas::enter_hall_of_flames(&mut world, PLAYER),
        Some("31540-06.htm"),
        "no stone, no antechamber"
    );

    give_stone(&mut world, PLAYER);
    assert_eq!(
        crate::game_loop::valakas::enter_hall_of_flames(&mut world, PLAYER),
        None,
        "admitted"
    );
    let p = world
        .objects
        .get_component::<Position>(&PLAYER)
        .copied()
        .unwrap();
    assert_eq!(
        (p.x, p.y),
        (183_813, -115_157),
        "teleported to the Hall of Flames"
    );
    assert!((p.z - -3_303).abs() < 50, "z near the Hall floor: {}", p.z);
}

/// Klein's crowding html tracks the lifetime count across the thresholds.
#[test]
fn klein_shows_the_crowding_html_by_count() {
    let (mut world, _db, _l) = entry_world();
    for (count, html) in [
        (0, "31540-01.htm"),
        (50, "31540-02.htm"),
        (150, "31540-04.htm"),
        (200, "31540-05.htm"),
    ] {
        world.valakas_entry_count = count;
        assert_eq!(
            crate::game_loop::valakas::klein_status_html(&world),
            html,
            "at {count}"
        );
    }
}

/// The Heart of Volcano refuses without the antechamber flag (`31385-04`),
/// while fighting (`-02`), and at the cap (`-03`).
#[test]
fn the_heart_refuses_without_the_flag_or_when_locked() {
    let (mut world, _db, _l) = entry_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER),
        Some("31385-04.htm"),
        "no allowEnter flag"
    );

    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = FIGHTING;
    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER),
        Some("31385-02.htm"),
        "fighting"
    );

    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = DORMANT;
    world.valakas_entry_count = MAX_PEOPLE;
    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER),
        Some("31385-03.htm"),
        "full"
    );
}

/// The full first-entry arc: admitted into the lair, the count ticks, the
/// FIRST entry arms `beginning` + WAITING, and a second entrant during the
/// window does NOT re-arm the clock — the boss begins exactly once, on the
/// first deadline.
#[test]
fn the_first_entry_arms_beginning_and_the_second_does_not() {
    let (mut world, _db, _l) = entry_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let _rx2 = ingame_caster(&mut world, 2, PLAYER + 1, 10, 0);
    // Both have the antechamber flag (from Klein).
    for oid in [PLAYER, PLAYER + 1] {
        world
            .objects
            .get_component_mut::<model::components::PlayerVariables>(&oid)
            .unwrap()
            .0
            .insert("VALAKAS_ALLOW_ENTER".into(), "1".into());
    }

    let before = world.scheduler.len();
    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER),
        None,
        "first admitted"
    );
    assert_eq!(world.valakas_entry_count, 1);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, VALAKAS),
        Some(WAITING),
        "WAITING"
    );
    assert_eq!(world.scheduler.len() - before, 1, "beginning armed once");
    let pos = world
        .objects
        .get_component::<Position>(&PLAYER)
        .copied()
        .unwrap();
    assert!((204_328..=204_928).contains(&pos.x), "in the lair: {pos:?}");
    // The flag was consumed — a re-talk without a fresh Klein visit is refused.
    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER),
        Some("31385-04.htm"),
        "flag consumed"
    );

    // Second player enters mid-window: count ticks, no new timer.
    let mid = world.scheduler.len();
    assert_eq!(
        crate::game_loop::valakas::heart_enter(&mut world, PLAYER + 1),
        None,
        "second admitted"
    );
    assert_eq!(world.valakas_entry_count, 2);
    assert_eq!(world.scheduler.len(), mid, "the clock is NOT restarted");

    // The window elapses → beginning fires → the cinematic runs (boss on the
    // lair coords, ten beats scheduled).
    let before_beats = world.scheduler.len();
    advance_ticks(&mut world, 30 * 60 * 10 + 5);
    // begin_cinematic teleported the boss to the lair and armed the beats;
    // running them out flips FIGHTING.
    advance_ticks(&mut world, 300);
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, VALAKAS),
        Some(FIGHTING),
        "the fight began"
    );
    let _ = before_beats;
}

/// The count NEVER resets across a kill+respawn cycle — Java's static
/// `playerCount` only increments (the Core-minions "port what it does" call).
#[test]
fn the_entry_count_never_resets() {
    let (mut world, _db, _l) = entry_world();
    world.valakas_entry_count = 7;

    crate::game_loop::grand_boss::on_grand_boss_killed(&mut world, VALAKAS);
    world.grand_bosses.get_mut(&VALAKAS).unwrap().respawn_time = 1;
    crate::game_loop::grand_boss::resolve_at_boot(&mut world);

    assert_eq!(
        world.valakas_entry_count, 7,
        "a respawn does not reset the lifetime count"
    );
}

/// The router e2e (the slice-20 lesson): the Heart of Volcano's dist-html
/// `Quest ValakasTeleporters` bypass reaches the entry through the real bypass
/// router and registered script — not a direct call.
#[test]
fn the_bypass_reaches_the_entry_through_the_router() {
    let (mut world, _db, _l) = entry_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    world
        .objects
        .get_component_mut::<model::components::PlayerVariables>(&PLAYER)
        .unwrap()
        .0
        .insert("VALAKAS_ALLOW_ENTER".into(), "1".into());
    let heart_oid = NPC_OID + 101;
    world.data.npc_data.insert_for_test({
        let mut t = crate::data::npc_data::default_template(31385);
        t.type_name = "Folk".into();
        t
    });
    add_test_npc(&mut world, heart_oid, 31385, "Folk", 70, 20, 0, 0);
    world
        .objects
        .add_components(&PLAYER, LastFolkNpc(heart_oid));

    handle_request_bypass_to_server(&mut world, CID, &bypass_body("Quest ValakasTeleporters"));

    assert_eq!(
        world.valakas_entry_count, 1,
        "the bypass admitted through the router"
    );
    assert_eq!(
        crate::game_loop::grand_boss::status(&world, VALAKAS),
        Some(WAITING),
        "WAITING set"
    );
}

// ---------------------------------------------------------------------------
// The death tail — exit cubes + zone clear (`onKill` / `remove_players`).
// ---------------------------------------------------------------------------

const CUBE: i32 = 31759;

fn spawned_cubes(world: &World) -> usize {
    world
        .npc_regions
        .values()
        .flatten()
        .filter(|oid| {
            world
                .objects
                .get_component::<model::npc::Npc>(oid)
                .is_some_and(|n| n.npc_id == CUBE)
        })
        .count()
}

/// Killing Valakas (through the real `npc_do_die` death path) arms the
/// eight-beat death cinematic.
#[test]
fn killing_valakas_arms_the_death_cinematic() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );

    crate::game_loop::npc::npc_do_die(&mut world, VALAKAS_OID, PLAYER);

    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, ScheduledTask::ValakasDeathCinematic { step: 0, .. })),
        "the death cinematic's first beat is armed"
    );
}

/// The death cinematic's final beat (`die_8`) drops the fifteen exit cubes and
/// arms the 15-minute `remove_players` oust — driven through the loop dispatch.
#[test]
fn the_death_cinematic_spawns_the_exit_cubes() {
    let (mut world, _db, _l) = valakas_world();
    register_cube(&mut world, CUBE);
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );

    crate::game_loop::npc::npc_do_die(&mut world, VALAKAS_OID, PLAYER);
    assert_eq!(spawned_cubes(&world), 0, "no cubes until die_8");

    // die_8 fires at 16_500 ms → 160 ticks; advance past it.
    advance_ticks(&mut world, 170);

    assert_eq!(
        spawned_cubes(&world),
        15,
        "die_8 dropped all fifteen exit cubes"
    );
    assert!(
        world
            .scheduler
            .pending_tasks_for_test()
            .iter()
            .any(|t| matches!(t, ScheduledTask::ValakasRemovePlayers)),
        "remove_players armed"
    );
}

/// `remove_players`, fired through the loop dispatch, ousts a lingering player
/// from the lair to the exit.
#[test]
fn remove_players_ousts_lingering_players_through_the_loop() {
    let (mut world, _db, _l) = valakas_world();
    let _rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);

    world
        .scheduler
        .schedule(world.tick, ScheduledTask::ValakasRemovePlayers);
    advance_ticks(&mut world, 1);

    let p = world
        .objects
        .get_component::<Position>(&PLAYER)
        .copied()
        .unwrap();
    assert!(
        (150_037..=150_537).contains(&p.x) && (-57_720..=-57_220).contains(&p.y),
        "the lingering player was ousted to the exit: {p:?}"
    );
}

/// Fifteen minutes with nobody landing a hit resets the fight: Valakas goes
/// home, reverts to DORMANT and heals to full (Java `regen_task`).
#[test]
fn a_fifteen_minute_idle_resets_valakas() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    world.objects.add_components(
        &VALAKAS_OID,
        BossCombat {
            last_attack_tick: 0,
            actual_victim: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Vitals>(&VALAKAS_OID)
        .unwrap()
        .cur_hp = 100.0;
    world.tick = 10_000; // > the 9000-tick idle window since last_attack 0

    crate::game_loop::valakas::handle_regen(&mut world, VALAKAS_OID);

    assert_eq!(
        world.grand_bosses.get(&VALAKAS).unwrap().status,
        DORMANT,
        "reverted to dormant"
    );
    let pos = world
        .objects
        .get_component::<Position>(&VALAKAS_OID)
        .unwrap();
    assert_eq!((pos.x, pos.y), (-105_200, -253_104), "sent home");
    let v = world.objects.get_component::<Vitals>(&VALAKAS_OID).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "healed to full");
}

/// A Valakas hit within the window keeps fighting and the regen beat re-arms.
#[test]
fn a_recently_hit_valakas_keeps_fighting() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    world.tick = 10_000;
    world.objects.add_components(
        &VALAKAS_OID,
        BossCombat {
            last_attack_tick: world.tick,
            actual_victim: 0,
        },
    );
    let before = world.scheduler.len();

    crate::game_loop::valakas::handle_regen(&mut world, VALAKAS_OID);

    assert_eq!(
        world.grand_bosses.get(&VALAKAS).unwrap().status,
        FIGHTING,
        "still fighting"
    );
    assert!(world.scheduler.len() > before, "the regen beat re-arms");
}

// ---------------------------------------------------------------------------
// skill_task — the combat skill AI
// ---------------------------------------------------------------------------

fn insert_valakas_skill(world: &mut World, id: i32, cast_range: i32) {
    world.data.skill_data.insert_for_test(Skill {
        self_continuous: false,
        id,
        level: 1,
        cast_range,
        ..Default::default()
    });
}

fn full_hp(world: &mut World, oid: i32) {
    let v = world.objects.get_component_mut::<Vitals>(&oid).unwrap();
    v.cur_hp = v.max_hp as f64;
    v.max_mp = 100_000;
    v.cur_mp = 100_000.0;
}

/// **Valakas picks a living lair target and breathes on it.** At full HP and
/// unsurrounded he draws from the regular pool; the first entry (4681) is short
/// range, floored to 600, and the target is right on top of him, so he casts.
#[test]
fn valakas_casts_a_skill_at_a_lair_target() {
    let (mut world, _db, _l) = valakas_world(); // status FIGHTING
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    full_hp(&mut world, VALAKAS_OID);
    world.objects.add_components(
        &VALAKAS_OID,
        BossCombat {
            last_attack_tick: 0,
            actual_victim: 0,
        },
    );
    let _rx = ingame_player(&mut world, 7, PLAYER, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    insert_valakas_skill(&mut world, 4681, 40);
    world.force_roll(0); // random target (only one alive)
    world.force_roll(0); // regular-pool pick -> 4681

    crate::game_loop::valakas::handle_skill_task(&mut world, VALAKAS_OID);

    assert!(
        world.objects.has_component::<Casting>(&VALAKAS_OID),
        "Valakas cast a breath skill at the lair target"
    );
    assert_eq!(
        world
            .objects
            .get_component::<BossCombat>(&VALAKAS_OID)
            .unwrap()
            .actual_victim,
        PLAYER,
        "the target was recorded"
    );
}

/// The beat stops once the fight is over — a dead/reset Valakas doesn't keep
/// casting or re-arming.
#[test]
fn the_skill_task_stops_when_the_fight_is_over() {
    let (mut world, _db, _l) = valakas_world();
    world.grand_bosses.get_mut(&VALAKAS).unwrap().status = DEAD;
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    let before = world.scheduler.len();

    crate::game_loop::valakas::handle_skill_task(&mut world, VALAKAS_OID);

    assert_eq!(world.scheduler.len(), before, "the beat did not re-arm");
    assert!(!world.objects.has_component::<Casting>(&VALAKAS_OID));
}

/// **A dead victim is dropped for a living one.** Valakas holds `_actualVictim`
/// between beats, but the next beat re-picks when that victim has died.
#[test]
fn valakas_re_picks_a_dead_victim() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    full_hp(&mut world, VALAKAS_OID);
    let dead_player = PLAYER;
    let live_player = PLAYER + 1;
    let _r1 = ingame_player(&mut world, 7, dead_player, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    let _r2 = ingame_player(&mut world, 8, live_player, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    world
        .objects
        .get_component_mut::<Vitals>(&dead_player)
        .unwrap()
        .dead = true;
    world.objects.add_components(
        &VALAKAS_OID,
        BossCombat {
            last_attack_tick: 0,
            actual_victim: dead_player, // stale, now dead
        },
    );
    insert_valakas_skill(&mut world, 4681, 40);
    world.force_roll(0); // random target among the living
    world.force_roll(0); // skill pick

    crate::game_loop::valakas::handle_skill_task(&mut world, VALAKAS_OID);

    assert_eq!(
        world
            .objects
            .get_component::<BossCombat>(&VALAKAS_OID)
            .unwrap()
            .actual_victim,
        live_player,
        "the dead victim was replaced by the living one"
    );
}

/// Java's `"broadcast_spawn"` — the lair theme and Valakas' roar animation,
/// armed 100 ms into `"beginning"`. The port had the cinematic beats but not
/// the two packets that open them, so the fight started in silence with no
/// roar.
///
/// The theme is `PlaySound(1, …)`, the **music** shape — not the type-0
/// quest-sound form, which is a different packet to the client rather than a
/// cosmetic variant of the same one.
#[test]
fn beginning_the_cinematic_plays_the_lair_theme_and_the_roar() {
    use crate::network::server_packets::opcodes;

    let (mut world, _db, _l) = valakas_world();
    add_test_npc(
        &mut world,
        VALAKAS_OID,
        VALAKAS,
        "GrandBoss",
        85,
        IN_LAIR.0,
        IN_LAIR.1,
        IN_LAIR.2,
    );
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    put_player_at(&mut world, IN_LAIR.0, IN_LAIR.1, IN_LAIR.2);
    while rx.try_recv().is_ok() {}

    crate::game_loop::valakas::begin_cinematic(&mut world, VALAKAS_OID);

    let packets: Vec<Vec<u8>> =
        std::iter::from_fn(|| rx.try_recv().ok().map(|b| b.to_vec())).collect();
    let sound = packets
        .iter()
        .find(|p| p[0] == opcodes::PLAY_SOUND)
        .expect("the lair theme is played");
    assert_eq!(
        i32::from_le_bytes([sound[1], sound[2], sound[3], sound[4]]),
        1,
        "type 1 — the music shape, not the type-0 quest sound"
    );
    assert!(
        packets.iter().any(|p| p[0] == opcodes::SOCIAL_ACTION
            && i32::from_le_bytes([p[1], p[2], p[3], p[4]]) == VALAKAS_OID
            && i32::from_le_bytes([p[5], p[6], p[7], p[8]]) == 3),
        "and Valakas performs social action 3"
    );
}

/// The exit cubes carry Java's 15-minute `addSpawn` lifetime. Without it they
/// stand in an empty lair until the next restart, and the next Valakas fight
/// adds fifteen more on top of them.
#[test]
fn the_exit_cubes_are_armed_to_despawn() {
    let (mut world, _db, _l) = valakas_world();
    add_test_npc(&mut world, VALAKAS_OID, VALAKAS, "GrandBoss", 85, 0, 0, 0);
    world.id_pool = 0x4400_0000..0x4400_0100;
    // The cubes need a template to spawn at all — without one
    // `spawn_npc_at` returns None and the despawn count is silently zero.
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(31759));

    let before = world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .filter(|t| matches!(t, ScheduledTask::DespawnNpc { .. }))
        .count();
    // The final death beat is the one that drops the cubes.
    let last = (crate::game_loop::valakas::death_cinematic_len() - 1) as u8;
    crate::game_loop::valakas::handle_death_cinematic_step(&mut world, VALAKAS_OID, last);
    let after = world
        .scheduler
        .pending_tasks_for_test()
        .iter()
        .filter(|t| matches!(t, ScheduledTask::DespawnNpc { .. }))
        .count();
    assert_eq!(
        after - before,
        15,
        "one despawn armed per exit cube, not zero and not one"
    );
}
