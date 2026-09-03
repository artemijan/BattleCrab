//! Per-connection tokio task + the acceptor. Replaces Async-mmocore's AIO
//! read/write handlers (CONCURRENCY_MODEL §2.3).
//!
//! One task per socket owns the [`GameClient`] transport state (and thus the
//! cipher). It reads → decrypts → either answers the transport handshake
//! (`ProtocolVersion` → `KeyPacket`) itself, or forwards the decrypted body to
//! the game thread. Outbound packet bodies queued by the game thread are
//! encrypted and framed here.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use commons::network::{HEADER_SIZE, read_frame, write_frame};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

use crate::config::SecurityConfig;

use super::client_packets::opcodes as cop;
use super::client_packets::session::ProtocolVersion;
use super::game_client::GameClient;
use super::server_packets::key_packet;
use super::{ConnectionState, NetEvent, NetEventTx};

/// Frame payloads never exceed the 16-bit length header.
const MAX_PAYLOAD: usize = u16::MAX as usize;

/// Stop coalescing outbound packets once the batch reaches this size and get
/// it on the wire; whatever is still queued rides the next wakeup. Bounds the
/// per-connection buffer against a pathological burst without capping the
/// common case (a tick's worth of packets is far below this).
const OUT_BATCH_SOFT_LIMIT: usize = 32 * 1024;

/// Capacity the outbound batch buffer is allowed to retain between wakeups.
/// Anything larger was a one-off burst and is released rather than held per
/// connection for the rest of the session.
const OUT_BATCH_KEEP_CAPACITY: usize = 8 * 1024;

/// The handshake-relevant config, resolved once from `ServerConfig`.
pub struct NetworkConfig {
    pub packet_encryption: bool,
    pub protocol_list: Vec<i32>,
    pub server_id: i32,
    pub is_classic: bool,
    /// `Security.ini` — the transport-level flood limits.
    pub security: SecurityConfig,
    /// `Network.ini` `DropPackets` — the outbound drop policy switch.
    pub drop_packets: bool,
    /// `Network.ini` `DropPacketThreshold` — outbound queue depth above which
    /// disposable packets are dropped.
    pub drop_packet_threshold: usize,
}

/// Inbound packets one connection may have *queued at the game thread* before
/// its socket stops being read (the read task blocks on a permit, so TCP flow
/// control pushes back to the client). Bounds what one connection can put in
/// flight during a game-thread stall to a constant, where the per-second rate
/// limit above alone would let the backlog grow for as long as the stall
/// lasts. Not config: like the frame-size limit, it is a structural bound far
/// above anything a real client queues (a tick drains the whole backlog), not
/// an operator tunable. Java has no equivalent — its unbounded instant pool
/// queue is one of the defects THREADING_MODEL §3 lists.
const MAX_PACKETS_IN_FLIGHT: usize = 256;

/// Java `FloodProtectedListener.ForeignConnection`: what one address is doing.
struct ForeignConnection {
    /// Live connections from this address.
    connection_number: i32,
    last_connection: Instant,
    /// Java's `isFlooding` latch — log the onset and the recovery, not each
    /// refused packet.
    is_flooding: bool,
}

