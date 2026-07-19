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
use crate::store::EntityStore;

/// One community-board favorite row (Java `bbs_favorites`). `add_date` is the
/// display string (`yyyy-MM-dd HH:mm:ss`, matching SQL `CURRENT_TIMESTAMP` and
/// Java's `SimpleDateFormat`), stored verbatim so it survives a round-trip.
#[derive(Clone, Debug)]
pub struct Favorite {
    pub fav_id: i32,
    pub title: String,
    pub bypass: String,
    pub add_date: String,
}

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
/// scans the whole world; player↔player checks get identical semantics from
/// each player's stored region coordinate + this adjacency test (few
/// players, no grid to keep in sync), while the 34.9k static NPCs *are*
/// indexed per region (`World::npc_regions`, built once at spawn).
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
    /// Fired once all boot data is loaded (static datapack synchronously at
    /// startup, then clans from the DB) to release the login-link task into its
    /// connect loop — Java runs `LoginServerThread.start()` dead-last, after
    /// `ClanTable`. Taken and signalled when `DbEvent::ClansLoaded` is
    /// processed; `None` in tests (nothing gates them).
    pub ready: Option<tokio::sync::oneshot::Sender<()>>,
}

impl LoginState {
    fn new(link: CommandTx) -> Self {
        Self {
            link,
            waiting: HashMap::new(),
            accounts_in_gameserver: HashMap::new(),
            server_id: None,
            server_name: None,
            ready: None,
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
    /// Every in-world object — players and NPCs — as entities in one
    /// `bevy_ecs` world, keyed by object id (stage 2 phase 6; the `Player`/
    /// `Npc` residual-core components are the kind markers). The `InGame`
    /// session links here by id.
    pub objects: EntityStore,
    /// Region cell → NPC object ids in it — the materialized side of Java's
    /// per-region object lists. NPCs are static (no AI movement yet), so this
    /// is built once at spawn; players keep using the cheap per-player
    /// adjacency compare instead (see `regions_adjacent`).
    pub npc_regions: HashMap<(i32, i32), Vec<i32>>,
    /// `dbSave` spawn definitions the static spawn pass deliberately left
    /// unplaced, awaiting their `npc_respawns` rows — see
    /// [`crate::game_loop::boss_respawn`]. Drained once at boot.
    /// Running tally of minions placed by the current spawn pass, so
    /// `spawn_all`'s reported count matches the world's NPC population.
    pub minions_placed: usize,
    pub pending_boss_spawns: Vec<(usize, usize, usize)>,
    /// npc id → its `dbSave` spawn definition, for the death/respawn writes
    /// (Java's `DBSpawnManager._spawns`).
    pub boss_spawn_refs: HashMap<i32, (usize, usize, usize)>,
    /// Region cell → door object ids in it — same shape as `npc_regions`
    /// (doors are static; built once by `model::door::spawn_doors`).
    pub door_regions: HashMap<(i32, i32), Vec<i32>>,
    /// Region cell → static-object (town map/throne) object ids.
    pub static_regions: HashMap<(i32, i32), Vec<i32>>,
    /// Next transient NPC object id (see `model::npc::FIRST_NPC_OBJECT_ID`).
    pub next_npc_object_id: i32,
    /// Persistent object-id block `[start, end)` reserved from the DB
    /// thread's `IdManager` counter (`DbEvent::IdBlock`) — runtime item
    /// creation (loot) allocates from here synchronously.
    pub id_pool: std::ops::Range<i64>,
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
    /// engine at boot — tests install synthetic regions instead. Shared with
    /// the path-worker thread, hence the `Arc` (read-only after boot).
    pub geo: std::sync::Arc<GeoEngine>,
    /// `Config.PATHFINDING` (`GeoEngine.ini`): non-zero = geodata movement
    /// checks are enforced and blocked moves are handed to the path worker.
    pub path_finding: i32,
    /// Request channel to the path-worker thread (`geo::worker`). `new()`
    /// installs a closed dummy channel (requests vanish — `NullRegion`-style
    /// no-op); boot and pathfinding tests replace it with a live worker's.
    pub path: crate::geo::worker::PathReqTx,
    /// Last issued path-request sequence number (`next_path_seq`) — replies
    /// carrying anything older than the requester's `PathWait` are stale.
    pub path_seq: u64,
    /// Combat/AI/reward config keys (`Character.ini`/`NPC.ini`/`Rates.ini`).
    /// Defaults (Java's, rates ×1) unless boot replaces it — same pattern as
    /// `geo`/`path_finding`.
    pub cfg: crate::config::CombatConfig,

    /// The compiled-in script registry (Java `QuestManager` + the boot-time
    /// `ScriptEngineManager` pass, minus the runtime compilation). `Arc` for
    /// the same reason as `geo`: call sites clone the handle, then hand
    /// `&mut World` into the script — no self-borrow. Immutable after boot.
    pub quests: std::sync::Arc<crate::game_loop::quests::QuestRegistry>,

    /// Every clan, keyed by clan id (Java `ClanTable._clans`). Loaded once
    /// at boot (`DbEvent::ClansLoaded`); `create_clan` inserts.
    pub clans: HashMap<i32, crate::model::clan::Clan>,

    /// Grand-boss spawn/status records, keyed by boss NPC id (Java
    /// `GrandBossManager._storedInfo`/`_bossStatus`). Loaded once at boot
    /// (`DbEvent::GrandBossesLoaded`), filtered to bosses with a known NPC
    /// template. Backs the read-only `//grandboss` admin panel.
    pub grand_bosses: HashMap<i32, crate::model::grand_boss::GrandBoss>,

    /// The cursed weapons (Java `CursedWeaponsManager._cursedWeapons`), built at
    /// boot from `CursedWeapons.xml` config + the `cursed_weapons` state table
    /// (`DbEvent::CursedWeaponsLoaded`). Two on this dist (Zariche/Akamanah).
    pub cursed_weapons: Vec<crate::model::cursed_weapon::CursedWeapon>,

    /// The castles (Java `CastleManager._castles`), keyed by residence id.
    /// Loaded once at boot (`DbEvent::CastlesLoaded`); ownership is resolved
    /// against `clans` (the owning clan's `castle_id`).
    pub castles: Vec<crate::model::castle::Castle>,

    /// Each castle's siege (Java `Castle.getSiege()`), keyed by residence id.
    /// One per castle, built at boot (`DbEvent::SiegesLoaded`) from the
    /// `siege_clans` table. In-progress state is runtime-only.
    pub sieges: HashMap<i32, crate::model::siege::Siege>,

    /// Per-castle siege-guard spawn points (`castle_siege_guards`, the stationed
    /// garrison), spawned at siege start (`DbEvent::SiegeGuardsLoaded`).
    pub siege_guards: HashMap<i32, Vec<crate::model::siege::SiegeSpawn>>,

    /// Account premium expirations (`account_name` lowercase → enddate millis),
    /// the in-memory mirror of `account_premium` (Java `PremiumManager._premiumData`).
    /// Boot-loaded from the whole table (`DbEvent::PremiumLoaded`) rather than
    /// Java's lazy per-login load, so `//premium_*` works for any account.
    pub premium: HashMap<String, i64>,

    /// Community-board buffer schemes, the in-memory mirror of `buffer_schemes`
    /// (Java `SchemeBufferTable._schemesTable`): character object-id → its list
    /// of `(scheme_name, skill_ids)`. Boot-loaded from the whole table
    /// (`DbEvent::BufferSchemesLoaded`); create/delete write through immediately
    /// (Java bulk-saves at shutdown — this port avoids the shutdown hook the same
    /// way `premium` does). Scheme names are matched case-insensitively, like
    /// Java's `TreeMap(CASE_INSENSITIVE_ORDER)`.
    pub buffer_schemes: HashMap<i32, Vec<(String, Vec<i32>)>>,

    /// Community-board favorites, the in-memory mirror of `bbs_favorites`
    /// (Java `FavoriteBoard`): character object-id (`playerId`) → its favorites,
    /// newest first (Java `ORDER BY favAddDate DESC`). Boot-loaded from the whole
    /// table (`DbEvent::FavoritesLoaded`); add/delete write through immediately,
    /// mirroring `buffer_schemes` (Java re-queries the DB after each change).
    pub bbs_favorites: HashMap<i32, Vec<Favorite>>,

    /// Next global `favId` to assign. `bbs_favorites.favId` is a table-wide
    /// AUTOINCREMENT primary key, so ids must be globally unique — Java lets SQL
    /// assign it, but the memory-first mirror needs the id up front to render the
    /// delete button, so this port allocates on the game thread. Seeded from the
    /// max loaded id + 1 at boot (`DbEvent::FavoritesLoaded`).
    pub next_fav_id: i32,

    /// Java `CommunityBoardHandler._bypasses`: the last board bypass a player
    /// navigated to (`title` → `bypass`), captured on `_bbshome`/`_bbstop` and
    /// popped by `bbs_add_fav` to build the favorite. Only the `HomeBoard` home
    /// branch registers one under the custom board.
    pub cb_last_bypass: HashMap<i32, (String, String)>,

    /// Live parties (`Party` objects have no Java-side registry — they only
    /// exist through member references; an id-keyed map is the Rust shape).
    pub parties: HashMap<u32, crate::model::party::Party>,
    pub next_party_id: u32,
    /// Running duels (Java `DuelManager._duels`), keyed by duel id.
    pub duels: HashMap<u32, crate::game_loop::duel::Duel>,
    pub next_duel_id: u32,
    /// GM mob groups (`MobGroupTable`), keyed by group id — `//mobgroup_*`.
    pub mob_groups: HashMap<i32, crate::model::mob_group::MobGroup>,
    /// Region cell → ground-item object ids in it (same shape as `npc_regions`;
    /// `ItemsOnGroundManager` visibility index).
    pub ground_item_regions: HashMap<(i32, i32), Vec<i32>>,
    /// Generation counter for `PendingRequest`s (stale `RequestTimeout`
    /// tasks no-op on mismatch, like `path_seq`).
    pub request_seq: u64,
    /// Generation counter for the per-player `RecoGive` task (stale firings
    /// left over from a previous session no-op on mismatch).
    pub reco_give_seq: u64,
    /// Command channel to the DB thread.
    pub db: db::CmdTx,
    /// Staggered periodic-autosave schedule (Java `PlayerAutoSaveTaskManager`):
    /// player object id → the tick its next flush is due. Populated on
    /// enter-world, drained one-per-cycle by `game_loop::autosave_tick`, and
    /// cleared on logout (where the final flush happens instead). The
    /// memory-first model's timer that bounds how much a crash can lose.
    pub player_autosave_due: HashMap<i32, u64>,
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
            objects: EntityStore::new(),
            npc_regions: HashMap::new(),
            minions_placed: 0,
            pending_boss_spawns: Vec::new(),
            boss_spawn_refs: HashMap::new(),
            door_regions: HashMap::new(),
            static_regions: HashMap::new(),
            next_npc_object_id: crate::model::npc::FIRST_NPC_OBJECT_ID,
            id_pool: 0..0,
            login: LoginState::new(link),
            max_characters_per_account,
            delete_days,
            starting_adena,
            data,
            geo: std::sync::Arc::new(GeoEngine::empty()),
            path_finding: 2,
            path: std::sync::mpsc::channel().0,
            path_seq: 0,
            cfg: crate::config::CombatConfig::default(),
            quests: std::sync::Arc::new(crate::scripts::build_registry()),
            clans: HashMap::new(),
            grand_bosses: HashMap::new(),
            cursed_weapons: Vec::new(),
            castles: Vec::new(),
            sieges: HashMap::new(),
            siege_guards: HashMap::new(),
            premium: HashMap::new(),
            buffer_schemes: HashMap::new(),
            bbs_favorites: HashMap::new(),
            next_fav_id: 1,
            cb_last_bypass: HashMap::new(),
            parties: HashMap::new(),
            next_party_id: 1,
            duels: HashMap::new(),
            next_duel_id: 1,
            mob_groups: HashMap::new(),
            ground_item_regions: HashMap::new(),
            request_seq: 0,
            reco_give_seq: 0,
            db,
            player_autosave_due: HashMap::new(),
            rng: StdRng::from_entropy(),
            #[cfg(test)]
            forced_rolls: std::collections::VecDeque::new(),
        }
    }

