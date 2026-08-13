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
use crate::session::{ClientSession, ClientTable, SessionKey};
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
    /// Carries its own object-id → client-id reverse index; see [`ClientTable`].
    pub clients: ClientTable,
    /// Per-connection hardware fingerprint (Java `GameClient._hardwareInfo`,
    /// G31), keyed by client id — reported by `RequestHardWareInfo`, read by the
    /// HWID punishment matching and `//hwid`. Cleared on disconnect.
    pub hwids: HashMap<u32, crate::network::client_packets::HardwareInfo>,
    /// Per-connection client protocol version (Java
    /// `GameClient._protocolVersion`), keyed by client id. Reported by the
    /// handshake, read only by `//charinfo`. Cleared on disconnect alongside
    /// [`World::hwids`], for the same reason: it belongs to the connection,
    /// not the character.
    pub protocol_versions: HashMap<u32, i32>,
    /// Players whose inventory refreshes are suppressed (Java
    /// `Player._inventoryDisable`), by object id.
    ///
    /// Set when a shop / warehouse / wear window opens and cleared 1500 ms
    /// later by [`crate::scheduler::ScheduledTask::InventoryEnable`]. The
    /// client emits spurious `RequestItemList`s while such a window is coming
    /// up, and answering them clobbers the window it just drew.
    pub inventory_blocked: std::collections::HashSet<i32>,
    /// Players whose siege-zone fame task is already running. Java holds a
    /// per-player `ScheduledFuture` and null-checks it; this set is that check
    /// — without it, every zone revalidation inside the zone would arm another
    /// earner on top of the last.
    pub siege_fame_armed: std::collections::HashSet<i32>,
    /// The unattended private shops (Java: players whose `GameClient` is
    /// *detached*), keyed by player object id. They are the only players in
    /// `objects` with no entry in `clients`, so the visibility scans — which
    /// enumerate players through `clients` — read this as a second source of
    /// subjects. See `game_loop::offline_trade`.
    pub offline_traders: HashMap<i32, crate::game_loop::offline_trade::OfflineTrader>,
    /// Every in-world object — players and NPCs — as entities in one
    /// `bevy_ecs` world, keyed by object id (stage 2 phase 6; the `Player`/
    /// `Npc` residual-core components are the kind markers). The `InGame`
    /// session links here by id.
    pub objects: EntityStore,
    /// Region cell → NPC object ids in it — the materialized side of Java's
    /// per-region object lists, built at spawn and kept current by
    /// `visibility::update_npc_region`.
    pub npc_regions: rustc_hash::FxHashMap<(i32, i32), Vec<i32>>,
    /// Region cell → **player** object ids in it — the player half of the same
    /// index, and the reason a broadcast no longer costs one adjacency compare
    /// per connected client.
    ///
    /// Every broadcast is scoped to a 3×3 block of cells
    /// ([`regions_adjacent`]), so the recipients of one are the occupants of at
    /// most nine cells. Enumerating sessions instead made every send O(players
    /// online) — fine at 20 players, quadratic at a siege.
    ///
    /// Indexes **every** object carrying a `Player` component, including
    /// unattended shops (`offline_traders`), which have no session; consumers
    /// resolve a session per id and skip the misses. Maintained only through
    /// [`set_player_region`](Self::set_player_region) /
    /// [`index_player`](Self::index_player) /
    /// [`unindex_player`](Self::unindex_player) — never written directly, so it
    /// cannot drift from the `RegionCell` components it mirrors.
    /// [`debug_check_player_regions`](Self::debug_check_player_regions)
    /// re-derives it from the ECS and is asserted on every tick in debug and
    /// test builds.
    player_regions: rustc_hash::FxHashMap<(i32, i32), Vec<i32>>,
    /// `dbSave` spawn definitions the static spawn pass deliberately left
    /// unplaced, awaiting their `npc_respawns` rows — see
    /// [`crate::game_loop::boss_respawn`]. Drained once at boot.
    /// Running tally of minions placed by the current spawn pass, so
    /// `spawn_all`'s reported count matches the world's NPC population.
    /// Per-effect-zone next-fire tick, keyed by index into
    /// `data.zone_data.zones` — see [`crate::game_loop::effect_zones`].
    pub effect_zone_next_tick: HashMap<usize, u64>,
    /// The shadow items with a mana beat already in flight, keyed by object id
    /// and valued with the tick that beat is due to fire — Java's per-`Item`
    /// `_consumingMana` flag, which `ItemManaTaskManager.add` consults so one
    /// item never queues twice. The due tick is the staleness check a bare set
    /// cannot give us: Java's flag dies with the `Item` instance at logout, so
    /// a beat left in flight by a previous session must not be mistaken for
    /// this one's. See [`crate::game_loop::item_mana`].
    pub item_mana_consuming: std::collections::HashMap<i32, u64>,
    /// Items shift-clicked into a chat line, as item object id → the object id
    /// of the player who linked it — Java's per-`Item` `_published` flag, set
    /// by `Say2.parseAndPublishItem` and read by `RequestExRqItemLink` before
    /// it answers a reader's click on the link. Held world-side (not on the
    /// item) because it is session state, not saved item state; the publisher
    /// id lets it die with that player's logout, as Java's flag does with the
    /// `Item` instance. See [`crate::game_loop::chat`].
    pub published_items: HashMap<i32, i32>,
    /// World-chat reuse: speaker object id → the unix-millis instant at which
    /// their next line is allowed (Java `ChatWorld`'s static `REUSE` map).
    ///
    /// **Deliberately not cleared on logout.** Java sweeps it only lazily
    /// (`REUSE.values().removeIf(now::isAfter)` at the top of each call), so an
    /// entry outlives its speaker's session and a relog inside the window is
    /// still refused. Clearing it in `on_player_leave_world` would turn a
    /// logout into a way to skip the cooldown.
    pub world_chat_reuse: HashMap<i32, i64>,
    pub minions_placed: usize,
    /// Forge of the Gods: kills since the last 15 s `FogRefresh` reset — the
    /// escalation counter behind the Lavasaurus ambush tiers (Java's static
    /// `_npcCount`).
    pub fog_kill_count: i32,
    /// The Mammon merchants' script-owned spawns (`ai/others/Mammons/*`):
    /// npc id → the object id of the copy this script placed, i.e. Java's
    /// `_lastSpawn`. Tracked rather than looked up by npc id because the
    /// Priest of Mammon (33511) also has seven *static* spawns in the dist —
    /// searching by id would relocate one of those instead.
    pub mammon_spawns: HashMap<i32, i32>,
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
    /// Ferry route definitions (waypoints + dock schedules), registered by
    /// `boats::spawn_boats` at boot (tests register synthetic routes). `Boat`
    /// components and boat scheduler tasks refer to these by `RouteId`.
    pub boat_routes: crate::model::boat::BoatRoutes,
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
    /// The same `GeoEngine.ini` pathfinding tuning the worker runs with, for
    /// the one caller that searches on the game thread: `//path_find` (Java's
    /// `AdminPathNode` calls `PathFinding.findPath` inline as well).
    pub path_cfg: crate::geo::path::PathConfig,
    /// `Config.GEOEDIT_PATH` (`GeoEngine.ini` `GeoEditPath`) — where
    /// `//geosave`/`//geosaveall` export edited regions.
    pub geoedit_path: String,
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

    /// Active castle functions, keyed `(castle_id, func_type)` (Java
    /// `Castle._function`). **Runtime-only, like Java on this dist**: the
    /// reference's `Castle` constructor has `initFunctions()` commented out,
    /// so bought functions never survive a restart there either — the only
    /// divergence is that Java still writes the (never-read) DB rows.
    pub castle_functions: HashMap<(i32, i32), crate::model::castle::CastleFunc>,

    /// Clan notices (`clan_notices`, Java `Clan._notice`/`_noticeEnabled`),
    /// keyed by clan id — the board's clan page edits them. Loaded at boot
    /// beside the clans, saved through `DbCommand::SaveClanNotice`.
    pub clan_notices: HashMap<i32, (bool, String)>,

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

    /// Every character's ignore list — owner id → blocked ids (Java
    /// `BlockList`, the `relation = 1` rows of `character_friends`).
    ///
    /// **World state rather than a player component, because the party that
    /// matters can be offline.** `RequestSendPost` must know whether an offline
    /// addressee has blocked the sender; Java answers that with a static
    /// `OFFLINE_LIST` sitting beside the per-player object, and this map is
    /// both of those at once. Loaded whole at boot next to
    /// [`Self::char_ids_by_name`] and kept current on every add/remove — which
    /// can only happen while the *owner* is online, so it cannot go stale.
    ///
    /// Holds only the `isInBlockList` half of Java's `isBlocked`; the
    /// `isBlockAll` half is the live message-refusal flag. Always ask through
    /// `game_loop::block_list::is_blocked`.
    pub block_lists: std::collections::HashMap<i32, std::collections::HashSet<i32>>,

    /// The Monster Race Track runtime (G26.5).
    pub monster_race: crate::model::monster_race::MonsterRaceState,

    /// The active-punishment registry — jail/ban/chat-ban (G31).
    pub punishments: crate::model::punishment::PunishmentManager,

    /// Java `BotReportTable`'s three registries — who reported whom, each
    /// reporter's daily point budget, and the per-address cooldown.
    pub bot_reports: crate::game_loop::bot_report::BotReportTable,

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
    /// Java `Creature.getSkillChannelized()` — per target, which channelers are
    /// currently holding which `channelingSkillId` on them.
    ///
    /// Keyed target → channeled-skill-id → set of channeler object ids. The set
    /// **size is the level** the channeled skill lands at, so this is not
    /// bookkeeping: it is the mechanic. Entries are added every channeling tick
    /// and dropped by `stop_channelizing` when a cast ends.
    pub channelized: HashMap<i32, HashMap<i32, std::collections::HashSet<i32>>>,
    /// Java `GlobalVariablesManager` — the `global_variables` table, in memory.
    ///
    /// A flat string map on purpose: Java's is the same, and its keys are
    /// composed at the call site (`"FourSepulchers" + npcId`). Read through
    /// [`crate::game_loop::global_vars`], which owns the typed accessors and
    /// the write-through to the DB.
    pub global_vars: HashMap<String, String>,
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
    /// Same, for the per-player `PcCafeReward` task.
    pub pc_cafe_seq: u64,
    /// Command channel to the DB thread.
    pub db: db::CmdTx,
    /// Staggered periodic-autosave schedule (Java `PlayerAutoSaveTaskManager`):
    /// player object id → the tick its next flush is due. Populated on
    /// enter-world, drained one-per-cycle by `game_loop::autosave_tick`, and
    /// cleared on logout (where the final flush happens instead). The
    /// memory-first model's timer that bounds how much a crash can lose.
    pub player_autosave_due: HashMap<i32, u64>,
    /// Java `Player._teleportWatchdog` (`TeleportWatchdogTask`): player object
    /// id → the tick its teleport is force-completed on if the client's
    /// `Appearing` never arrives. One entry per in-flight teleport; armed by
    /// `death::teleport_player` when `TeleportWatchdogTimeout > 0`, and removed
    /// on completion (`Appearing`) or logout — Java's `ScheduledFuture` handle
    /// and its `cancel(false)` calls, expressed as a map rather than a
    /// scheduler entry precisely because it has to be cancellable.
    pub teleport_watchdog_due: HashMap<i32, u64>,
    /// Java `AutoPotionTaskManager.PLAYERS` — who has `.apon` switched on.
    /// Transient: a relog needs the command again, as it does in Java.
    pub auto_potion_players: std::collections::HashSet<i32>,
    /// Java `AutoPlayTaskManager.IDLE_COUNT` — passes an auto-player has spent
    /// doing nothing, which drives the unstick nudge.
    pub auto_play_idle: HashMap<i32, u32>,
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
    /// Test hook: a fixed wall clock for handlers that read [`World::now_millis`].
    ///
    /// Some Java behaviour is calendar-gated — the teleport fee halves from
    /// 20:00 on Mondays and Tuesdays, for one. A test that asserts a price
    /// while reading the real clock passes for most of the week and fails
    /// inside the window, which is a flake that reproduces on a schedule
    /// rather than at random and so reads as a broken build.
    #[cfg(test)]
    pub forced_now_millis: Option<i64>,
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
            clients: ClientTable::new(),
            hwids: HashMap::new(),
            protocol_versions: HashMap::new(),
            inventory_blocked: std::collections::HashSet::new(),
            siege_fame_armed: std::collections::HashSet::new(),
            offline_traders: HashMap::new(),
            objects: EntityStore::new(),
            npc_regions: rustc_hash::FxHashMap::default(),
            player_regions: rustc_hash::FxHashMap::default(),
            effect_zone_next_tick: HashMap::new(),
            item_mana_consuming: std::collections::HashMap::new(),
            published_items: HashMap::new(),
            world_chat_reuse: HashMap::new(),
            minions_placed: 0,
            fog_kill_count: 0,
            mammon_spawns: HashMap::new(),
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
            boat_routes: Default::default(),
            geo: std::sync::Arc::new(GeoEngine::empty()),
            path_finding: 2,
            path: std::sync::mpsc::channel().0,
            path_cfg: crate::geo::path::PathConfig::default(),
            geoedit_path: "saves/".to_string(),
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
            castle_functions: HashMap::new(),
            clan_notices: HashMap::new(),
            auction_end_tick: 0,
            siege_guards: HashMap::new(),
            olympiad: crate::model::olympiad::OlympiadState::default(),
            bot_reports: crate::game_loop::bot_report::BotReportTable::default(),
            instances: crate::model::instance::InstanceManager::default(),
            events: crate::model::event::EventManager::default(),
            lottery: crate::model::lottery::LotteryState::default(),
            item_auctions: crate::model::item_auction::ItemAuctionManager::default(),
            matching_rooms: crate::model::matching_room::MatchingRoomManager::default(),
            mail: crate::model::mail::MailManager::default(),
            char_ids_by_name: std::collections::HashMap::new(),
            block_lists: std::collections::HashMap::new(),
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
            channelized: HashMap::new(),
            global_vars: HashMap::new(),
            duels: HashMap::new(),
            next_duel_id: 1,
            mob_groups: HashMap::new(),
            ground_item_regions: HashMap::new(),
            request_seq: 0,
            reco_give_seq: 0,
            pc_cafe_seq: 0,
            db,
            player_autosave_due: HashMap::new(),
            teleport_watchdog_due: HashMap::new(),
            auto_potion_players: std::collections::HashSet::new(),
            auto_play_idle: HashMap::new(),
            rng: rand::rngs::StdRng::from_entropy(),
            quest_attack_skill: None,
            #[cfg(test)]
            forced_rolls: std::collections::VecDeque::new(),
            #[cfg(test)]
            forced_now_millis: None,
        }
    }

    /// The wall clock as handlers should read it — `commons::util::now_millis`,
    /// except that tests can pin it via `forced_now_millis`.
    ///
    /// Use this instead of calling `now_millis()` directly wherever the value
    /// feeds a *decision the client can observe* (a price, a gate, an
    /// availability window). Timestamps merely being *recorded* — a respawn
    /// deadline, a siege date — can keep using the free function.
    pub(crate) fn now_millis(&self) -> i64 {
        #[cfg(test)]
        if let Some(t) = self.forced_now_millis {
            return t;
        }
        commons::util::now_millis()
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

    /// Next `PcCafeReward` task generation (see `pc_cafe_seq`).
    pub fn next_pc_cafe_seq(&mut self) -> u64 {
        self.pc_cafe_seq += 1;
        self.pc_cafe_seq
    }

    /// The player object id behind `client_id`, or `None` when that client has
    /// no session or has not reached `InGame` — Java's `GameClient.getPlayer()`
    /// null check, which nearly every packet handler opens with.
    ///
    /// Handlers spell this `let Some(oid) = world.player_oid(client_id) else {
    /// return; };`, matching Java's "no player, no packet" bail.
    pub fn player_oid(&self, client_id: u32) -> Option<i32> {
        match self.clients.get(&client_id)? {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        }
    }

    /// Object ids of every in-game player — Java `World.getPlayers()`.
    ///
    /// Lazy on purpose: callers that only iterate pay no allocation. Anything
    /// needing `&mut World` inside the loop must `.collect::<Vec<_>>()` first,
    /// since the iterator borrows `self`.
    ///
    /// Iteration order follows the `clients` hash map and is therefore
    /// **unspecified** — sort at the call site when order is observable (as
    /// the admin character list does).
    pub fn in_game_player_oids(&self) -> impl Iterator<Item = i32> + '_ {
        self.in_game_clients().map(|(_, object_id)| object_id)
    }

    /// `(client id, player object id)` for every in-game player — the pair form
    /// of [`in_game_player_oids`], for the sweeps that send each player a packet
    /// on its own connection and so need the client id too.
    ///
    /// Same laziness and same unspecified iteration order as
    /// [`in_game_player_oids`].
    ///
    /// [`in_game_player_oids`]: Self::in_game_player_oids
    pub fn in_game_clients(&self) -> impl Iterator<Item = (u32, i32)> + '_ {
        self.clients.iter().filter_map(|(&client_id, cs)| match cs {
            ClientSession::InGame(s) => Some((client_id, s.player_object_id())),
            _ => None,
        })
    }

    /// Add a freshly spawned player object to the region index. Called once,
    /// where the entity enters the store (enter-world).
    pub fn index_player(&mut self, object_id: i32, region: (i32, i32)) {
        let ids = self.player_regions.entry(region).or_default();
        if !ids.contains(&object_id) {
            ids.push(object_id);
        }
    }

    /// Drop a player object from the region index — the despawn half. Call it
    /// **before** despawning, while the `RegionCell` is still readable.
    ///
    /// Finds the cell itself rather than trusting a caller-supplied one, and
    /// falls back to a full sweep if the id is not where its component says it
    /// is: a dangling entry would send a dead object id's packets forever.
    pub fn unindex_player(&mut self, object_id: i32) {
        use crate::model::components::RegionCell;
        if let Some(region) = self
            .objects
            .get_component::<RegionCell>(&object_id)
            .map(|r| r.0)
            && let Some(ids) = self.player_regions.get_mut(&region)
        {
            let before = ids.len();
            ids.retain(|&id| id != object_id);
            if ids.len() != before {
                if ids.is_empty() {
                    self.player_regions.remove(&region);
                }
                return;
            }
        }
        self.player_regions.retain(|_, ids| {
            ids.retain(|&id| id != object_id);
            !ids.is_empty()
        });
    }

    /// **The** writer of a player's `RegionCell`: moves the component and the
    /// index together, so the two cannot disagree. Returns the previous cell
    /// when it actually changed (the callers' "did I switch region?" test),
    /// `None` when the object stayed put or has no region cell.
    ///
    /// Every site that relocates a player — the movement tick, respawn, the
    /// teleport skill effects — must go through here.
    ///
    /// Objects that are **not** players (an NPC caught by a teleport effect)
    /// get their cell written and nothing else, exactly as the direct
    /// component writes this replaced did. Their own index, `npc_regions`, is
    /// `update_npc_region`'s business.
    pub fn set_player_region(&mut self, object_id: i32, new: (i32, i32)) -> Option<(i32, i32)> {
        use crate::model::components::RegionCell;
        let is_player = self
            .objects
            .has_component::<crate::model::Player>(&object_id);
        let cell = self.objects.get_component_mut::<RegionCell>(&object_id)?;
        let old = cell.0;
        if old == new {
            return None;
        }
        cell.0 = new;
        if !is_player {
            return Some(old);
        }
        if let Some(ids) = self.player_regions.get_mut(&old) {
            ids.retain(|&id| id != object_id);
            if ids.is_empty() {
                self.player_regions.remove(&old);
            }
        }
        self.player_regions.entry(new).or_default().push(object_id);
        Some(old)
    }

    /// [`players_visible_from`](Self::players_visible_from) narrowed to players
    /// with a live `InGame` session.
    ///
    /// The plain form also yields unattended shops (`offline_traders`), which
    /// are `Player` objects with no connection. Anything that used to enumerate
    /// `clients` and test adjacency wants **this** one, or it would start
    /// counting shops as present players — the difference between an empty town
    /// and one that keeps every nearby monster's region awake.
    pub fn in_game_players_visible_from(
        &self,
        region: (i32, i32),
    ) -> impl Iterator<Item = i32> + '_ {
        self.players_visible_from(region)
            .filter(|&oid| self.clients.client_of_player(oid).is_some())
    }

    /// The region cells holding at least one **connected** player — the seed
    /// set the NPC AI expands into Java's "active regions"
    /// (`WorldRegion.areNeighborsActive`).
    ///
    /// Unattended shops do not make a cell occupied: a town full of offline
    /// stores must not keep every monster around it thinking.
    pub fn occupied_player_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.player_regions.iter().filter_map(|(&cell, ids)| {
            ids.iter()
                .any(|&oid| self.clients.client_of_player(oid).is_some())
                .then_some(cell)
        })
    }

    /// Object ids of every player whose region cell lies in `region`'s 3×3
    /// surrounding block — the player half of Java's
    /// `World.forEachVisibleObject`, and the scope of every broadcast.
    ///
    /// Includes unattended shops; see
    /// [`in_game_players_visible_from`](Self::in_game_players_visible_from)
    /// when only connected players count.
    ///
    /// Borrows `self`, so a caller needing `&mut World` inside the loop must
    /// collect first. Order is unspecified (hash-map order), matching what the
    /// session scan it replaced gave.
    pub fn players_visible_from(&self, region: (i32, i32)) -> impl Iterator<Item = i32> + '_ {
        (-1..=1).flat_map(move |dx| {
            (-1..=1).flat_map(move |dy| {
                self.player_regions
                    .get(&(region.0 + dx, region.1 + dy))
                    .into_iter()
                    .flatten()
                    .copied()
            })
        })
    }

    /// Re-derive the player region index straight from the ECS and panic if it
    /// disagrees with the maintained one.
    ///
    /// The index is only correct while every `RegionCell` write on a player
    /// goes through [`set_player_region`](Self::set_player_region) — and a site
    /// that forgets does not fail to compile, it silently stops delivering
    /// broadcasts to whoever is nearby. So the game loop asserts this on every
    /// tick in debug and test builds, which puts the whole test suite behind
    /// the invariant; release builds pay nothing.
    #[cfg(debug_assertions)]
    pub fn debug_check_player_regions(&mut self) {
        use crate::model::components::RegionCell;
        let mut expected: rustc_hash::FxHashMap<(i32, i32), Vec<i32>> =
            rustc_hash::FxHashMap::default();
        self.objects
            .for_each_mut::<(&crate::model::Player, &RegionCell)>(|(p, cell)| {
                expected.entry(cell.0).or_default().push(p.object_id);
            });
        let mut actual: rustc_hash::FxHashMap<(i32, i32), Vec<i32>> = self.player_regions.clone();
        for ids in expected.values_mut() {
            ids.sort_unstable();
        }
        for ids in actual.values_mut() {
            ids.sort_unstable();
        }
        assert_eq!(
            actual, expected,
            "player region index drifted from the RegionCell components — a \
             player was moved without World::set_player_region"
        );
    }

    /// Java `Broadcast.toAllOnlinePlayers` — every session that has finished
    /// entering the world, with no region or instance filter at all.
    ///
    /// The `InGame` test is the whole point and is easy to drop: sessions still
    /// authenticating or sitting in character select are in [`World::clients`]
    /// too, and have no player to draw the packet against. Unattended private
    /// shops ([`World::offline_traders`]) have no session, so they never receive
    /// one of these — which matches Java, where a detached client is not an
    /// online player.
    ///
    /// This is the server-wide announcement channel: sieges, cursed weapons, the
    /// lottery, TvT, GM `//announce`, raid-boss spawns. Anything a player could
    /// plausibly *see* belongs on a region broadcast instead.
    pub fn broadcast_to_all_online(&self, packet: &[u8]) {
        // One refcounted buffer rather than a `to_vec()` per recipient — this
        // fans out to every player on the server.
        let shared = bytes::Bytes::copy_from_slice(packet);
        for cs in self.clients.values() {
            if matches!(cs, ClientSession::InGame(_)) {
                cs.send(shared.clone());
            }
        }
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
    /// The castle row for `castle_id` — Java `CastleManager.getCastleById`,
    /// which fifteen call sites re-derived with a linear find.
    pub fn castle(&self, castle_id: i32) -> Option<&crate::model::castle::Castle> {
        self.castles.iter().find(|c| c.id == castle_id)
    }

    /// Mutable [`Self::castle`].
    pub fn castle_mut(&mut self, castle_id: i32) -> Option<&mut crate::model::castle::Castle> {
        self.castles.iter_mut().find(|c| c.id == castle_id)
    }

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
