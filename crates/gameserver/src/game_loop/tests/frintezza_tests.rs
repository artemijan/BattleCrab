//! Frintezza (Last Imperial Tomb) slice 1: entry + the room-crawl status machine.

use super::*;

use crate::data::door_data::{DoorOpenMethod, DoorTemplate};
use crate::data::instance_data::{ExitType, InstanceTemplate, SpawnGroup, TemplateSpawn};
use crate::game_loop::frintezza;
use crate::game_loop::helpers::instance_of;
use crate::model::components::{GroundItem, InstanceDoorOpen};
use crate::model::door::Door;
use crate::model::npc::AggroList;

const HALL_ALARM: i32 = 18328;
const TRASH: i32 = 18329;
/// One of the FIRST_ROOM_DOORS the crawl opens when the alarm falls.
const FIRST_ROOM_DOOR: i32 = 17130051;

/// A minimal template 136 (the real one has ~50 mobs per room) with `room_size`
/// mobs per room — 1 makes the crawl advance on a single kill per room.
fn seed_frintezza(world: &mut World) {
    seed_frintezza_rooms(world, 1);
}

fn seed_frintezza_rooms(world: &mut World, room_size: usize) {
    // Crawl mobs + the intro ensemble (Frintezza, Scarlet, demons, portraits,
    // cube) so their spawns resolve.
    for id in [
        HALL_ALARM, TRASH, 29045, 29046, 29047, 29048, 29049, 29050, 29051, 29061,
        // 29052/29053 are the invisible camera dummies the cinematic anchors on.
        29052, 29053,
    ] {
        if world.data.npc_data.get(id).is_none() {
            let mut t = crate::data::npc_data::default_template(id);
            t.base_hp_max = 100_000.0; // so HP-fraction morph thresholds are meaningful
            world.data.npc_data.insert_for_test(t);
        }
    }
    // A door template so the instance's FIRST_ROOM_DOOR copy resolves (closed
    // by default; the crawl opens it).
    if world.data.door_data.get(FIRST_ROOM_DOOR).is_none() {
        world.data.door_data.insert_for_test(DoorTemplate {
            id: FIRST_ROOM_DOOR,
            name: "frintezza_door".into(),
            node_x: [-87000; 4],
            node_y: [-141000; 4],
            node_z: -9168,
            height: 150,
            x: -87000,
            y: -141000,
            z: -9168,
            hp_max: 100,
            p_def: 0,
            m_def: 0,
            targetable: false,
            show_hp: false,
            open_by_default: false,
            open_method: DoorOpenMethod::None,
            open_time: 0,
            close_time: -1,
            random_time: 0,
        });
    }
    let room = |name: &str| SpawnGroup {
        name: name.to_string(),
        spawn_by_default: false,
        npcs: (0..room_size)
            .map(|_| TemplateSpawn {
                npc_id: TRASH,
                x: -87000,
                y: -141000,
                z: -9168,
                heading: 0,
            })
            .collect(),
    };
    world
        .data
        .instance_templates
        .insert_for_test(InstanceTemplate {
            id: frintezza::TEMPLATE_ID,
            name: Some("Last Imperial Tomb".into()),
            max_worlds: 5,
            duration_min: 120,
            empty_destroy_min: 5,
            enter: Some((-88015, -141153, -9168)),
            exit: ExitType::Origin,
            doors: vec![FIRST_ROOM_DOOR],
            groups: vec![
                SpawnGroup {
                    name: "default".into(),
                    spawn_by_default: true,
                    npcs: vec![TemplateSpawn {
                        npc_id: HALL_ALARM,
                        x: -87904,
                        y: -141296,
                        z: -9168,
                        heading: 0,
                    }],
                },
                room("room1"),
                room("room2_part1"),
                room("room2_part2"),
            ],
        });
}

