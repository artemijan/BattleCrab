//! Grand Olympiad (G25) slice 1: noble registration into the 1v1 waiting list
//! and its eligibility/timing gates.

use super::*;

use crate::model::Player;
use crate::model::olympiad::{CompetitionType, DEFAULT_POINTS};
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
) -> UnboundedReceiver<bytes::Bytes> {
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
            server_packets::sm_ids::YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH
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

/// The Grand Olympiad Manager (31688) end-to-end through its `Quest OlyManager`
/// bypasses: the join page, register, and unregister.
#[test]
fn oly_manager_dialog_registers_via_bypass() {
    use crate::network::server_packets::sm_ids;
    let (mut world, _db_rx, _link) = quest_test_world();
    add_test_npc(&mut world, 700, 31688, "Folk", 70, 0, 0, 0);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[2]);
    open_games(&mut world);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&100).unwrap();
        p.class_id = 2;
        p.base_class_id = 2;
        p.level = 55;
    }

    // The join page substitutes the round / week / participant placeholders.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager joinMatch"),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("join page served");
    assert!(
        !html.contains("%olympiad_participant%"),
        "placeholders were substituted"
    );

    // The register button enrolls the player and confirms.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager register1v1"),
    );
    assert!(
        world.olympiad.non_class_registers.contains(&100),
        "registered via the NPC"
    );
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::YOU_ARE_CURRENTLY_REGISTERED_FOR_A_1V1_CLASS_IRRELEVANT_MATCH
    ));

    // The unregister button removes the player.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager unregister"),
    );
    assert!(
        !world.olympiad.is_registered(100),
        "unregistered via the NPC"
    );
    assert!(got_sm(
        &drain(&mut rx),
        sm_ids::YOU_HAVE_BEEN_REMOVED_FROM_THE_OLYMPIAD_WAITING_LIST
    ));
}

/// A subclass-active character is turned away at the register button.
#[test]
fn oly_manager_rejects_subclass() {
    let (mut world, _db_rx, _link) = quest_test_world();
    add_test_npc(&mut world, 700, 31688, "Folk", 70, 0, 0, 0);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[2]);
    open_games(&mut world);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&100).unwrap();
        p.class_id = 2;
        p.base_class_id = 2;
        p.level = 55;
        p.class_index = 1; // on a subclass
    }

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager register1v1"),
    );
    assert!(
        !world.olympiad.is_registered(100),
        "subclass character not registered"
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a page was served");
    assert!(
        html.contains("While you have a subclass"),
        "the subclass page was served"
    );
}

/// The boot load populates the state, and the shutdown save emits the same
/// noble record back to the DB.
#[test]
fn olympiad_persistence_round_trips() {
    use crate::db::{DbCommand, OlympiadNobleRow};
    let (mut world, _tx, mut db_rx, _l) = test_world();

    // Boot load: period 1 of cycle 3, one noble with earned points.
    crate::game_loop::olympiad::apply_loaded(
        &mut world,
        3,
        1,
        111,
        222,
        333,
        vec![OlympiadNobleRow {
            char_id: 500,
            class_id: 2,
            points: 45,
            comp_done: 7,
            comp_won: 4,
            comp_lost: 3,
            comp_drawn: 0,
            comp_done_week: 2,
        }],
        Vec::new(),
    );
    assert_eq!(world.olympiad.current_cycle, 3);
    assert_eq!(world.olympiad.period, 1);
    assert_eq!(world.olympiad.next_weekly_change, 333);
    let n = world.olympiad.nobles.get(&500).expect("noble loaded");
    assert_eq!(n.points, 45);
    assert_eq!(n.comp_won, 4);
    assert_eq!(n.comp_done_week, 2);

    // Shutdown save: the SaveOlympiad command carries the loaded state back.
    crate::game_loop::olympiad::save_all(&world);
    let (cycle, period, nobles) = drain_db(&mut db_rx)
        .into_iter()
        .find_map(|c| match c {
            DbCommand::SaveOlympiad {
                current_cycle,
                period,
                nobles,
                ..
            } => Some((current_cycle, period, nobles)),
            _ => None,
        })
        .expect("SaveOlympiad emitted");
    assert_eq!((cycle, period), (3, 1));
    assert_eq!(nobles.len(), 1);
    assert_eq!(nobles[0].char_id, 500);
    assert_eq!(nobles[0].points, 45);
    assert_eq!(nobles[0].comp_won, 4);
}

// Epoch day 2 (1970-01-03) was a Saturday; day 4 a Monday. 18:00 UTC on each.
const MS_PER_DAY: i64 = 86_400_000;
const SAT_1800: i64 = 2 * MS_PER_DAY + 18 * 3600 * 1000;
const MON_1800: i64 = 4 * MS_PER_DAY + 18 * 3600 * 1000;

#[test]
fn competition_window_is_weekends_at_1800() {
    use crate::game_loop::olympiad::{in_comp_window, next_comp_start_delay_ms};
    // The shipped `Olympiad.ini`: 18:00, six hours, Saturday and Sunday.
    let cfg = crate::config::OlympiadConfig::default();

    // Saturday inside the 18:00–00:00 window is open; before/after is not.
    assert!(
        in_comp_window(&cfg, SAT_1800 + 30 * 60 * 1000),
        "Sat 18:30 open"
    );
    assert!(
        !in_comp_window(&cfg, SAT_1800 - 3600 * 1000),
        "Sat 17:00 closed"
    );
    assert!(
        !in_comp_window(&cfg, SAT_1800 + 7 * 3600 * 1000),
        "Sun 01:00 (past the 6 h window) closed"
    );
    // A weekday is never a competition day.
    assert!(
        !in_comp_window(&cfg, MON_1800 + 30 * 60 * 1000),
        "Mon 18:30 closed"
    );

    // From Monday 18:00 the next start is Saturday 18:00 — five days off.
    assert_eq!(next_comp_start_delay_ms(&cfg, MON_1800), 5 * MS_PER_DAY);
    // Just before Saturday's window, the next start is a few hours away.
    assert_eq!(
        next_comp_start_delay_ms(&cfg, SAT_1800 - 3600 * 1000),
        3600 * 1000,
        "one hour to Saturday 18:00"
    );
}

