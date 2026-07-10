//! The game thread and its 100 ms tick loop (CONCURRENCY_MODEL §2.2).
//!
//! Runs on one dedicated OS thread that owns [`World`]. The base tick is 100 ms,
//! matching Java's `GameTimeTaskManager` and high-priority task-manager rate.
//! Steps: drain network events → drain login-link events → fire timers → run
//! tick systems (G4+) → flush. Packet dispatch and login handoff land here on
//! the game thread, keeping handler code sequential and 1:1 with Java `run()`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::loginlink::{CommandTx, LoginLinkCommand, LoginLinkEvent, LoginLinkEventRx};
use crate::network::client_packets::{opcodes as cop, AuthLogin};
use crate::network::{server_packets, NetEvent, NetEventRx};
use crate::session::{ClientSession, Session, SessionKey};
use crate::world::{World, WaitingClient};

/// Base tick period. Slower Java rates (1 s, 5 s…) become `world.tick % N == 0`
/// systems on top of this.
pub const TICK: Duration = Duration::from_millis(100);

/// A tick that runs longer than this is the failure mode of the single-thread
/// design, so it must be visible from day one (CONCURRENCY_MODEL §2.6 rule 4).
const TICK_OVERRUN_WARN: Duration = Duration::from_millis(50);

/// Signal shared with the async side (ctrl-c / scheduled restart) to stop the
/// loop after the current tick finishes.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Everything the game thread needs to start.
pub struct GameThreadChannels {
    pub net_rx: NetEventRx,
    pub login_rx: LoginLinkEventRx,
    pub link_tx: CommandTx,
    pub max_characters_per_account: i32,
}

/// Spawn the game thread. Returns its join handle so `main` can wait for the
/// final tick (drain + save) before exiting.
pub fn spawn(shutdown: Shutdown, ch: GameThreadChannels) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("game-thread".to_string())
        .spawn(move || run(shutdown, ch))
        .expect("failed to spawn game thread")
}

fn run(shutdown: Shutdown, ch: GameThreadChannels) {
    let GameThreadChannels { net_rx, login_rx, link_tx, max_characters_per_account } = ch;
    let mut world = World::new(link_tx, max_characters_per_account);
    info!("GameLoop: started ({} ms tick).", TICK.as_millis());

    while !shutdown.is_requested() {
        let tick_start = Instant::now();

        // 1. Network events: connects, disconnects, and inbound packets.
        drain_network(&mut world, &net_rx);
        // 2. Service results: login-link (DB / path added G3+).
        drain_login_link(&mut world, &login_rx);

        // 3. One-shot timers due this tick.
        world.run_due_tasks();

        // 4. Fixed-rate tick systems (movement, AI, attack…) — added in G4+.
        // 5. Flush outbound packets / DB commands — added in G3+.

        let elapsed = tick_start.elapsed();
        if elapsed > TICK_OVERRUN_WARN {
            warn!("GameLoop: tick {} ran {} ms (budget {} ms).", world.tick, elapsed.as_millis(), TICK.as_millis());
        }
        if let Some(remaining) = TICK.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }

        world.tick += 1;
    }

    info!("GameLoop: stopped after {} ticks.", world.tick);
    // Final drain + save-all lands with the DB thread (G3).
}

/// Bounded, non-blocking drain of the network→game channel (step 1 of the tick).
fn drain_network(world: &mut World, net_rx: &NetEventRx) {
    while let Ok(event) = net_rx.try_recv() {
        match event {
            NetEvent::Connected { client_id, out, addr } => {
                world.clients.insert(client_id, ClientSession::Connecting(Session::new(client_id, out, addr)));
                debug!("GameLoop: client {client_id} connected from {addr} ({} online).", world.clients.len());
            }
            NetEvent::Received { client_id, data } => {
                on_packet(world, client_id, data);
            }
            NetEvent::Disconnected { client_id } => {
                on_disconnect(world, client_id);
            }
        }
    }
}

