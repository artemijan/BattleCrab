//! Service-event handling and session lifecycle: network connect/disconnect
//! events, the login-link and DB results, and restart/logout/kick handling.
//! [`handle_game_event`] routes each unified-channel event to its handler.

use crate::events::GameEvent;

use crate::session::ClientSession;
use crate::world::World;

pub mod broadcast;
mod clients;
mod db_events;
mod persistence;
mod session;

pub use clients::ClientsReplyTx;
#[cfg(test)]
pub(crate) use clients::client_records;
pub(crate) use db_events::handle_db_event;

#[cfg(test)]
pub(crate) use persistence::build_save_data;
pub(crate) use persistence::{
    autosave_tick, save_all_players, store_and_remove_player, store_player_now,
};

use persistence::{henna_rows, reuses_to_save};
pub(crate) use session::{
    handle_login_link_event, handle_logout, handle_net_event, handle_request_restart,
    on_characters_loaded,
};
#[cfg(test)]
pub(crate) use session::{handle_player_auth_response, on_disconnect};

/// Route one unified-channel event to its service's handler. Called by the
/// game loop both from the boundary drain and from the between-ticks sleep
/// (`recv_timeout`), so an event runs the moment it arrives.
pub(crate) fn handle_game_event(world: &mut World, event: GameEvent) {
    match event {
        GameEvent::Net(e) => handle_net_event(world, e),
        GameEvent::Login(e) => handle_login_link_event(world, e),
        GameEvent::Db(e) => handle_db_event(world, e),
        GameEvent::Path(e) => super::space::position::handle_path_result(world, e),
        GameEvent::Monitor(reply) => clients::answer_clients(world, reply),
    }
}

/// The per-packet counter, resolved once. Looking a metric up by name takes the
/// registry lock, so the hot path holds the handle instead — after the first
/// call this is a relaxed atomic add and nothing else.
fn packets_handled() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_handled"))
}

/// Game-server sessions by lifecycle stage, refreshed once per tick by
/// [`refresh_session_gauges`]. Together they sum to the sessions the game
/// thread knows about; `connections_open` can briefly exceed that between
/// accept and the game thread's `Connected` event.
///
/// - `sessions_authenticating`: `Connecting` + `Authenticated` — socket open,
///   protocol / session-key check with the login server not finished yet.
/// - `sessions_lobby`: `InLobby` — at the character-selection screen.
/// - `sessions_entering`: `Entering` — a character picked, loading into the
///   world.
/// - `players_online`: `InGame` — a character actually in the world.
///
/// `offline_traders` is not a session at all: shops left standing after their
/// owner disconnected, counted separately because they are in the world but
/// hold no connection.
struct SessionGauges {
    authenticating: commons::metrics::Gauge,
    lobby: commons::metrics::Gauge,
    entering: commons::metrics::Gauge,
    in_game: commons::metrics::Gauge,
    offline_traders: commons::metrics::Gauge,
}

fn session_gauges() -> &'static SessionGauges {
    static G: std::sync::OnceLock<SessionGauges> = std::sync::OnceLock::new();
    G.get_or_init(|| SessionGauges {
        authenticating: commons::metrics::gauge("sessions_authenticating"),
        lobby: commons::metrics::gauge("sessions_lobby"),
        entering: commons::metrics::gauge("sessions_entering"),
        in_game: commons::metrics::gauge("players_online"),
        offline_traders: commons::metrics::gauge("offline_traders"),
    })
}

/// Recounts [`SessionGauges`] from `world.clients`. Per tick rather than at
/// each transition: stages also change on DB replies and login-link answers,
/// not just network events, and one pass over the client table is cheaper
/// than keeping a dozen transition sites honest.
pub(crate) fn refresh_session_gauges(world: &World) {
    let (mut authenticating, mut lobby, mut entering, mut in_game) = (0u64, 0u64, 0u64, 0u64);
    for c in world.clients.values() {
        match c {
            ClientSession::Connecting(_) | ClientSession::Authenticated(_) => authenticating += 1,
            ClientSession::InLobby(_) => lobby += 1,
            ClientSession::Entering(_) => entering += 1,
            ClientSession::InGame(_) => in_game += 1,
        }
    }
    let g = session_gauges();
    g.authenticating.set(authenticating);
    g.lobby.set(lobby);
    g.entering.set(entering);
    g.in_game.set(in_game);
    g.offline_traders.set(world.offline_traders.len() as u64);
}

/// Registers the metrics above at boot so they read `0` from the first snapshot
/// instead of being *absent* until the first packet arrives. An absent series
/// and a zero one graph very differently, and "no players yet" is exactly the
/// state worth being able to see.
pub fn register_metrics() {
    packets_handled();
    let g = session_gauges();
    for gauge in [
        &g.authenticating,
        &g.lobby,
        &g.entering,
        &g.in_game,
        &g.offline_traders,
    ] {
        gauge.set(0);
    }
    super::tick_busy_micros().set(0);
    super::tick_busy_micros_total();
    super::ticks();
    super::tick_overruns();
    crate::network::register_metrics();
}