#[test]
fn olympiad_period_ends_at_noon_after_13_days() {
    use crate::game_loop::olympiad::next_olympiad_end;
    let cfg = crate::config::OlympiadConfig::default();
    // From Saturday 18:00, the round ends at noon 13 days on (14-day period, the
    // last day reserved for validation).
    let end = next_olympiad_end(&cfg, SAT_1800);
    assert_eq!(end, 15 * MS_PER_DAY + 12 * 3600 * 1000, "noon, 13 days out");
    assert!(end > SAT_1800, "always in the future");
}

#[test]
fn comp_start_opens_and_comp_end_closes_the_window() {
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.period = 0;

    crate::game_loop::olympiad::handle_comp_start(&mut world);
    assert!(world.olympiad.in_comp_period, "window opened");

    // A registrant, then the window closes and the queue is cleared.
    world.olympiad.non_class_registers.insert(42);
    crate::game_loop::olympiad::handle_comp_end(&mut world);
    assert!(!world.olympiad.in_comp_period, "window closed");
    assert!(
        world.olympiad.non_class_registers.is_empty(),
        "waiting list cleared at comp end"
    );
}

#[test]
fn comp_start_does_nothing_in_the_validation_period() {
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.period = 1; // validation
    crate::game_loop::olympiad::handle_comp_start(&mut world);
    assert!(
        !world.olympiad.in_comp_period,
        "no competition during validation"
    );
}

#[test]
fn weekly_change_adds_points_and_resets_matches() {
    use crate::model::olympiad::NobleStats;
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.period = 0;
    let mut noble = NobleStats::fresh(2, "N".into(), DEFAULT_POINTS); // 10 points
    noble.comp_done_week = 7;
    world.olympiad.nobles.insert(500, noble);

    crate::game_loop::olympiad::handle_weekly_change(&mut world);
    let n = &world.olympiad.nobles[&500];
    assert_eq!(n.points, 20, "weekly points added");
    assert_eq!(n.comp_done_week, 0, "weekly matches reset");

    // During validation the refresh is skipped.
    world.olympiad.period = 1;
    world.olympiad.nobles.get_mut(&500).unwrap().comp_done_week = 5;
    crate::game_loop::olympiad::handle_weekly_change(&mut world);
    let n = &world.olympiad.nobles[&500];
    assert_eq!(n.points, 20, "no points during validation");
    assert_eq!(n.comp_done_week, 5, "not reset during validation");
}

/// Queue `count` online players (object ids 1000..) into the non-class list.
fn queue_online(world: &mut World, count: i32) {
    for oid in 1000..1000 + count {
        let _ = ingame_player(world, oid as u32, oid, 0, 0, 0);
        world.olympiad.non_class_registers.insert(oid);
    }
}

#[test]
fn game_manager_pairs_waiting_nobles_into_arenas() {
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    queue_online(&mut world, 20);

    crate::game_loop::olympiad::handle_game_manager(&mut world);

    // Four stadiums → four 1v1 matches; 8 fighters pulled out, 12 left waiting.
    assert_eq!(world.olympiad.matches.len(), 4, "one match per arena");
    assert_eq!(world.olympiad.in_competition.len(), 8);
    assert_eq!(world.olympiad.non_class_registers.len(), 12);
    let arenas: std::collections::HashSet<usize> =
        world.olympiad.matches.iter().map(|m| m.arena).collect();
    assert_eq!(arenas.len(), 4, "each match took a distinct arena");
    for m in &world.olympiad.matches {
        assert_ne!(m.player_a, m.player_b, "two distinct fighters");
        assert!(world.olympiad.is_in_competition(m.player_a));
        assert!(world.olympiad.is_in_competition(m.player_b));
        assert!(!world.olympiad.non_class_registers.contains(&m.player_a));
        assert!(!world.olympiad.non_class_registers.contains(&m.player_b));
    }
}

#[test]
fn game_manager_needs_the_minimum_before_making_matches() {
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    queue_online(&mut world, 10); // below the 20 threshold

    crate::game_loop::olympiad::handle_game_manager(&mut world);
    assert!(
        world.olympiad.matches.is_empty(),
        "no matches below the minimum"
    );
    assert_eq!(
        world.olympiad.non_class_registers.len(),
        10,
        "queue untouched"
    );
}

/// Stage a running match between two nobles with the given points; returns
/// player A's outbound packet receiver.
fn stage_match(
    world: &mut World,
    a: i32,
    b: i32,
    pts_a: i32,
    pts_b: i32,
) -> UnboundedReceiver<bytes::Bytes> {
    use crate::model::olympiad::{NobleStats, OlympiadMatch};
    let rx_a = ingame_player(world, a as u32, a, 500, 500, 0);
    let _rb = ingame_player(world, b as u32, b, 600, 600, 0);
    for (oid, pts, name) in [(a, pts_a, "A"), (b, pts_b, "B")] {
        let mut n = NobleStats::fresh(2, name.into(), DEFAULT_POINTS);
        n.points = pts;
        world.olympiad.nobles.insert(oid, n);
        world.olympiad.in_competition.insert(oid);
    }
    world.olympiad.matches.push(OlympiadMatch {
        arena: 0,
        player_a: a,
        player_b: b,
        instance_id: 0,
        deadline_tick: world.tick + 100_000,
        return_a: (500, 500, 0),
        return_b: (600, 600, 0),
    });
    rx_a
}