/// Dispatch one decrypted client packet body (opcode + payload) on the game
/// thread. G2 handles `AuthLogin`; gameplay packets arrive from G3 on.
fn on_packet(world: &mut World, client_id: u32, data: Vec<u8>) {
    let Some(&opcode) = data.first() else { return };
    let body = &data[1..];
    match opcode {
        cop::AUTH_LOGIN => handle_auth_login(world, client_id, body),
        _ => debug!("GameLoop: client {client_id} sent opcode 0x{opcode:02x}, unhandled in G2."),
    }
}

/// Port of `clientpackets/AuthLogin.runImpl`: register the account on this game
/// server and ask the login server to validate the session key.
fn handle_auth_login(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = AuthLogin::read(body) else { return };
    if pkt.login_name.is_empty() {
        world.clients.remove(&client_id); // closeNow
        return;
    }
    // Only valid once, from a still-connecting client (Java: accountName == null).
    if !matches!(world.clients.get(&client_id), Some(ClientSession::Connecting(_))) {
        return;
    }
    let account = pkt.login_name;
    // addGameServerLogin: reject a duplicate login for the account.
    if world.login.accounts_in_gameserver.contains_key(&account) {
        world.clients.remove(&client_id); // close(null)
        return;
    }
    world.login.accounts_in_gameserver.insert(account.clone(), client_id);
    let key = SessionKey::new(pkt.login_key1, pkt.login_key2, pkt.play_key1, pkt.play_key2);
    world.login.waiting.insert(account.clone(), WaitingClient { client_id, session_key: key });
    let _ = world.login.link.send(LoginLinkCommand::PlayerAuthRequest { account, key });
}

/// Clean up a disconnected client and inform the login server.
fn on_disconnect(world: &mut World, client_id: u32) {
    world.clients.remove(&client_id);
    let account = world
        .login
        .accounts_in_gameserver
        .iter()
        .find(|(_, &id)| id == client_id)
        .map(|(a, _)| a.clone());
    if let Some(account) = account {
        world.login.accounts_in_gameserver.remove(&account);
        world.login.waiting.remove(&account);
        let _ = world.login.link.send(LoginLinkCommand::PlayerLogout { account });
    }
    debug!("GameLoop: client {client_id} disconnected ({} online).", world.clients.len());
}

/// Bounded, non-blocking drain of the login-link→game channel (step 2).
fn drain_login_link(world: &mut World, login_rx: &LoginLinkEventRx) {
    while let Ok(event) = login_rx.try_recv() {
        match event {
            LoginLinkEvent::Registered { server_id, server_name } => {
                info!("GameLoop: registered as Server {server_id}: {server_name}.");
                world.login.server_id = Some(server_id);
                world.login.server_name = Some(server_name);
            }
            LoginLinkEvent::PlayerAuthResponse { account, authed } => {
                handle_player_auth_response(world, account, authed);
            }
            LoginLinkEvent::KickPlayer { account } => handle_kick(world, account),
            LoginLinkEvent::RequestCharacters { account } => {
                // Full char count needs the DB (G3); reply 0 for now.
                let _ = world.login.link.send(LoginLinkCommand::ReplyCharacters {
                    account,
                    chars: 0,
                    del_times: Vec::new(),
                });
            }
            LoginLinkEvent::Failed { reason } => {
                warn!("GameLoop: login-server registration failed (reason {reason}).");
            }
        }
    }
}

/// Port of the `PlayerAuthResponse` (0x03) branch of `LoginServerThread.run`.
fn handle_player_auth_response(world: &mut World, account: String, authed: bool) {
    let Some(waiting) = world.login.waiting.remove(&account) else { return };
    let client_id = waiting.client_id;
    if authed {
        let _ = world.login.link.send(LoginLinkCommand::PlayerInGame { accounts: vec![account.clone()] });
        let max_chars = world.max_characters_per_account;
        if let Some(ClientSession::Connecting(s)) = world.clients.remove(&client_id) {
            let s = s.into_authenticated(account, waiting.session_key);
            s.send(server_packets::login_success());
            s.send(server_packets::char_selection_info_empty(max_chars));
            info!("GameLoop: client {} authenticated as '{}'.", s.client_id, s.account());
            world.clients.insert(client_id, ClientSession::Authenticated(s));
        }
    } else {
        warn!("GameLoop: session key incorrect, closing connection for account {account}.");
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::login_fail(0, 1)); // SYSTEM_ERROR_LOGIN_LATER
        }
        world.login.accounts_in_gameserver.remove(&account);
        world.clients.remove(&client_id); // disconnect after the queued packet
        let _ = world.login.link.send(LoginLinkCommand::PlayerLogout { account });
    }
}