/// Java: `ConnectionBuilder<>(addr, GameClient::new, GamePacketHandler, ...).build().start()`.
///
/// Plus the per-IP accept rules Java keeps in `FloodProtectedListener` (which
/// upstream wires only to the game-server↔login listener, never to players).
///
/// The per-address table is owned **solely by this task** and connections
/// report their close over a channel, rather than sharing a `Mutex`: the game
/// server deliberately holds exactly one lock process-wide
/// (`geo::GeoEngine::nswe_overrides`), which is the standing argument for not
/// needing Java's deadlock detector, and a second lock would cost that argument
/// for no benefit here.
pub async fn accept_loop(listener: TcpListener, net_tx: NetEventTx, cfg: Arc<NetworkConfig>) {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    let (closed_tx, mut closed_rx) = tokio::sync::mpsc::unbounded_channel::<IpAddr>();
    let mut per_ip: HashMap<IpAddr, ForeignConnection> = HashMap::new();

    loop {
        tokio::select! {
            // Java `removeFloodProtection`: drop the address once its last
            // connection is gone, so the table tracks live clients only.
            Some(ip) = closed_rx.recv() => {
                if let Some(entry) = per_ip.get_mut(&ip) {
                    entry.connection_number -= 1;
                    if entry.connection_number <= 0 {
                        per_ip.remove(&ip);
                    }
                }
            }
            accepted = listener.accept() => match accepted {
                Ok((stream, addr)) => {
                    if cfg.security.enable_connection_flood_protection
                        && refuse_connection(&mut per_ip, addr.ip(), &cfg.security, Instant::now())
                    {
                        // Dropping the stream closes it, as Java's
                        // `connection.close()` does.
                        continue;
                    }
                    let client_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                    let net_tx = net_tx.clone();
                    let cfg = cfg.clone();
                    let closed_tx = closed_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle(stream, addr, client_id, net_tx, cfg).await {
                            debug!("client {client_id} ({addr}) ended: {e}");
                        }
                        let _ = closed_tx.send(addr.ip());
                    });
                }
                Err(e) => {
                    warn!("GameServer: accept error: {e}");
                }
            },
        }
    }
}

/// Java `FloodProtectedListener.run`'s accept test, verbatim in structure:
/// count the new connection first, then refuse when *any* of the three rules
/// trips. Returns `true` when the connection must be dropped.
fn refuse_connection(
    per_ip: &mut HashMap<IpAddr, ForeignConnection>,
    ip: IpAddr,
    cfg: &SecurityConfig,
    now: Instant,
) -> bool {
    let Some(entry) = per_ip.get_mut(&ip) else {
        per_ip.insert(
            ip,
            ForeignConnection {
                connection_number: 1,
                last_connection: now,
                is_flooding: false,
            },
        );
        return false;
    };

    entry.connection_number += 1;
    let since_last = now.duration_since(entry.last_connection);
    let too_many_too_fast = entry.connection_number > cfg.fast_connection_limit
        && since_last < Duration::from_millis(cfg.normal_connection_time.max(0) as u64);
    let too_fast = since_last < Duration::from_millis(cfg.fast_connection_time.max(0) as u64);
    let too_many = entry.connection_number > cfg.max_connections_per_ip;

    if too_many_too_fast || too_fast || too_many {
        // Java updates `lastConnection` on the *refused* attempt too, so a
        // client hammering the port keeps pushing its own window out.
        entry.last_connection = now;
        entry.connection_number -= 1;
        if !entry.is_flooding {
            warn!("GameServer: potential connection flood from {ip}");
        }
        entry.is_flooding = true;
        return true;
    }

    if entry.is_flooding {
        entry.is_flooding = false;
        info!("GameServer: {ip} is not considered as flooding anymore.");
    }
    entry.last_connection = now;
    false
}

/// A fixed one-second window of inbound packets for one connection.
///
/// The game thread's inbound queue is unbounded, so without this a single
/// socket can put an unbounded amount of work in flight between two 100 ms
/// ticks. This is a transport backstop — per-*action* limits are
/// `FloodProtector.ini`'s job — so it only trips far above any real client,
/// and it closes the connection rather than silently dropping packets, which
/// would desynchronise the client's view of its own requests.
struct PacketRateLimiter {
    window_start: Instant,
    count: u32,
    max_per_second: u32,
}

impl PacketRateLimiter {
    fn new(max_per_second: u32) -> Self {
        Self {
            window_start: Instant::now(),
            count: 0,
            max_per_second,
        }
    }

    /// `true` when this packet is over the limit and the connection should die.
    fn is_over_limit(&mut self, now: Instant) -> bool {
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.count = 0;
        }
        self.count += 1;
        self.count > self.max_per_second
    }
}

