//! `org.l2jmobius.gameserver.model.World` — the single owner of all mutable
//! game state. Exactly one thread (the game thread) ever touches it, so it holds
//! no locks (CONCURRENCY_MODEL §2, challenge #2).
//!
//! Through G2 it carries the tick counter, the scheduler, the connected-client
//! sessions, and the login-link bookkeeping. Object registries, the region grid,
//! and managers land in the world/enter-world milestones (G3–G5).

use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::data::GameData;
use crate::db;
use crate::geo::GeoEngine;
use crate::loginlink::CommandTx;
use crate::scheduler::{ScheduledTask, Scheduler};
use crate::session::{ClientSession, SessionKey};

/// Java `World.SHIFT_BY`: world coordinates >> 11 ⇒ 2048-unit region cells
/// (16×16 regions per 32768-unit map tile).
pub const REGION_SHIFT: i32 = 11;

/// The region cell a world position falls in (Java `World.getRegion`, minus
/// the `OFFSET_X/Y` re-basing that only exists to index Java's fixed array).
pub fn region_of(x: i32, y: i32) -> (i32, i32) {
    (x >> REGION_SHIFT, y >> REGION_SHIFT)
}

/// Whether `b` lies in `a`'s 3×3 surrounding-region block (Java
/// `WorldRegion.isSurroundingRegion`) — the visibility rule every knownlist
/// query and broadcast is scoped by. Symmetric.
///
/// Java additionally materializes per-region object lists so a query never
/// scans the whole world; with players as the only world objects (until G8
/// NPCs) we get identical semantics from each player's stored region
/// coordinate + this adjacency test, with no grid to keep in sync.
pub fn regions_adjacent(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1
}

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
    /// `Config.STARTING_ADENA`, applied at character creation.
    pub starting_adena: i64,
    /// Static game data (templates, experience table, …).
    pub data: GameData,
    /// Geodata queries (LOS, walkability, heights). Constructed empty
    /// (`NullRegion` behaviour everywhere) and replaced with the loaded
    /// engine at boot — tests install synthetic regions instead.
    pub geo: GeoEngine,
    /// `Config.PATHFINDING` (`GeoEngine.ini`): non-zero = geodata movement
    /// checks are enforced (the pathfinder itself is not ported yet).
    pub path_finding: i32,
    /// Command channel to the DB thread.
    pub db: db::CmdTx,
    /// Game RNG (Java `Rnd`) — owned here so handlers roll through `roll()`,
    /// which tests can force (`forced_rolls`) for deterministic combat.
    pub rng: StdRng,
    /// Test hook: pre-queued values returned by `roll()` before touching the
    /// RNG. Cheaper and more explicit than seed archaeology in tests.
    #[cfg(test)]
    pub forced_rolls: std::collections::VecDeque<i32>,
}

impl World {
    pub fn new(link: CommandTx, max_characters_per_account: i32, delete_days: i32, starting_adena: i64, data: GameData, db: db::CmdTx) -> Self {
        Self {
            tick: 0,
            scheduler: Scheduler::new(),
            clients: HashMap::new(),
            players: HashMap::new(),
            login: LoginState::new(link),
            max_characters_per_account,
            delete_days,
            starting_adena,
            data,
            geo: GeoEngine::empty(),
            path_finding: 2,
            db,
            rng: StdRng::from_entropy(),
            #[cfg(test)]
            forced_rolls: std::collections::VecDeque::new(),
        }
    }

    /// Java `Rnd.get(bound)`: uniform in `[0, bound)`. Tests can pre-queue
    /// outcomes via `forced_rolls`.
    pub fn roll(&mut self, bound: i32) -> i32 {
        #[cfg(test)]
        if let Some(v) = self.forced_rolls.pop_front() {
            return v;
        }
        self.rng.gen_range(0..bound.max(1))
    }

    /// Every task the scheduler says is due this tick, drained for the caller
    /// to dispatch (`game_loop::apply_due_tasks`) — task handlers need to send
    /// packets to `self.clients`, so dispatch lives on the game-loop side
    /// rather than here (mirrors how packet handlers already work).
    pub fn drain_due_tasks(&mut self) -> Vec<ScheduledTask> {
        self.scheduler.drain_due(self.tick)
    }
}