#[test]
fn entry_builds_the_instance_and_spawns_the_alarm() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);

    assert!(
        frintezza::try_enter(&mut world, 100),
        "scroll-holder let in"
    );
    let iid = instance_of(&world, 100);
    assert!(iid >= 1, "player is inside a fresh instance");
    assert_eq!(
        world.instances.get(iid).unwrap().npcs.len(),
        1,
        "the default HALL_ALARM group spawned with the instance"
    );
    assert_eq!(world.instances.status(iid), 0, "crawl starts at status 0");
}

#[test]
fn the_crawl_advances_room_by_room_to_status_4() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    // The alarm falls → status 1, room1 populated.
    frintezza::on_monster_killed(&mut world, 100, 0, HALL_ALARM);
    assert_eq!(world.instances.status(iid), 1, "alarm opened room 1");

    // Each cleared one-mob room advances the status.
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(
        world.instances.status(iid),
        2,
        "room1 cleared → room2_part1"
    );
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(
        world.instances.status(iid),
        3,
        "part1 cleared → room2_part2"
    );
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(
        world.instances.status(iid),
        4,
        "final room cleared → ready for Frintezza"
    );
}

/// Find a spawned room guard (a `TRASH` mob) inside the instance.
fn a_room_guard(world: &World, iid: i32) -> i32 {
    world
        .instances
        .get(iid)
        .unwrap()
        .npcs
        .iter()
        .copied()
        .find(|&o| npc_id_of(world, o) == TRASH)
        .expect("a room guard was spawned")
}

#[test]
fn a_spawned_room_immediately_aggros_the_intruder() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    frintezza::on_monster_killed(&mut world, 100, 0, HALL_ALARM); // spawn room1 + nudge
    let mob = a_room_guard(&world, iid);
    let hate = world
        .objects
        .get_component::<AggroList>(&mob)
        .and_then(|a| a.0.get(&100))
        .map(|h| h.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "the room woke and aggroed the intruder");
}

#[test]
fn crawl_trash_can_drop_a_dewdrop_of_destruction() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    frintezza::on_monster_killed(&mut world, 100, 0, HALL_ALARM); // status 1, a room1 guard
    let mob = a_room_guard(&world, iid);

    // Force the 5% drop roll to hit; the trash kill then drops item 8556.
    world.forced_rolls.push_back(3); // < 5
    frintezza::on_monster_killed(&mut world, 100, mob, TRASH);

    let dropped = world
        .ground_item_regions
        .values()
        .flatten()
        .copied()
        .any(|o| {
            world
                .objects
                .get_component::<GroundItem>(&o)
                .is_some_and(|g| g.item_id == 8556)
        });
    assert!(dropped, "the forced roll dropped a Dewdrop of Destruction");
}

#[test]
fn killing_the_alarm_opens_the_first_room_doors() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    // The instance's door copy starts closed.
    let door = *world.instances.get(iid).unwrap().doors.first().unwrap();
    assert!(
        !world
            .objects
            .get_component::<InstanceDoorOpen>(&door)
            .unwrap()
            .0
    );
    assert_eq!(
        world.objects.get_component::<Door>(&door).unwrap().door_id,
        FIRST_ROOM_DOOR
    );

    // The alarm's death opens FIRST_ROOM_DOORS.
    frintezza::on_monster_killed(&mut world, 100, 0, HALL_ALARM);
    assert!(
        world
            .objects
            .get_component::<InstanceDoorOpen>(&door)
            .unwrap()
            .0,
        "room 1 opened when the alarm fell"
    );
}