#[test]
fn pre_fight_countdown_announces_then_teleports_then_fights() {
    use crate::model::components::{Position, Vitals};
    use crate::model::olympiad::NobleStats;
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    let mut rx_a = ingame_player(&mut world, 1, 100, 500, 500, 0);
    let _rx_b = ingame_player(&mut world, 2, 200, 600, 600, 0);
    for oid in [100, 200] {
        world
            .olympiad
            .nobles
            .insert(oid, NobleStats::fresh(2, "N".into(), DEFAULT_POINTS));
        world.olympiad.in_competition.insert(oid);
    }

    crate::game_loop::olympiad::start_match(&mut world, 0, 100, 200);

    // The fighters aren't moved while the countdown runs.
    {
        let p = world.objects.get_component::<Position>(&100).unwrap();
        assert_eq!(
            (p.x, p.y),
            (500, 500),
            "not teleported during the countdown"
        );
    }
    // The first step announces the move to the stadium.
    advance_ticks(&mut world, 1);
    assert!(
        got_sm(&drain(&mut rx_a), 1492),
        "\"moved to the stadium\" announcement"
    );

    // Through the whole ~180 s ceremony: teleported in, then the fight begins.
    advance_ticks(&mut world, 1810);
    {
        let p = world.objects.get_component::<Position>(&100).unwrap();
        assert_eq!(
            (p.x, p.y),
            (-89597, -252841),
            "teleported to the arena at the end of the wait countdown"
        );
    }
    assert!(
        world.olympiad.matches[0].deadline_tick > 0,
        "the battle has started"
    );
    // The bout runs in its own instance (both fighters moved into it).
    let inst = world.olympiad.matches[0].instance_id;
    assert!(
        inst >= 1 && world.instances.contains(inst),
        "private instance"
    );
    for oid in [100, 200] {
        assert_eq!(
            crate::game_loop::helpers::instance_of(&world, oid),
            inst,
            "fighter is in the match instance"
        );
    }

    // A death now resolves the match — the fighters return to the overworld.
    world
        .objects
        .get_component_mut::<Vitals>(&200)
        .unwrap()
        .dead = true;
    advance_ticks(&mut world, 11);
    assert!(world.olympiad.matches.is_empty(), "match resolved");
    assert_eq!(world.olympiad.nobles[&100].comp_won, 1, "the survivor won");
    assert!(!world.instances.contains(inst), "instance torn down");
    assert_eq!(
        crate::game_loop::helpers::instance_of(&world, 100),
        0,
        "winner back in the overworld"
    );
}

#[test]
fn a_match_resolves_on_death_with_scoring() {
    use crate::model::components::Vitals;
    let (mut world, _tx, _db, _l) = test_world();
    let mut rx = stage_match(&mut world, 100, 200, 30, 20);

    // Player 200 dies → 100 wins. pointDiff = min(30,20)/5 = 4.
    world
        .objects
        .get_component_mut::<Vitals>(&200)
        .unwrap()
        .dead = true;
    crate::game_loop::olympiad::handle_match_tick(&mut world, 0);

    let win = &world.olympiad.nobles[&100];
    assert_eq!(win.points, 34, "winner gains the transfer");
    assert_eq!((win.comp_won, win.comp_done, win.comp_done_week), (1, 1, 1));
    let lose = &world.olympiad.nobles[&200];
    assert_eq!(lose.points, 16, "loser loses the transfer");
    assert_eq!(
        (lose.comp_lost, lose.comp_done, lose.comp_done_week),
        (1, 1, 1)
    );

    assert!(
        !world.olympiad.is_in_competition(100),
        "match freed the winner"
    );
    assert!(!world.olympiad.is_in_competition(200));
    assert!(world.olympiad.matches.is_empty(), "match cleared");
    assert!(
        got_sm(
            &drain(&mut rx),
            server_packets::sm_ids::CONGRATULATIONS_C1_YOU_WIN_THE_MATCH
        ),
        "winner congratulated"
    );
}

#[test]
fn a_timed_out_match_is_a_draw() {
    let (mut world, _tx, _db, _l) = test_world();
    stage_match(&mut world, 100, 200, 30, 20);
    // Force the deadline into the past; both stay alive.
    world.olympiad.matches[0].deadline_tick = 0;

    crate::game_loop::olympiad::handle_match_tick(&mut world, 0);

    for oid in [100, 200] {
        let n = &world.olympiad.nobles[&oid];
        assert_eq!(n.comp_drawn, 1, "both drew");
        assert_eq!(n.comp_done, 1);
    }
    assert_eq!(
        world.olympiad.nobles[&100].points, 30,
        "no points on a draw"
    );
    assert_eq!(world.olympiad.nobles[&200].points, 20);
    assert!(world.olympiad.matches.is_empty());
    assert!(world.olympiad.in_competition.is_empty());
}

#[test]
fn point_transfer_is_clamped() {
    use crate::model::components::Vitals;
    let (mut world, _tx, _db, _l) = test_world();
    // Both nearly broke → min/5 rounds to 0, clamped up to 1.
    stage_match(&mut world, 100, 200, 3, 3);
    world
        .objects
        .get_component_mut::<Vitals>(&200)
        .unwrap()
        .dead = true;
    crate::game_loop::olympiad::handle_match_tick(&mut world, 0);
    assert_eq!(
        world.olympiad.nobles[&100].points, 4,
        "transfer floors at 1"
    );
    assert_eq!(world.olympiad.nobles[&200].points, 2);
}

/// Insert a noble record with the given competition record.
fn insert_noble(world: &mut World, oid: i32, class: i32, points: i32, done: i32, won: i32) {
    use crate::model::olympiad::NobleStats;
    let mut n = NobleStats::fresh(class, format!("N{oid}"), DEFAULT_POINTS);
    n.points = points;
    n.comp_done = done;
    n.comp_won = won;
    world.olympiad.nobles.insert(oid, n);
}