    /// Next path-request sequence number (see `path_seq`).
    pub fn next_path_seq(&mut self) -> u64 {
        self.path_seq += 1;
        self.path_seq
    }

    /// Next transaction-request sequence number (see `request_seq`).
    pub fn next_request_seq(&mut self) -> u64 {
        self.request_seq += 1;
        self.request_seq
    }

    /// Next `RecoGive` task generation (see `reco_give_seq`).
    pub fn next_reco_give_seq(&mut self) -> u64 {
        self.reco_give_seq += 1;
        self.reco_give_seq
    }

    /// Object ids of every NPC whose region cell lies in `region`'s 3×3
    /// surrounding block (the NPC half of Java's
    /// `World.forEachVisibleObject`), via the `npc_regions` index.
    pub fn npcs_visible_from(&self, region: (i32, i32)) -> Vec<i32> {
        let mut out = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.npc_regions.get(&(region.0 + dx, region.1 + dy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out
    }

    /// Ground-item object ids visible from a region's 3×3 block (same shape as
    /// `npcs_visible_from`).
    pub fn ground_items_visible_from(&self, region: (i32, i32)) -> Vec<i32> {
        let mut out = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.ground_item_regions.get(&(region.0 + dx, region.1 + dy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out
    }

    /// Door object ids visible from a region's 3×3 block (the door half of
    /// `npcs_visible_from`).
    pub fn doors_visible_from(&self, region: (i32, i32)) -> Vec<i32> {
        let mut out = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.door_regions.get(&(region.0 + dx, region.1 + dy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out
    }

    /// Static-object ids visible from a region's 3×3 block.
    pub fn statics_visible_from(&self, region: (i32, i32)) -> Vec<i32> {
        let mut out = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(ids) = self.static_regions.get(&(region.0 + dx, region.1 + dy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out
    }

    /// Allocate one persistent object id from the reserved block, topping the
    /// block up (`DbCommand::ReserveIds`) when it runs low. `None` only when
    /// the pool is exhausted before the DB thread's refill lands — callers
    /// (loot) skip the item and log; Java can't hit this because `IdManager`
    /// is a shared in-process bitmap.
    pub fn alloc_object_id(&mut self) -> Option<i32> {
        const LOW_WATER: i64 = 200;
        let remaining = self.id_pool.end - self.id_pool.start;
        if remaining == LOW_WATER {
            let _ = self.db.send(crate::db::DbCommand::ReserveIds { count: crate::db::ID_BLOCK_SIZE });
        }
        self.id_pool.next().map(|id| id as i32)
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

    /// Java `Rnd.nextDouble()` in `[0, 1)`, quantized through `roll()` so
    /// tests can force it with the same `forced_rolls` queue (a forced value
    /// `v` reads as `v / 1_000_000`).
    pub fn roll_f64(&mut self) -> f64 {
        self.roll(1_000_000) as f64 / 1_000_000.0
    }

    /// Roll an augmentation's two option ids (Java `generateRandomVariation`).
    /// Lives here so the split borrow — `data.variations` (read) vs. the RNG
    /// draw — stays disjoint; the closure mirrors [`roll_f64`] so tests can
    /// force it via `forced_rolls`.
    pub fn roll_augment(&mut self, mineral_id: i32, is_magic_weapon: bool) -> Option<(i32, i32)> {
        use rand::Rng;
        #[cfg(test)]
        {
            let World { data, forced_rolls, rng, .. } = self;
            let mut roll = || {
                if let Some(v) = forced_rolls.pop_front() {
                    return v as f64 / 1_000_000.0;
                }
                rng.gen_range(0..1_000_000) as f64 / 1_000_000.0
            };
            data.variations.generate(mineral_id, is_magic_weapon, &mut roll)
        }
        #[cfg(not(test))]
        {
            let World { data, rng, .. } = self;
            let mut roll = || rng.gen_range(0..1_000_000) as f64 / 1_000_000.0;
            data.variations.generate(mineral_id, is_magic_weapon, &mut roll)
        }
    }

    /// Every task the scheduler says is due this tick, drained for the caller
    /// to dispatch (`game_loop::apply_due_tasks`) — task handlers need to send
    /// packets to `self.clients`, so dispatch lives on the game-loop side
    /// rather than here (mirrors how packet handlers already work).
    pub fn drain_due_tasks(&mut self) -> Vec<ScheduledTask> {
        self.scheduler.drain_due(self.tick)
    }
}
