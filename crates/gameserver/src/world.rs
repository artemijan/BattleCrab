//! `org.l2jmobius.gameserver.model.World` — the single owner of all mutable
//! game state. Exactly one thread (the game thread) ever touches it, so it holds
//! no locks (CONCURRENCY_MODEL §2, challenge #2).
//!
//! Through G2 it carries the tick counter, the scheduler, the connected-client
//! sessions, and the login-link bookkeeping. Object registries, the region grid,
//! and managers land in the world/enter-world milestones (G3–G5).

use std::collections::HashMap;

use crate::data::GameData;
use crate::db;
use crate::loginlink::CommandTx;
use crate::scheduler::{ScheduledTask, Scheduler};
use crate::session::{ClientSession, SessionKey};

/// A client that finished `AuthLogin` and is awaiting the login server's
/// `PlayerAuthResponse` (Java `LoginServerThread.WaitingClient`).
pub struct WaitingClient {
    pub client_id: u32,
    pub session_key: SessionKey,
}

/// Login-link bookkeeping owned by the game thread (Java `LoginServerThread`'s
/// `_waitingClients` / `_accountsInGameServer`, moved here per the single-owner
/// model). The link task itself is just the encrypted pipe.
pub struct LoginState {
    /// Command channel to the login-link task.
    pub link: CommandTx,
    /// Accounts awaiting `PlayerAuthResponse`, keyed by account name.
    pub waiting: HashMap<String, WaitingClient>,
    /// Accounts currently logged into this game server → their client id.
    pub accounts_in_gameserver: HashMap<String, u32>,
    /// Assigned once the login server registers us.
    pub server_id: Option<i32>,
    pub server_name: Option<String>,
}

impl LoginState {
    fn new(link: CommandTx) -> Self {
        Self {
            link,
            waiting: HashMap::new(),
            accounts_in_gameserver: HashMap::new(),
            server_id: None,
            server_name: None,
        }
    }
}

pub struct World {
    /// Monotonic tick counter (10 ticks/s). This *is* `GameTimeTaskManager` —
    /// no dedicated game-time thread (CONCURRENCY_MODEL §2.4).
    pub tick: u64,
    pub scheduler: Scheduler,
    /// Connected clients keyed by network id, as type-state sessions (§3.1).
    pub clients: HashMap<u32, ClientSession>,
    /// In-world player entities keyed by object id (the `InGame` session links
    /// here). Object registries for NPCs/items/regions arrive in G5+.
    pub players: HashMap<i32, crate::model::Player>,
    pub login: LoginState,
    /// `Config.MAX_CHARACTERS_NUMBER_PER_ACCOUNT`, needed by `CharSelectionInfo`.
    pub max_characters_per_account: i32,
    /// `Config.DELETE_DAYS`: 0 = delete immediately, else mark with a timer.
    pub delete_days: i32,
    /// Static game data (templates, experience table, …).
    pub data: GameData,
    /// Command channel to the DB thread.
    pub db: db::CmdTx,
}

impl World {
    pub fn new(link: CommandTx, max_characters_per_account: i32, delete_days: i32, data: GameData, db: db::CmdTx) -> Self {
        Self {
            tick: 0,
            scheduler: Scheduler::new(),
            clients: HashMap::new(),
            players: HashMap::new(),
            login: LoginState::new(link),
            max_characters_per_account,
            delete_days,
            data,
            db,
        }
    }

    /// Run every task the scheduler says is due this tick. Dead-id tasks are
    /// no-ops (handled per-variant as real tasks are added).
    pub fn run_due_tasks(&mut self) {
        for task in self.scheduler.drain_due(self.tick) {
            match task {
                ScheduledTask::Noop { .. } => {}
            }
        }
    }
}
