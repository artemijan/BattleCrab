//! Port of `gameserver/LoginServerThread` — the game server's client end of the
//! GS↔LS link. A single tokio task connects to the login server, runs the
//! handshake (`InitLS` → `BlowFishKey` + `AuthRequest` → `AuthResponse`), then
//! relays: game-thread commands out (`PlayerAuthRequest`, `PlayerLogout`, …) and
//! login-server packets in (`PlayerAuthResponse`, `KickPlayer`, …) as events to
//! the game thread. Reconnects every 5s, like Java.
//!
//! All client/account state stays on the game thread (single-owner model); this
//! task is just the encrypted pipe.

pub mod packets;

use std::time::Duration;

use commons::crypt::{GS_STATIC_BLOWFISH_KEY, NewCrypt, RsaPublicModulus, gs_decrypt, gs_encrypt};
use commons::network::{read_frame, write_frame};
use commons::util::generate_hex;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::session::SessionKey;

/// `LoginServerThread.REVISION`.
const REVISION: i32 = 0x0106;
const MAX_PAYLOAD: usize = u16::MAX as usize;

/// `ServerStatus` attribute ids / values (game-side `ServerStatus` constants).
pub mod status {
    pub const SERVER_LIST_STATUS: i32 = 0x01;
    pub const SERVER_TYPE: i32 = 0x02;
    pub const SERVER_LIST_SQUARE_BRACKET: i32 = 0x03;
    pub const MAX_PLAYERS: i32 = 0x04;
    pub const SERVER_AGE: i32 = 0x05;

    pub const STATUS_AUTO: i32 = 0x00;
    pub const STATUS_GM_ONLY: i32 = 0x05;
    pub const ON: i32 = 0x01;
    pub const OFF: i32 = 0x00;
    pub const SERVER_AGE_ALL: i32 = 0x00;
    pub const SERVER_AGE_15: i32 = 0x0F;
    pub const SERVER_AGE_18: i32 = 0x12;
}

/// Static config for the link task, resolved from `Config` at boot.
pub struct LoginLinkConfig {
    pub host: String,
    pub port: u16,
    pub game_port: u16,
    pub hex_id: Vec<u8>,
    pub request_id: i32,
    pub accept_alternate: bool,
    pub reserve_host: bool,
    pub max_players: i32,
    /// (subnet, host) pairs advertised to the login server for its ServerList.
    pub hosts: Vec<(String, String)>,
    pub server_list_type: i32,
    pub server_list_bracket: bool,
    pub server_list_age: i32,
    pub gmonly: bool,
}

/// Commands the game thread sends to the link (game → LS).
pub enum LoginLinkCommand {
    PlayerAuthRequest {
        account: String,
        key: SessionKey,
    },
    PlayerInGame {
        accounts: Vec<String>,
    },
    PlayerLogout {
        account: String,
    },
    ReplyCharacters {
        account: String,
        chars: u8,
        del_times: Vec<i64>,
    },
    /// Java `LoginServerThread.sendAccessLevel` (G31): relay an account's new
    /// access level to the login server. `level < 0` bans it there.
    SetAccountAccessLevel {
        account: String,
        level: i32,
    },
    /// `AdminLogin`'s runtime `ServerStatus` updates (`//server_gm_only`,
    /// `//server_max_player`, `//server_list_age`, `//server_list_type`).
    ServerStatus {
        attrs: Vec<(i32, i32)>,
    },
}

/// Events the link reports to the game thread (LS → game).
pub enum LoginLinkEvent {
    Registered { server_id: i32, server_name: String },
    PlayerAuthResponse { account: String, authed: bool },
    KickPlayer { account: String },
    RequestCharacters { account: String },
    Failed { reason: u8 },
}

pub type CommandTx = tokio::sync::mpsc::UnboundedSender<LoginLinkCommand>;
pub type CommandRx = tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>;

/// Sender facade for the login-link's share of the unified service→game
/// channel ([`crate::events::GameEvent`]); a send wakes the sleeping game
/// loop.
#[derive(Clone)]
pub struct EventTx(pub crate::events::GameEventTx);

impl EventTx {
    /// An `Err` means the game thread is gone — callers treat it as shutdown.
    pub fn send(&self, event: LoginLinkEvent) -> Result<(), std::sync::mpsc::SendError<()>> {
        self.0
            .send(crate::events::GameEvent::Login(event))
            .map_err(|_| std::sync::mpsc::SendError(()))
    }
}

/// The link task: connect / handshake / relay, reconnecting forever.
///
/// `ready` fires once the game thread has loaded all boot data (including clans
/// from the DB); the task blocks on it before its first connect so the login
/// server never registers us — and thus never routes players to us — before the
/// world is fully populated. Mirrors Java running `LoginServerThread.start()`
/// dead-last in `GameServer`, after `ClanTable`.
pub async fn run(
    cfg: LoginLinkConfig,
    mut cmd_rx: CommandRx,
    event_tx: EventTx,
    ready: tokio::sync::oneshot::Receiver<()>,
) {
    // If the game thread drops the sender without signalling (e.g. DB open
    // failed and the server is going down anyway), give up rather than connect.
    if ready.await.is_err() {
        warn!("LoginServerThread: data load never completed; not connecting to login.");
        return;
    }
    loop {
        info!(
            "LoginServerThread: Connecting to login on {}:{}",
            cfg.host, cfg.port
        );
        match TcpStream::connect((cfg.host.as_str(), cfg.port)).await {
            Ok(stream) => {
                if let Err(e) = session(&cfg, stream, &mut cmd_rx, &event_tx).await {
                    warn!("LoginServerThread: Disconnected from Login: {e}");
                } else {
                    info!("LoginServerThread: Login terminated the connection.");
                }
            }
            Err(_) => warn!("LoginServerThread: LoginServer not available, trying to reconnect..."),
        }
        tokio::time::sleep(Duration::from_secs(5)).await; // 5 seconds tempo.
    }
}

