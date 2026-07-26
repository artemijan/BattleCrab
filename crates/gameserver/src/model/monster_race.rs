//! Monster Race Track runtime state — the World-side counterpart of the Java
//! `MonsterRace` singleton (G26.5). This slice is the data + the pure odds/speed
//! math + the `MonRaceInfo` packet; the 1-second race-cycle state machine, the
//! Derby-zone broadcast, the monster spawns, and betting/payout via the
//! `RaceManager` NPC are later slices.

use std::collections::HashMap;

/// The eight lanes (Java hard-codes 8 monsters).
pub const LANES: usize = 8;

/// The race-cycle phase (Java `MonsterRace.RaceState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RaceState {
    AcceptingBets,
    Waiting,
    StartingRace,
    #[default]
    RaceEnd,
}

/// One finished race's record (Java `MonsterRace.HistoryInfo`), persisted in
/// `mdt_history`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryInfo {
    pub race_id: i32,
    /// Winning / runner-up lane (1..=8).
    pub first: i32,
    pub second: i32,
    /// The winning lane's odds at race time.
    pub odd_rate: f64,
}

/// The Monster Race runtime (Java `MonsterRace` fields). Inert until enabled
/// (`AllowRace`).
#[derive(Debug, Default)]
pub struct MonsterRaceState {
    /// Current race number (Java `_raceNumber`; the first race is 1).
    pub race_number: i32,
    /// Countdown tick 0..=1200 driving the 1-second cycle (Java `_finalCountdown`).
    pub countdown: i32,
    pub state: RaceState,
    /// Adena bet on each lane this race (Java `_betsPerLane`, lane 1..=8 →
    /// amount; reset to 0 after each race).
    pub bets: HashMap<i32, i64>,
    /// Odds per lane in lane order (Java `_odds`), recomputed when sales close.
    pub odds: Vec<f64>,
    /// Past race records (Java `_history`).
    pub history: Vec<HistoryInfo>,
    /// The eight racer object ids (Java `_monsters`), lane `i` = index `i`. `0`
    /// when none is set. These are packet-only holders (Java `new Npc(template)`
    /// is never added to the world), so an id is allocated, not a real spawn.
    pub monsters: [i32; LANES],
    /// The eight racers' NPC template ids (Java the `_monsters[i]` template),
    /// for the `MonRaceInfo` display id + collision dims.
    pub monster_templates: [i32; LANES],
    /// Per-lane 20-step speed table for this race (Java `_speeds`).
    pub speeds: [[i32; 20]; LANES],
    /// `(lane, total_speed)` of the fastest / second-fastest racer, decided at
    /// speed roll (Java `_first`/`_second`).
    pub first: (i32, i32),
    pub second: (i32, i32),
}