#[test]
fn the_intro_freezes_players_then_spawns_the_ensemble_and_hands_control_back() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    let paralyzed = |world: &World| {
        world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&100)
            .map(|f| f.paralyzed)
            .unwrap_or(false)
    };

    // The steps mirror Java's `FRINTEZZA_INTRO_START`/`_1`..`_20` one for one.
    // `_2` is the beat that freezes the party and raises Frintezza.
    for step in 0..=2 {
        frintezza::handle_intro_step(&mut world, iid, step);
    }
    assert!(paralyzed(&world), "players are frozen during the cinematic");
    let frintezza_oid = world.instances.get_var(iid, "frintezza") as i32;
    assert!(
        frintezza_oid != 0
            && world
                .objects
                .get_component::<crate::model::npc::Npc>(&frintezza_oid)
                .is_some(),
        "Frintezza is on the field"
    );

    // Run the rest: the camera work, Scarlet, the portraits, the hand-back.
    for step in 3..=20 {
        frintezza::handle_intro_step(&mut world, iid, step);
    }
    let scarlet = world.instances.get_var(iid, "activeScarlet") as i32;
    assert!(
        scarlet != 0
            && world
                .objects
                .get_component::<crate::model::npc::Npc>(&scarlet)
                .is_some(),
        "Scarlet is fightable"
    );
    assert_eq!(
        world.instances.get_var(iid, "fightActive"),
        1,
        "the fight is on"
    );
    assert!(!paralyzed(&world), "control returns to the players");
    // All four portraits and demons were recorded.
    for i in 0..4 {
        assert_ne!(world.instances.get_var(iid, &format!("portrait{i}")), 0);
        assert_ne!(world.instances.get_var(iid, &format!("demon{i}")), 0);
    }
}

/// Drive the crawl + intro so Scarlet1 is on the field, returning `(iid,
/// scarlet1_oid)`.
fn arena_with_scarlet(
    world: &mut World,
) -> (i32, i32, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    seed_frintezza(world);
    let rx = ingame_player(world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(world, 100);
    let iid = instance_of(world, 100);
    for step in 0..=20 {
        frintezza::handle_intro_step(world, iid, step);
    }
    let scarlet1 = world.instances.get_var(iid, "activeScarlet") as i32;
    (iid, scarlet1, rx)
}

/// Skill ids of every `MagicSkillUse` in a drained batch.
fn cast_ids(pkts: &[Vec<u8>]) -> Vec<i32> {
    pkts.iter()
        .filter(|p| p[0] == crate::network::server_packets::opcodes::MAGIC_SKILL_USE)
        .filter_map(|p| {
            let mut r = commons::network::PacketReader::new(&p[1..]);
            r.read_i32()?; // cast bar
            r.read_i32()?; // caster
            r.read_i32()?; // target
            r.read_i32()
        })
        .collect()
}

fn set_hp_fraction(world: &mut World, oid: i32, frac: f64) {
    if let Some(v) = world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&oid)
    {
        v.cur_hp = v.max_hp as f64 * frac;
    }
}

fn npc_id_of(world: &World, oid: i32) -> i32 {
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .map_or(0, |n| n.npc_id)
}

#[test]
fn scarlet_morphs_at_eighty_then_twenty_percent_into_its_final_form() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, scarlet1, mut arena_rx) = arena_with_scarlet(&mut world);
    assert_eq!(npc_id_of(&world, scarlet1), frintezza::SCARLET1);

    // Above 80 %: no morph.
    set_hp_fraction(&mut world, scarlet1, 0.90);
    frintezza::on_scarlet_attack(&mut world, scarlet1, frintezza::SCARLET1);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&scarlet1)
            .unwrap()
            .script_value,
        0,
        "healthy Scarlet has not morphed"
    );

    // Crossing 80 % arms the first morph (script value 1).
    set_hp_fraction(&mut world, scarlet1, 0.79);
    frintezza::on_scarlet_attack(&mut world, scarlet1, frintezza::SCARLET1);
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&scarlet1)
            .unwrap()
            .script_value,
        1
    );
    drain(&mut arena_rx);
    frintezza::handle_fight_step(&mut world, iid, 1); // first morph cast
    // Java's SCARLET_FIRST_MORPH ends with `playRandomSong`, so the morph cast
    // (FIRST_MORPH_SKILL) and a song animation (5007) both go out.
    let first = cast_ids(&drain(&mut arena_rx));
    assert!(
        first.contains(&5007),
        "the first morph plays a song: {first:?}"
    );

    // Crossing 20 % arms the second morph → Scarlet1 is replaced by Scarlet2.
    set_hp_fraction(&mut world, scarlet1, 0.19);
    frintezza::on_scarlet_attack(&mut world, scarlet1, frintezza::SCARLET1);
    drain(&mut arena_rx);
    frintezza::handle_fight_step(&mut world, iid, 2); // second morph A
    // The second morph plays one too. This site never had a marker — only the
    // first morph did — so nothing recorded that it was missing.
    let second = cast_ids(&drain(&mut arena_rx));
    assert!(
        second.contains(&5007),
        "the second morph plays a song: {second:?}"
    );
    let scarlet2 = world.instances.get_var(iid, "activeScarlet") as i32;
    assert_ne!(scarlet2, scarlet1, "a new actor took the field");
    assert_eq!(
        npc_id_of(&world, scarlet2),
        frintezza::SCARLET2,
        "final form"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&scarlet1)
            .is_none(),
        "the first form was despawned"
    );

    // The final form wakes: no longer invulnerable.
    frintezza::handle_fight_step(&mut world, iid, 3);
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&scarlet2)
            .map(|f| f.invul)
            .unwrap_or(false),
        "Scarlet2 is killable once control returns"
    );
}

