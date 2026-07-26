//! Weekly Lucky Lottery runtime state — the World-side counterpart of the Java
//! `Lottery` singleton's fields (G26.5). The round lifecycle + persistence live
//! in `game_loop/lottery.rs`; ticket purchase + the prize draw are slice 2.

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
}