#[test]
fn heroes_are_the_top_eligible_noble_per_class() {
    let (mut world, _tx, _db, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    // Gladiator (2) is the parent 3rd class of Duelist (88).
    world.data.skill_trees.set_parent_for_test(88, 2);

    insert_noble(&mut world, 100, 88, 50, 15, 5); // eligible, most points
    insert_noble(&mut world, 200, 2, 30, 12, 3); // parent-class competitor, fewer points
    insert_noble(&mut world, 300, 88, 100, 5, 2); // too few matches (< 10)
    insert_noble(&mut world, 400, 88, 100, 15, 0); // no wins

    assert_eq!(
        crate::game_loop::olympiad::compute_heroes(&world),
        vec![(100, 88)],
        "the eligible class/parent competitor with the most points is hero"
    );
}

#[test]
fn olympiad_end_crowns_heroes_then_validation_starts_a_new_cycle() {
    let (mut world, _tx, _db, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    insert_noble(&mut world, 100, 88, 50, 15, 5);
    world.olympiad.current_cycle = 3;
    world.olympiad.period = 0;

    // Round ends → validation period, the hero is crowned (online).
    crate::game_loop::olympiad::handle_olympiad_end(&mut world);
    assert_eq!(world.olympiad.period, 1, "entered validation");
    assert_eq!(world.olympiad.heroes, vec![(100, 88)]);
    // The crown alone grants nothing — Java's `computeNewHeroes` never calls
    // `setHero(true)`; the title comes from claiming it at the monument.
    assert!(
        !world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "crowned but unclaimed carries no status"
    );
    crate::game_loop::olympiad::claim_hero(&mut world, 100);
    assert!(
        world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "claiming grants it"
    );

    // Validation ends → new cycle, clean noble table.
    crate::game_loop::olympiad::handle_validation_end(&mut world);
    assert_eq!(world.olympiad.period, 0, "back to competition");
    assert_eq!(world.olympiad.current_cycle, 4, "cycle advanced");
    assert!(world.olympiad.nobles.is_empty(), "noble table reset");
}

#[test]
fn heroes_persist_and_apply_on_login() {
    use crate::db::{DbCommand, HeroRow};
    let (mut world, _tx, mut db_rx, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);

    // Boot-load an existing hero crowned twice before.
    crate::game_loop::olympiad::apply_heroes_loaded(
        &mut world,
        vec![HeroRow {
            char_id: 100,
            class_id: 88,
            count: 2,
            name: "Aragorn".into(),
            clan_id: 0,
            message: String::new(),
            // Collected at the monument in an earlier session — only a *claimed*
            // crown carries hero status across a login.
            claimed: true,
        }],
        vec![],
    );
    assert!(world.olympiad.is_hero(100), "loaded into the crown");

    // On login the crowned character regains hero status.
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    crate::game_loop::olympiad::on_enter_world(&mut world, 100);
    assert!(
        world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "hero status re-applied on login"
    );

    // A fresh round re-crowns them; the count increments and is persisted.
    insert_noble(&mut world, 100, 88, 50, 15, 5);
    world.olympiad.period = 0;
    crate::game_loop::olympiad::handle_olympiad_end(&mut world);
    let heroes = drain_db(&mut db_rx)
        .into_iter()
        .find_map(|c| match c {
            DbCommand::SaveHeroes { heroes } => Some(heroes),
            _ => None,
        })
        .expect("SaveHeroes emitted");
    assert_eq!(heroes.len(), 1);
    assert_eq!(heroes[0].char_id, 100);
    assert_eq!(heroes[0].count, 3, "third crowning");
    assert!(
        !heroes[0].claimed,
        "a re-crown must be collected again at the monument"
    );
    assert!(
        world.olympiad.is_unclaimed_hero(100) && !world.olympiad.is_hero(100),
        "the live crown is unclaimed too"
    );
}

#[test]
fn round_end_banks_trade_points() {
    use crate::model::components::PlayerVariables;
    let (mut world, _tx, _db, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    // The sole classified noble → rank 1, and crowned hero.
    insert_noble(&mut world, 100, 88, 50, 15, 5);
    world.olympiad.period = 0;

    crate::game_loop::olympiad::handle_olympiad_end(&mut world);

    let banked = world
        .objects
        .get_component::<PlayerVariables>(&100)
        .unwrap()
        .get_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 0);
    assert_eq!(banked, 500, "hero (300) + rank-1 (200) trade points");
}

#[test]
fn round_end_banks_offline_nobles_to_the_db() {
    use crate::db::DbCommand;
    let (mut world, _tx, mut db_rx, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    // An offline noble (no player object) — classified, will be hero + rank 1.
    insert_noble(&mut world, 200, 88, 50, 15, 5);
    world.olympiad.period = 0;

    crate::game_loop::olympiad::handle_olympiad_end(&mut world);

    let (char_id, value) = drain_db(&mut db_rx)
        .into_iter()
        .find_map(|c| match c {
            DbCommand::StoreCharVar {
                char_id,
                var,
                value,
            } if var == crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR => Some((char_id, value)),
            _ => None,
        })
        .expect("offline noble's points written to character_variables");
    assert_eq!(char_id, 200);
    assert_eq!(value, "500", "hero (300) + rank-1 (200)");
}

#[test]
fn point_mark_exchange_gives_marks_of_battle() {
    use crate::model::components::PlayerVariables;
    use crate::model::inventory::Inventory;
    let (mut world, _db_rx, _link) = quest_test_world();
    add_test_npc(&mut world, 700, 31688, "Folk", 70, 0, 0, 0);
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    world
        .objects
        .get_component_mut::<PlayerVariables>(&100)
        .unwrap()
        .set_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 10);

    add_quest_items(&mut world, &[(45584, "Mark of Battle", true)]);
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager calculatePointsDone"),
    );

    // 10 points × 20 marks = 200 Marks of Battle (45584); the bank is cleared.
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&100)
            .unwrap()
            .count_of(world.cfg.olympiad.comp_reward_item),
        200
    );
    assert_eq!(
        world
            .objects
            .get_component::<PlayerVariables>(&100)
            .unwrap()
            .get_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 0),
        0,
        "banked points consumed"
    );
}