#[test]
fn each_standing_portrait_emits_a_demon_capped_at_the_maximum() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _scarlet1, _arena_rx) = arena_with_scarlet(&mut world);

    // Four intro demons seeded the count; a spawn pass adds one per portrait.
    assert_eq!(world.instances.get_var(iid, "demonCount"), 4);
    frintezza::handle_demon_spawn(&mut world, iid);
    assert_eq!(
        world.instances.get_var(iid, "demonCount"),
        8,
        "each of the four portraits emitted a demon"
    );

    // At the cap, no more spawn.
    world.instances.set_var(iid, "demonCount", 24);
    frintezza::handle_demon_spawn(&mut world, iid);
    assert_eq!(
        world.instances.get_var(iid, "demonCount"),
        24,
        "capped at MAX_DEMONS"
    );
}

#[test]
fn a_downed_portrait_stops_feeding_demons() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _scarlet1, _arena_rx) = arena_with_scarlet(&mut world);
    let portrait0 = world.instances.get_var(iid, "portrait0") as i32;

    frintezza::on_portrait_killed(&mut world, 100, portrait0);
    assert_eq!(
        world.instances.get_var(iid, "portrait0"),
        0,
        "its slot is cleared"
    );

    // Only the three survivors emit demons now.
    world.instances.set_var(iid, "demonCount", 0);
    frintezza::handle_demon_spawn(&mut world, iid);
    assert_eq!(
        world.instances.get_var(iid, "demonCount"),
        3,
        "three portraits left → three demons"
    );
}

#[test]
fn the_dewdrop_of_destruction_makes_a_portrait_suicide() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _scarlet1, _arena_rx) = arena_with_scarlet(&mut world);
    let portrait0 = world.instances.get_var(iid, "portrait0") as i32;

    // A normal skill does nothing; the Dewdrop (2276) kills it.
    frintezza::on_portrait_attacked(&mut world, portrait0, 100, Some(1234));
    assert!(
        !world
            .objects
            .get_component::<crate::model::components::Vitals>(&portrait0)
            .unwrap()
            .dead
    );
    frintezza::on_portrait_attacked(&mut world, portrait0, 100, Some(2276));
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Vitals>(&portrait0)
            .unwrap()
            .dead,
        "the Dewdrop made the portrait suicide"
    );
}

#[test]
fn a_slain_demon_frees_a_slot_under_the_cap() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _scarlet1, _arena_rx) = arena_with_scarlet(&mut world);
    world.instances.set_var(iid, "demonCount", 10);
    frintezza::on_demon_killed(&mut world, 100);
    assert_eq!(world.instances.get_var(iid, "demonCount"), 9);
}