async fn handle(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    client_id: u32,
    net_tx: NetEventTx,
    cfg: Arc<NetworkConfig>,
) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let (mut read, mut write) = stream.into_split();

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<bytes::Bytes>();
    let out = super::OutboundTx::new(out_tx, cfg.drop_packets, cfg.drop_packet_threshold);
    // The decrement side of the queue-depth estimate; the game thread's sends
    // increment it inside `OutboundTx::send`.
    let out_depth = out.depth_handle();
    // The inbound in-flight bound (`MAX_PACKETS_IN_FLIGHT`): each forwarded
    // packet carries a permit the game thread releases by dropping the event.
    let in_flight = Arc::new(tokio::sync::Semaphore::new(MAX_PACKETS_IN_FLIGHT));
    // Tell the game thread this client exists (registry, later broadcast).
    if net_tx
        .send(NetEvent::Connected {
            client_id,
            out,
            addr,
        })
        .is_err()
    {
        return Ok(()); // game thread gone
    }

    let mut client = GameClient::new(client_id, cfg.packet_encryption);
    // Reused across wakeups so the steady state allocates nothing to send.
    let mut out_batch: Vec<u8> = Vec::with_capacity(OUT_BATCH_KEEP_CAPACITY);
    let mut rate = cfg
        .security
        .enable_packet_rate_limit
        .then(|| PacketRateLimiter::new(cfg.security.max_packets_per_second));

    let result = loop {
        tokio::select! {
            frame = read_frame(&mut read, MAX_PAYLOAD) => {
                match frame {
                    Ok(Some(payload)) => {
                        // Charged before decryption: the cost this bounds is
                        // the work the frame is about to create, and a frame
                        // that fails to decode has already consumed a read.
                        if let Some(rate) = rate.as_mut()
                            && rate.is_over_limit(Instant::now())
                        {
                            warn!(
                                "GameServer: client {client_id} ({addr}) exceeded \
                                 {} packets/s — closing.",
                                cfg.security.max_packets_per_second
                            );
                            break Ok(());
                        }
                        let mut body = payload;
                        client.decrypt(&mut body);
                        if body.is_empty() {
                            continue;
                        }
                        // Errors break the loop (not `?`) so the Disconnected
                        // cleanup below always runs.
                        match on_packet(&mut client, &mut write, &net_tx, &cfg, &in_flight, body).await {
                            Ok(true) => {}
                            Ok(false) => break Ok(()), // clean close requested by a handler
                            Err(e) => break Err(e),
                        }
                    }
                    Ok(None) => break Ok(()),          // clean EOF
                    Err(e) => break Err(e),            // framing/IO error
                }
            }
            out = out_rx.recv() => {
                match out {
                    Some(first) => {
                        out_depth.fetch_sub(1, Ordering::Relaxed);
                        // Coalesce every packet already queued into one write.
                        // A game tick hands this task a burst (CharInfo +
                        // StatusUpdate + MoveToLocation + …), and with
                        // TCP_NODELAY each `write_frame` would be its own
                        // syscall and its own TCP segment. Draining with
                        // `try_recv` costs nothing when only one is waiting.
                        //
                        // Encryption stays strictly in queue order: the cipher
                        // rolls its key forward by the bytes processed, so the
                        // client's decrypt only lines up if we encrypt each
                        // body in the order it was sent.
                        out_batch.clear();
                        let mut body = first;
                        loop {
                            // The packet arrives as shared, immutable `Bytes`
                            // (one broadcast, many recipients), so it is copied
                            // into the batch and encrypted *there* — the cipher
                            // needs a mutable buffer, and this is the one copy
                            // that has to exist. It happens here, on a tokio
                            // worker, rather than on the game thread.
                            let header_at = out_batch.len();
                            out_batch.extend_from_slice(&[0, 0]);
                            let body_at = out_batch.len();
                            out_batch.extend_from_slice(&body);
                            client.encrypt(&mut out_batch[body_at..]);
                            let total = (body.len() + HEADER_SIZE) as u16;
                            out_batch[header_at..body_at]
                                .copy_from_slice(&total.to_le_bytes());
                            // Bound the buffer so a pathological burst can't
                            // grow it without limit — flush and come back.
                            if out_batch.len() >= OUT_BATCH_SOFT_LIMIT {
                                break;
                            }
                            match out_rx.try_recv() {
                                Ok(next) => {
                                    out_depth.fetch_sub(1, Ordering::Relaxed);
                                    body = next;
                                }
                                Err(_) => break,
                            }
                        }
                        if let Err(e) = write.write_all(&out_batch).await {
                            break Err(e);
                        }
                        if let Err(e) = write.flush().await {
                            break Err(e);
                        }
                        // Keep the steady-state buffer small: only a burst
                        // should hold a large allocation, and only until the
                        // next one.
                        if out_batch.capacity() > OUT_BATCH_KEEP_CAPACITY {
                            out_batch = Vec::with_capacity(OUT_BATCH_KEEP_CAPACITY);
                        }
                    }
                    None => break Ok(()),              // all senders dropped
                }
            }
        }
    };

    let _ = net_tx.send(NetEvent::Disconnected { client_id });
    result
}

