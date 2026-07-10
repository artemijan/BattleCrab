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

use crate::character::CharData;
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

/// State: `AuthLogin` session key validated; character list not yet loaded.
pub struct Authenticated {
    pub account: String,
    pub session_key: SessionKey,
}

/// State: character-selection screen — the character list is loaded and the
/// player may create/delete/restore/select (Java `AUTHENTICATED`).
pub struct InLobby {
    pub account: String,
    pub session_key: SessionKey,
    /// The characters as last sent in `CharSelectionInfo`; slot = list index.
    pub chars: Vec<CharData>,
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

    /// Character list loaded → move to the selection screen.
    pub fn into_lobby(self, chars: Vec<CharData>) -> Session<InLobby> {
        Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: InLobby { account: self.state.account, session_key: self.state.session_key, chars },
        }
    }
}

impl Session<InLobby> {
    pub fn account(&self) -> &str {
        &self.state.account
    }

    /// `getSessionId().playOkID1` — the session id sent in `CharSelectionInfo`.
    pub fn play_ok1(&self) -> i32 {
        self.state.session_key.play_ok1
    }

    /// The character at a client-supplied slot (list index).
    pub fn char_at(&self, slot: i32) -> Option<&CharData> {
        usize::try_from(slot).ok().and_then(|i| self.state.chars.get(i))
    }

    /// Replace the cached character list after a reload.
    pub fn set_chars(&mut self, chars: Vec<CharData>) {
        self.state.chars = chars;
    }
}

/// Runtime-tagged wrapper stored in the client registry.
pub enum ClientSession {
    Connecting(Session<Connecting>),
    Authenticated(Session<Authenticated>),
    InLobby(Session<InLobby>),
}

impl ClientSession {
    pub fn client_id(&self) -> u32 {
        match self {
            ClientSession::Connecting(s) => s.client_id,
            ClientSession::Authenticated(s) => s.client_id,
            ClientSession::InLobby(s) => s.client_id,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        match self {
            ClientSession::Connecting(s) => s.addr,
            ClientSession::Authenticated(s) => s.addr,
            ClientSession::InLobby(s) => s.addr,
        }
    }

    /// Queue a packet regardless of state.
    pub fn send(&self, body: Vec<u8>) {
        match self {
            ClientSession::Connecting(s) => s.send(body),
            ClientSession::Authenticated(s) => s.send(body),
            ClientSession::InLobby(s) => s.send(body),
        }
    }
}
