//! The `LoginController` actor: single owner of all mutable login state
//! (authed clients, failed-login counters, IP bans), per the concurrency
//! model. Connection tasks talk to it via messages; DB work runs inside the
//! actor (acceptable at login rates, see PLAN_LOGIN_SERVER.md §5).

use std::collections::HashMap;

use crate::dao;
use crate::enums::LoginFailReason;
use crate::gs_table::{
    GameServerEntry, GameServerTable, GsCommand, Subnet, hexid_to_string, login_server_fail,
    server_status,
};
use crate::session::SessionKey;
use commons::crypt::hash_password;
use commons::util;
use models::sea_orm::DatabaseConnection;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

/// `LoginResult` outcomes seen by `RequestAuthLogin`.
#[derive(Debug)]
pub enum AuthOutcome {
    /// AUTH_SUCCESS — account checked in on the LS.
    Success {
        key: SessionKey,
        access_level: i32,
        last_server: i32,
    },
    /// Java: `retriveAccountInfo` returned null (bad account/password).
    AccessFailed,
    /// Java: `canCheckin` failed (ipauth lists / DB error).
    InvalidPassword,
    /// accessLevel < 0.
    AccountBanned,
    AlreadyOnLs,
    AlreadyOnGs,
}

struct AuthedEntry {
    key: SessionKey,
    kick: mpsc::Sender<LoginFailReason>,
    /// server_id → character count, filled by ReplyCharacters
    /// (`LoginClient._charsOnServers`). Only servers with ≥1 char get an entry
    /// (Java's `if (charsNum > 0)`); `None` until such a reply arrives.
    chars_on_servers: Option<HashMap<i32, i32>>,
    /// How many game servers were asked for this account's char count
    /// (`RequestCharacters` fan-out) and how many have replied. The ServerList
    /// waits for `chars_received >= chars_expected` so a slow second server
    /// isn't dropped — the map alone can't tell "0 chars" from "not replied".
    chars_expected: usize,
    chars_received: usize,
}

/// One row of the client `ServerList` packet, fully resolved.
#[derive(Debug, Clone)]
pub struct ServerListEntry {
    pub server_id: u8,
    pub ip: [u8; 4],
    pub port: i32,
    pub age_limit: u8,
    pub pvp: bool,
    pub current_players: u16,
    pub max_players: u16,
    pub up: bool,
    pub server_type: i32,
    pub brackets: bool,
}

/// One game server's live state, for the internal status channel.
///
/// Deliberately *not* [`ServerListEntry`]: that is the client-facing view and
/// applies visibility rules — a GM-only server reports DOWN to ordinary
/// players, and the address is resolved per client subnet. An operator dashboard
/// wants the truth, so this reports what the login server actually knows.
#[derive(Debug, Clone)]
pub struct GameServerStatus {
    pub id: i32,
    pub name: String,
    /// Registered, authenticated **and** holding a live link. The link is the
    /// part that matters: it drops when the game-server process dies, which is
    /// exactly the case a database-derived "online" flag cannot see.
    pub up: bool,
    /// Accounts currently in game, as the link reports them.
    pub players: u16,
    pub max_players: u16,
}

/// Result of a `GameServerAuth` registration attempt.
pub struct GsRegistration {
    pub server_id: i32,
    pub server_name: String,
}