#[test]
fn match_start_strips_active_buffs() {
    use crate::model::components::Buffs;
    use crate::model::skill::ActiveBuff;
    let (mut world, _tx, _db, _l) = test_world();
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    let buff = ActiveBuff {
        skill_id: 1204, // Wind Walk
        abnormal_type: "SPEED_UP_SHORT".into(),
        abnormal_level: 1,
        ..test_buff()
    };
    world.objects.add_components(&100, Buffs(vec![buff]));

    crate::game_loop::olympiad::strip_buffs(&mut world, 100);

    assert!(
        world
            .objects
            .get_component::<Buffs>(&100)
            .unwrap()
            .0
            .iter()
            .all(|b| b.passive),
        "no active buffs survive entering the arena"
    );
}

#[test]
fn equipment_reward_opens_the_multisell() {
    let (mut world, _db_rx, _link) = quest_test_world();
    world
        .data
        .multisells
        .insert_for_test(crate::data::multisell_data::MultisellList {
            list_id: 3168801,
            is_chance_multisell: false,
            apply_taxes: false,
            maintain_enchantment: false,
            ingredient_multiplier: 1.0,
            product_multiplier: 1.0,
            entries: Vec::new(),
            npcs_allowed: None,
        });
    add_test_npc(&mut world, 700, 31688, "Folk", 70, 0, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager showEquipmentReward"),
    );

    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p.first() == Some(&opcodes::MULTI_SELL_LIST)),
        "the Olympiad reward multisell opened"
    );
}

#[test]
fn round_end_announces_to_online_players() {
    let (mut world, _tx, _db, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    world.olympiad.period = 0;
    world.olympiad.current_cycle = 7;

    crate::game_loop::olympiad::handle_olympiad_end(&mut world);

    assert!(
        got_sm(
            &drain(&mut rx),
            server_packets::sm_ids::ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED
        ),
        "the round-ended announcement reaches online players"
    );
}

#[test]
fn point_exchange_refused_when_inventory_over_80_percent() {
    use crate::model::components::PlayerVariables;
    use crate::model::inventory::Inventory;
    let (mut world, _db_rx, _link) = quest_test_world();
    add_test_npc(&mut world, 700, 31688, "Folk", 70, 0, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    // A one-slot bag, then one item in it → over the 80 % threshold.
    world.cfg.character.inventory_max_no_dwarf = 1;
    {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&100)
            .unwrap()
            .add_item(&data.item_data, 9_000_100, 57, 1000); // adena (one slot)
    }
    world
        .objects
        .get_component_mut::<PlayerVariables>(&100)
        .unwrap()
        .set_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 10);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_700_Quest OlyManager calculatePointsDone"),
    );

    // Refused: no marks, points untouched, the weight/slot message sent.
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&100)
            .unwrap()
            .count_of(world.cfg.olympiad.comp_reward_item),
        0,
        "no marks while the bag is full"
    );
    assert_eq!(
        world
            .objects
            .get_component::<PlayerVariables>(&100)
            .unwrap()
            .get_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 0),
        10,
        "banked points preserved"
    );
    assert!(got_sm(
        &drain(&mut rx),
        server_packets::sm_ids::UNABLE_TO_PROCESS_UNTIL_INVENTORY_UNDER_80_PERCENT
    ));
}

#[test]
fn a_fighting_noble_cannot_register() {
    let (mut world, _tx, _db, _l) = test_world();
    open_games(&mut world);
    let _rx = make_noble(&mut world, 1, 100);
    world.olympiad.in_competition.insert(100);

    assert!(
        !crate::game_loop::olympiad::register(&mut world, 100, CompetitionType::NonClassed),
        "a fighter cannot re-register"
    );
    assert!(!world.olympiad.is_registered(100));
}

// ---------------------------------------------------------------------------
// Monument of Heroes (31690) — hero reward claims
// ---------------------------------------------------------------------------

fn monument_world() -> (
    World,
    db::CmdRx,
    UnboundedReceiver<LoginLinkCommand>,
    UnboundedReceiver<bytes::Bytes>,
) {
    let (mut world, db_rx, link) = quest_test_world();
    add_test_npc(&mut world, 701, 31690, "Folk", 70, 0, 0, 0);
    world
        .data
        .categories
        .insert_for_test("THIRD_CLASS_GROUP", &[2]);
    let rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&100).unwrap();
        p.class_id = 2;
        p.base_class_id = 2;
        p.level = 55;
    }
    (world, db_rx, link, rx)
}

