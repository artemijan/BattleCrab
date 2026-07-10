//! Per-connection tokio task + the acceptor. Replaces Async-mmocore's AIO
//! read/write handlers (CONCURRENCY_MODEL §2.3).
//!
//! One task per socket owns the [`GameClient`] transport state (and thus the
//! cipher). It reads → decrypts → either answers the transport handshake
//! (`ProtocolVersion` → `KeyPacket`) itself, or forwards the decrypted body to
//! the game thread. Outbound packet bodies queued by the game thread are
//! encrypted and framed here.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use commons::network::{read_frame, write_frame};
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

use super::client_packets::{opcodes as cop, ProtocolVersion};
use super::game_client::GameClient;
use super::server_packets::key_packet;
use super::{ConnectionState, NetEvent, NetEventTx};

/// Frame payloads never exceed the 16-bit length header.
const MAX_PAYLOAD: usize = u16::MAX as usize;

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
    if net_tx.send(NetEvent::Connected { client_id, out: out_tx, addr }).is_err() {
        return Ok(()); // game thread gone
    }

    let mut client = GameClient::new(client_id, cfg.packet_encryption);

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
                    Some(mut body) => {
                        client.encrypt(&mut body);
                        if let Err(e) = write_frame(&mut write, &body).await {
                            break Err(e);
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
                warn!("Wrong protocol version {} from client {}", pv.version, client.client_id);
                client.protocol_ok = false;
                let key = client.enable_crypt();
                send(client, write, key_packet(key8(&key), 0, cfg.packet_encryption, cfg.server_id, cfg.is_classic))
                    .await?;
                return Ok(false); // Java: close(KeyPacket) → disconnect
            }
            client.protocol_version = pv.version;
            client.protocol_ok = true;
            let key = client.enable_crypt();
            send(client, write, key_packet(key8(&key), 1, cfg.packet_encryption, cfg.server_id, cfg.is_classic))
                .await?;
            info!("Client {} accepted protocol {}.", client.client_id, pv.version);
            Ok(true)
        }
        // Past the handshake: hand the decrypted body to the game thread.
        _ => {
            if net_tx.send(NetEvent::Received { client_id: client.client_id, data: body }).is_err() {
                return Ok(false);
            }
            Ok(true)
        }
    }
}

/// Encrypt (first call = pass-through) and frame one packet body.
async fn send<W: AsyncWrite + Unpin>(client: &mut GameClient, write: &mut W, mut body: Vec<u8>) -> std::io::Result<()> {
    client.encrypt(&mut body);
    write_frame(write, &body).await
}

fn key8(key: &[u8; 16]) -> &[u8; 8] {
    key[..8].try_into().unwrap()
}
