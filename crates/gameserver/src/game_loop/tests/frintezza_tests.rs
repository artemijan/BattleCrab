//! Frintezza (Last Imperial Tomb) slice 1: entry + the room-crawl status machine.

use super::*;

use crate::data::instance_data::{ExitType, InstanceTemplate, SpawnGroup, TemplateSpawn};
use crate::game_loop::frintezza;
use crate::game_loop::helpers::instance_of;

const HALL_ALARM: i32 = 18328;
const TRASH: i32 = 18329;

/// A minimal template 136 (the real one has ~50 mobs per room) with `room_size`
/// mobs per room — 1 makes the crawl advance on a single kill per room.
fn seed_frintezza(world: &mut World) {
    seed_frintezza_rooms(world, 1);
}

fn seed_frintezza_rooms(world: &mut World, room_size: usize) {
    for id in [HALL_ALARM, TRASH] {
        if world.data.npc_data.get(id).is_none() {
            world
                .data
                .npc_data
                .insert_for_test(crate::data::npc_data::default_template(id));
        }
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
            doors: vec![],
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
