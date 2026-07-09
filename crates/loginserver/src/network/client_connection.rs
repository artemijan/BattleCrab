//! Per-client connection task: the LoginClient lifecycle
//! (`LoginClient.java` + `ReadHandler` dispatch + `LoginClientPackets` state
//! checks), sequential per connection.

use std::sync::Arc;
use std::time::Duration;

use commons::crypt::ScrambledKeyPair;
use commons::network::{read_frame, write_frame, PacketReader};
use commons::util::rnd;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tracing::{debug, info};

use crate::context::LoginContext;
use crate::enums::{ConnectionState, LoginFailReason};
use crate::network::encryption::LoginEncryption;
use crate::network::server_packets;

/// `LoginController.LOGIN_TIMEOUT` — 5 minutes.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_PAYLOAD: usize = 8192;

pub struct ClientSession {
    pub session_id: i32,
    pub keypair: Arc<ScrambledKeyPair>,
    pub blowfish_key: [u8; 16],
    pub state: ConnectionState,
    pub ip: String,
    pub account: Option<String>,
}

pub async fn handle(ctx: Arc<LoginContext>, stream: TcpStream, ip: String) {
    let (mut read, mut write) = stream.into_split();

    let mut session = ClientSession {
        session_id: rnd::next_int(),
        keypair: ctx.random_keypair(),
        blowfish_key: ctx.random_blowfish_key(),
        state: ConnectionState::Connected,
        ip,
        account: None,
    };
    let mut encryption = LoginEncryption::new(&session.blowfish_key);

    // onConnected: send Init (goes out under the static key + XOR pass).
    let init = server_packets::init(session.session_id, session.keypair.scrambled_modulus(), &session.blowfish_key);
    if send(&mut write, &mut encryption, init).await.is_err() {
        return;
    }

    loop {
        let frame = tokio::time::timeout(LOGIN_TIMEOUT, read_frame(&mut read, MAX_PAYLOAD)).await;
        let mut payload = match frame {
            Ok(Ok(Some(payload))) => payload,
            _ => break, // EOF, IO error, or login timeout
        };

        if !encryption.decrypt(&mut payload) {
            debug!("Checksum/decrypt failure from {}", session.ip);
            break;
        }

        match dispatch(&ctx, &mut session, &payload, &mut write, &mut encryption).await {
            Ok(true) => {}
            _ => break,
        }
    }

    debug!("Client {} disconnected", session.ip);
}

/// Returns Ok(false) to close the connection; unknown packets are ignored
/// like Java's null from `handlePacket`.
async fn dispatch(
    _ctx: &LoginContext,
    session: &mut ClientSession,
    payload: &[u8],
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
) -> std::io::Result<bool> {
    let mut r = PacketReader::new(payload);
    let Some(opcode) = r.read_u8() else {
        return Ok(false);
    };

    match (session.state, opcode) {
        // AUTH_GAME_GUARD(0x07, ConnectionState.CONNECTED)
        (ConnectionState::Connected, 0x07) => {
            if r.remaining() < 20 {
                return Ok(false);
            }
            let session_id = r.read_i32().unwrap();
            if session_id == session.session_id {
                session.state = ConnectionState::AuthedGg;
                send(write, encryption, server_packets::gg_auth(session.session_id)).await?;
                Ok(true)
            } else {
                close(write, encryption, LoginFailReason::ReasonAccessFailed).await
            }
        }
        // REQUEST_AUTH_LOGIN(0x00, ConnectionState.AUTHED_GG) — M3.
        (ConnectionState::AuthedGg, 0x00) => {
            info!("RequestAuthLogin from {} — authentication lands in M3", session.ip);
            close(write, encryption, LoginFailReason::ReasonSystemErrorLoginLater).await
        }
        _ => {
            debug!("Ignored packet 0x{opcode:02x} in state {:?} from {}", session.state, session.ip);
            Ok(true)
        }
    }
}

pub async fn send(
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
    body: Vec<u8>,
) -> std::io::Result<()> {
    let encrypted = encryption.encrypt(body);
    write_frame(write, &encrypted).await
}

/// Java `close(LoginFailReason)`: send the failure packet, then disconnect.
async fn close(
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
    reason: LoginFailReason,
) -> std::io::Result<bool> {
    send(write, encryption, server_packets::login_fail(reason)).await?;
    Ok(false)
}

pub async fn accept_loop(ctx: Arc<LoginContext>, listener: tokio::net::TcpListener) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let _ = stream.set_nodelay(true); // Java: TCP_NODELAY unless UseNagle
                let ctx = ctx.clone();
                tokio::spawn(handle(ctx, stream, addr.ip().to_string()));
            }
            Err(e) => {
                debug!("accept failed: {e}");
            }
        }
    }
}