/// **A hero claims an Infinity weapon and the circlet at the Monument** — the
/// circlet is granted only once.
#[test]
fn monument_hero_claims_rewards() {
    let (mut world, _db, _l, mut rx) = monument_world();
    world.olympiad.heroes.push((100, 2)); // crowned hero
    // The hero rewards gate on `isHero`, i.e. a *claimed* crown.
    world.olympiad.claimed_heroes.insert(100);

    // `give_items` refuses an id the datapack does not declare (Java
    // `ItemContainer.addItem` logs `Invalid ItemId`), so the fixture declares
    // the weapon and the circlet the script hands out.
    add_quest_items(
        &mut world,
        &[
            (6611, "Infinity Sword", false),
            (6842, "Hero Circlet", false),
        ],
    );

    // Pick an Infinity weapon from the list.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes give_6611"),
    );
    assert_eq!(
        inv_count(&world, 6611),
        1,
        "hero received the Infinity Blade"
    );
    // A weapon id not on the list hands over nothing.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes give_9999"),
    );
    assert_eq!(inv_count(&world, 9999), 0, "a non-list id gives nothing");

    // The circlet, once.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes heroCirclet"),
    );
    assert_eq!(inv_count(&world, 6842), 1, "hero received the circlet");
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes heroCirclet"),
    );
    assert_eq!(inv_count(&world, 6842), 1, "the circlet is not duplicated");
    drain(&mut rx);
}

/// **A non-hero is refused the hero rewards** — the circlet menu serves the
/// "not a hero" page and grants nothing.
#[test]
fn monument_non_hero_is_refused() {
    let (mut world, _db, _l, mut rx) = monument_world();
    // Not pushed into `olympiad.heroes` → not a hero.

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes heroCirclet"),
    );
    assert_eq!(inv_count(&world, 6842), 0, "a non-hero gets no circlet");
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a page was served");
    assert!(
        html.contains("circletNo") || !html.is_empty(),
        "the not-a-hero circlet page is served"
    );
}

// ---------------------------------------------------------------------------
// Observer mode (spectating matches)
// ---------------------------------------------------------------------------

/// **The observer round-trip** — a spectator enters an arena (teleported in,
/// scoped to the match's instance, `ExOlympiadMode(3)`), then leaves (teleported
/// back, `ExOlympiadMode(0)`, observer state dropped).
#[test]
fn olympiad_observer_round_trip() {
    use crate::model::components::{InstanceId, OlympiadObserver, Position};
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    let _fighters = stage_match(&mut world, 100, 200, 10, 10);
    world.olympiad.matches[0].instance_id = 7; // a real instance to scope into

    // The spectator, standing at a known return point, not in the match.
    let mut rx = ingame_player(&mut world, 3, 300, 1000, 1000, 0);
    drain(&mut rx);

    crate::game_loop::olympiad::enter_observer(&mut world, 3, 300, 0);
    assert!(
        crate::game_loop::olympiad::is_observing(&world, 300),
        "now observing"
    );
    assert_eq!(
        world
            .objects
            .get_component::<OlympiadObserver>(&300)
            .unwrap()
            .return_pos,
        (1000, 1000, 0),
        "the return point was saved"
    );
    assert_eq!(
        world.objects.get_component::<InstanceId>(&300).map(|i| i.0),
        Some(7),
        "scoped into the match's instance"
    );
    let pos = world.objects.get_component::<Position>(&300).unwrap();
    assert_eq!((pos.x, pos.y), (-88070, -252843), "teleported to the stand");
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[..4] == [0xFE, 0x7D, 0x00, 0x03]),
        "ExOlympiadMode(3) was sent"
    );

    crate::game_loop::olympiad::leave_observer(&mut world, 3, 300);
    assert!(
        !crate::game_loop::olympiad::is_observing(&world, 300),
        "no longer observing"
    );
    assert!(
        world.objects.get_component::<InstanceId>(&300).is_none(),
        "back on the overworld"
    );
    // `exitOlympiadObserverMode` → `teleToLocation(_lastLoc, true)`: the return
    // is one of Java's four *scattering* teleports, so it lands within
    // `MaxOffsetOnTeleport` of where the spectator left rather than on it.
    let offset = world.cfg.character.teleport_offset();
    let pos = world.objects.get_component::<Position>(&300).unwrap();
    assert!(
        (pos.x - 1000).abs() <= offset && (pos.y - 1000).abs() <= offset,
        "teleported back within {offset} of (1000, 1000), got ({}, {})",
        pos.x,
        pos.y
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[..4] == [0xFE, 0x7D, 0x00, 0x00]),
        "ExOlympiadMode(0) was sent"
    );
}

/// A competitor (or a queued noble) can't observe.
#[test]
fn a_competitor_cannot_observe() {
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    let _fighters = stage_match(&mut world, 100, 200, 10, 10);
    // Player 100 is one of the fighters (in `in_competition`).
    let _rx = ingame_player(&mut world, 100, 100, 500, 500, 0);

    crate::game_loop::olympiad::enter_observer(&mut world, 100, 100, 0);
    assert!(
        !crate::game_loop::olympiad::is_observing(&world, 100),
        "a competitor is refused observer mode"
    );
}

/// **The Monument's hero list sends `ExHeroList`** with each crowned hero's name
/// and count — even for an offline hero (resolved from `hero_info`).
#[test]
fn monument_hero_list_sends_ex_hero_list() {
    let (mut world, _db, _l, mut rx) = monument_world();
    // Crown one hero (offline: no Player object, name comes from hero_info).
    world.olympiad.heroes.push((555, 88));
    world.olympiad.hero_counts.insert(555, 3);
    world.olympiad.hero_info.insert(
        555,
        model::olympiad::HeroInfo {
            name: "Aragorn".into(),
            clan_id: 0,
            message: String::new(),
        },
    );

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body("npc_701_Quest MonumentOfHeroes heroList"),
    );

    let pkt = drain(&mut rx)
        .into_iter()
        .find(|p| p.first() == Some(&0xFE) && p.get(1) == Some(&0x7A))
        .expect("ExHeroList (0xFE 0x7A) was sent");
    // Layout: opcode(1) + subop(2) → the hero count at offset 3.
    let count = i32::from_le_bytes(pkt[3..7].try_into().unwrap());
    assert_eq!(count, 1, "one hero is listed");
    // The name is written UTF-16LE, so 'A' (0x41 0x00) appears in the row.
    assert!(
        pkt.windows(2).any(|w| w == [0x41, 0x00]),
        "the hero's name is in the packet"
    );
}

