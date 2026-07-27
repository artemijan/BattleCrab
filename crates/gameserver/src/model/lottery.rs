//! Weekly Lucky Lottery runtime state — the World-side counterpart of the Java
//! `Lottery` singleton's fields (G26.5). The round lifecycle, persistence,
//! ticket draw + claim live in `game_loop/lottery.rs`.

use std::collections::HashMap;

/// One `lottery` table row (Java the DB record), as loaded at boot. Only the
/// columns the round engine reads on resume.
#[derive(Debug, Clone, Copy, Default)]
pub struct LotteryRow {
    /// `idnr` — the round number.
    pub idnr: i32,
    /// `prize` — the round's jackpot at draw time.
    pub prize: i64,
    /// `newprize` — the pot carried to the next round (the jackpot minus paid
    /// prizes; equals `prize` while the round is live).
    pub newprize: i64,
    /// `enddate` — epoch millis the round draws.
    pub enddate: i64,
    /// `finished` — whether the draw has happened.
    pub finished: bool,
}

/// A drawn round's result (Java the `number1`/`number2`/`prize1..3` columns),
/// cached so `checkTicket` can score an old ticket without a DB round-trip.
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawnRound {
    /// Winning-number bitmask words (`enchant`/`type2`).
    pub number1: i32,
    pub number2: i32,
    /// Per-tier payout stored at draw time.
    pub prize1: i64,
    pub prize2: i64,
    pub prize3: i64,
}

/// The lottery manager runtime (Java `Lottery` `_number`/`_prize`/… fields).
/// Inert until [`crate::game_loop::lottery::on_loaded`] initializes it at boot
/// (only when `AllowLottery`).
#[derive(Debug, Default)]
pub struct LotteryState {
    /// Current round number (Java `_number`; the first round is 1).
    pub number: i32,
    /// Current jackpot (Java `_prize`).
    pub prize: i64,
    /// Whether tickets can be bought right now (Java `_isSellingTickets`).
    pub selling: bool,
    /// Whether a round is running (Java `_isStarted`).
    pub started: bool,
    /// Epoch millis the current round draws (Java `_enddate`).
    pub enddate: i64,
    /// The winning-number bitmask rolled by `finish_begin`, held until the
    /// async ticket load lets `finish_complete` score the tickets (the two words
    /// of Java `finishLottery`'s `enchant`/`type2`). `1..=16 → draw_enchant`,
    /// `17..=20 → draw_type2`.
    pub draw_enchant: i32,
    pub draw_type2: i32,
    /// Past drawn rounds by round id (Java re-queries the `lottery` row per
    /// `checkTicket`; cached here so prize claim stays synchronous).
    pub drawn: HashMap<i32, DrawnRound>,
}
