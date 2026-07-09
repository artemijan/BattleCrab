//! Port of `GameServerThread`: one task per connected game server.
//! State machine: CONNECTED → (BlowFishKey) → BF_CONNECTED →
//! (GameServerAuth) → AUTHED.

use std::sync::Arc;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame, PacketReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::context::LoginContext;
use crate::gs_link::packets::{self, GS_STATIC_BLOWFISH_KEY};
use crate::gs_table::{login_server_fail, GsCommand};
use crate::session::SessionKey;

/// `LoginServer.PROTOCOL_REV`.
const PROTOCOL_REV: i32 = 0x0106;
const MAX_PAYLOAD: usize = 65533;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameServerState {
    Connected,
    BfConnected,
    Authed,
}

pub async fn handle(ctx: Arc<LoginContext>, stream: TcpStream, ip: String) {
    let (mut read, mut write) = stream.into_split();

    let keypair = ctx.random_gs_keypair();
    let mut crypt = NewCrypt::new(GS_STATIC_BLOWFISH_KEY);
    let mut state = GameServerState::Connected;
    let mut server_id: Option<i32> = None;

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<GsCommand>(32);

    // InitLS goes out immediately, under the static key.
    if send(&mut write, &crypt, packets::init_ls(PROTOCOL_REV, &keypair.modulus_java_bytes())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            frame = read_frame(&mut read, MAX_PAYLOAD) => {
                let mut payload = match frame {
                    Ok(Some(payload)) => payload,
                    _ => break,
                };
                if !packets::gs_decrypt(&crypt, &mut payload) {
                    warn!("Incorrect packet checksum, closing connection (LS)");
                    break;
                }
                let mut r = PacketReader::new(&payload);
                let Some(opcode) = r.read_u8() else { break };

                match (state, opcode) {
                    // BlowFishKey
                    (GameServerState::Connected, 0x00) => {
                        let Some(size) = r.read_i32() else { break };
                        let Some(block) = r.read_bytes(size as usize) else { break };
                        let decrypted = keypair.decrypt_raw(block);
                        let first = decrypted.iter().position(|&b| b != 0).unwrap_or(decrypted.len());
                        let key = &decrypted[first..];
                        if key.is_empty() {
                            break;
                        }
                        crypt = NewCrypt::new(key);
                        state = GameServerState::BfConnected;
                    }
                    // GameServerAuth
                    (GameServerState::BfConnected, 0x01) => {
                        match handle_game_server_auth(&ctx, &mut r, cmd_tx.clone()).await {
                            Ok((id, name)) => {
                                info!("Game Server [{id}] {name} is connected.");
                                server_id = Some(id);
                                state = GameServerState::Authed;
                                if send(&mut write, &crypt, packets::auth_response(id, &name)).await.is_err() {
                                    break;
                                }
                            }
                            Err(reason) => {
                                // forceClose(reason)
                                let _ = send(&mut write, &crypt, packets::login_server_fail(reason)).await;
                                break;
                            }
                        }
                    }
                    // PlayerInGame
                    (GameServerState::Authed, 0x02) => {
                        let Some(count) = r.read_i16() else { break };
                        let mut accounts = Vec::with_capacity(count as usize);
                        for _ in 0..count {
                            match r.read_string() {
                                Some(account) => accounts.push(account),
                                None => break,
                            }
                        }
                        ctx.controller.player_in_game(server_id.unwrap_or(-1), accounts).await;
                    }
                    // PlayerLogout
                    (GameServerState::Authed, 0x03) => {
                        if let Some(account) = r.read_string() {
                            ctx.controller.player_logout(server_id.unwrap_or(-1), account).await;
                        }
                    }
                    // ChangeAccessLevel
                    (GameServerState::Authed, 0x04) => {
                        if let (Some(level), Some(account)) = (r.read_i32(), r.read_string()) {
                            let _ = sqlx::query("UPDATE accounts SET accessLevel = ? WHERE login = ?")
                                .bind(level)
                                .bind(&account)
                                .execute(&ctx.pool)
                                .await;
                            info!("Changed {account} access level to {level}.");
                        }
                    }
                    // PlayerAuthRequest
                    (GameServerState::Authed, 0x05) => {
                        let (Some(account), Some(play1), Some(play2), Some(login1), Some(login2)) =
                            (r.read_string(), r.read_i32(), r.read_i32(), r.read_i32(), r.read_i32())
                        else {
                            break;
                        };
                        let key = SessionKey { login_ok1: login1, login_ok2: login2, play_ok1: play1, play_ok2: play2 };
                        let ok = ctx.controller.player_auth_request(account.clone(), key).await;
                        if send(&mut write, &crypt, packets::player_auth_response(&account, ok)).await.is_err() {
                            break;
                        }
                    }
                    // ServerStatus
                    (GameServerState::Authed, 0x06) => {
                        let Some(count) = r.read_i32() else { break };
                        let mut attributes = Vec::with_capacity(count as usize);
                        for _ in 0..count {
                            if let (Some(kind), Some(value)) = (r.read_i32(), r.read_i32()) {
                                attributes.push((kind, value));
                            }
                        }
                        ctx.controller.set_server_status(server_id.unwrap_or(-1), attributes).await;
                    }
                    // PlayerTracert / ReplyCharacters / RequestTempBan / ChangePassword → M5.
                    (GameServerState::Authed, 0x07 | 0x08 | 0x0A | 0x0B) => {}
                    _ => {
                        warn!("Unknown Opcode (0x{opcode:02X}) in state {state:?} from GameServer, closing connection.");
                        let _ = send(&mut write, &crypt, packets::login_server_fail(login_server_fail::NOT_AUTHED)).await;
                        break;
                    }
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                let body = match cmd {
                    GsCommand::KickPlayer { account } => packets::kick_player(&account),
                    GsCommand::RequestCharacters { account } => packets::request_characters(&account),
                };
                if send(&mut write, &crypt, body).await.is_err() {
                    break;
                }
            }
        }
    }

    if let Some(id) = server_id {
        ctx.controller.gs_disconnected(id).await;
    }
    info!("GameServer connection from {ip} closed.");
}

