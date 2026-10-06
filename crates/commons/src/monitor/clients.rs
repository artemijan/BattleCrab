//! Live client listing — `docs/MONITORING.md` §10.
//!
//! Unlike samples, nothing here is buffered: a `clients` request on the
//! monitor channel asks the server, right then, for one [`ClientRecord`] per
//! open connection. Each server answers through the [`ClientsProvider`] it
//! installs with [`set_provider`]. The game server's provider is a request
//! into the game loop (the session table belongs to the game thread). The
//! login server's provider reads its own connection registry. A server that
//! installs none answers with an error line.
//!
//! What the connection task knows, and the game thread doesn't, is the
//! traffic. [`ConnectionStats`] is the connection's own counters: atomics the
//! connection task bumps next to the server-wide `packets_in`/`bytes_in`
//! counters, shared by `Arc` with whoever builds the record.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::epoch_ms;

/// One connection's traffic, counted where the server-wide counters are
/// (`docs/MONITORING.md` §2): frames off the wire before decrypt, and frames
/// written, header included.
#[derive(Debug)]
pub struct ConnectionStats {
    connected_ms: u64,
    packets_in: AtomicU64,
    bytes_in: AtomicU64,
    packets_out: AtomicU64,
    bytes_out: AtomicU64,
    /// `0` until the first inbound frame.
    last_in_ms: AtomicU64,
}

impl ConnectionStats {
    /// Stamped with the current time as the connection's start.
    pub fn new() -> Self {
        Self::started_at(epoch_ms())
    }

    pub fn started_at(connected_ms: u64) -> Self {
        Self {
            connected_ms,
            packets_in: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            packets_out: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            last_in_ms: AtomicU64::new(0),
        }
    }

    pub fn connected_ms(&self) -> u64 {
        self.connected_ms
    }

    /// One frame read off the socket, `wire_bytes` with its header.
    pub fn note_in(&self, wire_bytes: u64) {
        self.packets_in.fetch_add(1, Ordering::Relaxed);
        self.bytes_in.fetch_add(wire_bytes, Ordering::Relaxed);
        self.last_in_ms.store(epoch_ms(), Ordering::Relaxed);
    }

    /// One socket write carrying `packets` frames in `wire_bytes` bytes.
    pub fn note_out(&self, packets: u64, wire_bytes: u64) {
        self.packets_out.fetch_add(packets, Ordering::Relaxed);
        self.bytes_out.fetch_add(wire_bytes, Ordering::Relaxed);
    }

    pub fn traffic(&self) -> Traffic {
        let last = self.last_in_ms.load(Ordering::Relaxed);
        Traffic {
            packets_in: self.packets_in.load(Ordering::Relaxed),
            bytes_in: self.bytes_in.load(Ordering::Relaxed),
            packets_out: self.packets_out.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            last_packet_ms: (last != 0).then_some(last),
        }
    }
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Totals since the connection opened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    pub packets_in: u64,
    pub bytes_in: u64,
    pub packets_out: u64,
    pub bytes_out: u64,
    /// When the client last sent anything, epoch ms. `None` before its first
    /// frame.
    pub last_packet_ms: Option<u64>,
}

/// One open connection, as the dashboard's Audit page shows it.
///
/// camelCase on the wire, unlike a sample line: the dashboard hands records
/// to the browser as they are, so this is the one shape end to end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientRecord {
    /// Set by the channel from the server's logging name; whatever a
    /// provider puts here is overwritten.
    #[serde(default)]
    pub service: String,
    /// The server's own id for the connection, unique within one process
    /// lifetime. Paired with `service` and `connected_ms` it names one
    /// connection across polls.
    pub id: u64,
    pub ip: String,
    pub port: u16,
    pub connected_ms: u64,
    /// Lifecycle stage, one of the keys `docs/MONITORING.md` §10 lists
    /// (`handshaking`, `logged_in`, `joining_game`, `authenticating`,
    /// `lobby`, `entering`, `in_game`).
    pub stage: String,
    /// `None` until the client has authenticated.
    pub account: Option<String>,
    /// The character in (or entering) the world.
    pub character: Option<String>,
    /// The hardware id the server matches punishments on: the MAC address a
    /// client reports in `RequestHardWareInfo`. `None` when the client never
    /// sent one, which this chronicle's client doesn't unless a protection
    /// layer adds it.
    pub hwid: Option<String>,
    pub traffic: Traffic,
    /// Server-specific extras, shown as-is on the client's detail view.
    #[serde(default)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

/// The answer to one `clients` request.
pub type ClientsReply = tokio::sync::oneshot::Receiver<Vec<ClientRecord>>;

/// Starts building a client list. `None` when the server can't answer at all
/// (the game thread is gone).
pub type ClientsProvider = Box<dyn Fn() -> Option<ClientsReply> + Send + Sync>;

/// Where the channel looks for the provider. A slot rather than an argument to
/// [`super::spawn`] because the game server starts its monitor before the game
/// loop's channel exists.
#[derive(Default)]
pub struct ProviderSlot(OnceLock<ClientsProvider>);

impl ProviderSlot {
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    pub(crate) fn request(&self) -> Option<ClientsReply> {
        (self.0.get()?)()
    }

    fn set(&self, provider: ClientsProvider) -> bool {
        self.0.set(provider).is_ok()
    }

    #[cfg(test)]
    pub(crate) fn set_for_test(&self, provider: ClientsProvider) -> bool {
        self.set(provider)
    }
}

pub(crate) static PROVIDER: ProviderSlot = ProviderSlot::new();

/// Installs this process's client-list provider. The first call wins; a
/// second is a wiring bug and is logged, not obeyed.
pub fn set_provider(provider: impl Fn() -> Option<ClientsReply> + Send + Sync + 'static) {
    if !PROVIDER.set(Box::new(provider)) {
        tracing::warn!("monitor: a clients provider was already installed; ignoring another");
    }
}

/// A provider for a server that already has its list in hand.
pub fn ready(records: Vec<ClientRecord>) -> Option<ClientsReply> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = tx.send(records);
    Some(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_accumulates_and_stamps_the_last_inbound_frame() {
        let stats = ConnectionStats::started_at(1);
        assert_eq!(stats.traffic().last_packet_ms, None);
        stats.note_in(10);
        stats.note_in(5);
        stats.note_out(3, 40);
        let t = stats.traffic();
        assert_eq!(
            (t.packets_in, t.bytes_in, t.packets_out, t.bytes_out),
            (2, 15, 3, 40)
        );
        assert!(t.last_packet_ms.is_some_and(|ms| ms > 1));
    }

    #[test]
    fn a_record_round_trips_in_camel_case() {
        let record = ClientRecord {
            service: "game_server".into(),
            id: 7,
            ip: "10.0.0.1".into(),
            port: 51000,
            connected_ms: 1,
            stage: "in_game".into(),
            account: Some("acc".into()),
            character: Some("Hero".into()),
            hwid: None,
            traffic: Traffic::default(),
            details: serde_json::Map::new(),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["connectedMs"], 1);
        assert_eq!(json["traffic"]["packetsIn"], 0);
        assert_eq!(
            serde_json::from_value::<ClientRecord>(json).unwrap(),
            record
        );
    }
}
