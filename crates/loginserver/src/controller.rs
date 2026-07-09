//! The `LoginController` actor: single owner of all mutable login state
//! (authed clients, failed-login counters, IP bans), per the concurrency
//! model. Connection tasks talk to it via messages; DB work runs inside the
//! actor (acceptable at login rates, see PLAN_LOGIN_SERVER.md §5).

use std::collections::HashMap;

use commons::crypt::hash_password;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::dao;
use crate::enums::LoginFailReason;
use crate::session::SessionKey;

/// `LoginResult` outcomes seen by `RequestAuthLogin`.
#[derive(Debug)]
pub enum AuthOutcome {
    /// AUTH_SUCCESS — account checked in on the LS.
    Success { key: SessionKey, access_level: i32, last_server: i32 },
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
}

pub enum Msg {
    IsBanned { ip: String, reply: oneshot::Sender<bool> },
    AddBan { ip: String, duration_ms: i64 },
    TryAuthLogin {
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
        reply: oneshot::Sender<AuthOutcome>,
    },
    RemoveAuthedClient { account: String },
    GetSessionKey { account: String, reply: oneshot::Sender<Option<SessionKey>> },
}

pub struct ControllerSettings {
    pub auto_create_accounts: bool,
    pub login_try_before_ban: i32,
    pub login_block_after_ban_ms: i64,
}

struct Controller {
    settings: ControllerSettings,
    pool: SqlitePool,
    authed_clients: HashMap<String, AuthedEntry>,
    failed_login_attempts: HashMap<String, i32>,
    banned_ips: HashMap<String, i64>,
}

#[derive(Clone)]
pub struct ControllerHandle {
    tx: mpsc::Sender<Msg>,
}

pub fn spawn(settings: ControllerSettings, pool: SqlitePool) -> ControllerHandle {
    let (tx, mut rx) = mpsc::channel(256);
    let mut controller = Controller {
        settings,
        pool,
        authed_clients: HashMap::new(),
        failed_login_attempts: HashMap::new(),
        banned_ips: HashMap::new(),
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
        let _ = self.tx.send(Msg::IsBanned { ip: ip.to_string(), reply }).await;
        rx.await.unwrap_or(false)
    }

    pub async fn add_ban(&self, ip: &str, duration_ms: i64) {
        let _ = self.tx.send(Msg::AddBan { ip: ip.to_string(), duration_ms }).await;
    }

    pub async fn try_auth_login(
        &self,
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
    ) -> AuthOutcome {
        let (reply, rx) = oneshot::channel();
        let _ = self.tx.send(Msg::TryAuthLogin { login, password, ip, kick, reply }).await;
        rx.await.unwrap_or(AuthOutcome::InvalidPassword)
    }

    pub async fn remove_authed_client(&self, account: &str) {
        let _ = self.tx.send(Msg::RemoveAuthedClient { account: account.to_string() }).await;
    }

    pub async fn session_key(&self, account: &str) -> Option<SessionKey> {
        let (reply, rx) = oneshot::channel();
        let _ = self.tx.send(Msg::GetSessionKey { account: account.to_string(), reply }).await;
        rx.await.ok().flatten()
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Controller {
    async fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::IsBanned { ip, reply } => {
                let _ = reply.send(self.is_banned_address(&ip));
            }
            Msg::AddBan { ip, duration_ms } => self.add_ban_for_address(ip, duration_ms),
            Msg::TryAuthLogin { login, password, ip, kick, reply } => {
                let outcome = self.try_auth_login(login, password, ip, kick).await;
                let _ = reply.send(outcome);
            }
            Msg::RemoveAuthedClient { account } => {
                self.authed_clients.remove(&account);
            }
            Msg::GetSessionKey { account, reply } => {
                let _ = reply.send(self.authed_clients.get(&account).map(|e| e.key));
            }
        }
    }

    /// `RequestAuthLogin.run` DB half: `retriveAccountInfo` + `tryCheckinAccount`.
    async fn try_auth_login(
        &mut self,
        login: String,
        password: String,
        ip: String,
        kick: mpsc::Sender<LoginFailReason>,
    ) -> AuthOutcome {
        let hash = hash_password(&password);
        let now = now_millis();

        // retriveAccountInfo
        let mut info = dao::select_account_info(&self.pool, &login, now).await;
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
                if let Err(e) = dao::auto_create_account(&self.pool, &login, &hash, now, &ip).await {
                    warn!("Exception while auto creating account for '{login}': {e}");
                    return AuthOutcome::AccessFailed;
                }
                info!("Auto created account '{login}'.");
                info = dao::select_account_info(&self.pool, &login, now).await;
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
        let (white, black) = dao::select_ipauth(&self.pool, &info.login).await;
        if (!white.is_empty() && !white.contains(&ip)) || (!black.is_empty() && black.contains(&ip)) {
            warn!("Account checkin attempt from address({ip}) blocked by ipauth for account '{}'.", info.login);
            return AuthOutcome::InvalidPassword;
        }
        dao::update_account_info(&self.pool, &info.login, &ip, now).await;

        // ALREADY_ON_GS: stub until the GS link exists (M4).

        // ALREADY_ON_LS: kick the previous client, like RequestAuthLogin does.
        if let Some(old) = self.authed_clients.remove(&info.login) {
            let _ = old.kick.try_send(LoginFailReason::ReasonAccountInUse);
            return AuthOutcome::AlreadyOnLs;
        }

        let key = SessionKey::random();
        self.authed_clients.insert(info.login.clone(), AuthedEntry { key, kick });
        AuthOutcome::Success { key, access_level: info.access_level, last_server: info.last_server }
    }

    fn record_failed_login_attempt(&mut self, ip: &str) {
        let attempts = self.failed_login_attempts.entry(ip.to_string()).or_insert(0);
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
        let expiry = if duration_ms > 0 { now_millis() + duration_ms } else { i64::MAX };
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
                if expiry > 0 && expiry < now_millis() {
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
