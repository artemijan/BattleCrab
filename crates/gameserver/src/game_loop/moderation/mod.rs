//! Player-facing enforcement — the runtime half of keeping order: the
//! punishment ledger (ban, chat ban, jail), the bot-report quota and the GM
//! petition queue.
//!
//! The `//` command surface that a GM drives these from lives in
//! [`crate::game_loop::admin`]; what lives here is the state those commands
//! mutate and the rules that apply it to a session on its own.

pub(crate) mod bot_report;
pub(crate) mod petition;
pub(crate) mod punishment;