/// `GameServerAuth` parse + registration via the controller.
async fn handle_game_server_auth(
    ctx: &LoginContext,
    r: &mut PacketReader<'_>,
    link: mpsc::Sender<GsCommand>,
) -> Result<(i32, String), u8> {
    let desired_id = r.read_u8().ok_or(login_server_fail::NOT_AUTHED)? as i32;
    let accept_alternative = r.read_u8().ok_or(login_server_fail::NOT_AUTHED)? != 0;
    let _host_reserved = r.read_u8().ok_or(login_server_fail::NOT_AUTHED)?;
    let port = r.read_i16().ok_or(login_server_fail::NOT_AUTHED)? as u16;
    let max_players = r.read_i32().ok_or(login_server_fail::NOT_AUTHED)?;
    let hex_size = r.read_i32().ok_or(login_server_fail::NOT_AUTHED)? as usize;
    let hex_id = r.read_bytes(hex_size).ok_or(login_server_fail::NOT_AUTHED)?.to_vec();
    let pair_count = r.read_i32().ok_or(login_server_fail::NOT_AUTHED)?;
    let mut hosts = Vec::with_capacity(pair_count as usize);
    for _ in 0..pair_count {
        let subnet = r.read_string().ok_or(login_server_fail::NOT_AUTHED)?;
        let host = r.read_string().ok_or(login_server_fail::NOT_AUTHED)?;
        hosts.push((subnet, host));
    }

    ctx.controller
        .register_game_server(desired_id, accept_alternative, port, max_players, hex_id, hosts, link)
        .await
        .map(|reg| (reg.server_id, reg.server_name))
}

async fn send(write: &mut OwnedWriteHalf, crypt: &NewCrypt, body: Vec<u8>) -> std::io::Result<()> {
    let encrypted = packets::gs_encrypt(crypt, body);
    write_frame(write, &encrypted).await
}