pub enum Msg {
    IsBanned {
        ip: String,
        reply: oneshot::Sender<bool>,
    },
    AddBan {
        ip: String,
        duration_ms: i64,
    },
    TryAuthLogin {
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
        reply: oneshot::Sender<AuthOutcome>,
    },
    RemoveAuthedClient {
        account: String,
    },
    GetSessionKey {
        account: String,
        reply: oneshot::Sender<Option<SessionKey>>,
    },
    // --- GS link ---
    RegisterGameServer {
        desired_id: i32,
        accept_alternative: bool,
        port: u16,
        max_players: i32,
        hex_id: Vec<u8>,
        hosts: Vec<(String, String)>,
        link: mpsc::Sender<GsCommand>,
        reply: oneshot::Sender<Result<GsRegistration, u8>>,
    },
    GsDisconnected {
        server_id: i32,
    },
    SetServerStatus {
        server_id: i32,
        attributes: Vec<(i32, i32)>,
    },
    PlayerInGame {
        server_id: i32,
        accounts: Vec<String>,
    },
    PlayerLogout {
        server_id: i32,
        account: String,
    },
    PlayerAuthRequest {
        account: String,
        key: SessionKey,
        reply: oneshot::Sender<bool>,
    },
    StatusSnapshot {
        reply: oneshot::Sender<Vec<GameServerStatus>>,
    },
    ServerListData {
        client_ip: String,
        access_level: i32,
        reply: oneshot::Sender<Vec<ServerListEntry>>,
    },
    IsLoginPossible {
        server_id: i32,
        access_level: i32,
        account: String,
        last_server: i32,
        reply: oneshot::Sender<bool>,
    },
    /// ReplyCharacters (0x08) — `setCharactersOnServer`.
    SetCharactersOnServer {
        server_id: i32,
        account: String,
        chars: i32,
    },
    /// `LoginClient.getCharsOnServ` — returns `(all_replied, counts)`. The
    /// ServerList polls this until `all_replied` (or it times out) so every
    /// connected game server's count is included, not just the first to answer.
    GetCharsOnServers {
        account: String,
        reply: oneshot::Sender<(bool, Option<HashMap<i32, i32>>)>,
    },
    /// RequestTempBan (0x0A).
    TempBan {
        account: String,
        ip: String,
        ban_time: i64,
    },
    /// ChangePassword (0x0B) — full flow incl. the response to the right GS.
    ChangePassword {
        account: String,
        character_name: String,
        current_password: String,
        new_password: String,
    },
}

pub struct ControllerSettings {
    pub auto_create_accounts: bool,
    pub login_try_before_ban: i32,
    pub login_block_after_ban_ms: i64,
    pub show_licence: bool,
    pub accept_new_gameserver: bool,
}

struct Controller {
    settings: ControllerSettings,
    db: DatabaseConnection,
    authed_clients: HashMap<String, AuthedEntry>,
    failed_login_attempts: HashMap<String, i32>,
    banned_ips: HashMap<String, i64>,
    gs: GameServerTable,
    /// `LoginServer._loginStatus` — global override (STATUS_NORMAL default).
    login_status: i32,
}

#[derive(Clone)]
pub struct ControllerHandle {
    tx: mpsc::Sender<Msg>,
}

pub fn spawn(
    settings: ControllerSettings,
    db: DatabaseConnection,
    gs: GameServerTable,
) -> ControllerHandle {
    let (tx, mut rx) = mpsc::channel(256);
    let mut controller = Controller {
        settings,
        db,
        authed_clients: HashMap::new(),
        failed_login_attempts: HashMap::new(),
        banned_ips: HashMap::new(),
        gs,
        login_status: server_status::STATUS_NORMAL,
    };
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            controller.handle(msg).await;
        }
    });
    ControllerHandle { tx }
}