/// Dispatch one decrypted packet body (opcode + payload). Returns `Ok(false)`
/// when the connection should close. Mirrors `GamePacketHandler` for the
/// transport handshake; everything else is forwarded to the game thread.
async fn on_packet<W: AsyncWrite + Unpin>(
    client: &mut GameClient,
    write: &mut W,
    net_tx: &NetEventTx,
    cfg: &NetworkConfig,
    in_flight: &Arc<tokio::sync::Semaphore>,
    body: Vec<u8>,
) -> std::io::Result<bool> {
    let opcode = body[0];
    match (client.state, opcode) {
        // Port of clientpackets/ProtocolVersion.runImpl (never encrypted).
        (ConnectionState::Connected, cop::PROTOCOL_VERSION) => {
            let pv = ProtocolVersion::read(&body[1..]);
            if pv.version == -2 {
                // Ping attempt from the new C2 client — just disconnect.
                return Ok(false);
            }
            if !cfg.protocol_list.contains(&pv.version) {
                warn!(
                    "Wrong protocol version {} from client {}",
                    pv.version, client.client_id
                );
                client.protocol_ok = false;
                let key = client.enable_crypt();
                send(
                    client,
                    write,
                    key_packet(
                        key8(&key),
                        0,
                        cfg.packet_encryption,
                        cfg.server_id,
                        cfg.is_classic,
                    ),
                )
                .await?;
                return Ok(false); // Java: close(KeyPacket) → disconnect
            }
            client.protocol_version = pv.version;
            client.protocol_ok = true;
            let key = client.enable_crypt();
            send(
                client,
                write,
                key_packet(
                    key8(&key),
                    1,
                    cfg.packet_encryption,
                    cfg.server_id,
                    cfg.is_classic,
                ),
            )
            .await?;
            info!(
                "Client {} accepted protocol {}.",
                client.client_id, pv.version
            );
            Ok(true)
        }
        // Past the handshake: hand the decrypted body to the game thread.
        _ => {
            // Waits when `MAX_PACKETS_IN_FLIGHT` packets are already queued —
            // which stops this task reading the socket, so the client is
            // backpressured by TCP instead of growing the game thread's
            // queue. The permit rides the event; the game thread releases it
            // by dropping the handled event.
            let permit = in_flight
                .clone()
                .acquire_owned()
                .await
                .expect("the in-flight semaphore is never closed");
            if net_tx
                .send(NetEvent::Received {
                    client_id: client.client_id,
                    data: body,
                    permit,
                })
                .is_err()
            {
                return Ok(false);
            }
            Ok(true)
        }
    }
}

/// Encrypt (first call = pass-through) and frame one packet body.
async fn send<W: AsyncWrite + Unpin>(
    client: &mut GameClient,
    write: &mut W,
    mut body: Vec<u8>,
) -> std::io::Result<()> {
    client.encrypt(&mut body);
    write_frame(write, &body).await
}

