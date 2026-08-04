//! Per-connection tokio task + the acceptor. Replaces Async-mmocore's AIO
//! read/write handlers (CONCURRENCY_MODEL §2.3).
//!
//! One task per socket owns the [`GameClient`] transport state (and thus the
//! cipher). It reads → decrypts → either answers the transport handshake
//! (`ProtocolVersion` → `KeyPacket`) itself, or forwards the decrypted body to
//! the game thread. Outbound packet bodies queued by the game thread are
//! encrypted and framed here.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use commons::network::{frame_into, read_frame, write_frame};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

use super::client_packets::{ProtocolVersion, opcodes as cop};
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
}

/// Java: `ConnectionBuilder<>(addr, GameClient::new, GamePacketHandler, ...).build().start()`.
pub async fn accept_loop(listener: TcpListener, net_tx: NetEventTx, cfg: Arc<NetworkConfig>) {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let client_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                let net_tx = net_tx.clone();
                let cfg = cfg.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle(stream, addr, client_id, net_tx, cfg).await {
                        debug!("client {client_id} ({addr}) ended: {e}");
                    }
                });
            }
            Err(e) => {
                warn!("GameServer: accept error: {e}");
            }
        }
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

    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    // Tell the game thread this client exists (registry, later broadcast).
    if net_tx
        .send(NetEvent::Connected {
            client_id,
            out: out_tx,
            addr,
        })
        .is_err()
    {
        return Ok(()); // game thread gone
    }

    let mut client = GameClient::new(client_id, cfg.packet_encryption);
    // Reused across wakeups so the steady state allocates nothing to send.
    let mut out_batch: Vec<u8> = Vec::with_capacity(OUT_BATCH_KEEP_CAPACITY);

    let result = loop {
        tokio::select! {
            frame = read_frame(&mut read, MAX_PAYLOAD) => {
                match frame {
                    Ok(Some(payload)) => {
                        let mut body = payload;
                        client.decrypt(&mut body);
                        if body.is_empty() {
                            continue;
                        }
                        // Errors break the loop (not `?`) so the Disconnected
                        // cleanup below always runs.
                        match on_packet(&mut client, &mut write, &net_tx, &cfg, body).await {
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
                            client.encrypt(&mut body);
                            frame_into(&mut out_batch, &body);
                            // Bound the buffer so a pathological burst can't
                            // grow it without limit — flush and come back.
                            if out_batch.len() >= OUT_BATCH_SOFT_LIMIT {
                                break;
                            }
                            match out_rx.try_recv() {
                                Ok(next) => body = next,
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
            if net_tx
                .send(NetEvent::Received {
                    client_id: client.client_id,
                    data: body,
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
