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

    // Saturday inside the 18:00–00:00 window is open; before/after is not.
    assert!(in_comp_window(SAT_1800 + 30 * 60 * 1000), "Sat 18:30 open");
    assert!(!in_comp_window(SAT_1800 - 3600 * 1000), "Sat 17:00 closed");
    assert!(
        !in_comp_window(SAT_1800 + 7 * 3600 * 1000),
        "Sun 01:00 (past the 6 h window) closed"
    );
    // A weekday is never a competition day.
    assert!(
        !in_comp_window(MON_1800 + 30 * 60 * 1000),
        "Mon 18:30 closed"
    );

    // From Monday 18:00 the next start is Saturday 18:00 — five days off.
    assert_eq!(next_comp_start_delay_ms(MON_1800), 5 * MS_PER_DAY);
    // Just before Saturday's window, the next start is a few hours away.
    assert_eq!(
        next_comp_start_delay_ms(SAT_1800 - 3600 * 1000),
        3600 * 1000,
        "one hour to Saturday 18:00"
    );
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
    let mut noble = NobleStats::fresh(2, "N".into()); // 10 points
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
) -> tokio::sync::mpsc::UnboundedReceiver<Vec<u8>> {
    use crate::model::olympiad::{NobleStats, OlympiadMatch};
    let rx_a = ingame_player(world, a as u32, a, 500, 500, 0);
    let _rb = ingame_player(world, b as u32, b, 600, 600, 0);
    for (oid, pts, name) in [(a, pts_a, "A"), (b, pts_b, "B")] {
        let mut n = NobleStats::fresh(2, name.into());
        n.points = pts;
        world.olympiad.nobles.insert(oid, n);
        world.olympiad.in_competition.insert(oid);
    }
    world.olympiad.matches.push(OlympiadMatch {
        arena: 0,
        player_a: a,
        player_b: b,
        deadline_tick: world.tick + 100_000,
        return_a: (500, 500, 0),
        return_b: (600, 600, 0),
    });
    rx_a
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
            crate::network::server_packets::sm_ids::CONGRATULATIONS_C1_YOU_WIN_THE_MATCH
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
    let mut n = NobleStats::fresh(class, format!("N{oid}"));
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
    assert!(
        world.objects.get_component::<Player>(&100).unwrap().is_hero,
        "the online hero is crowned"
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
        }],
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
            .count_of(crate::game_loop::olympiad::MARK_ITEM),
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
    use crate::model::skill::{ActiveBuff, BuffSlot};
    let (mut world, _tx, _db, _l) = test_world();
    let _rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    let buff = ActiveBuff {
        skill_id: 1204, // Wind Walk
        skill_level: 1,
        abnormal_type_client_id: 0,
        abnormal_type: "SPEED_UP_SHORT".into(),
        abnormal_level: 1,
        slot: BuffSlot::Buff,
        expires_at_tick: u64::MAX,
        passive: false,
        effect_flags: 0,
        abnormal_visuals: Vec::new(),
        blocked_abnormals: Vec::new(),
        effects: Vec::new(),
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
            crate::network::server_packets::sm_ids::ROUND_S1_OF_THE_OLYMPIAD_GAMES_HAS_NOW_ENDED
        ),
        "the round-ended announcement reaches online players"
    );
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
