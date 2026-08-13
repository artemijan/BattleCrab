//! Service-event handling and session lifecycle: network connect/disconnect
//! events, the login-link and DB results, and restart/logout/kick handling.
//! [`handle_game_event`] routes each unified-channel event to its handler.

use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::send_to_client;
use tracing::{debug, error, info, warn};

use crate::db::{self, DbEvent};
use crate::events::GameEvent;
use crate::loginlink::{LoginLinkCommand, LoginLinkEvent};
use crate::network::{NetEvent, server_packets};
use crate::session::{ClientSession, Session};
use crate::world::World;

use super::dispatch::on_packet;

/// Route one unified-channel event to its service's handler. Called by the
/// game loop both from the boundary drain and from the between-ticks sleep
/// (`recv_timeout`), so an event runs the moment it arrives.
pub(crate) fn handle_game_event(world: &mut World, event: GameEvent) {
    match event {
        GameEvent::Net(e) => handle_net_event(world, e),
        GameEvent::Login(e) => handle_login_link_event(world, e),
        GameEvent::Db(e) => handle_db_event(world, e),
        GameEvent::Path(e) => super::position::handle_path_result(world, e),
    }
}

/// The per-packet counter, resolved once. Looking a metric up by name takes the
/// registry lock, so the hot path holds the handle instead — after the first
/// call this is a relaxed atomic add and nothing else.
fn packets_handled() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_handled"))
}

/// Players currently connected, refreshed as connections come and go.
fn players_online() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("players_online"))
}

/// Registers the metrics above at boot so they read `0` from the first snapshot
/// instead of being *absent* until the first packet arrives. An absent series
/// and a zero one graph very differently, and "no players yet" is exactly the
/// state worth being able to see.
pub fn register_metrics() {
    packets_handled();
    players_online().set(0);
    super::tick_busy_micros().set(0);
    crate::network::register_metrics();
}

mod db_events;
mod persistence;
mod session;

pub(crate) use db_events::*;
pub(crate) use persistence::*;
pub(crate) use session::*;
