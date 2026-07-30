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
use crate::geo::GeoEngine;
use crate::loginlink::CommandTx;
use crate::scheduler::{ScheduledTask, Scheduler};
use crate::session::{ClientSession, SessionKey};
use crate::store::EntityStore;
use rand::{Rng, SeedableRng};

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
    /// Per-connection hardware fingerprint (Java `GameClient._hardwareInfo`,
    /// G31), keyed by client id — reported by `RequestHardWareInfo`, read by the
    /// HWID punishment matching and `//hwid`. Cleared on disconnect.
    pub hwids: HashMap<u32, crate::network::client_packets::HardwareInfo>,
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
    /// Per-effect-zone next-fire tick, keyed by index into
    /// `data.zone_data.zones` — see [`crate::game_loop::effect_zones`].
    pub effect_zone_next_tick: HashMap<usize, u64>,
    pub minions_placed: usize,
    /// Forge of the Gods: kills since the last 15 s `FogRefresh` reset — the
    /// escalation counter behind the Lavasaurus ambush tiers (Java's static
    /// `_npcCount`).
    pub fog_kill_count: i32,
    /// Four Sepulchers per-hall run state (progress, entry clock, tracked
    /// wave spawns).
    pub four_sepulchers: crate::game_loop::four_sepulchers::FsState,
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
    /// `Config.DEBUG_CLIENT_PACKETS` — runtime-mutable from the GM Debug
    /// panel (`//debug packets on|off`); `dispatch::on_packet` logs every
    /// inbound opcode at info level while set.
    pub debug_packets: bool,
    /// The game thread's shutdown handle (`//server_shutdown`); `None` only in
    /// tests, where requesting it is a no-op.
    pub shutdown_signal: Option<crate::game_loop::Shutdown>,
    /// A running `//server_shutdown|restart` countdown: (deadline tick,
    /// restart?). Cleared by `//server_abort`.
    pub pending_shutdown: Option<(u64, bool)>,
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
    /// Live clan wars (Java: the shared `ClanWar` objects both clans map in
    /// `_atWarWith`; here one flat list, looked up by either side's id).
    /// Loaded from `clan_wars` at boot, mutated by `game_loop/clans.rs`.
    pub clan_wars: Vec<crate::model::clan::ClanWar>,
    /// Java `CrestTable._crests` — every stored crest bitmap, keyed by id.
    /// Loaded from `crests` at boot, mutated by `game_loop/clans.rs`.
    pub crests: HashMap<i32, crate::model::clan::Crest>,
    /// Java `CrestTable._nextId` — the next id `create_crest` allocates.
    /// Never reused even after the crest is deleted, so a stale client-side
    /// cache never shows the wrong image for a new crest at the same id.
    pub next_crest_id: i32,
    /// Java `ClanEntryManager._waitingList` — clanless players advertising
    /// themselves, keyed by player id.
    pub recruit_waiting: HashMap<i32, crate::model::clan_entry::PledgeWaitingInfo>,
    /// Java `ClanEntryManager._clanList` — clans advertising on the
    /// recruitment board, keyed by clan id.
    pub recruit_clans: HashMap<i32, crate::model::clan_entry::PledgeRecruitInfo>,
    /// Java `ClanEntryManager._applicantList` — pending applications, keyed
    /// by clan id then applicant player id.
    pub recruit_applicants:
        HashMap<i32, HashMap<i32, crate::model::clan_entry::PledgeApplicantInfo>>,
    /// Java `ClanEntryManager._playerLocked`/`_clanLocked` — the 5-minute
    /// re-registration cooldown after a cancelled waiting-list/board entry,
    /// as the tick it expires at.
    pub recruit_player_lock: HashMap<i32, u64>,
    pub recruit_clan_lock: HashMap<i32, u64>,

    /// Grand-boss spawn/status records, keyed by boss NPC id (Java
    /// `GrandBossManager._storedInfo`/`_bossStatus`). Loaded once at boot
    /// (`DbEvent::GrandBossesLoaded`), filtered to bosses with a known NPC
    /// template. Backs the read-only `//grandboss` admin panel.
    pub grand_bosses: HashMap<i32, crate::model::grand_boss::GrandBoss>,

    /// Valakas's lifetime lair-entry count (Java `ValakasTeleporters`'s
    /// `static int playerCount`). It **only ever increments** — never reset on
    /// spawn/death/window — so after 200 entries the lair locks until restart.
    /// Ported faithfully (the Core-minions precedent); the 200 cap and the
    /// Klein crowding htmls both read it.
    pub valakas_entry_count: u32,

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

    /// Castle-manor production/procure state (Java `CastleManorManager`), loaded
    /// at boot (`DbEvent::ManorLoaded`) from `castle_manor_production` /
    /// `castle_manor_procure`. The static seed catalogue is on `data.manor`.
    pub manor: crate::model::manor::ManorState,

    /// The clan halls (Java `ClanHallData`), keyed by id — the static defs from
    /// `GameData` with ownership overlaid at boot (`DbEvent::ClanHallsLoaded`
    /// from the `clanhall` table).
    pub clan_halls: HashMap<i32, crate::model::clan_hall::ClanHall>,

    /// Live clan-hall auction bids (Java `ClanHallAuction._bidders`), keyed by
    /// `hall_id → clan_id → bid`. Loaded from `clanhall_auctions_bidders`.
    pub clan_hall_bids: HashMap<i32, HashMap<i32, crate::model::clan_hall::ClanHallBid>>,

    /// Active clan-hall functions (Java `ClanHall._functions`), keyed by
    /// `hall_id → func_id → function`. Loaded from `residence_functions`.
    pub clan_hall_functions: HashMap<i32, HashMap<i32, crate::model::clan_hall::ActiveFunction>>,

    /// The `tick` the current clan-hall auction cycle closes (Java
    /// `ClanHallAuctionManager.getRemainingTime`); set when the weekly close is
    /// armed. Drives the auctioneer's countdown fields.
    pub auction_end_tick: u64,

    /// Per-castle siege-guard spawn points (`castle_siege_guards`, the stationed
    /// garrison), spawned at siege start (`DbEvent::SiegeGuardsLoaded`).
    pub siege_guards: HashMap<i32, Vec<crate::model::siege::SiegeSpawn>>,

    /// The Grand Olympiad (G25): period state, the noble registry, and the two
    /// registration queues (Java `Olympiad` + `OlympiadManager`).
    pub olympiad: crate::model::olympiad::OlympiadState,

    /// Instance allocator + live-instance registry (G27, Java `InstanceManager`).
    pub instances: crate::model::instance::InstanceManager,

    /// The event engine's runtime (TvT and future events — G28).
    pub events: crate::model::event::EventManager,

    /// The weekly Lucky Lottery runtime (G26.5).
    pub lottery: crate::model::lottery::LotteryState,

    /// The item-auction house runtime (G30.5).
    pub item_auctions: crate::model::item_auction::ItemAuctionManager,

    /// Party matching rooms + the looking-for-party waiting list (G30) — the
    /// party half of Java `MatchingRoomManager`.
    pub matching_rooms: crate::model::matching_room::MatchingRoomManager,

    /// Every mail message in the world + their attachment containers (G30) —
    /// Java `MailManager`. Both parties to a message can be offline, so this
    /// is world state with write-through persistence, not a player component.
    pub mail: crate::model::mail::MailManager,

    /// Offline character name -> object id, loaded once at boot (Java
    /// `CharInfoTable`). Mail is addressed by name to characters who need not
    /// be online, which is the only reason this exists. Keys are lowercased.
    pub char_ids_by_name: std::collections::HashMap<String, i32>,

    /// The Monster Race Track runtime (G26.5).
    pub monster_race: crate::model::monster_race::MonsterRaceState,

    /// The active-punishment registry — jail/ban/chat-ban (G31).
    pub punishments: crate::model::punishment::PunishmentManager,

    /// The in-memory GM petition queue (G31).
    pub petitions: crate::model::petition::PetitionManager,

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
    /// Live command channels (Java `CommandChannel` objects, also
    /// registry-less there — linked from each `Party`).
    pub command_channels: HashMap<u32, crate::model::command_channel::CommandChannel>,
    pub next_command_channel_id: u32,
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
    pub rng: rand::rngs::StdRng,
    /// The skill id driving the damage currently being applied, so quest
    /// `onAttack` handlers can tell a skill hit from a melee swing (Java passes
    /// `Skill skill` to `onAttack`). Set by the skill-damage path around
    /// `apply_physical_damage` and read by `quests::notify_attack`; `None` on
    /// the auto-attack path. Transient — never persisted, cleared after use.
    pub(crate) quest_attack_skill: Option<i32>,
    /// Test hook: pre-queued values returned by `roll()` before touching the
    /// RNG. Cheaper and more explicit than seed archaeology in tests.
    #[cfg(test)]
    pub forced_rolls: std::collections::VecDeque<i32>,
}