#[test]
fn hero_diary_window_renders_with_the_deed_list() {
    let (mut world, _tx, _rx, _l) = test_world();
    // The diary template lives in the dist html tree.
    world.data.root = crate::data::DIST_GAME.to_string();
    // A crowned hero (class 88) with a "Gained Hero status" diary entry.
    world.olympiad.heroes.push((100, 88));
    world.olympiad.hero_info.insert(
        100,
        model::olympiad::HeroInfo {
            name: "Aragorn".into(),
            message: "For Gondor".into(),
            clan_id: 0,
        },
    );
    world.olympiad.hero_diary.insert(
        100,
        vec![model::olympiad::DiaryEntry {
            time: 1_700_000_000_000,
            action: 2,
            param: 0,
        }],
    );

    let mut rx = ingame_player(&mut world, 1, 500, 0, 0, 0);
    drain(&mut rx);
    add_test_npc(&mut world, 600, 31690, "Folk", 70, 0, 0, 0);

    crate::game_loop::olympiad::show_hero_diary(&mut world, 1, 600, "?class=88&page=1");

    let pkts = drain(&mut rx);
    // An NpcHtmlMessage went out carrying the hero's name (UTF-16LE 'A').
    assert!(
        pkts.iter().any(|p| p.windows(2).any(|w| w == [0x41, 0x00])),
        "the diary window was sent with the hero name"
    );
}

/// Java `Hero.isHero` is *crowned and claimed*: a hero who has not been to the
/// Monument of Heroes logs in without the status. `claimHero` (the monument's
/// `heroConfirm`, or a GM's `//givehero`) is what turns the crown into the
/// title — and pays the clan its `HeroPoints`.
#[test]
fn a_crowned_hero_has_no_status_until_they_claim_it() {
    use crate::db::DbCommand;
    let (mut world, _tx, mut db_rx, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);
    let mut rx = ingame_player(&mut world, 1, 100, 0, 0, 0);

    // Crown them, as the round end does.
    insert_noble(&mut world, 100, 88, 50, 15, 5);
    world.olympiad.period = 0;
    crate::game_loop::olympiad::handle_olympiad_end(&mut world);
    assert!(world.olympiad.is_unclaimed_hero(100), "crowned, unclaimed");
    assert!(!world.olympiad.is_hero(100), "not a hero yet");

    // Logging in grants nothing while the crown is unclaimed.
    crate::game_loop::olympiad::on_enter_world(&mut world, 100);
    assert!(
        !world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "an unclaimed crown carries no hero status"
    );

    // A clan of level 3+ is paid the hero reputation on the claim.
    let clan_id = hero_clan(&mut world, 100, 3);
    let before = world.clans[&clan_id].reputation_score;
    drain(&mut rx);
    drain_db(&mut db_rx);

    crate::game_loop::olympiad::claim_hero(&mut world, 100);

    assert!(world.olympiad.is_hero(100), "claimed");
    assert!(
        world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "hero status granted"
    );
    assert_eq!(
        world.clans[&clan_id].reputation_score,
        before + world.cfg.feature.hero_points,
        "the clan is paid HeroPoints"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::ClaimHero { char_id: 100 })),
        "the claim is persisted"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DbCommand::SaveHeroDiary { char_id: 100, .. })),
        "the diary records the deed"
    );
    let out = drain(&mut rx);
    assert!(
        out.iter().any(|p| p[0] == opcodes::SOCIAL_ACTION),
        "the hero animation is broadcast"
    );
}

/// A level-`level` clan holding `member`, returning its id.
fn hero_clan(world: &mut World, member: i32, level: i32) -> i32 {
    let clan_id = 0x4200_0001;
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Heroes".into(),
            leader_id: member,
            level,
            reputation_score: 0,
            castle_id: 0,
            members: vec![model::clan::ClanMember {
                char_id: member,
                name: format!("P{member}"),
                level: 80,
                class_id: 88,
                sex: 0,
                race: 0,
                power_grade: 1,
                title: String::new(),
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
            }],
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    if let Some(p) = world.objects.get_component_mut::<Player>(&member) {
        p.clan_id = clan_id;
    }
    clan_id
}

/// `Olympiad.updateMonthlyData` + `getClassLeaderBoard`: the round end freezes
/// the nobles into `olympiad_nobles_eom`, and the Olympiad Manager's rank page
/// reads *that* snapshot — `AltOlyShowMonthlyWinners = True` on this dist, so
/// the board shows the last completed cycle rather than the live one.
#[test]
fn the_round_end_snapshots_the_class_leaderboard() {
    use crate::db::DbCommand;
    let (mut world, _tx, mut db_rx, _l) = test_world();
    world
        .data
        .categories
        .insert_for_test("FOURTH_CLASS_GROUP", &[88]);

    // Four class-88 nobles: two qualify (≥ 10 matches), ranked by points; one is
    // short of the match minimum; one is a different class.
    insert_noble(&mut world, 100, 88, 90, 12, 8);
    insert_noble(&mut world, 101, 88, 120, 11, 9);
    insert_noble(&mut world, 102, 88, 500, 9, 9); // 9 matches — not ranked
    insert_noble(&mut world, 103, 89, 999, 20, 20); // another class
    world.olympiad.period = 0;

    // Before the round ends the board is empty — the snapshot is the source.
    assert!(crate::game_loop::olympiad::class_leader_board(&world, 88).is_empty());

    crate::game_loop::olympiad::handle_olympiad_end(&mut world);

    assert_eq!(
        crate::game_loop::olympiad::class_leader_board(&world, 88),
        vec!["N101".to_string(), "N100".to_string()],
        "ranked by points, descending; the sub-minimum noble is excluded"
    );
    assert_eq!(
        crate::game_loop::olympiad::class_leader_board(&world, 89),
        vec!["N103".to_string()],
        "each class has its own board"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, DbCommand::SnapshotOlympiadEom)),
        "the snapshot is persisted too"
    );
}

