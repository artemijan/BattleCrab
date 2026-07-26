//! Monster Race Track (G26.5) — this slice is the pure race math ported from
//! Java `MonsterRace`: the per-lane speed roll (which also decides the winner),
//! the pari-mutuel odds, and bet accumulation. The 1-second race-cycle state
//! machine, the Derby-zone broadcast, the monster spawns, and the `RaceManager`
//! NPC betting/payout are later slices.

use std::collections::HashMap;

use rand::Rng;

use crate::model::monster_race::LANES;

// The pure helpers below are wired to callers by the race-cycle state machine +
// the RaceManager NPC betting (next slices); only the tests exercise them so far.

/// Roll the eight lanes' 20-step speed tables and decide the placings (Java
/// `MonsterRace.newSpeeds`). Returns `(speeds, first, second)` where `first`/
/// `second` are `(lane, total_speed)` — lane `8 - i` for monster index `i`, so
/// index 0 is lane 8 and index 7 is lane 1. Each step is `Rnd.get(60) + 65`
/// (65..=124), except the final step which is a flat 100.
#[allow(dead_code)]
pub(crate) fn roll_speeds(rng: &mut impl Rng) -> ([[i32; 20]; LANES], (i32, i32), (i32, i32)) {
    let mut speeds = [[0i32; 20]; LANES];
    let mut first = (0i32, 0i32);
    let mut second = (0i32, 0i32);
    for (i, lane_speeds) in speeds.iter_mut().enumerate() {
        let mut total = 0;
        for (j, step) in lane_speeds.iter_mut().enumerate() {
            *step = if j == 19 {
                100
            } else {
                rng.gen_range(65..=124)
            };
            total += *step;
        }
        let lane = 8 - i as i32;
        if total >= first.1 {
            second = first;
            first = (lane, total);
        } else if total >= second.1 {
            second = (lane, total);
        }
    }
    (speeds, first, second)
}

/// Pari-mutuel odds per lane in lane order 1..=8 (Java `calculateOdds`): a lane
/// with no bets pays `0`, else `max(1.25, totalPool * 0.7 / laneBets)`.
#[allow(dead_code)]
pub(crate) fn calculate_odds(bets: &HashMap<i32, i64>) -> Vec<f64> {
    let total: i64 = (1..=LANES as i32)
        .map(|l| bets.get(&l).copied().unwrap_or(0))
        .sum();
    (1..=LANES as i32)
        .map(|lane| {
            let amount = bets.get(&lane).copied().unwrap_or(0);
            if amount == 0 {
                0.0
            } else {
                (total as f64 * 0.7 / amount as f64).max(1.25)
            }
        })
        .collect()
}

/// Add `amount` to a lane's pooled bet (Java `setBetOnLane`, the in-memory half).
#[allow(dead_code)]
pub(crate) fn add_bet(bets: &mut HashMap<i32, i64>, lane: i32, amount: i64) {
    *bets.entry(lane).or_insert(0) += amount;
}
