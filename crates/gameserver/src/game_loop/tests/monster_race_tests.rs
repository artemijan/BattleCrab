//! Monster Race Track (G26.5) — the pure race math (speed roll + winner, odds)
//! and the `MonRaceInfo` packet shape. The race-cycle state machine, betting,
//! and the RaceManager NPC are later slices.

use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::game_loop::monster_race::{add_bet, calculate_odds, roll_speeds};
use crate::network::server_packets;

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