impl ControllerHandle {
    pub async fn is_banned(&self, ip: &str) -> bool {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::IsBanned {
                ip: ip.to_string(),
                reply,
            })
            .await;
        rx.await.unwrap_or(false)
    }

    pub async fn add_ban(&self, ip: &str, duration_ms: i64) {
        let _ = self
            .tx
            .send(Msg::AddBan {
                ip: ip.to_string(),
                duration_ms,
            })
            .await;
    }

    pub async fn try_auth_login(
        &self,
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
    ) -> AuthOutcome {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::TryAuthLogin {
                login,
                password,
                ip,
                kick,
                reply,
            })
            .await;
        rx.await.unwrap_or(AuthOutcome::InvalidPassword)
    }

    pub async fn remove_authed_client(&self, account: &str) {
        let _ = self
            .tx
            .send(Msg::RemoveAuthedClient {
                account: account.to_string(),
            })
            .await;
    }

    pub async fn session_key(&self, account: &str) -> Option<SessionKey> {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::GetSessionKey {
                account: account.to_string(),
                reply,
            })
            .await;
        rx.await.ok().flatten()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_game_server(
        &self,
        desired_id: i32,
        accept_alternative: bool,
        port: u16,
        max_players: i32,
        hex_id: Vec<u8>,
        hosts: Vec<(String, String)>,
        link: mpsc::Sender<GsCommand>,
    ) -> Result<GsRegistration, u8> {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::RegisterGameServer {
                desired_id,
                accept_alternative,
                port,
                max_players,
                hex_id,
                hosts,
                link,
                reply,
            })
            .await;
        rx.await.unwrap_or(Err(login_server_fail::NOT_AUTHED))
    }

    pub async fn gs_disconnected(&self, server_id: i32) {
        let _ = self.tx.send(Msg::GsDisconnected { server_id }).await;
    }

    pub async fn set_server_status(&self, server_id: i32, attributes: Vec<(i32, i32)>) {
        let _ = self
            .tx
            .send(Msg::SetServerStatus {
                server_id,
                attributes,
            })
            .await;
    }

    pub async fn player_in_game(&self, server_id: i32, accounts: Vec<String>) {
        let _ = self
            .tx
            .send(Msg::PlayerInGame {
                server_id,
                accounts,
            })
            .await;
    }

    pub async fn player_logout(&self, server_id: i32, account: String) {
        let _ = self.tx.send(Msg::PlayerLogout { server_id, account }).await;
    }

    pub async fn player_auth_request(&self, account: String, key: SessionKey) -> bool {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::PlayerAuthRequest {
                account,
                key,
                reply,
            })
            .await;
        rx.await.unwrap_or(false)
    }

    /// Every registered game server's live state, for the internal status
    /// channel. Unlike [`Controller::server_list_data`] this applies no client
    /// visibility rules.
    pub async fn status_snapshot(&self) -> Vec<GameServerStatus> {
        let (reply, rx) = oneshot::channel();
        let _ = self.tx.send(Msg::StatusSnapshot { reply }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn server_list_data(
        &self,
        client_ip: String,
        access_level: i32,
    ) -> Vec<ServerListEntry> {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::ServerListData {
                client_ip,
                access_level,
                reply,
            })
            .await;
        rx.await.unwrap_or_default()
    }

    pub async fn is_login_possible(
        &self,
        server_id: i32,
        access_level: i32,
        account: String,
        last_server: i32,
    ) -> bool {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::IsLoginPossible {
                server_id,
                access_level,
                account,
                last_server,
                reply,
            })
            .await;
        rx.await.unwrap_or(false)
    }

    pub async fn set_characters_on_server(&self, server_id: i32, account: String, chars: i32) {
        let _ = self
            .tx
            .send(Msg::SetCharactersOnServer {
                server_id,
                account,
                chars,
            })
            .await;
    }

    /// `(all_expected_servers_replied, counts)` for the account. The caller
    /// (ServerList) polls this, sleeping between tries, until the first element
    /// is `true` or its own timeout elapses — then sends whatever counts it has.
    pub async fn chars_on_servers(&self, account: &str) -> (bool, Option<HashMap<i32, i32>>) {
        let (reply, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(Msg::GetCharsOnServers {
                account: account.to_string(),
                reply,
            })
            .await;
        rx.await.unwrap_or((true, None))
    }

    pub async fn temp_ban(&self, account: String, ip: String, ban_time: i64) {
        let _ = self
            .tx
            .send(Msg::TempBan {
                account,
                ip,
                ban_time,
            })
            .await;
    }

    pub async fn change_password(
        &self,
        account: String,
        character_name: String,
        current: String,
        new: String,
    ) {
        let _ = self
            .tx
            .send(Msg::ChangePassword {
                account,
                character_name,
                current_password: current,
                new_password: new,
            })
            .await;
    }
}

