//! Monster Race Track (G26.5) — the pure race math (speed roll + winner, odds),
//! the `MonRaceInfo` packet shape, and the 1-second race-cycle state machine.
//! Betting + payout via the RaceManager NPC are slice 4.

use super::*;

use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::game_loop::monster_race::{self, add_bet, calculate_odds, roll_speeds};
use crate::model::monster_race::RaceState;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;

#[test]
fn roll_speeds_picks_the_fastest_lane_as_the_winner() {
    let mut rng = StdRng::seed_from_u64(42);
    let (speeds, first, second) = roll_speeds(&mut rng);

    // Totals per lane (monster index i → lane 8 - i).
    let totals: Vec<(i32, i32)> = (0..8)
        .map(|i| (8 - i as i32, speeds[i].iter().sum()))
        .collect();
    let max = totals.iter().map(|&(_, t)| t).max().unwrap();
    assert_eq!(first.1, max, "first place has the highest total speed");
    assert!(totals.iter().any(|&(lane, t)| lane == first.0 && t == max));
    assert!(first.1 >= second.1, "first outranks second");
    assert!((1..=8).contains(&first.0) && (1..=8).contains(&second.0));
    // Every step is in [65, 124] except the flat-100 finish.
    for lane in &speeds {
        assert_eq!(lane[19], 100);
        assert!(lane[..19].iter().all(|&s| (65..=124).contains(&s)));
    }
}

#[test]
fn calculate_odds_is_pari_mutuel() {
    let mut bets = HashMap::new();
    bets.insert(1, 100);
    bets.insert(2, 300);
    // Pool = 400. lane1 = 400*0.7/100 = 2.8; lane2 = 0.93 → floored to 1.25.
    let odds = calculate_odds(&bets);
    assert_eq!(odds.len(), 8);
    assert!((odds[0] - 2.8).abs() < 1e-9);
    assert!((odds[1] - 1.25).abs() < 1e-9); // the 1.25 floor
    assert_eq!(odds[2], 0.0); // no bets on lane 3
}

#[test]
fn add_bet_accumulates_per_lane() {
    let mut bets = HashMap::new();
    add_bet(&mut bets, 3, 100);
    add_bet(&mut bets, 3, 150);
    add_bet(&mut bets, 5, 50);
    assert_eq!(bets.get(&3), Some(&250));
    assert_eq!(bets.get(&5), Some(&50));
}

#[test]
fn mon_race_info_has_the_expected_wire_shape() {
    let monsters: [(i32, i32, f64, f64); 8] =
        std::array::from_fn(|i| (1000 + i as i32, 30 + i as i32, 8.0, 4.0));
    let speeds = [[70i32; 20]; 8];
    let pkt = server_packets::mon_race_info(0, 15322, &monsters, &speeds);

    assert_eq!(pkt[0], server_packets::opcodes::MON_RACE_INFO);
    // header (op + 2 codes + count) + 8 * (8 ints + 2 doubles + 1 int + 20 bytes)
    assert_eq!(pkt.len(), 13 + 8 * (32 + 16 + 4 + 20));
}

// --- The 1-second race-cycle state machine (slice 3b) ---

/// A test world with the race enabled (dist ships it off), race #1.
fn race_world() -> World {
    let (mut world, _tx, _rx, _link) = test_world();
    world.cfg.general.allow_race = true;
    world.monster_race.race_number = 1;
    world
}

#[test]
fn tick_at_zero_opens_a_race_with_eight_distinct_racers() {
    let mut world = race_world();

    monster_race::tick(&mut world);

    assert_eq!(world.monster_race.state, RaceState::AcceptingBets);
    assert!(world.monster_race.monsters.iter().all(|&o| o != 0));
    let mut templates = world.monster_race.monster_templates.to_vec();
    assert!(templates.iter().all(|&t| (31003..=31026).contains(&t)));
    templates.sort();
    templates.dedup();
    assert_eq!(templates.len(), 8, "eight distinct racer templates");
    assert!((1..=8).contains(&world.monster_race.first.0));
    assert_eq!(world.monster_race.countdown, 1);
    assert!(world
        .scheduler
        .pending_tasks_for_test()
        .contains(&ScheduledTask::MonsterRaceTick));
}

#[test]
fn sales_close_posts_the_odds() {
    let mut world = race_world();
    world.monster_race.countdown = 900;
    world.monster_race.bets.insert(1, 100);
    world.monster_race.bets.insert(2, 300);

    monster_race::tick(&mut world);

    assert_eq!(world.monster_race.state, RaceState::Waiting);
    assert_eq!(world.monster_race.odds.len(), 8);
    assert!((world.monster_race.odds[0] - 2.8).abs() < 1e-9); // 400*0.7/100
}

#[test]
fn race_end_records_the_winner_clears_bets_and_advances() {
    let mut world = race_world();
    monster_race::tick(&mut world); // countdown 0: spawn + roll + history row
    let (winner, runner) = (world.monster_race.first.0, world.monster_race.second.0);
    world.monster_race.bets.insert(1, 50);
    world.monster_race.countdown = 1115;

    monster_race::tick(&mut world);

    assert_eq!(world.monster_race.state, RaceState::RaceEnd);
    let h = world.monster_race.history.last().unwrap();
    assert_eq!(h.first, winner);
    assert_eq!(h.second, runner);
    assert_eq!(world.monster_race.race_number, 2);
    assert!(world.monster_race.bets.values().all(|&v| v == 0));
}

#[test]
fn a_disabled_race_is_inert() {
    let (mut world, _tx, _rx, _link) = test_world(); // AllowRace defaults false
    monster_race::tick(&mut world);
    assert_eq!(world.monster_race.state, RaceState::RaceEnd); // unchanged default
    assert!(world.scheduler.pending_tasks_for_test().is_empty());
}
