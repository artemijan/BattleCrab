//! Per-client connection task: the LoginClient lifecycle
//! (`LoginClient.java` + `ReadHandler` dispatch + `LoginClientPackets` state
//! checks), sequential per connection. Kicks (account-in-use) arrive over a
//! channel the read loop selects on.

use std::sync::Arc;
use std::time::Duration;

use commons::crypt::ScrambledKeyPair;
use commons::network::{read_frame, write_frame, PacketReader};
use commons::util::rnd;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::context::LoginContext;
use crate::controller::AuthOutcome;
use crate::enums::{AccountKickedReason, ConnectionState, LoginFailReason, PlayFailReason};
use crate::network::encryption::LoginEncryption;
use crate::network::server_packets;
use crate::session::SessionKey;

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
    pub session_key: Option<SessionKey>,
    pub access_level: i32,
    pub last_server: i32,
    pub joined_gs: bool,
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
        session_key: None,
        access_level: 0,
        last_server: 1,
        joined_gs: false,
    };
    let mut encryption = LoginEncryption::new(&session.blowfish_key);
    // LoginController.purge: the whole login session may last at most
    // LOGIN_TIMEOUT before the client is dropped.
    let session_deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;

    // LoginClient constructor: banned address → LoginFail(REASON_NOT_AUTHED),
    // no Init.
    if ctx.controller.is_banned(&session.ip).await {
        let _ = send(&mut write, &mut encryption, server_packets::login_fail(LoginFailReason::ReasonNotAuthed)).await;
        return;
    }

    // Kick channel: the controller uses it when another login steals the account.
    let (kick_tx, mut kick_rx) = mpsc::channel::<LoginFailReason>(1);

    // onConnected: send Init (goes out under the static key + XOR pass).
    let init = server_packets::init(session.session_id, session.keypair.scrambled_modulus(), &session.blowfish_key);
    if send(&mut write, &mut encryption, init).await.is_err() {
        return;
    }

    loop {
        let mut payload = tokio::select! {
            frame = tokio::time::timeout_at(session_deadline, read_frame(&mut read, MAX_PAYLOAD)) => {
                match frame {
                    Ok(Ok(Some(payload))) => payload,
                    _ => break, // EOF, IO error, or login timeout
                }
            }
            Some(reason) = kick_rx.recv() => {
                let _ = send(&mut write, &mut encryption, server_packets::login_fail(reason)).await;
                // The controller already removed us from the authed map.
                session.account = None;
                break;
            }
        };

        if !encryption.decrypt(&mut payload) {
            debug!("Checksum/decrypt failure from {}", session.ip);
            break;
        }

        match dispatch(&ctx, &mut session, &payload, &mut write, &mut encryption, &kick_tx).await {
            Ok(true) => {}
            _ => break,
        }
    }

    // onDisconnection: not joined a GS yet → free the account on the LS.
    if let Some(account) = session.account.take() {
        if !session.joined_gs {
            ctx.controller.remove_authed_client(&account).await;
        }
    }
    debug!("Client {} disconnected", session.ip);
}

