//! Client session lifecycle as a **type-state machine** (plan §3.1).
//!
//! `Session<S>` is the game-thread's view of one connected client; the state
//! parameter `S` decides which methods exist, so out-of-state actions are
//! compile errors. Transitions consume `self`. The storage/dispatch boundary
//! wraps the typed sessions in [`ClientSession`], whose variant is the runtime
//! tag matched during packet dispatch (Java's per-`ConnectionState` gating).
//!
//! G2 introduces `Connecting` and `Authenticated`; `InLobby`/`Entering`/`InGame`
//! arrive in G3/G4.

use std::net::SocketAddr;

use crate::network::OutboundTx;

/// Java `LoginServerThread.SessionKey`: the two 2×int keys agreed with the login
/// server, echoed by the client in `AuthLogin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub login_ok1: i32,
    pub login_ok2: i32,
    pub play_ok1: i32,
    pub play_ok2: i32,
}

impl SessionKey {
    pub fn new(login_ok1: i32, login_ok2: i32, play_ok1: i32, play_ok2: i32) -> Self {
        Self { login_ok1, login_ok2, play_ok1, play_ok2 }
    }
}

/// Common per-connection data, present in every state.
pub struct Session<S> {
    pub client_id: u32,
    pub out: OutboundTx,
    pub addr: SocketAddr,
    pub state: S,
}

impl<S> Session<S> {
    /// Queue a serialized packet body for this client (the connection task
    /// encrypts + frames it). Silently dropped if the connection is gone.
    pub fn send(&self, body: Vec<u8>) {
        let _ = self.out.send(body);
    }
}

/// State: just connected (post-`ProtocolVersion`); no account yet.
pub struct Connecting;

/// State: `AuthLogin` session key validated by the login server.
pub struct Authenticated {
    pub account: String,
    pub session_key: SessionKey,
}

impl Session<Connecting> {
    pub fn new(client_id: u32, out: OutboundTx, addr: SocketAddr) -> Self {
        Self { client_id, out, addr, state: Connecting }
    }

    /// `PlayerAuthResponse` authed → move to the character-selection lifecycle.
    pub fn into_authenticated(self, account: String, session_key: SessionKey) -> Session<Authenticated> {
        Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: Authenticated { account, session_key },
        }
    }
}

impl Session<Authenticated> {
    pub fn account(&self) -> &str {
        &self.state.account
    }
}

/// Runtime-tagged wrapper stored in the client registry.
pub enum ClientSession {
    Connecting(Session<Connecting>),
    Authenticated(Session<Authenticated>),
}

impl ClientSession {
    pub fn client_id(&self) -> u32 {
        match self {
            ClientSession::Connecting(s) => s.client_id,
            ClientSession::Authenticated(s) => s.client_id,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        match self {
            ClientSession::Connecting(s) => s.addr,
            ClientSession::Authenticated(s) => s.addr,
        }
    }

    /// Queue a packet regardless of state.
    pub fn send(&self, body: Vec<u8>) {
        match self {
            ClientSession::Connecting(s) => s.send(body),
            ClientSession::Authenticated(s) => s.send(body),
        }
    }
}