async fn session(
    cfg: &LoginLinkConfig,
    stream: TcpStream,
    cmd_rx: &mut CommandRx,
    event_tx: &EventTx,
) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let (mut read, mut write) = stream.into_split();

    let mut crypt = NewCrypt::new(GS_STATIC_BLOWFISH_KEY);
    let blowfish_key = generate_hex(40);
    let mut authed = false;

    loop {
        tokio::select! {
            frame = read_frame(&mut read, MAX_PAYLOAD) => {
                let mut payload = match frame? {
                    Some(p) => p,
                    None => return Ok(()), // clean EOF
                };
                if !gs_decrypt(&crypt, &mut payload) {
                    warn!("LoginServerThread: Incorrect packet checksum, closing connection (LS)");
                    return Ok(());
                }
                let Some(opcode) = payload.first().copied() else { continue };
                let body = &payload[1..];
                match opcode {
                    0x00 => { // InitLS
                        let Some(init) = packets::InitLs::read(body) else { return Ok(()) };
                        if init.revision != REVISION {
                            warn!("/!\\ Revision mismatch between LS and GS /!\\");
                            return Ok(());
                        }
                        let modulus = RsaPublicModulus::from_java_bytes(&init.rsa_key);
                        send(&mut write, &crypt, packets::blowfish_key(&blowfish_key, &modulus)).await?;
                        // From now on, only the session Blowfish key is used.
                        crypt = NewCrypt::new(&blowfish_key);
                        send(&mut write, &crypt, packets::auth_request(
                            cfg.request_id, cfg.accept_alternate, cfg.reserve_host, cfg.game_port,
                            cfg.max_players, &cfg.hex_id, &cfg.hosts,
                        )).await?;
                    }
                    0x01 => { // LoginServerFail
                        let reason = packets::read_login_server_fail(body).unwrap_or(0);
                        let reason_str = packets::FAIL_REASONS.get(reason as usize).copied().unwrap_or("?");
                        info!("LoginServerThread: Registration Failed: {reason_str}");
                        let _ = event_tx.send(LoginLinkEvent::Failed { reason });
                        return Ok(()); // Login will close the connection here.
                    }
                    0x02 => { // AuthResponse
                        let Some(ar) = packets::AuthResponse::read(body) else { return Ok(()) };
                        info!("LoginServerThread: Registered on login as Server {}: {}", ar.server_id, ar.server_name);
                        let _ = event_tx.send(LoginLinkEvent::Registered {
                            server_id: ar.server_id, server_name: ar.server_name,
                        });
                        send(&mut write, &crypt, packets::server_status(&status_attributes(cfg))).await?;
                        authed = true;
                    }
                    0x03 => { // PlayerAuthResponse
                        if let Some(par) = packets::PlayerAuthResponse::read(body) {
                            let _ = event_tx.send(LoginLinkEvent::PlayerAuthResponse {
                                account: par.account, authed: par.authed,
                            });
                        }
                    }
                    0x04 => { // KickPlayer
                        if let Some(account) = packets::read_account(body) {
                            let _ = event_tx.send(LoginLinkEvent::KickPlayer { account });
                        }
                    }
                    0x05 => { // RequestCharacters
                        if let Some(account) = packets::read_account(body) {
                            let _ = event_tx.send(LoginLinkEvent::RequestCharacters { account });
                        }
                    }
                    0x06 => {} // ChangePasswordResponse — ignored
                    other => warn!("LoginServerThread: Unknown opcode 0x{other:02X} from login."),
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                if !authed {
                    // Socket/crypto not ready — Java's sendPacket no-ops here too.
                    continue;
                }
                let body = match cmd {
                    LoginLinkCommand::PlayerAuthRequest { account, key } => packets::player_auth_request(&account, &key),
                    LoginLinkCommand::PlayerInGame { accounts } => packets::player_in_game(&accounts),
                    LoginLinkCommand::PlayerLogout { account } => packets::player_logout(&account),
                    LoginLinkCommand::ReplyCharacters { account, chars, del_times } => {
                        packets::reply_characters(&account, chars, &del_times)
                    }
                    LoginLinkCommand::SetAccountAccessLevel { account, level } => {
                        packets::change_access_level(&account, level)
                    }
                    LoginLinkCommand::ServerStatus { attrs } => packets::server_status(&attrs),
                };
                send(&mut write, &crypt, body).await?;
            }
        }
    }
}

/// The `ServerStatus` sent right after registration (Java AuthResponse handler).
fn status_attributes(cfg: &LoginLinkConfig) -> Vec<(i32, i32)> {
    let mut attrs = Vec::new();
    attrs.push((
        status::SERVER_LIST_SQUARE_BRACKET,
        if cfg.server_list_bracket {
            status::ON
        } else {
            status::OFF
        },
    ));
    attrs.push((status::SERVER_TYPE, cfg.server_list_type));
    attrs.push((
        status::SERVER_LIST_STATUS,
        if cfg.gmonly {
            status::STATUS_GM_ONLY
        } else {
            status::STATUS_AUTO
        },
    ));
    let age = match cfg.server_list_age {
        15 => status::SERVER_AGE_15,
        18 => status::SERVER_AGE_18,
        _ => status::SERVER_AGE_ALL,
    };
    attrs.push((status::SERVER_AGE, age));
    attrs
}

async fn send<W: AsyncWrite + Unpin>(
    write: &mut W,
    crypt: &NewCrypt,
    body: Vec<u8>,
) -> std::io::Result<()> {
    write_frame(write, &gs_encrypt(crypt, body)).await
}
