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
        Self {
            login_ok1,
            login_ok2,
            play_ok1,
            play_ok2,
        }
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
        tracing::trace!(
            "client {} ← opcode 0x{:02x}{} ({} B)",
            self.client_id,
            body.first().copied().unwrap_or(0),
            if body.first() == Some(&0xFE) && body.len() >= 3 {
                format!(":0x{:04x}", u16::from_le_bytes([body[1], body[2]]))
            } else {
                String::new()
            },
            body.len()
        );
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
        Self {
            client_id,
            out,
            addr,
            state: Connecting,
        }
    }

    /// `PlayerAuthResponse` authed → move to the character-selection lifecycle.
    pub fn into_authenticated(
        self,
        account: String,
        session_key: SessionKey,
    ) -> Session<Authenticated> {
        Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: Authenticated {
                account,
                session_key,
            },
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
            state: InLobby {
                account: self.state.account,
                session_key: self.state.session_key,
                chars,
            },
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
        usize::try_from(slot)
            .ok()
            .and_then(|i| self.state.chars.get(i))
    }

    /// Replace the cached character list after a reload.
    pub fn set_chars(&mut self, chars: Vec<CharData>) {
        self.state.chars = chars;
    }

    /// `CharacterSelect`: a character is chosen and its `Player` built; move to
    /// the loading screen (Java `ConnectionState.ENTERING`).
    pub fn into_entering(self, player: crate::model::PlayerData) -> Session<Entering> {
        // Java `Player.restore` reads the account's *other* characters into
        // `_chars` — the freight's "send to" list. The lobby already holds them.
        let selected = player.player.object_id;
        let account_chars = self
            .state
            .chars
            .iter()
            .filter(|c| c.object_id != selected)
            .map(|c| (c.object_id, c.name.clone()))
            .collect();
        Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: Entering {
                account: self.state.account,
                session_key: self.state.session_key,
                player,
                account_chars,
            },
        }
    }
}

/// State: character selected, entering the world (Java `ENTERING`). Holds the
/// built `Player` (as its full component bundle — the player is outside the
/// ECS world until `EnterWorld` spawns it into the `World` registry).
pub struct Entering {
    pub account: String,
    pub session_key: SessionKey,
    pub player: crate::model::PlayerData,
    /// The account's *other* characters (id, name) — Java `Player._chars`,
    /// the freight "send to" list.
    pub account_chars: Vec<(i32, String)>,
}

/// State: in the world. The `Player` lives in the `World` object registry; this
/// links to it by id (plan §3.1).
pub struct InGame {
    pub account: String,
    pub session_key: SessionKey,
    pub player_object_id: i32,
    /// The account's other characters (id, name) — Java `Player.getAccountChars()`,
    /// which the freight send validates its recipient against.
    pub account_chars: Vec<(i32, String)>,
    /// Java `Player._adminConfirmCmd` + `PlayerAction.ADMIN_COMMAND`: the full
    /// admin command awaiting a `ConfirmDlg` "yes" (`None` = no pending
    /// confirm). Consumed by the `DlgAnswer` reply.
    pub pending_admin_confirm: Option<String>,
}

impl Session<Entering> {
    pub fn account(&self) -> &str {
        &self.state.account
    }
    pub fn play_ok1(&self) -> i32 {
        self.state.session_key.play_ok1
    }
    pub fn player(&self) -> &crate::model::PlayerData {
        &self.state.player
    }

    /// `EnterWorld`: hand the `Player` to the world and move to `InGame`.
    pub fn into_ingame(self) -> (Session<InGame>, crate::model::PlayerData) {
        let object_id = self.state.player.player.object_id;
        let session = Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: InGame {
                account: self.state.account,
                session_key: self.state.session_key,
                player_object_id: object_id,
                account_chars: self.state.account_chars,
                pending_admin_confirm: None,
            },
        };
        (session, self.state.player)
    }
}

impl Session<InGame> {
    pub fn account(&self) -> &str {
        &self.state.account
    }
    pub fn player_object_id(&self) -> i32 {
        self.state.player_object_id
    }

    /// Java `Player.getAccountChars()` — the account's other characters.
    pub fn account_chars(&self) -> &[(i32, String)] {
        &self.state.account_chars
    }

    /// Java `Player.setAdminConfirmCmd` — stash a command awaiting confirm.
    pub fn set_admin_confirm(&mut self, command: String) {
        self.state.pending_admin_confirm = Some(command);
    }

    /// Take the pending confirm command (clears it), like Java
    /// `removeAction(ADMIN_COMMAND)` + `getAdminConfirmCmd()`.
    pub fn take_admin_confirm(&mut self) -> Option<String> {
        self.state.pending_admin_confirm.take()
    }

    /// `RequestRestart`: back to the character-selection lifecycle (Java
    /// `client.setConnectionState(ConnectionState.AUTHENTICATED)`); the
    /// character list is re-loaded through the normal `Authenticated → InLobby`
    /// path, same as after login.
    pub fn into_authenticated(self) -> Session<Authenticated> {
        Session {
            client_id: self.client_id,
            out: self.out,
            addr: self.addr,
            state: Authenticated {
                account: self.state.account,
                session_key: self.state.session_key,
            },
        }
    }
}

/// Runtime-tagged wrapper stored in the client registry.
pub enum ClientSession {
    Connecting(Session<Connecting>),
    Authenticated(Session<Authenticated>),
    InLobby(Session<InLobby>),
    Entering(Session<Entering>),
    InGame(Session<InGame>),
}

impl ClientSession {
    pub fn client_id(&self) -> u32 {
        match self {
            ClientSession::Connecting(s) => s.client_id,
            ClientSession::Authenticated(s) => s.client_id,
            ClientSession::InLobby(s) => s.client_id,
            ClientSession::Entering(s) => s.client_id,
            ClientSession::InGame(s) => s.client_id,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        match self {
            ClientSession::Connecting(s) => s.addr,
            ClientSession::Authenticated(s) => s.addr,
            ClientSession::InLobby(s) => s.addr,
            ClientSession::Entering(s) => s.addr,
            ClientSession::InGame(s) => s.addr,
        }
    }

    /// Queue a packet regardless of state.
    pub fn send(&self, body: Vec<u8>) {
        match self {
            ClientSession::Connecting(s) => s.send(body),
            ClientSession::Authenticated(s) => s.send(body),
            ClientSession::InLobby(s) => s.send(body),
            ClientSession::Entering(s) => s.send(body),
            ClientSession::InGame(s) => s.send(body),
        }
    }

    /// A clone of this client's outbound queue, for the rare job that has to
    /// keep talking to one client from a worker thread (`//geosaveall`). The
    /// queue is unbounded and its receiver lives in the connection task, so a
    /// disconnect just drops the sends.
    pub fn outbound(&self) -> OutboundTx {
        match self {
            ClientSession::Connecting(s) => s.out.clone(),
            ClientSession::Authenticated(s) => s.out.clone(),
            ClientSession::InLobby(s) => s.out.clone(),
            ClientSession::Entering(s) => s.out.clone(),
            ClientSession::InGame(s) => s.out.clone(),
        }
    }
}
