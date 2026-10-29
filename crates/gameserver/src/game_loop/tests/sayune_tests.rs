//! Sayune: the zones, the jump button and the flight along a map's path.

use super::*;
use crate::data::sayune_data::SayuneData;
use crate::data::spawn_data::Territory;
use crate::data::zone_data::{Zone, ZoneData, ZoneKind};
use crate::game_loop::space::sayune;
use crate::model::components::space::SayuneRequest;
use crate::network::client_packets::ex_opcodes as exop;

const PLAYER: i32 = 9801;
const CID: u32 = 1;
/// Sigel Phoenix Knight — `SIXTH_CLASS_GROUP`.
const AWAKENED: i32 = 148;

/// A Sayune zone ±100 around the origin on `map_id`.
fn sayune_zone(map_id: i32) -> Zone {
    Zone {
        id: 0,
        name: format!("Sayune_{map_id}"),
        kind: ZoneKind::Sayune,
        territory: Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -100,
                x2: 100,
                y1: -100,
                y2: 100,
            },
            min_z: -100,
            max_z: 100,
        },
        castle_id: 0,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
        sayune_map_id: map_id,
    }
}

/// The real Sayune maps, a zone on `map_id` at the origin, and a player of
/// `class_id` standing well outside it.
fn sayune_world(map_id: i32, class_id: i32) -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.data.sayune = SayuneData::load_from(crate::data::DIST_GAME);
    world.data.zone_data.insert(sayune_zone(map_id));
    world
        .data
        .categories
        .insert_for_test("SIXTH_CLASS_GROUP", &[AWAKENED]);
    let rx = ingame_player(&mut world, CID, PLAYER, 5000, 5000, 0);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .class_id = class_id;
    (world, rx)
}

fn walk_to(world: &mut World, xyz: (i32, i32, i32)) {
    crate::game_loop::space::position::set_xyz(world, PLAYER, xyz);
    crate::game_loop::space::zones::revalidate_zone(world, PLAYER, true);
}

fn pos(world: &World) -> (i32, i32, i32) {
    let p = world.objects.get_component::<Position>(&PLAYER).unwrap();
    (p.x, p.y, p.z)
}

fn ex_body(opcode: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0xD0];
    v.extend_from_slice(&opcode.to_le_bytes());
    v.extend_from_slice(body);
    v
}

fn press_jump(world: &mut World) {
    sayune::handle_request_fly_move_start(world, CID);
}

fn fly(world: &mut World, location_id: i32) {
    sayune::handle_request_fly_move(world, CID, &location_id.to_le_bytes());
}

/// The datapack: map 12 is a straight path, map 14 opens on a junction with
/// three choices, and every Sayune zone in the dist names a map.
#[test]
fn the_real_sayune_maps_and_zones_load() {
    let data = SayuneData::load_from(crate::data::DIST_GAME);
    let straight = data.map(12).expect("map 12");
    assert_eq!(straight.inner.len(), 13);
    assert!(straight.inner.iter().all(|e| !e.selector));
    assert_eq!(straight.inner[0].xyz(), (-112860, 256154, -1232));

    let junction = data.map(14).expect("map 14");
    let start = &junction.inner[0];
    assert!(start.selector);
    let choices: Vec<i32> = start.inner.iter().map(|e| e.id).collect();
    assert_eq!(choices, [-1, 1, 5]);
    assert_eq!(start.inner[2].inner.len(), 6, "the Arms of Timiniel path");

    let zones = ZoneData::load_from(crate::data::DIST_GAME);
    let sayune: Vec<&Zone> = zones
        .zones
        .iter()
        .filter(|z| z.kind == ZoneKind::Sayune)
        .collect();
    assert_eq!(sayune.len(), 84);
    assert!(sayune.iter().all(|z| z.sayune_map_id != -1));
    let at = zones
        .sayune_zone_at(-113211, 256454, -1500)
        .expect("Sayune_12");
    assert_eq!(zones.zones[at].sayune_map_id, 12);
}

/// `SayuneZone.onEnter`: an Awakened player gets the zone flag and the jump
/// button, re-lit every second until they leave; anyone else gets neither.
#[test]
fn entering_lights_the_jump_button_for_the_awakened_only() {
    let notify = server_packets::ex_notify_fly_move_start();

    let (mut world, mut rx) = sayune_world(12, AWAKENED);
    drain(&mut rx);
    walk_to(&mut world, (0, 0, 0));
    assert!(sayune::in_sayune_zone(&world, PLAYER));
    assert!(drain(&mut rx).contains(&notify));
    advance_ticks(&mut world, crate::scheduler::ms_to_ticks(1000));
    assert!(drain(&mut rx).contains(&notify), "re-sent a second later");

    walk_to(&mut world, (5000, 5000, 0));
    assert!(!sayune::in_sayune_zone(&world, PLAYER), "onExit clears it");
    advance_ticks(&mut world, crate::scheduler::ms_to_ticks(2000));
    assert!(!drain(&mut rx).contains(&notify), "and the task stops");

    let (mut world, mut rx) = sayune_world(12, 0);
    walk_to(&mut world, (0, 0, 0));
    assert!(!sayune::in_sayune_zone(&world, PLAYER));
    assert!(!drain(&mut rx).contains(&notify));

    // `FreeJumpsForAll` opens it to every class.
    let (mut world, _rx) = sayune_world(12, 0);
    world.cfg.custom_misc.free_jumps_for_all = true;
    walk_to(&mut world, (0, 0, 0));
    assert!(sayune::in_sayune_zone(&world, PLAYER));
}