#[test]
fn scarlet_wakes_its_skill_ai_when_struck_and_stops_it_when_slain() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, scarlet1, _arena_rx) = arena_with_scarlet(&mut world);
    assert_eq!(
        world.instances.get_var(iid, "scarletAi"),
        0,
        "dormant at first"
    );

    // The first blow arms the skill AI (Java's ATTACK/RANDOM_TARGET timers).
    frintezza::on_scarlet_attack(&mut world, scarlet1, frintezza::SCARLET1);
    assert_eq!(world.instances.get_var(iid, "scarletAi"), 1, "AI armed");

    // Once Scarlet is dead, the tick shuts the AI down.
    if let Some(v) = world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&scarlet1)
    {
        v.dead = true;
    }
    frintezza::handle_scarlet_skill(&mut world, iid);
    assert_eq!(
        world.instances.get_var(iid, "scarletAi"),
        0,
        "the AI stops when Scarlet falls"
    );
}

#[test]
fn scarlets_skill_table_only_yields_its_daemon_skills() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _s, _arena_rx) = arena_with_scarlet(&mut world);

    // First form: charge / yoke / attack only.
    let first_form = [(5015, 2), (5015, 5), (5016, 1), (5014, 2)];
    for _ in 0..300 {
        let pick = frintezza::pick_daemon_skill(&mut world, iid, frintezza::SCARLET1);
        assert!(
            first_form.contains(&pick),
            "unexpected first-form skill {pick:?}"
        );
    }

    // Final form with its ranged skills off cooldown: the full table.
    world.tick = 10_000; // past RANGED_SKILL_MIN_COOLTIME so field/morph unlock
    let final_form = [
        (5015, 3),
        (5015, 6),
        (5015, 2),
        (5019, 1),
        (5018, 1),
        (5016, 1),
        (5014, 3),
    ];
    for _ in 0..300 {
        let pick = frintezza::pick_daemon_skill(&mut world, iid, frintezza::SCARLET2);
        assert!(
            final_form.contains(&pick),
            "unexpected final-form skill {pick:?}"
        );
    }
}

#[test]
fn killing_the_final_form_runs_the_finish_cinematic() {
    let (mut world, _tx, _db, _l) = test_world();
    let (iid, _scarlet1, _arena_rx) = arena_with_scarlet(&mut world);
    let frintezza_oid = world.instances.get_var(iid, "frintezza") as i32;

    // Player 100 (inside the instance) lands the killing blow on Scarlet2.
    frintezza::on_scarlet_killed(&mut world, 100);
    // The fight stops at once, but the encounter isn't cleared until the
    // cinematic plays out.
    assert_eq!(world.instances.get_var(iid, "fightActive"), 0);
    assert_eq!(
        world.instances.get_var(iid, "cleared"),
        0,
        "cinematic pending"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&frintezza_oid)
            .is_some(),
        "Frintezza still stands during the opening shot"
    );

    frintezza::handle_finish_step(&mut world, iid, 0); // parting shot
    frintezza::handle_finish_step(&mut world, iid, 1); // Frintezza dies
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&frintezza_oid)
            .is_none(),
        "Frintezza fell with its guardian"
    );
    frintezza::handle_finish_step(&mut world, iid, 2); // doors reopen
    assert_eq!(
        world.instances.get_var(iid, "cleared"),
        1,
        "encounter cleared"
    );
}

#[test]
fn a_room_advances_only_once_every_mob_is_dead() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza_rooms(&mut world, 3); // three mobs per room
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    frintezza::on_monster_killed(&mut world, 100, 0, HALL_ALARM); // → status 1, room1 (3 mobs)

    // The first two kills leave two/one mobs standing → still status 1.
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(world.instances.status(iid), 1, "two mobs left");
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(world.instances.status(iid), 1, "one mob left");
    // The third clears the room → advance.
    frintezza::on_monster_killed(&mut world, 100, 0, TRASH);
    assert_eq!(world.instances.status(iid), 2, "room cleared → advance");
}