/// Returns Ok(false) to close the connection; unknown packets are ignored
/// like Java's null from `handlePacket`.
async fn dispatch(
    ctx: &LoginContext,
    session: &mut ClientSession,
    payload: &[u8],
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
    kick_tx: &mpsc::Sender<LoginFailReason>,
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
        // REQUEST_AUTH_LOGIN(0x00, ConnectionState.AUTHED_GG)
        (ConnectionState::AuthedGg, 0x00) => {
            request_auth_login(ctx, session, r, write, encryption, kick_tx).await
        }
        // REQUEST_SERVER_LIST(0x05, ConnectionState.AUTHED_LOGIN)
        (ConnectionState::AuthedLogin, 0x05) => {
            if r.remaining() < 8 {
                return Ok(false);
            }
            let skey1 = r.read_i32().unwrap();
            let skey2 = r.read_i32().unwrap();
            let key = session.session_key.expect("authed implies session key");
            if key.login_ok1 == skey1 && key.login_ok2 == skey2 {
                let servers = ctx.controller.server_list_data(session.ip.clone(), session.access_level).await;
                // ServerList constructor: wait up to 500 ms (10 × 50 ms) for
                // the ReplyCharacters data before sending without it.
                let account = session.account.clone().unwrap_or_default();
                let mut chars = ctx.controller.chars_on_servers(&account).await;
                let mut tries = 0;
                while chars.is_none() && tries < 10 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    chars = ctx.controller.chars_on_servers(&account).await;
                    tries += 1;
                }
                send(
                    write,
                    encryption,
                    server_packets::server_list(&servers, session.last_server, chars.as_ref()),
                )
                .await?;
                Ok(true)
            } else {
                close(write, encryption, LoginFailReason::ReasonAccessFailed).await
            }
        }
        // REQUEST_LOGIN / RequestCmdLogin (0x0B, ConnectionState.AUTHED_GG)
        (ConnectionState::AuthedGg, 0x0B) => {
            if !ctx.config.enable_cmd_line_login {
                return Ok(true);
            }
            if r.remaining() < 132 {
                return Ok(false); // readImpl: int + 128-byte block
            }
            let _ = r.read_i32();
            let raw: [u8; 0x80] = r.read_bytes(0x80).unwrap().try_into().unwrap();
            let decrypted = session.keypair.decrypt_raw(&raw);
            let user = java_trim(&decrypted[0x40..0x40 + 14]);
            let password = java_trim(&decrypted[0x60..0x60 + 16]);
            finish_auth(ctx, session, user, password, write, encryption, kick_tx).await
        }
        // REQUEST_PI_AGREEMENT_CHECK(0x0E, ConnectionState.AUTHED_LOGIN)
        (ConnectionState::AuthedLogin, 0x0E) => {
            let Some(account_id) = r.read_i32() else {
                return Ok(false);
            };
            let status = if ctx.config.show_pi_agreement { 0x01 } else { 0x00 };
            send(write, encryption, server_packets::pi_agreement_check(account_id, status)).await?;
            Ok(true)
        }
        // REQUEST_PI_AGREEMENT(0x0F, ConnectionState.AUTHED_LOGIN)
        (ConnectionState::AuthedLogin, 0x0F) => {
            let (Some(account_id), Some(status)) = (r.read_i32(), r.read_u8()) else {
                return Ok(false);
            };
            send(write, encryption, server_packets::pi_agreement_ack(account_id, status)).await?;
            Ok(true)
        }
        // REQUEST_SERVER_LOGIN(0x02, ConnectionState.AUTHED_LOGIN)
        (ConnectionState::AuthedLogin, 0x02) => {
            if r.remaining() < 9 {
                return Ok(false);
            }
            let skey1 = r.read_i32().unwrap();
            let skey2 = r.read_i32().unwrap();
            let server_id = r.read_u8().unwrap() as i32;
            let key = session.session_key.expect("authed implies session key");
            // If the license screen was skipped, the client never got the
            // loginOk pair, so it can't be checked.
            if !ctx.config.show_licence || (key.login_ok1 == skey1 && key.login_ok2 == skey2) {
                let account = session.account.clone().unwrap_or_default();
                if ctx
                    .controller
                    .is_login_possible(server_id, session.access_level, account, session.last_server)
                    .await
                {
                    session.joined_gs = true;
                    send(write, encryption, server_packets::play_ok(&key)).await?;
                    Ok(true)
                } else {
                    send(write, encryption, server_packets::play_fail(PlayFailReason::ReasonServerOverloaded)).await?;
                    Ok(false)
                }
            } else {
                close(write, encryption, LoginFailReason::ReasonAccessFailed).await
            }
        }
        _ => {
            debug!("Ignored packet 0x{opcode:02x} in state {:?} from {}", session.state, session.ip);
            Ok(true)
        }
    }
}

