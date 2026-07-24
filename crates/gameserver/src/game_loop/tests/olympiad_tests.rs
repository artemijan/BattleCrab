//! Grand Olympiad (G25) slice 1: noble registration into the 1v1 waiting list
//! and its eligibility/timing gates.

use super::*;

use crate::model::olympiad::{CompetitionType, DEFAULT_POINTS};
use crate::model::Player;
use crate::network::server_packets::opcodes;

/// True if a `SystemMessage` (0x62) with `id` was sent.
fn got_sm(packets: &[Vec<u8>], id: i16) -> bool {
    packets.iter().any(|p| {
        p.len() >= 3 && p[0] == opcodes::SYSTEM_MESSAGE && i16::from_le_bytes([p[1], p[2]]) == id
    })
}

/// An Olympiad-eligible character: 3rd class (Gladiator, id 2), level 55.
fn make_noble(
    world: &mut World,
    client_id: u32,
    object_id: i32,
) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
    // Gladiator (class 2) is in the 3rd-class group; the test data has no
    // categories, so seed the one the eligibility gate reads.
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[2]);
    let rx = ingame_player(world, client_id, object_id, 0, 0, 0);
    let p = world
        .objects
        .get_component_mut::<Player>(&object_id)
        .unwrap();
    p.class_id = 2;
    p.base_class_id = 2;
    p.level = 55;
    rx
}

/// Open the competition window with plenty of time left to register.
fn open_games(world: &mut World) {
    world.olympiad.in_comp_period = true;
    world.olympiad.comp_end_tick = world.tick + 100_000; // ~2.7h at 100ms/tick
}

#[test]
fn noble_registers_for_the_1v1_list() {
    let (mut world, _tx, _db, _l) = test_world();
    open_games(&mut world);
    let mut rx = make_noble(&mut world, 1, 100);

    assert!(crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(
        world.olympiad.non_class_registers.contains(&100),
        "queued in the non-class list"
    );
    let noble = world
        .olympiad
        .nobles
        .get(&100)
        .expect("noble record created");
    assert_eq!(
        noble.points, DEFAULT_POINTS,
        "starts with the default points"
    );
    assert_eq!(noble.comp_done_week, 0);
    assert!(
        got_sm(
            &drain(&mut rx),
            crate::network::server_packets::sm_ids::YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH
        ),
        "confirmation message sent"
    );
}

#[test]
fn registration_gates_reject_the_ineligible() {
    use crate::network::server_packets::sm_ids;
    let (mut world, _tx, _db, _l) = test_world();
    let mut rx = make_noble(&mut world, 1, 100);

    // Games not running yet.
    assert!(!crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::THE_OLYMPIAD_GAMES_ARE_NOT_CURRENTLY_IN_PROGRESS
    ));

    open_games(&mut world);

    // Under level 55 → does-not-meet-conditions.
    world
        .objects
        .get_component_mut::<Player>(&100)
        .unwrap()
        .level = 54;
    assert!(!crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::CHARACTER_C1_DOES_NOT_MEET_THE_OLYMPIAD_CONDITIONS
    ));
    world
        .objects
        .get_component_mut::<Player>(&100)
        .unwrap()
        .level = 55;

    // Registration window closed (<20 min to comp end).
    world.olympiad.comp_end_tick = world.tick + 10; // 1 s left
    assert!(!crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::PARTICIPATION_REQUESTS_ARE_NO_LONGER_BEING_ACCEPTED
    ));
    open_games(&mut world);

    // Weekly cap reached.
    crate::game_loop::olympiad::register(&mut world, 100, CompetitionType::NonClassed);
    let _ = drain(&mut rx);
    world.olympiad.remove_registration(100);
    world.olympiad.nobles.get_mut(&100).unwrap().comp_done_week = 30;
    assert!(!crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::THE_MAXIMUM_MATCHES_YOU_CAN_PARTICIPATE_IN_1_WEEK_IS_30
    ));
}

#[test]
fn double_registration_is_rejected() {
    use crate::network::server_packets::sm_ids;
    let (mut world, _tx, _db, _l) = test_world();
    open_games(&mut world);
    let mut rx = make_noble(&mut world, 1, 100);

    assert!(crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    let _ = drain(&mut rx);
    assert!(!crate::game_loop::olympiad::register(
        &mut world,
        100,
        CompetitionType::NonClassed
    ));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::C1_IS_ALREADY_REGISTERED_ON_THE_WAITING_LIST_FOR_THE_ALL_CLASS_BATTLE
    ));
}

#[test]
fn noble_unregisters_from_the_list() {
    use crate::network::server_packets::sm_ids;
    let (mut world, _tx, _db, _l) = test_world();
    open_games(&mut world);
    let mut rx = make_noble(&mut world, 1, 100);

    // Nothing to leave yet.
    assert!(!crate::game_loop::olympiad::unregister(&mut world, 100));
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::YOU_ARE_NOT_CURRENTLY_REGISTERED_FOR_THE_OLYMPIAD
    ));

    crate::game_loop::olympiad::register(&mut world, 100, CompetitionType::NonClassed);
    let _ = drain(&mut rx);
    assert!(crate::game_loop::olympiad::unregister(&mut world, 100));
    assert!(!world.olympiad.is_registered(100), "left the queue");
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::YOU_HAVE_BEEN_REMOVED_FROM_THE_OLYMPIAD_WAITING_LIST
    ));
}