/// Every `SpecialCamera` in a drained batch, as
/// `(target_oid, force, angle1, angle2, time, duration)` — the six fields the
/// choreography is actually written in terms of.
fn cameras(pkts: &[Vec<u8>]) -> Vec<(i32, i32, i32, i32, i32, i32)> {
    // Opcode plus the eleven ints the wire carries.
    const CAMERA_LEN: usize = 1 + 11 * 4;
    pkts.iter()
        .filter(|p| p[0] == 0xD6 && p.len() >= CAMERA_LEN)
        .map(|p| {
            let rd = |i: usize| i32::from_le_bytes(p[1 + i * 4..5 + i * 4].try_into().unwrap());
            (rd(0), rd(1), rd(2), rd(3), rd(4), rd(5))
        })
        .collect()
}

/// **The heart of the cinematic.** Java's intro fires 34 `SpecialCamera` shots
/// off five invisible dummy actors; the port used to send two and skip the
/// dummies entirely, so the camera sat on Frintezza throughout.
///
/// The assertion that matters is the **duration**: every shot in this script
/// uses Java's 11-argument `SpecialCamera` overload, which forwards `duration`
/// and `range` into each other's slots — so the argument the script writes as
/// *range* is the one that reaches the wire. Transcribing the literals into the
/// canonical 12-arg order instead would put a 0 there and the client would cut
/// each shot instantly.
#[test]
fn the_intro_plays_its_full_camera_choreography_off_the_dummies() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza(&mut world);
    let mut rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    for step in 0..=2 {
        frintezza::handle_intro_step(&mut world, iid, step);
    }
    // The camera anchors exist. Without them the shots have nothing to hang off.
    for key in [
        "frintezzaDummy",
        "overheadDummy",
        "portraitDummy1",
        "portraitDummy3",
        "scarletDummy",
    ] {
        assert_ne!(
            world.instances.get_var(iid, key),
            0,
            "{key} was spawned for the cinematic"
        );
    }
    let overhead = world.instances.get_var(iid, "overheadDummy") as i32;

    // INTRO_2's own two shots, both anchored on the overhead dummy. The second
    // is the swap's witness: written `(…, 6500, 7000, 0, …)`, it must reach the
    // wire as time 6500 / duration 7000, not duration 0.
    let shots = cameras(&drain(&mut rx));
    assert!(
        shots.contains(&(overhead, 0, 75, -89, 0, 100)),
        "the snap-in shot, duration 100 (its `range` argument): {shots:?}"
    );
    assert!(
        shots.contains(&(overhead, 300, 90, -10, 6500, 7000)),
        "the 7 s hold — a 0 here means the 11-arg swap was missed: {shots:?}"
    );

    // Run the rest, counting per step against Java's own tally. Written out
    // rather than as a total so a shot moving between beats is caught too.
    // `_8` is two because `sendPacketX` sends one of its pair to each player,
    // twice; `_6`, `_7`, `_19` and `_20` carry no camera at all.
    let expected: [(u8, usize); 18] = [
        (3, 1),
        (4, 1),
        (5, 2),
        (6, 0),
        (7, 0),
        (8, 2),
        (9, 2),
        (10, 1),
        (11, 1),
        (12, 1),
        (13, 1),
        (14, 1),
        (15, 2),
        (16, 1),
        (17, 1),
        (18, 1),
        (19, 0),
        (20, 0),
    ];
    for (step, want) in expected {
        frintezza::handle_intro_step(&mut world, iid, step);
        let got = cameras(&drain(&mut rx)).len();
        assert_eq!(got, want, "INTRO_{step} fires {want} camera shot(s)");
    }

    // The dummies are cleaned up as their last shot lands — Java deletes each
    // one explicitly, and a leftover invisible NPC would linger in the arena.
    for key in [
        "frintezzaDummy",
        "portraitDummy1",
        "portraitDummy3",
        "overheadDummy",
        "scarletDummy",
    ] {
        assert_eq!(
            world.instances.get_var(iid, key),
            0,
            "{key} was deleted once its shots were done"
        );
    }
}