/// Port of `doKickPlayer`: disconnect the account's client and notify login.
fn handle_kick(world: &mut World, account: String) {
    if let Some(&client_id) = world.login.accounts_in_gameserver.get(&account) {
        world.clients.remove(&client_id); // disconnect
    }
    world.login.accounts_in_gameserver.remove(&account);
    world.login.waiting.remove(&account);
    let _ = world.login.link.send(LoginLinkCommand::PlayerLogout { account });
}

#[cfg(test)]
mod tests {
    use super::*;
    use commons::network::PacketWriter;

    fn auth_login_body(name: &str, key: SessionKey) -> Vec<u8> {
        // readImpl order: name, playKey2, playKey1, loginKey1, loginKey2.
        let mut w = PacketWriter::new();
        w.write_string(name);
        w.write_i32(key.play_ok2);
        w.write_i32(key.play_ok1);
        w.write_i32(key.login_ok1);
        w.write_i32(key.login_ok2);
        w.into_bytes()
    }

    #[test]
    fn auth_login_then_authed_reaches_char_list() {
        let (link_tx, mut link_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7);

        // A connecting client with its own outbound queue.
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let addr = "127.0.0.1:5000".parse().unwrap();
        world.clients.insert(1, ClientSession::Connecting(Session::new(1, out_tx, addr)));

        // AuthLogin → PlayerAuthRequest to the login server.
        let key = SessionKey::new(11, 12, 21, 22);
        handle_auth_login(&mut world, 1, &auth_login_body("Bob", key));
        assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&1));
        match link_rx.try_recv().unwrap() {
            LoginLinkCommand::PlayerAuthRequest { account, key: k } => {
                assert_eq!(account, "bob");
                assert_eq!(k, key);
            }
            _ => panic!("expected PlayerAuthRequest"),
        }

        // Login server confirms → transition to Authenticated, char list sent.
        handle_player_auth_response(&mut world, "bob".to_string(), true);
        assert!(matches!(world.clients.get(&1), Some(ClientSession::Authenticated(_))));
        assert!(matches!(link_rx.try_recv().unwrap(), LoginLinkCommand::PlayerInGame { .. }));
        assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_success());
        assert_eq!(out_rx.try_recv().unwrap(), server_packets::char_selection_info_empty(7));
    }

    #[test]
    fn wrong_session_key_closes_connection() {
        let (link_tx, mut link_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7);
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        world.clients.insert(1, ClientSession::Connecting(Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())));

        handle_auth_login(&mut world, 1, &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)));
        let _ = link_rx.try_recv(); // PlayerAuthRequest

        handle_player_auth_response(&mut world, "bob".to_string(), false);
        // Client gets LoginFail then is dropped (closing the connection).
        assert_eq!(out_rx.try_recv().unwrap(), server_packets::login_fail(0, 1));
        assert!(world.clients.get(&1).is_none());
        assert!(!world.login.accounts_in_gameserver.contains_key("bob"));
        assert!(matches!(link_rx.try_recv().unwrap(), LoginLinkCommand::PlayerLogout { .. }));
    }

    #[test]
    fn duplicate_account_login_is_rejected() {
        let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut world = World::new(link_tx, 7);
        world.login.accounts_in_gameserver.insert("bob".to_string(), 99); // already on

        let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel();
        world.clients.insert(1, ClientSession::Connecting(Session::new(1, out_tx, "127.0.0.1:1".parse().unwrap())));
        handle_auth_login(&mut world, 1, &auth_login_body("bob", SessionKey::new(1, 2, 3, 4)));

        // The second client is dropped; the original mapping is untouched.
        assert!(world.clients.get(&1).is_none());
        assert_eq!(world.login.accounts_in_gameserver.get("bob"), Some(&99));
    }
}