/// `RequestAuthLogin.java`: RSA-decrypt the credential block(s), then run the
/// controller's `retriveAccountInfo` + `tryCheckinAccount` flow.
async fn request_auth_login(
    ctx: &LoginContext,
    session: &mut ClientSession,
    mut r: PacketReader<'_>,
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
    kick_tx: &mpsc::Sender<LoginFailReason>,
) -> std::io::Result<bool> {
    if ctx.config.enable_cmd_line_login && ctx.config.only_cmd_line_login {
        return Ok(true);
    }

    // readImpl: >= 256 bytes = new auth method (two blocks), >= 128 = old.
    let new_auth_method = r.remaining() >= 256;
    let decrypted: Vec<u8> = if new_auth_method {
        let raw1: [u8; 0x80] = r.read_bytes(0x80).unwrap().try_into().unwrap();
        let raw2: [u8; 0x80] = r.read_bytes(0x80).unwrap().try_into().unwrap();
        let mut d = session.keypair.decrypt_raw(&raw1).to_vec();
        d.extend_from_slice(&session.keypair.decrypt_raw(&raw2));
        d
    } else if r.remaining() >= 128 {
        let raw1: [u8; 0x80] = r.read_bytes(0x80).unwrap().try_into().unwrap();
        session.keypair.decrypt_raw(&raw1).to_vec()
    } else {
        return Ok(false); // readImpl false → packet dropped, connection stays
    };

    let (user, password) = if new_auth_method {
        (
            format!("{}{}", java_trim(&decrypted[0x4E..0x4E + 50]), java_trim(&decrypted[0xCE..0xCE + 14])),
            java_trim(&decrypted[0xDC..0xDC + 16]),
        )
    } else {
        (java_trim(&decrypted[0x5E..0x5E + 14]), java_trim(&decrypted[0x6C..0x6C + 16]))
    };

    finish_auth(ctx, session, user, password, write, encryption, kick_tx).await
}

/// Shared tail of `RequestAuthLogin`/`RequestCmdLogin`: controller check-in
/// and the outcome packet.
async fn finish_auth(
    ctx: &LoginContext,
    session: &mut ClientSession,
    user: String,
    password: String,
    write: &mut OwnedWriteHalf,
    encryption: &mut LoginEncryption,
    kick_tx: &mpsc::Sender<LoginFailReason>,
) -> std::io::Result<bool> {
    let outcome = ctx
        .controller
        .try_auth_login(user.clone(), password, session.ip.clone(), kick_tx.clone())
        .await;

    match outcome {
        AuthOutcome::Success { key, access_level, last_server } => {
            session.account = Some(user.clone());
            session.session_key = Some(key);
            session.access_level = access_level;
            session.last_server = last_server;
            session.state = ConnectionState::AuthedLogin;
            if ctx.config.show_licence {
                send(write, encryption, server_packets::login_ok(&key)).await?;
            } else {
                let servers = ctx.controller.server_list_data(session.ip.clone(), session.access_level).await;
                let chars = ctx.controller.chars_on_servers(&user).await;
                send(
                    write,
                    encryption,
                    server_packets::server_list(&servers, session.last_server, chars.as_ref()),
                )
                .await?;
            }
            Ok(true)
        }
        AuthOutcome::AccessFailed => close(write, encryption, LoginFailReason::ReasonAccessFailed).await,
        AuthOutcome::InvalidPassword => close(write, encryption, LoginFailReason::ReasonUserOrPassWrong).await,
        AuthOutcome::AccountBanned => {
            info!("Banned account {user} tried to login from {}", session.ip);
            send(write, encryption, server_packets::account_kicked(AccountKickedReason::ReasonPermanentlyBanned)).await?;
            Ok(false)
        }
        AuthOutcome::AlreadyOnLs | AuthOutcome::AlreadyOnGs => {
            close(write, encryption, LoginFailReason::ReasonAccountInUse).await
        }
    }
}

/// `new String(bytes, off, len).trim()` — Java's trim strips every char
/// `<= ' '` from both ends, which is what removes the NUL padding.
fn java_trim(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_matches(|c| c <= ' ').to_string()
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
