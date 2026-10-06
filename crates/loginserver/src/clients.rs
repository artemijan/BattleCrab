//! The login server's live connections, for the monitor channel's `clients`
//! request (`docs/MONITORING.md` §10).
//!
//! Each connection task holds a [`Registration`]: it puts the connection in
//! the registry and takes it out again on drop, so a task that ends any way at
//! all, panics included, leaves no stale row (the same reasoning as
//! `metrics::LoginStage`). The task updates its row as the login progresses,
//! and watches it ([`Conn::kicked`]) for the dashboard's disconnect ([`kick`]).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use commons::monitor::clients::{ClientRecord, ConnectionStats};
use serde_json::{Map, json};

static REGISTRY: Mutex<BTreeMap<u64, Arc<Entry>>> = Mutex::new(BTreeMap::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    /// The connection whose task is running, so `network::client_connection::send`
    /// can count outbound frames without every call site passing it along.
    static CURRENT: Arc<Entry>;
}

#[derive(Debug)]
struct Entry {
    id: u64,
    addr: SocketAddr,
    stats: ConnectionStats,
    info: Mutex<Info>,
    /// Signalled by [`kick`]. A `Notify` keeps the permit when the task is
    /// not waiting yet, so a kick that lands mid-packet is still seen.
    kick: tokio::sync::Notify,
}

#[derive(Debug)]
struct Info {
    stage: &'static str,
    account: Option<String>,
    access_level: i32,
    last_server: i32,
    /// The game server the client was sent to, once it picked one.
    server_id: Option<i32>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// One connection's row in the registry, removed when this drops. Held by
/// the connection task for its whole life; the session updates the row
/// through a [`Conn`].
#[derive(Debug)]
pub struct Registration(Arc<Entry>);

/// A handle on a registered connection's row. Updating it after the
/// registration dropped is harmless: the row is no longer listed.
#[derive(Debug, Clone)]
pub struct Conn(Arc<Entry>);

impl Registration {
    pub fn new(addr: SocketAddr) -> Self {
        let entry = Arc::new(Entry {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            addr,
            stats: ConnectionStats::new(),
            info: Mutex::new(Info {
                stage: "handshaking",
                account: None,
                access_level: 0,
                last_server: 0,
                server_id: None,
            }),
            kick: tokio::sync::Notify::new(),
        });
        lock(&REGISTRY).insert(entry.id, entry.clone());
        Self(entry)
    }

    /// Runs `task` as this connection: outbound frames sent inside it are
    /// counted against this row.
    pub async fn scope<F: Future>(&self, task: F) -> F::Output {
        CURRENT.scope(self.0.clone(), task).await
    }

    pub fn conn(&self) -> Conn {
        Conn(self.0.clone())
    }
}

impl Conn {
    pub fn stats(&self) -> &ConnectionStats {
        &self.0.stats
    }

    /// Resolves once the dashboard asked for this connection to be closed.
    pub async fn kicked(&self) {
        self.0.kick.notified().await
    }

    /// Past `RequestAuthLogin`: at the server list.
    pub fn logged_in(&self, account: &str, access_level: i32, last_server: i32) {
        let mut info = lock(&self.0.info);
        info.stage = "logged_in";
        info.account = Some(account.to_string());
        info.access_level = access_level;
        info.last_server = last_server;
    }

    /// `PlayOk` sent: the client is leaving for a game server.
    pub fn joining_game(&self, server_id: i32) {
        let mut info = lock(&self.0.info);
        info.stage = "joining_game";
        info.server_id = Some(server_id);
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        lock(&REGISTRY).remove(&self.0.id);
    }
}

/// One frame written by the current connection task, header included.
/// Outside a connection task (tests, the GS link) it counts nothing.
pub fn note_outbound(wire_bytes: u64) {
    let _ = CURRENT.try_with(|entry| entry.stats.note_out(1, wire_bytes));
}

/// Asks connection `id` to close, if it is the one that opened at
/// `connected_ms`. Its task sends the client a login failure and hangs up.
pub fn kick(id: u64, connected_ms: u64) -> bool {
    let entry = lock(&REGISTRY).get(&id).cloned();
    match entry {
        Some(entry) if entry.stats.connected_ms() == connected_ms => {
            entry.kick.notify_one();
            true
        }
        _ => false,
    }
}

/// Every open connection, oldest first.
pub fn records() -> Vec<ClientRecord> {
    let entries: Vec<Arc<Entry>> = lock(&REGISTRY).values().cloned().collect();
    entries.iter().map(|e| record(e)).collect()
}

fn record(entry: &Entry) -> ClientRecord {
    let info = lock(&entry.info);
    let mut details = Map::new();
    if info.account.is_some() {
        details.insert("accessLevel".into(), json!(info.access_level));
        details.insert("lastServer".into(), json!(info.last_server));
    }
    if let Some(server_id) = info.server_id {
        details.insert("joiningServer".into(), json!(server_id));
    }
    ClientRecord {
        service: String::new(),
        id: entry.id,
        ip: entry.addr.ip().to_string(),
        port: entry.addr.port(),
        connected_ms: entry.stats.connected_ms(),
        stage: info.stage.to_string(),
        account: info.account.clone(),
        character: None,
        // The login protocol carries no hardware fingerprint.
        hwid: None,
        traffic: entry.stats.traffic(),
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([10, 0, 0, 1], port))
    }

    fn find(id: u64) -> Option<ClientRecord> {
        records().into_iter().find(|r| r.id == id)
    }

    #[tokio::test]
    async fn a_connection_is_listed_while_registered_and_tracks_its_login() {
        let reg = Registration::new(addr(50001));
        let id = reg.0.id;
        assert_eq!(find(id).unwrap().stage, "handshaking");

        reg.scope(async { note_outbound(42) }).await;
        // Outside the scope nothing is attributed to it.
        note_outbound(1000);
        let conn = reg.conn();
        conn.logged_in("alice", 0, 2);

        let r = find(id).unwrap();
        assert_eq!(
            (r.stage.as_str(), r.account.as_deref(), r.port),
            ("logged_in", Some("alice"), 50001)
        );
        assert_eq!((r.traffic.packets_out, r.traffic.bytes_out), (1, 42));
        assert_eq!(r.details["lastServer"], 2);

        conn.joining_game(1);
        assert_eq!(find(id).unwrap().stage, "joining_game");

        // A kick for another connection's start is refused; the right one is
        // remembered until the task next waits for it.
        let started = r.connected_ms;
        assert!(!kick(id, started + 1));
        assert!(kick(id, started));
        tokio::time::timeout(std::time::Duration::from_secs(1), conn.kicked())
            .await
            .expect("the kick was not delivered");

        drop(reg);
        assert!(find(id).is_none(), "dropping the registration unlists it");
        assert!(!kick(id, started), "nothing to kick once it is gone");
    }
}
