//! The unified service→game channel (THREADING_MODEL §1–2).
//!
//! Every service that reports to the game thread — the connection tasks, the
//! login-link task, the DB thread, the path worker — sends one [`GameEvent`]
//! variant into a single `std::sync::mpsc` channel. One channel instead of
//! four because the game loop *sleeps on it*: between tick boundaries the game
//! thread blocks in `recv_timeout` on this receiver, so any service event
//! wakes it and is handled the moment it arrives instead of waiting out the
//! tick sleep. Four separate channels would leave three of them able to sit
//! unnoticed until the boundary, since `std::sync::mpsc` can only block on one
//! receiver at a time.
//!
//! Services never see this type on the send side: each keeps its own typed
//! sender facade ([`crate::network::NetEventTx`], [`crate::db::EventTx`],
//! [`crate::loginlink::EventTx`], [`crate::geo::worker::PathEventTx`])
//! wrapping a clone of the one sender, so a service cannot send another
//! service's events and the call sites read exactly as before.

use crate::db::DbEvent;
use crate::geo::worker::PathEvent;
use crate::loginlink::LoginLinkEvent;
use crate::network::NetEvent;

/// One event from any service, tagged by source. Matched apart again in
/// `game_loop::net::handle_game_event`, which routes each variant to the same
/// handler the per-service drains used to call.
pub enum GameEvent {
    Net(NetEvent),
    Login(LoginLinkEvent),
    Db(DbEvent),
    Path(PathEvent),
}

/// Sender side. Cloned into each service's typed facade; sends are
/// non-blocking (the channel is unbounded) and an `Err` means the game thread
/// is gone, which every service treats as shutdown.
pub type GameEventTx = std::sync::mpsc::Sender<GameEvent>;
/// Receiver side, owned by the game loop, which drains it at the tick
/// boundary and sleeps on it (`recv_timeout`) in between.
pub type GameEventRx = std::sync::mpsc::Receiver<GameEvent>;
