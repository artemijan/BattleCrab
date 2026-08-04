//! G1 integration test: the transport handshake (`ProtocolVersion` → `KeyPacket`)
//! and a live encrypted round-trip through the connection task.
//!
//! Spins up the real `accept_loop`, drives it with a raw TCP client, and checks
//! the `KeyPacket` layout, then that a packet encrypted by the client's cipher
//! is decrypted and forwarded to the game thread byte-for-byte.

use std::sync::Arc;
use std::time::Duration;

use commons::network::{PacketReader, PacketWriter, read_frame, write_frame};
use gameserver::network::NetEvent;
use gameserver::network::cipher::Encryption;
use gameserver::network::connection::{NetworkConfig, accept_loop};
use tokio::net::TcpStream;

const PROTOCOL: i32 = 110;

async fn start_server(
    cfg: NetworkConfig,
) -> (std::net::SocketAddr, std::sync::mpsc::Receiver<NetEvent>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (net_tx, net_rx) = std::sync::mpsc::channel::<NetEvent>();
    tokio::spawn(accept_loop(listener, net_tx, Arc::new(cfg)));
    (addr, net_rx)
}

fn cfg() -> NetworkConfig {
    NetworkConfig {
        packet_encryption: true,
        protocol_list: vec![PROTOCOL],
        server_id: 1,
        is_classic: true,
        // The shipped `Security.ini` defaults, so the transport limits are
        // exercised by the end-to-end tests rather than bypassed by them.
        security: gameserver::config::SecurityConfig::default(),
    }
}

fn protocol_version_body(version: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x0E); // PROTOCOL_VERSION opcode
    w.write_i32(version);
    w.into_bytes()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepts_protocol_and_sends_keypacket() {
    let (addr, net_rx) = start_server(cfg()).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();

    write_frame(&mut stream, &protocol_version_body(PROTOCOL))
        .await
        .unwrap();
    let reply = read_frame(&mut stream, 65535)
        .await
        .unwrap()
        .expect("KeyPacket");

    // KeyPacket is the server's first packet → sent in the clear (cipher pass-through).
    let mut r = PacketReader::new(&reply);
    assert_eq!(r.read_u8().unwrap(), 0x2E, "VERSION_CHECK opcode");
    assert_eq!(r.read_u8().unwrap(), 1, "result = protocol ok");
    let mut key8 = [0u8; 8];
    for b in &mut key8 {
        *b = r.read_u8().unwrap();
    }
    assert_eq!(r.read_i32().unwrap(), 1, "packet encryption flag");
    assert_eq!(r.read_i32().unwrap(), 1, "server id");
    assert_eq!(r.read_u8().unwrap(), 1);
    assert_eq!(r.read_i32().unwrap(), 0, "obfuscation key");
    assert_eq!(r.read_u8().unwrap(), 1, "isClassic");

    // The game thread must have been told the client connected.
    let ev = net_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(ev, NetEvent::Connected { .. }));

    // Now drive an encrypted packet the way the real client would. The client
    // encrypts its first real packet (AuthLogin) — the KeyPacket-first
    // pass-through is a server-only quirk. Model that with an enabled cipher and
    // a fresh out-key (matching the server's fresh in-key on its first decrypt).
    let key = Encryption::key_from_random(&key8);
    let mut client = Encryption::new();
    client.set_key(&key);
    client.encrypt(&mut Vec::new()); // consume the pass-through (no key shift)

    let mut w = PacketWriter::new();
    w.write_u8(0x2B); // AUTH_LOGIN opcode (unhandled in G1, just forwarded)
    w.write_i32(0xCAFE);
    let plain = w.into_bytes();
    let mut wire = plain.clone();
    client.encrypt(&mut wire);
    assert_ne!(wire, plain, "client body must be encrypted");
    write_frame(&mut stream, &wire).await.unwrap();

    // The server must decrypt it back to the original bytes and forward it.
    let ev = net_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    match ev {
        NetEvent::Received { data, .. } => {
            assert_eq!(data, plain, "server decrypt must match client plaintext")
        }
        _ => panic!("expected Received"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_wrong_protocol_and_closes() {
    let (addr, _net_rx) = start_server(cfg()).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();

    write_frame(&mut stream, &protocol_version_body(999))
        .await
        .unwrap();
    let reply = read_frame(&mut stream, 65535)
        .await
        .unwrap()
        .expect("KeyPacket");
    let mut r = PacketReader::new(&reply);
    assert_eq!(r.read_u8().unwrap(), 0x2E);
    assert_eq!(r.read_u8().unwrap(), 0, "result = wrong protocol");

    // Server closes after the rejection: the next read hits EOF.
    let eof = read_frame(&mut stream, 65535).await.unwrap();
    assert!(
        eof.is_none(),
        "connection should be closed after wrong protocol"
    );
}