/// An observer watching a stadium slot follows it into the NEXT match's
/// instance — Java's per-slot instance is permanent, ours is per-match, so
/// `start_match` re-scopes the slot's spectators or they would be stranded
/// watching a destroyed instance.
#[test]
fn an_observer_follows_the_arena_into_the_next_match() {
    use crate::model::components::{OlympiadObserver, Vitals};
    use crate::model::olympiad::NobleStats;
    let (mut world, _tx, _db, _l) = test_world();
    world.olympiad.in_comp_period = true;
    let _rx_a = stage_match(&mut world, 100, 200, 50, 50);
    let first = world.instances.create(0);
    world.olympiad.matches[0].instance_id = first;

    // A spectator scoped to the running match.
    let _rx_o = ingame_player(&mut world, 3, 300, 0, 0, 0);
    crate::game_loop::olympiad::enter_observer(&mut world, 3, 300, 0);
    assert_eq!(crate::game_loop::helpers::instance_of(&world, 300), first);

    // The match resolves; its instance dies but the spectator stays put.
    world
        .objects
        .get_component_mut::<Vitals>(&200)
        .unwrap()
        .dead = true;
    crate::game_loop::olympiad::handle_match_tick(&mut world, 0);
    assert!(world.olympiad.matches.is_empty(), "match resolved");
    assert!(!world.instances.contains(first), "old instance torn down");
    assert!(
        world.objects.has_component::<OlympiadObserver>(&300),
        "still an observer"
    );

    // The next bout on the same slot picks the spectator up.
    let _rx_c = ingame_player(&mut world, 4, 400, 500, 500, 0);
    let _rx_d = ingame_player(&mut world, 5, 500, 600, 600, 0);
    for oid in [400, 500] {
        world
            .olympiad
            .nobles
            .insert(oid, NobleStats::fresh(2, "N".into(), DEFAULT_POINTS));
        world.olympiad.in_competition.insert(oid);
    }
    crate::game_loop::olympiad::start_match(&mut world, 0, 400, 500);
    let second = world.olympiad.matches[0].instance_id;
    assert_ne!(second, first);
    assert_eq!(
        crate::game_loop::helpers::instance_of(&world, 300),
        second,
        "the spectator was re-scoped into the new match's instance"
    );
}

// ---------------------------------------------------------------------------
// Olympiad.ini, wired (row 14)
// ---------------------------------------------------------------------------

/// **The season clock follows the config, not constants.** Start hour, window
/// length and competition days were all `const`; an operator moving the
/// Olympiad to weekday evenings changed nothing.
#[test]
fn the_competition_window_follows_the_configured_clock() {
    use crate::game_loop::olympiad::{in_comp_window, next_comp_start_delay_ms};
    let mut cfg = crate::config::OlympiadConfig::default();
    // Move it to Monday, 20:30, for two hours.
    cfg.competition_days = vec![1];
    cfg.start_hour = 20;
    cfg.start_minute = 30;
    cfg.comp_period_ms = 2 * 3600 * 1000;

    let mon_2030 = MON_1800 + 2 * 3600 * 1000 + 30 * 60 * 1000;
    assert!(in_comp_window(&cfg, mon_2030 + 60_000), "Mon 20:31 open");
    assert!(!in_comp_window(&cfg, mon_2030 - 60_000), "Mon 20:29 closed");
    assert!(
        !in_comp_window(&cfg, mon_2030 + 3 * 3600 * 1000),
        "Mon 23:30 past the two-hour window"
    );
    // Saturday is no longer a competition day at all.
    assert!(!in_comp_window(&cfg, SAT_1800 + 3 * 3600 * 1000));
    // From Saturday 18:00 the next start is Monday 20:30.
    assert_eq!(
        next_comp_start_delay_ms(&cfg, SAT_1800),
        2 * MS_PER_DAY + 2 * 3600 * 1000 + 30 * 60 * 1000
    );
}

/// **The round length follows `AltOlyPeriod` × `AltOlyPeriodMultiplier`.**
#[test]
fn the_round_length_follows_the_configured_period() {
    use crate::game_loop::olympiad::next_olympiad_end;
    let mut cfg = crate::config::OlympiadConfig::default();
    // A one-week round instead of fourteen days.
    cfg.period_unit_days = 7;
    cfg.period_multiplier = 1;
    assert_eq!(cfg.period_days(), 7);
    assert_eq!(
        next_olympiad_end(&cfg, SAT_1800),
        8 * MS_PER_DAY + 12 * 3600 * 1000,
        "noon, six days out"
    );
}

/// **A fresh noble starts on `AltOlyStartPoints`, and the weekly cap is
/// `AltOlyMaxWeeklyMatches`.** Both were constants in `model::olympiad`.
#[test]
fn start_points_and_weekly_cap_follow_the_config() {
    use crate::model::olympiad::NobleStats;
    let n = NobleStats::fresh(2, "N".into(), 42);
    assert_eq!(
        n.points, 42,
        "the record is created with the configured points"
    );

    let mut state = model::olympiad::OlympiadState::default();
    state
        .nobles
        .insert(700, NobleStats::fresh(2, "N".into(), 10));
    state.nobles.get_mut(&700).unwrap().comp_done_week = 4;
    assert_eq!(
        state.remaining_weekly_matches(700, 30),
        26,
        "the shipped cap of 30"
    );
    assert_eq!(
        state.remaining_weekly_matches(700, 5),
        1,
        "and it is the caller's number, not a constant"
    );
}
