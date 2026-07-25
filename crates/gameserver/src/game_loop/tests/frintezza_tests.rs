//! Frintezza (Last Imperial Tomb) slice 1: entry + the room-crawl status machine.

use super::*;

use crate::data::door_data::{DoorOpenMethod, DoorTemplate};
use crate::data::instance_data::{ExitType, InstanceTemplate, SpawnGroup, TemplateSpawn};
use crate::game_loop::frintezza;
use crate::game_loop::helpers::instance_of;
use crate::model::components::InstanceDoorOpen;
use crate::model::door::Door;

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
        HALL_ALARM, TRASH, 29045, 29046, 29048, 29049, 29050, 29051, 29061,
    ] {
        if world.data.npc_data.get(id).is_none() {
            world
                .data
                .npc_data
                .insert_for_test(crate::data::npc_data::default_template(id));
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
    frintezza::on_monster_killed(&mut world, 100, HALL_ALARM);
    assert_eq!(world.instances.status(iid), 1, "alarm opened room 1");

    // Each cleared one-mob room advances the status.
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(
        world.instances.status(iid),
        2,
        "room1 cleared → room2_part1"
    );
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(
        world.instances.status(iid),
        3,
        "part1 cleared → room2_part2"
    );
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(
        world.instances.status(iid),
        4,
        "final room cleared → ready for Frintezza"
    );
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
    frintezza::on_monster_killed(&mut world, 100, HALL_ALARM);
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

    // Step 1 spawns Frintezza and freezes the party for the cinematic.
    frintezza::handle_intro_step(&mut world, iid, 0);
    frintezza::handle_intro_step(&mut world, iid, 1);
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

    // Run the rest: Scarlet, the portraits, and the hand-back.
    for step in 2..=5 {
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

#[test]
fn a_room_advances_only_once_every_mob_is_dead() {
    let (mut world, _tx, _db, _l) = test_world();
    seed_frintezza_rooms(&mut world, 3); // three mobs per room
    let _rx = ingame_player(&mut world, 1, 100, 1000, 1000, 0);
    frintezza::try_enter(&mut world, 100);
    let iid = instance_of(&world, 100);

    frintezza::on_monster_killed(&mut world, 100, HALL_ALARM); // → status 1, room1 (3 mobs)

    // The first two kills leave two/one mobs standing → still status 1.
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(world.instances.status(iid), 1, "two mobs left");
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(world.instances.status(iid), 1, "one mob left");
    // The third clears the room → advance.
    frintezza::on_monster_killed(&mut world, 100, TRASH);
    assert_eq!(world.instances.status(iid), 2, "room cleared → advance");
}
