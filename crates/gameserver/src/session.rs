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
    ///
    /// Takes `impl Into<Bytes>` so a freshly built `Vec<u8>` still works
    /// unchanged, while a broadcast can build the packet once and hand every
    /// recipient a cheap `Bytes` clone.
    pub fn send(&self, body: impl Into<bytes::Bytes>) {
        let body = body.into();
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
    pub fn send(&self, body: impl Into<bytes::Bytes>) {
        let body = body.into();
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

/// The connected-client table: `client id → session`, plus the reverse
/// `player object id → client id` index for in-game sessions.
///
/// Java reaches a player's `GameClient` through the `Player` object itself, so
/// "which connection drives this object id?" is a field read there. The port
/// keys sessions by network id, and answering the same question used to mean a
/// linear scan of every connected client — on a path that ~340 call sites take
/// (`helpers::send_to_player`, `send_sm_to_player`, `broadcast_including_self`,
/// every scheduled task that resolves a target by object id).
///
/// The index is maintained **here** rather than at the two dozen
/// `clients.insert`/`clients.remove` sites, because an index kept by hand at
/// call sites fails open: a missed site does not fail to compile, it silently
/// sends packets to nobody. Mutation only happens through [`insert`](Self::insert)
/// and [`remove`](Self::remove), so the two maps cannot drift.
#[derive(Default)]
pub struct ClientTable {
    by_client: std::collections::HashMap<u32, ClientSession>,
    by_player: std::collections::HashMap<i32, u32>,
}

impl ClientTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The player object id an in-game session drives; `None` for every other
    /// state (no player is attached yet).
    fn player_of(session: &ClientSession) -> Option<i32> {
        match session {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        }
    }

    /// The client id driving `player_object_id` in O(1) — Java
    /// `Player.getClient()`. `None` once that player has disconnected.
    pub fn client_of_player(&self, player_object_id: i32) -> Option<u32> {
        self.by_player.get(&player_object_id).copied()
    }

    pub fn insert(&mut self, client_id: u32, session: ClientSession) -> Option<ClientSession> {
        let new_player = Self::player_of(&session);
        let previous = self.by_client.insert(client_id, session);
        // Drop the outgoing session's reverse entry before adding the new one,
        // so a state transition on the same connection (Entering → InGame) or
        // a session swap can't strand a stale mapping.
        if let Some(old_player) = previous.as_ref().and_then(Self::player_of)
            && Some(old_player) != new_player
        {
            self.unindex(old_player, client_id);
        }
        if let Some(player) = new_player {
            self.by_player.insert(player, client_id);
        }
        previous
    }

    pub fn remove(&mut self, client_id: &u32) -> Option<ClientSession> {
        let removed = self.by_client.remove(client_id);
        if let Some(player) = removed.as_ref().and_then(Self::player_of) {
            self.unindex(player, *client_id);
        }
        removed
    }

    /// Drop `player`'s reverse entry, but only while it still points at
    /// `client_id`. A relog can register the new connection before the old one
    /// is torn down; without this guard, cleaning up the dead session would
    /// delete the live client's mapping and leave the player unreachable.
    fn unindex(&mut self, player: i32, client_id: u32) {
        if self.by_player.get(&player) == Some(&client_id) {
            self.by_player.remove(&player);
        }
    }

    pub fn get(&self, client_id: &u32) -> Option<&ClientSession> {
        self.by_client.get(client_id)
    }

    /// Mutable access to a session's own state (the pending admin confirm).
    /// Callers must not change which player the session drives — that is a
    /// state transition, which goes through `remove` + `insert`.
    pub fn get_mut(&mut self, client_id: &u32) -> Option<&mut ClientSession> {
        self.by_client.get_mut(client_id)
    }

    pub fn contains_key(&self, client_id: &u32) -> bool {
        self.by_client.contains_key(client_id)
    }

    pub fn values(&self) -> std::collections::hash_map::Values<'_, u32, ClientSession> {
        self.by_client.values()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, u32, ClientSession> {
        self.by_client.iter()
    }

    pub fn len(&self) -> usize {
        self.by_client.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_client.is_empty()
    }

    pub fn clear(&mut self) {
        self.by_client.clear();
        self.by_player.clear();
    }
}

impl<'a> IntoIterator for &'a ClientTable {
    type Item = (&'a u32, &'a ClientSession);
    type IntoIter = std::collections::hash_map::Iter<'a, u32, ClientSession>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod client_table_tests {
    use super::*;

    fn in_game(client_id: u32, player_object_id: i32) -> ClientSession {
        let (out, _rx) = tokio::sync::mpsc::unbounded_channel();
        ClientSession::InGame(Session {
            client_id,
            out,
            addr: "127.0.0.1:0".parse().unwrap(),
            state: InGame {
                account: String::new(),
                session_key: SessionKey::new(0, 0, 0, 0),
                player_object_id,
                account_chars: Vec::new(),
                pending_admin_confirm: None,
            },
        })
    }

    fn connecting(client_id: u32) -> ClientSession {
        let (out, _rx) = tokio::sync::mpsc::unbounded_channel();
        ClientSession::Connecting(Session::new(client_id, out, "127.0.0.1:0".parse().unwrap()))
    }

    /// The reverse index tracks in-game sessions and only in-game sessions —
    /// a connection that has not reached `InGame` drives no player yet.
    #[test]
    fn indexes_in_game_sessions_and_clears_on_remove() {
        let mut t = ClientTable::new();
        t.insert(1, in_game(1, 100));
        t.insert(2, connecting(2));
        assert_eq!(t.client_of_player(100), Some(1));
        assert_eq!(t.client_of_player(999), None);
        assert_eq!(t.len(), 2);

        t.remove(&1);
        assert_eq!(
            t.client_of_player(100),
            None,
            "index drops with the session"
        );
        assert!(t.get(&1).is_none());
    }

    /// A state transition on the same connection replaces the entry rather
    /// than stranding the old player id.
    #[test]
    fn a_transition_on_one_connection_moves_the_index() {
        let mut t = ClientTable::new();
        t.insert(7, connecting(7));
        assert_eq!(t.client_of_player(100), None);
        t.insert(7, in_game(7, 100));
        assert_eq!(t.client_of_player(100), Some(7));
        // Back out of the world: the player id must not survive.
        t.insert(7, connecting(7));
        assert_eq!(t.client_of_player(100), None);
    }

    /// The relog race: the new connection registers before the dead one is
    /// torn down. Cleaning up the *old* client must not delete the *live*
    /// client's mapping, or the player becomes unreachable for the rest of
    /// their session.
    #[test]
    fn tearing_down_a_replaced_connection_keeps_the_live_mapping() {
        let mut t = ClientTable::new();
        t.insert(1, in_game(1, 100)); // original connection
        t.insert(2, in_game(2, 100)); // relog, same character
        assert_eq!(t.client_of_player(100), Some(2));

        t.remove(&1); // the old connection finally drops
        assert_eq!(
            t.client_of_player(100),
            Some(2),
            "the live connection keeps the mapping"
        );

        t.remove(&2);
        assert_eq!(t.client_of_player(100), None);
    }
}