fn key8(key: &[u8; 16]) -> &[u8; 8] {
    key[..8].try_into().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, last])
    }

    fn cfg() -> SecurityConfig {
        SecurityConfig::default()
    }

    /// A first connection from an unseen address is always allowed, whatever
    /// the rules say — there is nothing to compare it against.
    #[test]
    fn the_first_connection_from_an_address_is_allowed() {
        let mut per_ip = HashMap::new();
        let t = Instant::now();
        assert!(!refuse_connection(&mut per_ip, ip(1), &cfg(), t));
        assert_eq!(per_ip[&ip(1)].connection_number, 1);
    }

    /// `FastConnectionTime`: two connections closer together than 350 ms are
    /// refused outright, and the refusal does not leak a connection slot.
    #[test]
    fn a_second_connection_within_the_fast_window_is_refused() {
        let mut per_ip = HashMap::new();
        let t = Instant::now();
        assert!(!refuse_connection(&mut per_ip, ip(1), &cfg(), t));
        assert!(refuse_connection(
            &mut per_ip,
            ip(1),
            &cfg(),
            t + Duration::from_millis(100)
        ));
        assert_eq!(
            per_ip[&ip(1)].connection_number,
            1,
            "a refused attempt must not count as a live connection"
        );
        // Past the window it is allowed again.
        assert!(!refuse_connection(
            &mut per_ip,
            ip(1),
            &cfg(),
            t + Duration::from_millis(1_000)
        ));
        assert_eq!(per_ip[&ip(1)].connection_number, 2);
    }

    /// `MaxConnectionsPerIP` is the hard ceiling: even perfectly paced
    /// connections stop at the cap.
    #[test]
    fn the_per_ip_ceiling_holds_however_slowly_they_arrive() {
        let mut per_ip = HashMap::new();
        let cfg = cfg();
        let mut t = Instant::now();
        for i in 0..cfg.max_connections_per_ip {
            assert!(
                !refuse_connection(&mut per_ip, ip(1), &cfg, t),
                "connection {i} is under the cap"
            );
            t += Duration::from_secs(10);
        }
        assert!(
            refuse_connection(&mut per_ip, ip(1), &cfg, t),
            "the 51st live connection is refused"
        );
    }

    /// `FastConnectionLimit` + `NormalConnectionTime`: past 15 live
    /// connections, an address must also slow down to one per 700 ms.
    #[test]
    fn past_the_fast_limit_the_pace_requirement_tightens() {
        let mut per_ip = HashMap::new();
        let cfg = cfg();
        let mut t = Instant::now();
        // Build up to exactly the fast limit, paced past the *fast* window
        // (350 ms) but inside the *normal* one (700 ms).
        for _ in 0..cfg.fast_connection_limit {
            assert!(!refuse_connection(&mut per_ip, ip(2), &cfg, t));
            t += Duration::from_millis(400);
        }
        assert!(
            refuse_connection(&mut per_ip, ip(2), &cfg, t),
            "over the fast limit, 400 ms apart is no longer fast enough"
        );
        assert!(
            !refuse_connection(&mut per_ip, ip(2), &cfg, t + Duration::from_millis(800)),
            "…but 800 ms apart is"
        );
    }

    /// Addresses are independent — one flooder must not lock anyone else out.
    #[test]
    fn addresses_do_not_share_a_budget() {
        let mut per_ip = HashMap::new();
        let t = Instant::now();
        assert!(!refuse_connection(&mut per_ip, ip(1), &cfg(), t));
        assert!(refuse_connection(&mut per_ip, ip(1), &cfg(), t));
        assert!(
            !refuse_connection(&mut per_ip, ip(2), &cfg(), t),
            "a different address is unaffected"
        );
    }

    /// The packet limiter counts within a one-second window and resets after.
    #[test]
    fn the_packet_rate_limiter_trips_only_over_the_limit() {
        let mut rl = PacketRateLimiter::new(3);
        let t = rl.window_start;
        assert!(!rl.is_over_limit(t));
        assert!(!rl.is_over_limit(t));
        assert!(!rl.is_over_limit(t), "the third is still at the limit");
        assert!(rl.is_over_limit(t), "the fourth is over");
        assert!(
            !rl.is_over_limit(t + Duration::from_secs(1)),
            "a new window starts clean"
        );
    }
}