impl World {
    pub fn new(
        link: CommandTx,
        max_characters_per_account: i32,
        delete_days: i32,
        starting_adena: i64,
        data: GameData,
        db: db::CmdTx,
    ) -> Self {
        // Quest 350's Soul Crystal kill/skill-see NPCs come from data, not a
        // compile-time table, so gather them before `data` moves into the struct.
        let soul_crystal_npc_ids: Vec<i32> = data.soul_crystal_data.leveling_npc_ids().collect();
        Self {
            tick: 0,
            scheduler: Scheduler::new(),
            clients: HashMap::new(),
            hwids: HashMap::new(),
            objects: EntityStore::new(),
            npc_regions: HashMap::new(),
            effect_zone_next_tick: HashMap::new(),
            minions_placed: 0,
            fog_kill_count: 0,
            four_sepulchers: Default::default(),
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
            debug_packets: false,
            shutdown_signal: None,
            pending_shutdown: None,
            cfg: crate::config::CombatConfig::default(),
            quests: std::sync::Arc::new(crate::scripts::build_registry(soul_crystal_npc_ids)),
            clans: HashMap::new(),
            clan_wars: Vec::new(),
            crests: HashMap::new(),
            next_crest_id: 1,
            recruit_waiting: HashMap::new(),
            recruit_clans: HashMap::new(),
            recruit_applicants: HashMap::new(),
            recruit_player_lock: HashMap::new(),
            recruit_clan_lock: HashMap::new(),
            grand_bosses: HashMap::new(),
            valakas_entry_count: 0,
            cursed_weapons: Vec::new(),
            castles: Vec::new(),
            sieges: HashMap::new(),
            manor: crate::model::manor::ManorState::default(),
            clan_halls: HashMap::new(),
            clan_hall_bids: HashMap::new(),
            clan_hall_functions: HashMap::new(),
            auction_end_tick: 0,
            siege_guards: HashMap::new(),
            olympiad: crate::model::olympiad::OlympiadState::default(),
            instances: crate::model::instance::InstanceManager::default(),
            events: crate::model::event::EventManager::default(),
            lottery: crate::model::lottery::LotteryState::default(),
            item_auctions: crate::model::item_auction::ItemAuctionManager::default(),
            matching_rooms: crate::model::matching_room::MatchingRoomManager::default(),
            mail: crate::model::mail::MailManager::default(),
            char_ids_by_name: std::collections::HashMap::new(),
            monster_race: crate::model::monster_race::MonsterRaceState::default(),
            punishments: crate::model::punishment::PunishmentManager::default(),
            petitions: crate::model::petition::PetitionManager::default(),
            premium: HashMap::new(),
            buffer_schemes: HashMap::new(),
            bbs_favorites: HashMap::new(),
            next_fav_id: 1,
            cb_last_bypass: HashMap::new(),
            parties: HashMap::new(),
            next_party_id: 1,
            command_channels: HashMap::new(),
            next_command_channel_id: 1,
            duels: HashMap::new(),
            next_duel_id: 1,
            mob_groups: HashMap::new(),
            ground_item_regions: HashMap::new(),
            request_seq: 0,
            reco_give_seq: 0,
            db,
            player_autosave_due: HashMap::new(),
            rng: rand::rngs::StdRng::from_entropy(),
            quest_attack_skill: None,
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
                if let Some(ids) = self
                    .ground_item_regions
                    .get(&(region.0 + dx, region.1 + dy))
                {
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
            let _ = self.db.send(crate::db::DbCommand::ReserveIds {
                count: crate::db::ID_BLOCK_SIZE,
            });
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

    /// Java `Rnd.nextGaussian()` — a standard normal draw (mean 0, sd 1),
    /// used by `Formulas.calcMagicAffected`.
    ///
    /// Box–Muller over two [`roll_f64`](Self::roll_f64) draws rather than a
    /// distribution crate, so tests can still force the outcome through the
    /// same `forced_rolls` queue every other roll uses. The stream will not
    /// match Java's `java.util.Random` draw-for-draw — no RNG on this port
    /// does — only the distribution.
    pub fn roll_gaussian(&mut self) -> f64 {
        // `u1` must be non-zero for `ln`; `roll_f64` is [0, 1).
        let u1 = self.roll_f64().max(f64::MIN_POSITIVE);
        let u2 = self.roll_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Roll an augmentation's two option ids (Java `generateRandomVariation`).
    /// Lives here so the split borrow — `data.variations` (read) vs. the RNG
    /// draw — stays disjoint; the closure mirrors [`roll_f64`] so tests can
    /// force it via `forced_rolls`.
    pub fn roll_augment(&mut self, mineral_id: i32, is_magic_weapon: bool) -> Option<(i32, i32)> {
        #[cfg(test)]
        {
            let World {
                data,
                forced_rolls,
                rng,
                ..
            } = self;
            let mut roll = || {
                if let Some(v) = forced_rolls.pop_front() {
                    return v as f64 / 1_000_000.0;
                }
                use rand::Rng;
                rng.gen_range(0..1_000_000) as f64 / 1_000_000.0
            };
            data.variations
                .generate(mineral_id, is_magic_weapon, &mut roll)
        }
        #[cfg(not(test))]
        {
            let World { data, rng, .. } = self;
            let mut roll = || {
                use rand::Rng;
                rng.gen_range(0..1_000_000) as f64 / 1_000_000.0
            };
            data.variations
                .generate(mineral_id, is_magic_weapon, &mut roll)
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