/// `InetAddress.getByName(host).getAddress()` — accepts an IP literal or a
/// resolvable hostname.
fn resolve_host(host: &str) -> Option<[u8; 4]> {
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return Some(ip.octets());
    }
    use std::net::ToSocketAddrs;
    (host, 0)
        .to_socket_addrs()
        .ok()?
        .find_map(|addr| match addr.ip() {
            std::net::IpAddr::V4(v4) => Some(v4.octets()),
            std::net::IpAddr::V6(_) => None,
        })
}

impl Controller {
    async fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::IsBanned { ip, reply } => {
                let _ = reply.send(self.is_banned_address(&ip));
            }
            Msg::AddBan { ip, duration_ms } => self.add_ban_for_address(ip, duration_ms),
            Msg::TryAuthLogin {
                login,
                password,
                ip,
                kick,
                reply,
            } => {
                let outcome = self.try_auth_login(login, password, ip, kick).await;
                let _ = reply.send(outcome);
            }
            Msg::RemoveAuthedClient { account } => {
                self.authed_clients.remove(&account);
            }
            Msg::GetSessionKey { account, reply } => {
                let _ = reply.send(self.authed_clients.get(&account).map(|e| e.key));
            }
            Msg::RegisterGameServer {
                desired_id,
                accept_alternative,
                port,
                max_players,
                hex_id,
                hosts,
                link,
                reply,
            } => {
                let result = self
                    .register_game_server(
                        desired_id,
                        accept_alternative,
                        port,
                        max_players,
                        hex_id,
                        hosts,
                        link,
                    )
                    .await;
                let _ = reply.send(result);
            }
            Msg::GsDisconnected { server_id } => {
                if let Some(entry) = self.gs.servers.get_mut(&server_id) {
                    if entry.authed {
                        info!(
                            "Server [{server_id}] {} is now set as disconnected.",
                            self.gs
                                .server_names
                                .get(&server_id)
                                .cloned()
                                .unwrap_or_default()
                        );
                    }
                    entry.set_down();
                }
            }
            Msg::SetServerStatus {
                server_id,
                attributes,
            } => self.set_server_status(server_id, attributes),
            Msg::PlayerInGame {
                server_id,
                accounts,
            } => {
                if let Some(entry) = self.gs.servers.get_mut(&server_id) {
                    for account in accounts {
                        // addAccountOnGameServer also frees the LS-side slot.
                        self.authed_clients.remove(&account);
                        entry.accounts.insert(account);
                    }
                }
            }
            Msg::PlayerLogout { server_id, account } => {
                if let Some(entry) = self.gs.servers.get_mut(&server_id) {
                    entry.accounts.remove(&account);
                }
                self.authed_clients.remove(&account);
            }
            Msg::PlayerAuthRequest {
                account,
                key,
                reply,
            } => {
                let matches = match self.authed_clients.get(&account) {
                    Some(entry) => {
                        // SessionKey.equals: only the playOk pair when the
                        // license screen is skipped.
                        if self.settings.show_licence {
                            entry.key == key
                        } else {
                            entry.key.play_ok1 == key.play_ok1 && entry.key.play_ok2 == key.play_ok2
                        }
                    }
                    None => false,
                };
                if matches {
                    self.authed_clients.remove(&account);
                }
                let _ = reply.send(matches);
            }
            Msg::ServerListData {
                client_ip,
                access_level,
                reply,
            } => {
                let _ = reply.send(self.server_list_data(&client_ip, access_level));
            }
            Msg::SetCharactersOnServer {
                server_id,
                account,
                chars,
            } => {
                if let Some(entry) = self.authed_clients.get_mut(&account) {
                    entry.chars_received += 1;
                    // Java `setCharactersOnServer`: only servers with ≥1 char are
                    // recorded; a 0-char reply still counts as "replied" above.
                    if chars > 0 {
                        entry
                            .chars_on_servers
                            .get_or_insert_with(HashMap::new)
                            .insert(server_id, chars);
                    }
                }
            }
            Msg::GetCharsOnServers { account, reply } => {
                // Ready = every queried game server has answered (or there were
                // none). An unknown account (shouldn't happen post-auth) is
                // "ready" so the caller never blocks on it.
                let status = match self.authed_clients.get(&account) {
                    Some(e) => (
                        e.chars_received >= e.chars_expected,
                        e.chars_on_servers.clone(),
                    ),
                    None => (true, None),
                };
                let _ = reply.send(status);
            }
            Msg::TempBan {
                account,
                ip,
                ban_time,
            } => {
                // insert_or_update_account_data (SQLite dialect).
                let _ = models::repo::account_data::set(
                    &self.db,
                    &account,
                    "ban_temp",
                    &ban_time.to_string(),
                )
                .await;
                // Java quirk kept 1:1: the *absolute* ban-end timestamp is
                // passed as a duration to addBanForAddress.
                self.add_ban_for_address(ip, ban_time);
            }
            Msg::ChangePassword {
                account,
                character_name,
                current_password,
                new_password,
            } => {
                self.change_password(account, character_name, current_password, new_password)
                    .await;
            }
            Msg::StatusSnapshot { reply } => {
                let mut out: Vec<GameServerStatus> = self
                    .gs
                    .servers
                    .values()
                    .map(|gsi| GameServerStatus {
                        id: gsi.id,
                        name: self
                            .gs
                            .server_names
                            .get(&gsi.id)
                            .cloned()
                            .unwrap_or_else(|| format!("server{}", gsi.id)),
                        up: gsi.authed && gsi.link.is_some(),
                        players: gsi.accounts.len() as u16,
                        max_players: gsi.max_players as u16,
                    })
                    .collect();
                out.sort_by_key(|s| s.id);
                let _ = reply.send(out);
            }
            Msg::IsLoginPossible {
                server_id,
                access_level,
                account,
                last_server,
                reply,
            } => {
                let possible = match self.gs.servers.get(&server_id) {
                    Some(entry) if entry.authed => entry.can_login(access_level),
                    _ => false,
                };
                if possible && last_server != server_id {
                    let _ = models::repo::accounts::set_last_server(&self.db, &account, server_id)
                        .await;
                }
                let _ = reply.send(possible);
            }
        }
    }

    /// `GameServerAuth.handleRegProcess`.
    #[allow(clippy::too_many_arguments)]
    async fn register_game_server(
        &mut self,
        desired_id: i32,
        accept_alternative: bool,
        port: u16,
        max_players: i32,
        hex_id: Vec<u8>,
        hosts: Vec<(String, String)>,
        link: mpsc::Sender<GsCommand>,
    ) -> Result<GsRegistration, u8> {
        let assigned_id = match self.gs.servers.get(&desired_id) {
            Some(existing) if existing.hex_id == hex_id => {
                if existing.authed {
                    return Err(login_server_fail::REASON_ALREADY_LOGGED_IN);
                }
                desired_id
            }
            Some(_) => {
                // Registered with a different hexid: try an alternative id.
                if !(self.settings.accept_new_gameserver && accept_alternative) {
                    return Err(login_server_fail::REASON_WRONG_HEXID);
                }
                let free = self
                    .gs
                    .server_names
                    .keys()
                    .copied()
                    .find(|id| !self.gs.servers.contains_key(id))
                    .ok_or(login_server_fail::REASON_NO_FREE_ID)?;
                self.gs
                    .servers
                    .insert(free, GameServerEntry::new(free, hex_id.clone()));
                self.register_server_on_db(free, &hex_id, &hosts).await;
                free
            }
            None => {
                if !self.settings.accept_new_gameserver {
                    return Err(login_server_fail::REASON_WRONG_HEXID);
                }
                self.gs
                    .servers
                    .insert(desired_id, GameServerEntry::new(desired_id, hex_id.clone()));
                self.register_server_on_db(desired_id, &hex_id, &hosts)
                    .await;
                desired_id
            }
        };

        let entry = self.gs.servers.get_mut(&assigned_id).expect("just ensured");
        entry.port = port;
        entry.max_players = max_players;
        entry.addresses = hosts
            .iter()
            .filter_map(|(subnet, host)| Subnet::parse(subnet).map(|s| (s, host.clone())))
            .collect();
        entry.link = Some(link);
        entry.authed = true;

        let server_name = self
            .gs
            .server_names
            .get(&assigned_id)
            .cloned()
            .unwrap_or_default();
        info!("Updated Gameserver [{assigned_id}] {server_name} IP's:");
        for (_, host) in &hosts {
            info!("{host}");
        }
        Ok(GsRegistration {
            server_id: assigned_id,
            server_name,
        })
    }

    /// `ChangePassword.java`: verify against the stored hash, update, and
    /// report the result to the GS hosting the account.
    async fn change_password(
        &mut self,
        account: String,
        character_name: String,
        current: String,
        new: String,
    ) {
        let Some(link) = self
            .gs
            .servers
            .values()
            .find(|e| e.accounts.contains(&account))
            .and_then(|e| e.link.clone())
        else {
            return; // Java: no GS has the account → silently drop.
        };
        let respond = |message: &str| GsCommand::ChangePasswordResponse {
            character_name: character_name.clone(),
            message: message.to_string(),
        };

        let stored = models::repo::accounts::find(&self.db, &account)
            .await
            .ok()
            .flatten()
            .and_then(|row| row.password)
            .unwrap_or_default();

        if hash_password(&current) != stored {
            let _ = link.try_send(respond(
                "The typed current password doesn't match with your current one.",
            ));
            return;
        }

        let updated =
            models::repo::accounts::set_password(&self.db, &account, &hash_password(&new))
                .await
                .unwrap_or(false);
        info!("The password for account {account} has been changed.");
        if updated {
            let _ = link.try_send(respond("You have successfully changed your password!"));
        } else {
            let _ = link.try_send(respond("The password change was unsuccessful!"));
        }
    }

    async fn register_server_on_db(&self, id: i32, hex_id: &[u8], hosts: &[(String, String)]) {
        let external_host = hosts
            .first()
            .map(|(_, host)| host.clone())
            .unwrap_or_default();
        let _ = models::repo::gameservers::register(
            &self.db,
            id,
            &hexid_to_string(hex_id),
            &external_host,
        )
        .await;
    }

    /// `ServerStatus` packet application.
    fn set_server_status(&mut self, server_id: i32, attributes: Vec<(i32, i32)>) {
        let login_status = self.login_status;
        let Some(entry) = self.gs.servers.get_mut(&server_id) else {
            return;
        };
        for (kind, value) in attributes {
            match kind {
                server_status::SERVER_LIST_STATUS => {
                    // GameServerInfo.setStatus: the global LS status wins.
                    entry.status = match login_status {
                        server_status::STATUS_DOWN => server_status::STATUS_DOWN,
                        server_status::STATUS_GM_ONLY => server_status::STATUS_GM_ONLY,
                        _ => value,
                    };
                }
                server_status::SERVER_TYPE => entry.server_type = value,
                server_status::SERVER_LIST_SQUARE_BRACKET => entry.showing_brackets = value == 1,
                server_status::MAX_PLAYERS => entry.max_players = value,
                server_status::SERVER_AGE => entry.age_limit = value,
                _ => {}
            }
        }
    }

    /// `ServerList` data (`ServerData` construction).
    fn server_list_data(&self, client_ip: &str, access_level: i32) -> Vec<ServerListEntry> {
        let client_addr: std::net::Ipv4Addr =
            client_ip.parse().unwrap_or(std::net::Ipv4Addr::LOCALHOST);
        let mut entries: Vec<ServerListEntry> = self
            .gs
            .servers
            .values()
            .map(|gsi| {
                let ip = gsi
                    .address_for(client_addr)
                    .and_then(resolve_host)
                    .unwrap_or([127, 0, 0, 1]);
                let status = if access_level < 0
                    || (gsi.status == server_status::STATUS_GM_ONLY && access_level <= 0)
                {
                    server_status::STATUS_DOWN
                } else {
                    gsi.status
                };
                ServerListEntry {
                    server_id: gsi.id as u8,
                    ip,
                    port: gsi.port as i32,
                    age_limit: 0,
                    pvp: true, // GameServerInfo.IS_PVP
                    current_players: gsi.accounts.len() as u16,
                    max_players: gsi.max_players as u16,
                    up: status != server_status::STATUS_DOWN,
                    server_type: gsi.server_type,
                    brackets: gsi.showing_brackets,
                }
            })
            .collect();
        entries.sort_by_key(|e| e.server_id);
        entries
    }

    /// Records the outcome of every authentication attempt, then delegates.
    ///
    /// This is the login server's whole `accounting` story: who got in, who did
    /// not, and from where. Wrapping rather than recording at each `return` is
    /// deliberate — the inner function has eight exit points, and a ninth added
    /// later would silently escape the audit.
    ///
    /// Ungated and never dropped, like the game server's accounting records:
    /// Java has no config switch for these either, and a failed-login pattern is
    /// exactly what someone opens this file to reconstruct.
    async fn try_auth_login(
        &mut self,
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
    ) -> AuthOutcome {
        // Lowercased to match how the account is stored and looked up, so a
        // record joins against the game server's accounting records.
        let account = login.to_lowercase();
        let outcome = self
            .try_auth_login_inner(login, password, ip.clone(), kick)
            .await;
        let result = match &outcome {
            AuthOutcome::Success { .. } => "success",
            AuthOutcome::AccessFailed => "access_failed",
            AuthOutcome::InvalidPassword => "invalid_password",
            AuthOutcome::AccountBanned => "account_banned",
            AuthOutcome::AlreadyOnLs => "already_on_ls",
            AuthOutcome::AlreadyOnGs => "already_on_gs",
        };
        commons::audit::record(
            commons::audit::Category::Accounting,
            serde_json::json!({
                "event": "login_attempt",
                "account": account,
                "ip": ip,
                "result": result,
            }),
        );
        outcome
    }

    /// `RequestAuthLogin.run` DB half: `retriveAccountInfo` + `tryCheckinAccount`.
    async fn try_auth_login_inner(
        &mut self,
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
    ) -> AuthOutcome {
        // Accounts are case-insensitive: the login server works entirely in
        // lowercase (Java `AccountInfo._login = login.toLowerCase()`). This must
        // match the game server, which lowercases the account in `AuthLogin`;
        // otherwise the `PlayerAuthRequest` handoff misses `authed_clients` and
        // the player never reaches character selection.
        let login = login.to_lowercase();
        let hash = hash_password(&password);
        let now = util::now_millis();

        // retriveAccountInfo
        let mut info = dao::select_account_info(&self.db, &login, now).await;
        match &info {
            Some(acc) if !acc.check_pass_hash(&hash) => {
                self.record_failed_login_attempt(&ip);
                return AuthOutcome::AccessFailed;
            }
            Some(_) => self.clear_failed_login_attempts(&ip),
            None => {
                if !self.settings.auto_create_accounts {
                    self.record_failed_login_attempt(&ip);
                    return AuthOutcome::AccessFailed;
                }
                if let Err(e) =
                    models::repo::accounts::create(&self.db, &login, &hash, now, &ip).await
                {
                    warn!("Exception while auto creating account for '{login}': {e}");
                    return AuthOutcome::AccessFailed;
                }
                info!("Auto created account '{login}'.");
                info = dao::select_account_info(&self.db, &login, now).await;
                if info.is_none() {
                    return AuthOutcome::AccessFailed;
                }
            }
        }
        let info = info.unwrap();

        // tryCheckinAccount
        if info.access_level < 0 {
            return AuthOutcome::AccountBanned;
        }

        // canCheckin: accounts_ipauth white/black lists + lastactive/lastIP update.
        let (white, black) = dao::select_ipauth(&self.db, &info.login).await;
        if (!white.is_empty() && !white.contains(&ip)) || (!black.is_empty() && black.contains(&ip))
        {
            warn!(
                "Account checkin attempt from address({ip}) blocked by ipauth for account '{}'.",
                info.login
            );
            return AuthOutcome::InvalidPassword;
        }
        dao::update_account_info(&self.db, &info.login, &ip, now).await;

        // ALREADY_ON_GS: kick from the game server (RequestAuthLogin does
        // gsi.getGameServerThread().kickPlayer(login)).
        if let Some(entry) = self
            .gs
            .servers
            .values()
            .find(|e| e.accounts.contains(&info.login))
        {
            if entry.authed
                && let Some(link) = &entry.link
            {
                let _ = link.try_send(GsCommand::KickPlayer {
                    account: info.login.clone(),
                });
            }
            return AuthOutcome::AlreadyOnGs;
        }

        // ALREADY_ON_LS: kick the previous client, like RequestAuthLogin does.
        if let Some(old) = self.authed_clients.remove(&info.login) {
            let _ = old.kick.try_send(LoginFailReason::ReasonAccountInUse);
            return AuthOutcome::AlreadyOnLs;
        }

        let key = SessionKey::random();
        self.authed_clients.insert(
            info.login.clone(),
            AuthedEntry {
                key,
                kick,
                chars_on_servers: None,
                chars_expected: 0,
                chars_received: 0,
            },
        );

        // getCharactersOnAccount: ask every authed GS for character counts, and
        // remember how many we asked so the ServerList waits for them all.
        let mut expected = 0;
        for entry in self.gs.servers.values() {
            if entry.authed
                && let Some(link) = &entry.link
            {
                let _ = link.try_send(GsCommand::RequestCharacters {
                    account: info.login.clone(),
                });
                expected += 1;
            }
        }
        if let Some(entry) = self.authed_clients.get_mut(&info.login) {
            entry.chars_expected = expected;
        }

        AuthOutcome::Success {
            key,
            access_level: info.access_level,
            last_server: info.last_server,
        }
    }

    fn record_failed_login_attempt(&mut self, ip: &str) {
        let attempts = self
            .failed_login_attempts
            .entry(ip.to_string())
            .or_insert(0);
        *attempts += 1;
        if *attempts >= self.settings.login_try_before_ban {
            self.add_ban_for_address(ip.to_string(), self.settings.login_block_after_ban_ms);
            self.failed_login_attempts.remove(ip);
            warn!("Added banned address {ip}! Too many login attempts.");
        }
    }

    fn clear_failed_login_attempts(&mut self, ip: &str) {
        self.failed_login_attempts.remove(ip);
    }

    fn add_ban_for_address(&mut self, ip: String, duration_ms: i64) {
        let expiry = if duration_ms > 0 {
            util::now_millis() + duration_ms
        } else {
            i64::MAX
        };
        self.banned_ips.entry(ip).or_insert(expiry);
    }

    /// `isBannedAddress`: exact match, then the .0 / .0.0 / .0.0.0 subnet forms.
    fn is_banned_address(&mut self, ip: &str) -> bool {
        let parts: Vec<&str> = ip.split('.').collect();
        let mut candidates = vec![ip.to_string()];
        if parts.len() == 4 {
            candidates.push(format!("{}.{}.{}.0", parts[0], parts[1], parts[2]));
            candidates.push(format!("{}.{}.0.0", parts[0], parts[1]));
            candidates.push(format!("{}.0.0.0", parts[0]));
        }
        for candidate in candidates {
            if let Some(&expiry) = self.banned_ips.get(&candidate) {
                if expiry > 0 && expiry < util::now_millis() {
                    self.banned_ips.remove(&candidate);
                    info!("Removed expired ip address ban {candidate}.");
                    return false;
                }
                return true;
            }
        }
        false
    }
}