/// A straight path: the button makes the first hop, each `RequestFlyMove`
/// the next, and the request closes once the path runs out.
#[test]
fn a_straight_path_hops_point_by_point_and_closes() {
    let (mut world, mut rx) = sayune_world(12, AWAKENED);
    walk_to(&mut world, (0, 0, 0));
    drain(&mut rx);
    let path = world.data.sayune.map(12).unwrap().inner.clone();

    press_jump(&mut world);
    let first = server_packets::ex_fly_move(
        PLAYER,
        server_packets::SAYUNE_ONE_WAY_LOC,
        12,
        &[(path[0].id, path[0].x, path[0].y, path[0].z)],
    );
    assert!(drain(&mut rx).contains(&first));
    assert_eq!(pos(&world), path[0].xyz());

    for entry in &path[1..] {
        fly(&mut world, entry.id);
        assert_eq!(pos(&world), entry.xyz());
    }
    assert!(world.objects.has_component::<SayuneRequest>(&PLAYER));
    fly(&mut world, 0);
    assert!(
        !world.objects.has_component::<SayuneRequest>(&PLAYER),
        "past the last point the request closes"
    );
}

/// A junction: the first hop lands on its first choice and offers all three
/// (`START_LOC`); picking one hops there and follows that branch.
#[test]
fn a_junction_offers_its_choices_and_follows_the_pick() {
    let (mut world, mut rx) = sayune_world(14, AWAKENED);
    walk_to(&mut world, (0, 0, 0));
    drain(&mut rx);
    let start = world.data.sayune.map(14).unwrap().inner[0].clone();

    press_jump(&mut world);
    let choices: Vec<(i32, i32, i32, i32)> =
        start.inner.iter().map(|e| (e.id, e.x, e.y, e.z)).collect();
    let offer = server_packets::ex_fly_move(PLAYER, server_packets::SAYUNE_START_LOC, 14, &choices);
    assert!(drain(&mut rx).contains(&offer));
    assert_eq!(
        pos(&world),
        start.inner[0].xyz(),
        "Java lands on the first choice"
    );

    let arms = start.inner[2].clone();
    fly(&mut world, arms.id);
    assert_eq!(pos(&world), arms.xyz());
    for entry in &arms.inner {
        fly(&mut world, entry.id);
        assert_eq!(pos(&world), entry.xyz());
    }
    fly(&mut world, 0);
    assert!(!world.objects.has_component::<SayuneRequest>(&PLAYER));
}

/// The button's refusals: a chaotic player and one with another request open
/// are told why; nothing is flown.
#[test]
fn the_jump_is_refused_while_chaotic_or_busy() {
    let (mut world, mut rx) = sayune_world(12, AWAKENED);
    walk_to(&mut world, (0, 0, 0));
    drain(&mut rx);

    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .reputation = -1;
    press_jump(&mut world);
    assert!(!world.objects.has_component::<SayuneRequest>(&PLAYER));
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        1,
        "YOU_CANNOT_USE_SAYUNE_WHILE_IN_A_CHAOTIC_STATE"
    );

    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .reputation = 0;
    world.objects.add_components(
        &PLAYER,
        crate::model::components::commerce::EnchantRequest::default(),
    );
    press_jump(&mut world);
    assert!(!world.objects.has_component::<SayuneRequest>(&PLAYER));
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        1,
        "SAYUNE_CANNOT_BE_USED_WHILE_TAKING_OTHER_ACTIONS"
    );
}

/// The two client packets reach their handlers through the ex dispatch.
#[test]
fn the_sayune_client_packets_are_dispatched() {
    let (mut world, mut rx) = sayune_world(12, AWAKENED);
    walk_to(&mut world, (0, 0, 0));
    drain(&mut rx);
    let path = world.data.sayune.map(12).unwrap().inner.clone();

    on_packet(&mut world, CID, ex_body(exop::REQUEST_FLY_MOVE_START, &[]));
    assert_eq!(pos(&world), path[0].xyz());
    on_packet(
        &mut world,
        CID,
        ex_body(exop::REQUEST_FLY_MOVE, &path[1].id.to_le_bytes()),
    );
    assert_eq!(pos(&world), path[1].xyz());
}

/// `checkUseConditions`: no skill inside a Sayune zone.
#[test]
fn skills_are_refused_inside_a_sayune_zone() {
    let (mut world, ..) = cast_test_world();
    world.data.zone_data.insert(sayune_zone(12));
    world
        .data
        .categories
        .insert_for_test("SIXTH_CLASS_GROUP", &[AWAKENED]);
    let mut rx = ingame_caster(&mut world, CID, PLAYER, 5000, 5000);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .class_id = AWAKENED;
    crate::game_loop::combat::target::set_ref(&mut world, PLAYER, Some(PLAYER));
    let casting = |world: &World| {
        world
            .objects
            .has_component::<crate::model::components::combat::Casting>(&PLAYER)
    };

    // Control: the same cast outside the zone starts.
    crate::game_loop::skills::cast::use_magic(&mut world, CID, PLAYER, 1068, false, false);
    assert!(casting(&world), "outside the zone the cast starts");
    crate::game_loop::skills::cast::stop_casting(&mut world, PLAYER);

    walk_to(&mut world, (0, 0, 0));
    drain(&mut rx);
    crate::game_loop::skills::cast::use_magic(&mut world, CID, PLAYER, 1068, false, false);
    assert!(!casting(&world), "inside it is refused");
    assert_eq!(count_system_messages(&drain(&mut rx)), 1);
}
