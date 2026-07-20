//! The DB thread (CONCURRENCY_MODEL §2.4). A dedicated OS thread owns the SQLite
//! pool; the game thread never blocks on the database — it sends [`DbCommand`]s
//! and drains [`DbEvent`]s each tick. Character id allocation lives here too
//! (a minimal `IdManager`).

use std::thread::JoinHandle;

use sqlx::{Row, SqlitePool};
use tracing::{error, info, warn};

use crate::character::{CharData, ItemRow};
use commons::util::now_millis;

/// First object id handed out by `IdManager` (Java `FIRST_OID`). Shared by
/// every world-object type (characters, items, …) — Java's `IdManager` is a
/// single pool, not one per type.
const FIRST_OID: i64 = 0x10000000;

/// How many object ids each `IdBlock` reservation hands the game thread.
pub const ID_BLOCK_SIZE: i64 = 5000;

/// A starting item, already slot-resolved by the game thread (see
/// `game_loop::handle_character_create`) so the DB thread just persists rows.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub item_id: i32,
    pub count: i64,
    /// `Some(paperdoll_index)` if equipped, `None` for a plain inventory item.
    pub paperdoll_index: Option<usize>,
}

/// An initial shortcut to persist at creation (`InitialShortcutData.
/// registerAllShortcuts`), already filtered by the game thread (unknown
/// skills / missing macro presets dropped). For `ShortcutType::Item` the `id`
/// is still the *item id* — the DB thread resolves it to the freshly created
/// item's object id (the game thread never learns those).
#[derive(Debug, Clone, Copy)]
pub struct NewShortcut {
    pub slot: i32,
    pub page: i32,
    pub kind: crate::model::shortcut::ShortcutType,
    pub id: i32,
    pub level: i32,
}

/// A validated character to insert (built by the game thread from the template).
#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub account: String,
    pub name: String,
    pub race: i32,
    pub class_id: i32,
    pub sex: i32,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub max_hp: i32,
    pub max_mp: i32,
    /// Initial `(skill_id, skill_level)` from the class skill tree.
    pub skills: Vec<(i32, i32)>,
    /// Initial equipment + starting adena, pre-resolved by the game thread.
    pub items: Vec<NewItem>,
    /// Initial panel shortcuts (`initialShortcuts.xml`, global + class pages).
    pub shortcuts: Vec<NewShortcut>,
    /// Macro presets referenced by MACRO shortcuts above.
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// `CharacterCreate`: `min(StartingVitalityPoints, MAX_VITALITY_POINTS)`
    /// when `EnableVitality`, else the column default (0). Resolved on the game
    /// thread, which owns the config.
    pub vitality_points: i32,
}

/// The persistable slice of a `Player`, snapshotted on the game thread when the
/// character leaves the world (restart / logout / disconnect) — Java
/// `Disconnection.storeMe().deleteMe()`. Covers the `storeCharBase` columns the
/// Rust `Player` actually tracks; the rest (clan, title, online time, faction,
/// …) keep their stored values. Java's companion stores — `storeCharSub`,
/// `storeEffect` (`character_skills_save`), item reuse — need systems that
/// don't exist yet (subclasses, buff restore on login) and are TODO(G-later).
/// Items and learned skills are already persisted at mutation time.
#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub object_id: i32,
    pub level: i32,
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_cp: i32,
    pub cur_cp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub sex: i32,
    pub heading: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pvp_kills: i32,
    pub pk_kills: i32,
    pub rec_have: i32,
    pub rec_left: i32,
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub vitality_points: i32,
    pub pccafe_points: i32,
    /// `characters.nobless` — Olympiad nobless, toggled by `//setnoble`.
    pub noble: bool,
}

impl PlayerSnapshot {
    pub fn of(
        p: &crate::model::Player,
        pos: &crate::model::components::Position,
        vitals: &crate::model::components::Vitals,
        pvitals: &crate::model::components::PlayerVitals,
    ) -> Self {
        Self {
            object_id: p.object_id,
            level: p.level,
            max_hp: vitals.max_hp,
            cur_hp: vitals.cur_hp,
            max_cp: pvitals.max_cp,
            cur_cp: pvitals.cur_cp,
            max_mp: vitals.max_mp,
            cur_mp: vitals.cur_mp,
            face: p.face,
            hair_style: p.hair_style,
            hair_color: p.hair_color,
            sex: p.is_female as i32,
            heading: pos.heading,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            exp: p.exp,
            sp: p.sp,
            reputation: p.reputation,
            pvp_kills: p.pvp_kills,
            pk_kills: p.pk_kills,
            rec_have: p.rec_have,
            rec_left: p.rec_left,
            race: p.race,
            class_id: p.class_id,
            base_class_id: p.base_class_id,
            vitality_points: p.vitality_points,
            pccafe_points: p.pccafe_points,
            noble: p.is_noble,
        }
    }
}

/// The full persistable state of an online player, gathered on the game thread
/// and flushed by the DB thread in one transaction (`store_player`). Built by
/// `game_loop::net::build_save_data` at the four flush points — the staggered
/// periodic autosave, logout, class-transfer, and shutdown save-all. Between
/// flushes, gameplay mutations (equip, loot, skill learn, shortcuts, quests)
/// touch only in-memory ECS components; nothing is written on the packet path,
/// so no client packet can drive database load (the memory-first model — Java
/// `Player.store()` gathers the same data, but Java also writes eagerly on many
/// actions, which is exactly what this port deliberately does not do).
#[derive(Debug, Clone)]
pub struct PlayerSaveData {
    /// The `characters` row (level/exp/vitals/position/appearance).
    pub base: PlayerSnapshot,
    /// Every item the character owns — inventory + equipped — serialized from
    /// the `Inventory` component (`Inventory::to_rows`). The DB thread deletes
    /// any `items` row for this owner not present here, so this is the whole
    /// authoritative set, covering pickups, drops, stack changes and equips.
    pub items: Vec<ItemRow>,
    /// Learned skills as `(skill_id, skill_level)` for the **active** class
    /// index (see [`Self::class_index`]).
    pub skills: Vec<(i32, i32)>,
    /// The *inactive* class indices' books (G17 subclasses), so a slot keeps
    /// what it learned while it was active.
    pub skills_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Inactive indices' worn hennas.
    pub hennas_by_index: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// Inactive indices' shortcut bars.
    pub shortcuts_by_index:
        std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>>,
    /// Which class index [`Self::skills`] belongs to.
    pub class_index: i32,
    /// Worn henna dyes as `(slot 1-3, symbol_id)` (class_index 0).
    pub hennas: Vec<(i32, i32)>,
    /// Registered recipes as `(recipe_list_id, is_dwarven)` — the `type` column
    /// (1 dwarven / 0 common) is derived on the game thread from `RecipeData`.
    pub recipe_book: Vec<(i32, bool)>,
    /// Panel/hotbar shortcuts (`Shortcuts` component).
    pub shortcuts: Vec<crate::model::shortcut::Shortcut>,
    /// Macro definitions (`Macros` component).
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// Quest states + vars (`Quests` component), keyed by quest name.
    pub quests: std::collections::HashMap<String, crate::model::quest::QuestState>,
    /// Live skill reuse cooldowns (`Reuses` component) as `character_skills_save`
    /// rows — empty when `StoreSkillCooltime` is off. See [`SkillReuseRow`].
    pub skill_reuses: Vec<SkillReuseRow>,
    /// Active buffs (`Buffs` component) as `character_skills_save` rows —
    /// empty when `StoreSkillCooltime` is off. See [`SkillBuffRow`].
    pub skill_buffs: Vec<SkillBuffRow>,
    /// `character_variables` rows (`PlayerVariables` component) as `(var, val)`.
    pub variables: Vec<(String, String)>,
}

/// One `character_skills_save` reuse row (Java `Player.storeEffect`'s
/// `restore_type = 1` half). `systime_ms` is the **absolute** wall-clock end
/// time (Java `TimeStamp.getStamp()`), so cooldowns decay by real elapsed time
/// across a relog/restart; the game side converts it to/from a game tick.
#[derive(Debug, Clone, Copy)]
pub struct SkillReuseRow {
    /// The reuse-map key (Java `getReuseHashCode()`): the reuse group id, or the
    /// skill id when the skill has no group. Stored in the `skill_id` column —
    /// Java-schema-compatible for the (common) ungrouped case, and the value the
    /// `Reuses` map is re-keyed by on restore.
    pub reuse_key: i32,
    pub skill_level: i32,
    /// Full reuse duration ms (`reuse_delay` column / `SkillReuse::total_ms`).
    pub reuse_delay: i32,
    /// Absolute wall-clock instant the cooldown ends (`systime` column, ms).
    pub systime_ms: i64,
}

/// One `character_skills_save` **buff** row (Java `Player.storeEffect`'s
/// `restore_type = 0` half). Unlike a reuse row this stores a *relative*
/// `remaining_time` (seconds left at logout), never an absolute instant:
/// Java's `restoreEffects` feeds it straight back into
/// `skill.applyEffects(this, this, false, remainingTime)`, so a buff's
/// countdown is **frozen while the character is offline**. Buffs deliberately
/// do not decay across the offline gap the way cooldowns (which carry an
/// absolute `systime`) do.
#[derive(Debug, Clone, Copy)]
pub struct SkillBuffRow {
    pub skill_id: i32,
    pub skill_level: i32,
    /// Seconds of buff time left (`remaining_time` column, Java
    /// `BuffInfo.getTime()`).
    pub remaining_time_secs: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateResult {
    Ok,
    NameExists,
    TooMany,
    Fail,
}

/// Game thread → DB thread.
pub enum DbCommand {
    LoadCharacters { client_id: u32, account: String },
    CreateCharacter { client_id: u32, data: NewCharacter },
    MarkDelete { client_id: u32, account: String, char_id: i32, delete_time: i64 },
    RestoreCharacter { client_id: u32, account: String, char_id: i32 },
    /// Fire-and-forget hard delete (expired characters).
    DeleteCharacter { char_id: i32 },
    /// Char count + deletion times for the login server's `ReplyCharacters`.
    CountCharacters { account: String },
    /// Name availability check for `RequestCharacterNameCreatable` (name already
    /// passed the game thread's validity checks).
    CheckNameCreatable { client_id: u32, name: String },
    /// Flush a player's full state to the DB (`store_player`) — the memory-first
    /// model's only character-write path, sent by the staggered periodic
    /// autosave, on logout (`Disconnection.storeMe().deleteMe()`), on
    /// class-transfer, and by the shutdown save-all. Ordered before any
    /// following `LoadCharacters` on this channel, so a restart's re-sent list
    /// already reflects the save.
    StorePlayer { save: PlayerSaveData },
    /// Reserve a block of object ids for the game thread (Java `IdManager`
    /// semantics without a cross-thread round trip per item — the DB thread
    /// owns the counter, the game thread allocates out of its block and asks
    /// for another when it runs low). Replied with `DbEvent::IdBlock`.
    ReserveIds { count: i64 },
    /// Fire-and-forget friendship insert — both directions in one statement
    /// (Java `RequestAnswerFriendInvite`'s two-row INSERT). Kept immediate:
    /// needs a consenting second player, so it's not a packet-flood surface.
    InsertFriendPair { a: i32, b: i32 },
    /// Fire-and-forget friendship delete, both directions (`RequestFriendDel`).
    DeleteFriendPair { a: i32, b: i32 },
    /// Fire-and-forget `Clan.store()` — the 13-column `clan_data` INSERT
    /// with everything but id/name/leader at Java's defaults.
    InsertClan { clan_id: i32, name: String, leader_id: i32 },
    /// Fire-and-forget clan-membership update on a character
    /// (`ClanTable.createClan` side effects; `StorePlayer`'s UPDATE doesn't
    /// touch these columns).
    UpdateCharClan { char_id: i32, clan_id: i32, clan_privs: i32 },
    /// `CursedWeapon.saveData` — upsert the weapon's wielder state row.
    StoreCursedWeapon { item_id: i32, char_id: i32, reputation: i32, pk_kills: i32, nb_kills: i32, end_time: i64 },
    /// `DBSpawnManager.updateStatus`/`addNewSpawn` — upsert a raid boss's
    /// `npc_respawns` row (live HP/MP while alive, or the pending respawn time
    /// while dead). Keyed on npc id, matching Java's PRIMARY KEY.
    StoreNpcRespawn {
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
        respawn_time: i64,
        cur_hp: f64,
        cur_mp: f64,
    },
    /// `ADD_CHAR_SUBCLASS` / `UPDATE_CHAR_SUBCLASS` — upsert one subclass slot.
    /// Keyed on `(charId, class_id)` like Java's primary key.
    StoreSubClass { char_id: i32, class_id: i32, class_index: i32, level: i32, exp: i64, sp: i64 },
    /// `DBSpawnManager.deleteSpawn` — drop a raid boss's respawn row.
    DeleteNpcRespawn { npc_id: i32 },
    /// `CursedWeaponsManager.removeFromDb` — drop the weapon's state row.
    RemoveCursedWeapon { item_id: i32 },
    /// `Castle.setSide`/`switchSide` — persist a castle's side.
    UpdateCastleSide { castle_id: i32, side: String },
    /// Castle ownership on the clan side (`Castle.setOwner`/`removeOwner` →
    /// `clan_data.hasCastle`).
    UpdateClanCastle { clan_id: i32, castle_id: i32 },
    /// `Siege.saveSiegeClan` — register a clan for a castle's siege.
    SaveSiegeClan { castle_id: i32, clan_id: i32, kind: i32 },
    /// `Siege.removeSiegeClan` — drop a clan's `siege_clans` row.
    RemoveSiegeClan { castle_id: i32, clan_id: i32 },
    /// Fire-and-forget clan level persist (`Clan.changeLevel`'s single UPDATE).
    UpdateClanLevel { clan_id: i32, level: i32 },
    /// `Clan.addNewSkill` — upsert a learned clan skill (`sub_pledge_id = -2`,
    /// the main pledge). Keyed on `(clan_id, skill_id)`, so a re-grant at a
    /// higher level replaces the row.
    SaveClanSkill { clan_id: i32, skill_id: i32, skill_level: i32, skill_name: String },
    /// Delete one learned clan skill row (`sub_pledge_id = -2`). Used to purge a
    /// residence skill wrongly stored on the clan by a pre-fix `//give_clan_skills`.
    DeleteClanSkill { clan_id: i32, skill_id: i32 },
    /// Fire-and-forget clan reputation persist (`Clan.setReputationScore`, which
    /// Java writes via `updateClanScoreInDb`).
    UpdateClanReputation { clan_id: i32, reputation: i32 },
    /// `RequestOustPledgeMember` / village-master dissolve/recover — the two
    /// clan-side penalty stamps (`Clan.updateClanInDB`, narrowed).
    UpdateClanPenalties { clan_id: i32, char_penalty_expiry_time: i64, dissolving_expiry_time: i64 },
    /// A member left/was ousted (`Clan.removeClanMember` →
    /// `removeMemberInDatabase`): reset the character's clan columns and stamp
    /// the rejoin penalty (+ recreate penalty when the ex-member led the clan).
    RemoveClanMember { char_id: i32, clan_join_expiry: i64, clan_create_expiry: i64 },
    /// `Player.setClanJoinExpiryTime` persisted alone (invite accepted zeroes
    /// it; `characters.clan_join_expiry_time`).
    UpdateCharClanJoinExpiry { char_id: i32, expiry: i64 },
    /// `Clan.setRankPrivs` — upsert one `clan_privs` row (party is always 0).
    SaveClanRankPrivs { clan_id: i32, rank: i32, privs: i32 },
    /// `ClanMember.updatePowerGrade` — persist a member's rank.
    UpdateCharPowerGrade { char_id: i32, power_grade: i32 },
    /// `Clan.setNewLeaderId(id, true)` — the pending delegated leader transfer.
    UpdateClanNewLeader { clan_id: i32, new_leader_id: i32 },
    /// The ally half of `Clan.updateClanInDB` — membership + penalty stamps.
    UpdateClanAlly { clan_id: i32, ally_id: i32, ally_name: String, penalty_expiry: i64, penalty_type: i32 },
    /// `Clan.createSubPledge`'s insert — a new academy/royal/knight unit.
    InsertSubPledge { clan_id: i32, pledge_type: i32, name: String, leader_id: i32 },
    /// `Clan.updateSubPledgeInDB` — rename and/or leader reassignment.
    UpdateSubPledge { clan_id: i32, pledge_type: i32, name: String, leader_id: i32 },
    /// `ClanMember.updatePowerGrade`'s sibling for pledge type — persisted
    /// whenever a member's sub-unit membership changes (join/reorganize/
    /// leave-clears-captaincy).
    UpdateCharPledgeType { char_id: i32, pledge_type: i32 },
    /// `ClanTable.storeClanWars` — upsert one `clan_wars` row (ids, despite the
    /// varchar columns — Java binds ints too).
    SaveClanWar {
        attacker: i32,
        attacked: i32,
        attacker_kills: i32,
        attacked_kills: i32,
        winner: i32,
        start_time: i64,
        end_time: i64,
        state: i32,
    },
    /// `ClanTable.deleteClanWars` — drop the war row. Java deletes only the
    /// `(clan1, clan2)` order it was called with (a surrender can miss the row
    /// until the next boot's cleanup); both orders are deleted here — the same
    /// eventual outcome without the stale row.
    DeleteClanWar { clan1: i32, clan2: i32 },
    /// `ClanTable.destroyClan` — delete the `clan_data` row and reset every
    /// member's `characters` clan columns (online *and* offline, since the
    /// memory-first autosave never touches those columns). `leader_id` also gets
    /// the 10-day recreate cooldown stamped at `leader_expiry` (Java sets it on
    /// the leader during `removeClanMember`).
    DestroyClan { clan_id: i32, leader_id: i32, leader_expiry: i64 },
    /// Fire-and-forget clan-warehouse flush — delete every `owner_id = clan_id`
    /// item row (`loc = "CLANWH"`) and reinsert the current set (the same
    /// delete-then-reinsert the player item save uses).
    StoreClanWarehouse { clan_id: i32, items: Vec<ItemRow> },
    /// Java `Player.setAccessLevel(updateInDb=true)` — persist a GM access-level
    /// change immediately (the memory-first autosave doesn't carry accesslevel).
    SetAccessLevel { char_id: i32, level: i32 },
    /// Upsert one `account_gsdata` row (Java `AccountVariables.storeMe`,
    /// write-through). Used by `//primepoints` for the account-scoped
    /// "PRIME_POINTS" variable.
    StoreAccountVar { account_name: String, var: String, value: String },
    /// Upsert / delete an `account_premium` row (Java `PremiumManager`
    /// UPDATE/DELETE). Used by `//premium_*`.
    StorePremium { account_name: String, enddate: i64 },
    DeletePremium { account_name: String },
    /// Upsert / delete a `buffer_schemes` row (Java `SchemeBufferTable`; Java
    /// bulk-rewrites the table at shutdown, this port write-throughs per change).
    /// `skills` is the comma-joined skill-id list. Used by the community board's
    /// `_bbs_buff_scheme_create`/`_delete`.
    StoreBufferScheme { object_id: i32, scheme_name: String, skills: String },
    DeleteBufferScheme { object_id: i32, scheme_name: String },
    /// Insert / delete a `bbs_favorites` row (Java `FavoriteBoard`
    /// ADD_FAVORITE / DELETE_FAVORITE). `fav_id` is allocated on the game thread
    /// (the table-wide AUTOINCREMENT PK, written explicitly here) so the
    /// memory-first mirror carries it immediately. Used by the community board's
    /// `bbs_add_fav`/`_bbsdelfav_`.
    StoreFavorite { fav_id: i32, player_id: i32, title: String, bypass: String, add_date: String },
    DeleteFavorite { player_id: i32, fav_id: i32 },
    /// Fire-and-forget daily recommendation reset for offline characters
    /// (Java `DailyTaskManager.resetRecommends`'s two UPDATE statements).
    /// Online players are reset in memory on the game thread; their rows get
    /// rewritten from memory by the next autosave, so this only needs to fix
    /// the offline population.
    ResetRecommends,
    Shutdown,
}

/// DB thread → game thread (drained in tick step 2).
pub enum DbEvent {
    /// `send_list` = push a fresh `CharSelectionInfo` to the client (login,
    /// delete, restore). After character creation it is false — Java only caches
    /// the list (`setCharSelection`) and does not re-send it.
    CharactersLoaded { client_id: u32, account: String, chars: Vec<CharData>, send_list: bool },
    CharacterCreated { client_id: u32, result: CreateResult },
    CharCount { account: String, count: u8, del_times: Vec<i64> },
    /// `ExIsCharNameCreatable` result: -1 = creatable, else a failure code.
    NameCreatable { client_id: u32, result: i32 },
    /// A reserved object-id block `[start, end)` for the game thread's
    /// runtime allocations (loot items). One is pushed unprompted at boot.
    IdBlock { start: i64, end: i64 },
    /// The full clan table (`ClanTable` boot load), pushed unprompted at
    /// boot like the first `IdBlock`.
    ClansLoaded { clans: Vec<crate::model::clan::Clan>, wars: Vec<crate::model::clan::ClanWar> },
    /// The whole `npc_respawns` table (Java `DBSpawnManager.load`), pushed
    /// unprompted at boot. See [`NpcRespawnRow`].
    NpcRespawnsLoaded { rows: Vec<NpcRespawnRow> },
    /// The whole `account_premium` table (Java `PremiumManager` cache),
    /// pushed unprompted at boot. `(account_name lowercase, enddate millis)`.
    PremiumLoaded { entries: Vec<(String, i64)> },
    /// The whole `buffer_schemes` table (Java `SchemeBufferTable.load`), pushed
    /// unprompted at boot. `(object_id, scheme_name, skill_ids)`; skills not in
    /// the available-buff table are filtered on the game thread.
    BufferSchemesLoaded { entries: Vec<(i32, String, Vec<i32>)> },
    /// The whole `bbs_favorites` table (Java `FavoriteBoard` loads per-player on
    /// demand; this port caches all rows at boot like `buffer_schemes`), pushed
    /// unprompted at boot. `(player_id, fav_id, title, bypass, add_date)`,
    /// newest first.
    FavoritesLoaded { entries: Vec<(i32, i32, String, String, String)> },
    /// The `grandboss_data` table (Java `GrandBossManager.init`), pushed
    /// unprompted at boot. Filtered to known NPC templates on the game thread.
    GrandBossesLoaded { bosses: Vec<crate::model::grand_boss::GrandBoss> },
    /// The `cursed_weapons` state table (Java `CursedWeaponsManager.restore`),
    /// pushed unprompted at boot; overlaid onto the XML config on the game thread.
    CursedWeaponsLoaded { rows: Vec<CursedWeaponRow> },
    /// The `castle` table (Java `CastleManager.load`), pushed unprompted at boot.
    CastlesLoaded { castles: Vec<crate::model::castle::Castle> },
    /// The `siege_clans` table (Java `Siege.loadSiegeClan`), pushed unprompted at
    /// boot after `CastlesLoaded`. Grouped into per-castle sieges on the game thread.
    SiegesLoaded { rows: Vec<SiegeClanRow> },
    /// The `castle_siege_guards` table (the stationed garrison, `isHired=0`),
    /// pushed unprompted at boot. `(castle_id, spawn)`; grouped by castle on the
    /// game thread.
    SiegeGuardsLoaded { guards: Vec<(i32, crate::model::siege::SiegeSpawn)> },
}

/// One `siege_clans` row — a clan registered for a castle's siege.
#[derive(Debug, Clone)]
pub struct SiegeClanRow {
    pub castle_id: i32,
    pub clan_id: i32,
    pub kind: i32,
}

/// One `cursed_weapons` row — the persisted wielder state of a cursed weapon.
#[derive(Debug, Clone)]
pub struct CursedWeaponRow {
    pub item_id: i32,
    pub char_id: i32,
    pub player_reputation: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    pub end_time: i64,
}

pub type CmdTx = tokio::sync::mpsc::UnboundedSender<DbCommand>;
pub type CmdRx = tokio::sync::mpsc::UnboundedReceiver<DbCommand>;
pub type EventTx = std::sync::mpsc::Sender<DbEvent>;
pub type DbEventRx = std::sync::mpsc::Receiver<DbEvent>;

/// Spawn the DB thread. It creates and owns the pool on its own runtime.
pub fn spawn(url: String, max_connections: u32, max_characters: i32, cmd_rx: CmdRx, event_tx: EventTx) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("db-thread".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("db thread runtime");
            rt.block_on(run(url, max_connections, max_characters, cmd_rx, event_tx));
        })
        .expect("failed to spawn db thread")
}

async fn run(url: String, max_connections: u32, max_characters: i32, mut cmd_rx: CmdRx, event_tx: EventTx) {
    let pool = match commons::db::init(&url, max_connections).await {
        Ok(p) => p,
        Err(e) => {
            error!("DB thread: failed to open database: {e}");
            return;
        }
    };
    let mut next_id = load_next_id(&pool).await;

    // Hand the game thread its initial runtime-id block unprompted (it can't
    // ask before it knows the DB thread is up; see `DbCommand::ReserveIds`).
    let _ = event_tx.send(DbEvent::IdBlock { start: next_id, end: next_id + ID_BLOCK_SIZE });
    next_id += ID_BLOCK_SIZE;

    // Premium table cache, before clans so `ClansLoaded` stays the last boot
    // event (the game loop releases the login link on it).
    let _ = event_tx.send(DbEvent::PremiumLoaded { entries: load_premium(&pool).await });

    // `SchemeBufferTable.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::BufferSchemesLoaded { entries: load_buffer_schemes(&pool).await });

    // `FavoriteBoard` favorites cache — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::FavoritesLoaded { entries: load_favorites(&pool).await });

    // `DBSpawnManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::NpcRespawnsLoaded { rows: load_npc_respawns(&pool).await });

    // `GrandBossManager.init` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::GrandBossesLoaded { bosses: load_grandboss_data(&pool).await });

    // `CursedWeaponsManager.restore` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CursedWeaponsLoaded { rows: load_cursed_weapons(&pool).await });

    // `CastleManager.load` — likewise unprompted, before `ClansLoaded`.
    let _ = event_tx.send(DbEvent::CastlesLoaded { castles: load_castles(&pool).await });

    // `Siege.loadSiegeClan` — after castles (the game loop keys sieges off them).
    let _ = event_tx.send(DbEvent::SiegesLoaded { rows: load_siege_clans(&pool).await });

    // `SiegeGuardManager` — the stationed siege guards, spawned at siege start.
    let _ = event_tx.send(DbEvent::SiegeGuardsLoaded { guards: load_siege_guards(&pool).await });

    // `ClanTable`'s boot restore, likewise unprompted.
    let _ = event_tx.send(DbEvent::ClansLoaded { clans: load_clans(&pool).await, wars: load_clan_wars(&pool).await });

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            DbCommand::LoadCharacters { client_id, account } => {
                reload(&pool, &event_tx, client_id, account, true).await;
            }
            DbCommand::CreateCharacter { client_id, data } => {
                let result = create_character(&pool, &mut next_id, max_characters, &data).await;
                let _ = event_tx.send(DbEvent::CharacterCreated { client_id, result });
                if result == CreateResult::Ok {
                    // Java caches the list after creation but does not re-send it.
                    reload(&pool, &event_tx, client_id, data.account, false).await;
                }
            }
            DbCommand::MarkDelete { client_id, account, char_id, delete_time } => {
                exec(&pool, sqlx::query("UPDATE characters SET deletetime=? WHERE charId=?").bind(delete_time).bind(char_id)).await;
                reload(&pool, &event_tx, client_id, account, true).await;
            }
            DbCommand::RestoreCharacter { client_id, account, char_id } => {
                exec(&pool, sqlx::query("UPDATE characters SET deletetime=0 WHERE charId=?").bind(char_id)).await;
                reload(&pool, &event_tx, client_id, account, true).await;
            }
            DbCommand::DeleteCharacter { char_id } => {
                delete_char(&pool, char_id).await;
            }
            DbCommand::CountCharacters { account } => {
                let (count, del_times) = count_characters(&pool, &account).await;
                let _ = event_tx.send(DbEvent::CharCount { account, count, del_times });
            }
            DbCommand::CheckNameCreatable { client_id, name } => {
                // RequestCharacterNameCreatable: NAME_ALREADY_EXISTS=2,
                // INVALID_LENGTH=3, creatable=-1 (validity was checked already).
                let result = if name_exists(&pool, &name).await {
                    2
                } else if name.chars().count() > 16 {
                    3
                } else {
                    -1
                };
                let _ = event_tx.send(DbEvent::NameCreatable { client_id, result });
            }
            DbCommand::StorePlayer { save } => {
                store_player(&pool, &save).await;
            }
            DbCommand::ReserveIds { count } => {
                let _ = event_tx.send(DbEvent::IdBlock { start: next_id, end: next_id + count });
                next_id += count;
            }
            DbCommand::InsertFriendPair { a, b } => {
                exec(
                    &pool,
                    sqlx::query("INSERT OR IGNORE INTO character_friends (charId, friendId, relation) VALUES (?, ?, 0), (?, ?, 0)")
                        .bind(a)
                        .bind(b)
                        .bind(b)
                        .bind(a),
                )
                .await;
            }
            DbCommand::DeleteFriendPair { a, b } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM character_friends WHERE (charId=? AND friendId=?) OR (charId=? AND friendId=?)")
                        .bind(a)
                        .bind(b)
                        .bind(b)
                        .bind(a),
                )
                .await;
            }
            DbCommand::InsertClan { clan_id, name, leader_id } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT INTO clan_data (clan_id, clan_name, clan_level, hasCastle, \
                         blood_alliance_count, blood_oath_count, ally_id, ally_name, leader_id, \
                         crest_id, crest_large_id, ally_crest_id, new_leader_id) \
                         VALUES (?, ?, 0, 0, 0, 0, 0, NULL, ?, 0, 0, 0, 0)",
                    )
                    .bind(clan_id)
                    .bind(name)
                    .bind(leader_id),
                )
                .await;
            }
            DbCommand::UpdateCharClan { char_id, clan_id, clan_privs } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET clanid=?, clan_privs=? WHERE charId=?")
                        .bind(clan_id)
                        .bind(clan_privs)
                        .bind(char_id),
                )
                .await;
            }
            DbCommand::SaveClanSkill { clan_id, skill_id, skill_level, skill_name } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO clan_skills \
                         (clan_id, skill_id, skill_level, skill_name, sub_pledge_id) \
                         VALUES (?, ?, ?, ?, -2)",
                    )
                    .bind(clan_id)
                    .bind(skill_id)
                    .bind(skill_level)
                    .bind(skill_name),
                )
                .await;
            }
            DbCommand::DeleteClanSkill { clan_id, skill_id } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM clan_skills WHERE clan_id=? AND skill_id=?")
                        .bind(clan_id)
                        .bind(skill_id),
                )
                .await;
            }
            DbCommand::StoreCursedWeapon { item_id, char_id, reputation, pk_kills, nb_kills, end_time } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO cursed_weapons \
                         (itemId, charId, playerReputation, playerPkKills, nbKills, endTime) \
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(item_id)
                    .bind(char_id)
                    .bind(reputation)
                    .bind(pk_kills)
                    .bind(nb_kills)
                    .bind(end_time),
                )
                .await;
            }
            DbCommand::StoreNpcRespawn { npc_id, x, y, z, heading, respawn_time, cur_hp, cur_mp } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO npc_respawns \
                         (id, x, y, z, heading, respawnTime, currentHp, currentMp) \
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(npc_id)
                    .bind(x)
                    .bind(y)
                    .bind(z)
                    .bind(heading)
                    .bind(respawn_time)
                    .bind(cur_hp)
                    .bind(cur_mp),
                )
                .await;
            }
            DbCommand::StoreSubClass { char_id, class_id, class_index, level, exp, sp } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO character_subclasses \
                         (charId, class_id, exp, sp, level, vitality_points, class_index, dual_class) \
                         VALUES (?, ?, ?, ?, ?, 0, ?, 0)",
                    )
                    .bind(char_id)
                    .bind(class_id)
                    .bind(exp)
                    .bind(sp)
                    .bind(level)
                    .bind(class_index),
                )
                .await;
            }
            DbCommand::DeleteNpcRespawn { npc_id } => {
                exec(&pool, sqlx::query("DELETE FROM npc_respawns WHERE id=?").bind(npc_id)).await;
            }
            DbCommand::RemoveCursedWeapon { item_id } => {
                exec(&pool, sqlx::query("DELETE FROM cursed_weapons WHERE itemId=?").bind(item_id)).await;
            }
            DbCommand::UpdateCastleSide { castle_id, side } => {
                exec(&pool, sqlx::query("UPDATE castle SET side=? WHERE id=?").bind(side).bind(castle_id)).await;
            }
            DbCommand::UpdateClanCastle { clan_id, castle_id } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE clan_data SET hasCastle=? WHERE clan_id=?").bind(castle_id).bind(clan_id),
                )
                .await;
            }
            DbCommand::SaveSiegeClan { castle_id, clan_id, kind } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO siege_clans (clan_id, castle_id, type, castle_owner) VALUES (?, ?, ?, 0)",
                    )
                    .bind(clan_id)
                    .bind(castle_id)
                    .bind(kind),
                )
                .await;
            }
            DbCommand::RemoveSiegeClan { castle_id, clan_id } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM siege_clans WHERE castle_id=? AND clan_id=?").bind(castle_id).bind(clan_id),
                )
                .await;
            }
            DbCommand::UpdateClanLevel { clan_id, level } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE clan_data SET clan_level=? WHERE clan_id=?").bind(level).bind(clan_id),
                )
                .await;
            }
            DbCommand::UpdateClanReputation { clan_id, reputation } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE clan_data SET reputation_score=? WHERE clan_id=?").bind(reputation).bind(clan_id),
                )
                .await;
            }
            DbCommand::UpdateClanPenalties { clan_id, char_penalty_expiry_time, dissolving_expiry_time } => {
                exec(
                    &pool,
                    sqlx::query(
                        "UPDATE clan_data SET char_penalty_expiry_time=?, dissolving_expiry_time=? WHERE clan_id=?",
                    )
                    .bind(char_penalty_expiry_time)
                    .bind(dissolving_expiry_time)
                    .bind(clan_id),
                )
                .await;
            }
            DbCommand::RemoveClanMember { char_id, clan_join_expiry, clan_create_expiry } => {
                exec(
                    &pool,
                    sqlx::query(
                        "UPDATE characters SET clanid=0, title='', clan_privs=0, \
                         clan_join_expiry_time=?, clan_create_expiry_time=? WHERE charId=?",
                    )
                    .bind(clan_join_expiry)
                    .bind(clan_create_expiry)
                    .bind(char_id),
                )
                .await;
            }
            DbCommand::SaveClanRankPrivs { clan_id, rank, privs } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT INTO clan_privs (clan_id, `rank`, party, privs) VALUES (?, ?, 0, ?) \
                         ON CONFLICT(clan_id, `rank`, party) DO UPDATE SET privs=excluded.privs",
                    )
                    .bind(clan_id)
                    .bind(rank)
                    .bind(privs),
                )
                .await;
            }
            DbCommand::UpdateCharPowerGrade { char_id, power_grade } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET power_grade=? WHERE charId=?")
                        .bind(power_grade)
                        .bind(char_id),
                )
                .await;
            }
            DbCommand::UpdateClanAlly { clan_id, ally_id, ally_name, penalty_expiry, penalty_type } => {
                exec(
                    &pool,
                    sqlx::query(
                        "UPDATE clan_data SET ally_id=?, ally_name=?, ally_penalty_expiry_time=?, ally_penalty_type=? WHERE clan_id=?",
                    )
                    .bind(ally_id)
                    .bind(ally_name)
                    .bind(penalty_expiry)
                    .bind(penalty_type)
                    .bind(clan_id),
                )
                .await;
            }
            DbCommand::InsertSubPledge { clan_id, pledge_type, name, leader_id } => {
                exec(
                    &pool,
                    sqlx::query("INSERT INTO clan_subpledges (clan_id, sub_pledge_id, name, leader_id) VALUES (?, ?, ?, ?)")
                        .bind(clan_id)
                        .bind(pledge_type)
                        .bind(name)
                        .bind(leader_id),
                )
                .await;
            }
            DbCommand::UpdateSubPledge { clan_id, pledge_type, name, leader_id } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE clan_subpledges SET leader_id=?, name=? WHERE clan_id=? AND sub_pledge_id=?")
                        .bind(leader_id)
                        .bind(name)
                        .bind(clan_id)
                        .bind(pledge_type),
                )
                .await;
            }
            DbCommand::UpdateCharPledgeType { char_id, pledge_type } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET subpledge=? WHERE charId=?").bind(pledge_type).bind(char_id),
                )
                .await;
            }
            DbCommand::SaveClanWar { attacker, attacked, attacker_kills, attacked_kills, winner, start_time, end_time, state } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT INTO clan_wars (clan1, clan2, clan1Kill, clan2Kill, winnerClan, startTime, endTime, state) \
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
                         ON CONFLICT(clan1, clan2) DO UPDATE SET clan1Kill=excluded.clan1Kill, \
                         clan2Kill=excluded.clan2Kill, winnerClan=excluded.winnerClan, \
                         startTime=excluded.startTime, endTime=excluded.endTime, state=excluded.state",
                    )
                    .bind(attacker)
                    .bind(attacked)
                    .bind(attacker_kills)
                    .bind(attacked_kills)
                    .bind(winner)
                    .bind(start_time)
                    .bind(end_time)
                    .bind(state),
                )
                .await;
            }
            DbCommand::DeleteClanWar { clan1, clan2 } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM clan_wars WHERE (clan1=? AND clan2=?) OR (clan1=? AND clan2=?)")
                        .bind(clan1)
                        .bind(clan2)
                        .bind(clan2)
                        .bind(clan1),
                )
                .await;
            }
            DbCommand::UpdateClanNewLeader { clan_id, new_leader_id } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE clan_data SET new_leader_id=? WHERE clan_id=?")
                        .bind(new_leader_id)
                        .bind(clan_id),
                )
                .await;
            }
            DbCommand::UpdateCharClanJoinExpiry { char_id, expiry } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET clan_join_expiry_time=? WHERE charId=?")
                        .bind(expiry)
                        .bind(char_id),
                )
                .await;
            }
            DbCommand::DestroyClan { clan_id, leader_id, leader_expiry } => {
                exec(&pool, sqlx::query("DELETE FROM clan_data WHERE clan_id=?").bind(clan_id)).await;
                exec(&pool, sqlx::query("DELETE FROM clan_skills WHERE clan_id=?").bind(clan_id)).await;
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET clanid=0, clan_privs=0 WHERE clanid=?").bind(clan_id),
                )
                .await;
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET clan_create_expiry_time=? WHERE charId=?")
                        .bind(leader_expiry)
                        .bind(leader_id),
                )
                .await;
            }
            DbCommand::StoreClanWarehouse { clan_id, items } => {
                exec(&pool, sqlx::query("DELETE FROM items WHERE owner_id=?").bind(clan_id)).await;
                for it in &items {
                    exec(
                        &pool,
                        sqlx::query(
                            "INSERT INTO items \
                             (owner_id, object_id, item_id, count, enchant_level, loc, loc_data, \
                              custom_type1, custom_type2, mana_left, time) \
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(clan_id)
                        .bind(it.object_id)
                        .bind(it.item_id)
                        .bind(it.count)
                        .bind(it.enchant_level)
                        .bind(&it.loc)
                        .bind(it.loc_data)
                        .bind(it.custom_type1)
                        .bind(it.custom_type2)
                        .bind(it.mana_left)
                        .bind(it.time),
                    )
                    .await;
                }
            }
            DbCommand::SetAccessLevel { char_id, level } => {
                exec(
                    &pool,
                    sqlx::query("UPDATE characters SET accesslevel=? WHERE charId=?")
                        .bind(level)
                        .bind(char_id),
                )
                .await;
            }
            DbCommand::StoreAccountVar { account_name, var, value } => {
                exec(
                    &pool,
                    sqlx::query("INSERT OR REPLACE INTO account_gsdata (account_name, var, value) VALUES (?, ?, ?)")
                        .bind(account_name)
                        .bind(var)
                        .bind(value),
                )
                .await;
            }
            DbCommand::StorePremium { account_name, enddate } => {
                exec(
                    &pool,
                    sqlx::query("INSERT OR REPLACE INTO account_premium (account_name, enddate) VALUES (?, ?)")
                        .bind(account_name)
                        .bind(enddate),
                )
                .await;
            }
            DbCommand::DeletePremium { account_name } => {
                exec(&pool, sqlx::query("DELETE FROM account_premium WHERE account_name=?").bind(account_name)).await;
            }
            DbCommand::StoreBufferScheme { object_id, scheme_name, skills } => {
                exec(
                    &pool,
                    sqlx::query("INSERT OR REPLACE INTO buffer_schemes (object_id, scheme_name, skills) VALUES (?, ?, ?)")
                        .bind(object_id)
                        .bind(scheme_name)
                        .bind(skills),
                )
                .await;
            }
            DbCommand::DeleteBufferScheme { object_id, scheme_name } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM buffer_schemes WHERE object_id=? AND scheme_name=?")
                        .bind(object_id)
                        .bind(scheme_name),
                )
                .await;
            }
            DbCommand::StoreFavorite { fav_id, player_id, title, bypass, add_date } => {
                exec(
                    &pool,
                    sqlx::query(
                        "INSERT OR REPLACE INTO bbs_favorites (favId, playerId, favTitle, favBypass, favAddDate) VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(fav_id)
                    .bind(player_id)
                    .bind(title)
                    .bind(bypass)
                    .bind(add_date),
                )
                .await;
            }
            DbCommand::DeleteFavorite { player_id, fav_id } => {
                exec(
                    &pool,
                    sqlx::query("DELETE FROM bbs_favorites WHERE playerId=? AND favId=?").bind(player_id).bind(fav_id),
                )
                .await;
            }
            DbCommand::ResetRecommends => {
                // Java `DailyTaskManager.resetRecommends`: rec_left → 0 for
                // everyone; rec_have → 0 for those at/under 20, else -20.
                exec(&pool, sqlx::query("UPDATE character_reco_bonus SET rec_left = 0, rec_have = 0 WHERE rec_have <= 20")).await;
                exec(
                    &pool,
                    sqlx::query("UPDATE character_reco_bonus SET rec_left = 0, rec_have = MAX(rec_have - 20, 0) WHERE rec_have > 20"),
                )
                .await;
            }
            DbCommand::Shutdown => break,
        }
    }

    pool.close().await;
    info!("DB thread: stopped.");
}

async fn reload(pool: &SqlitePool, event_tx: &EventTx, client_id: u32, account: String, send_list: bool) {
    let chars = load_characters(pool, &account).await;
    let _ = event_tx.send(DbEvent::CharactersLoaded { client_id, account, chars, send_list });
}

/// Best-effort read of one `account_gsdata` variable (Java
/// `AccountVariables.restoreMe`). Returns `None` on a missing row or any error
/// (e.g. the table absent in a minimal test schema), mirroring Java's
/// catch-and-default-empty behaviour.
async fn load_account_var(pool: &SqlitePool, account: &str, var: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM account_gsdata WHERE account_name=? AND var=?")
        .bind(account)
        .bind(var)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Best-effort boot load of the whole `account_premium` table (Java
/// `PremiumManager` has no table-wide load; this port caches all rows so the
/// admin `//premium_*` commands work for offline accounts). Missing table → empty.
async fn load_premium(pool: &SqlitePool) -> Vec<(String, i64)> {
    match sqlx::query("SELECT account_name, enddate FROM account_premium").fetch_all(pool).await {
        Ok(rows) => rows.iter().map(|r| (gets(r, "account_name").to_lowercase(), geti(r, "enddate"))).collect(),
        Err(_) => Vec::new(),
    }
}

/// One `npc_respawns` row — a raid boss's persisted state.
#[derive(Debug, Clone, Copy)]
pub struct NpcRespawnRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Absolute unix millis the boss is due back, or 0 when it's alive (Java
    /// stores 0 for a living boss and the due time for a dead one).
    pub respawn_time: i64,
    pub cur_hp: f64,
    pub cur_mp: f64,
}

/// `RESTORE_CHAR_SUBCLASSES` — a character's subclass slots.
async fn load_subclasses(pool: &SqlitePool, char_id: i32) -> Vec<crate::model::SubClass> {
    match sqlx::query(
        "SELECT class_id, exp, sp, level, class_index FROM character_subclasses \
         WHERE charId=? ORDER BY class_index",
    )
    .bind(char_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows
            .iter()
            .map(|r| crate::model::SubClass {
                class_id: geti(r, "class_id") as i32,
                class_index: geti(r, "class_index") as i32,
                level: geti(r, "level") as i32,
                exp: geti(r, "exp"),
                sp: geti(r, "sp"),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Boot load of the whole `npc_respawns` table (Java `DBSpawnManager.load`).
/// Missing table → empty, like the other boot loads.
async fn load_npc_respawns(pool: &SqlitePool) -> Vec<NpcRespawnRow> {
    match sqlx::query("SELECT id, x, y, z, heading, respawnTime, currentHp, currentMp FROM npc_respawns")
        .fetch_all(pool)
        .await
    {
        Ok(rows) => rows
            .iter()
            .map(|r| NpcRespawnRow {
                npc_id: geti(r, "id") as i32,
                x: geti(r, "x") as i32,
                y: geti(r, "y") as i32,
                z: geti(r, "z") as i32,
                heading: geti(r, "heading") as i32,
                respawn_time: geti(r, "respawnTime"),
                cur_hp: getf(r, "currentHp"),
                cur_mp: getf(r, "currentMp"),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Boot load of the whole `buffer_schemes` table (Java `SchemeBufferTable.load`).
/// `skills` is stored comma-joined; parse it here, drop empties. Availability
/// filtering (skills still in the buffer table) happens on the game thread,
/// where the datapack lives. Missing table → empty.
async fn load_buffer_schemes(pool: &SqlitePool) -> Vec<(i32, String, Vec<i32>)> {
    match sqlx::query("SELECT object_id, scheme_name, skills FROM buffer_schemes").fetch_all(pool).await {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                let skills = gets(r, "skills")
                    .split(',')
                    .filter_map(|s| s.trim().parse::<i32>().ok())
                    .collect();
                (geti(r, "object_id") as i32, gets(r, "scheme_name"), skills)
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Boot load of the whole `bbs_favorites` table (Java `FavoriteBoard` loads it
/// per-player on `_bbsgetfav`; this port caches all rows at boot like the
/// buffer schemes). `ORDER BY favAddDate DESC` matches Java's list order.
/// Missing table → empty.
async fn load_favorites(pool: &SqlitePool) -> Vec<(i32, i32, String, String, String)> {
    match sqlx::query("SELECT playerId, favId, favTitle, favBypass, favAddDate FROM bbs_favorites ORDER BY favAddDate DESC")
        .fetch_all(pool)
        .await
    {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                (
                    geti(r, "playerId") as i32,
                    geti(r, "favId") as i32,
                    gets(r, "favTitle"),
                    gets(r, "favBypass"),
                    gets(r, "favAddDate"),
                )
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Java's `IdManager` hands out ids from a single pool shared by every
/// world-object type, so the next free id must clear the high-water mark of
/// every table that stores one — not just `characters` (a fresh id here that
/// collides with an existing `items.object_id` fails its INSERT silently).
async fn load_next_id(pool: &SqlitePool) -> i64 {
    let max_char: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(charId), 0) FROM characters")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let max_item: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(object_id), 0) FROM items")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    (max_char.max(max_item) + 1).max(FIRST_OID)
}

/// `loadCharacterSelectInfo`: rows for an account, expired deletions purged.
async fn load_characters(pool: &SqlitePool, account: &str) -> Vec<CharData> {
    let rows = match sqlx::query("SELECT * FROM characters WHERE account_name=? ORDER BY createDate")
        .bind(account)
        .fetch_all(pool)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("DB thread: load_characters failed: {e}");
            return Vec::new();
        }
    };

    // Account-scoped prime (NCoin) balance — same for every char on the
    // account. Best-effort: absent table/row → 0 (Java `restoreMe` catch).
    let prime_points = load_account_var(pool, account, "PRIME_POINTS")
        .await
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let now = now_millis();
    let mut out = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        let delete_time = geti(row, "deletetime");
        let object_id = geti(row, "charId") as i32;
        if delete_time > 0 && now > delete_time {
            delete_char(pool, object_id).await; // restoreChar: purge expired
            continue;
        }
        let items = load_items(pool, object_id).await;
        let skills_by_index = load_skills(pool, object_id).await;
        let subclasses = load_subclasses(pool, object_id).await;
        let class_id_now = geti(row, "classid") as i32;
        // Java keeps the *active* class in `characters.classid`; the index is
        // whichever subclass slot carries it (0 when it's the base class).
        let active_index = subclasses
            .iter()
            .find(|s| s.class_id == class_id_now)
            .map(|s| s.class_index)
            .unwrap_or(0);
        let hennas_by_index = load_hennas(pool, object_id).await;
        let recipe_book = load_recipe_book(pool, object_id).await;
        let variables = load_variables(pool, object_id).await;
        let shortcuts_by_index = load_shortcuts(pool, object_id).await;
        let macros = load_macros(pool, object_id).await;
        let friends = load_friends(pool, object_id).await;
        let quests = load_quests(pool, object_id).await;
        let skill_reuses = load_skill_reuses(pool, object_id, active_index).await;
        let skill_buffs = load_skill_buffs(pool, object_id, active_index).await;
        let (rec_have, rec_left) = load_reco_bonus(pool, object_id).await;
        out.push(CharData {
            object_id,
            name: gets(row, "char_name"),
            account_name: gets(row, "account_name"),
            level: geti(row, "level") as i32,
            max_hp: geti(row, "maxHp") as i32,
            cur_hp: getf(row, "curHp"),
            max_mp: geti(row, "maxMp") as i32,
            cur_mp: getf(row, "curMp"),
            face: geti(row, "face") as i32,
            hair_style: geti(row, "hairStyle") as i32,
            hair_color: geti(row, "hairColor") as i32,
            sex: geti(row, "sex") as i32,
            x: geti(row, "x") as i32,
            y: geti(row, "y") as i32,
            z: geti(row, "z") as i32,
            exp: geti(row, "exp"),
            sp: geti(row, "sp"),
            reputation: geti(row, "reputation") as i32,
            pk_kills: geti(row, "pkkills") as i32,
            pvp_kills: geti(row, "pvpkills") as i32,
            rec_have,
            rec_left,
            clan_id: geti(row, "clanid") as i32,
            clan_privs: geti(row, "clan_privs") as i32,
            clan_create_expiry_time: geti(row, "clan_create_expiry_time"),
            clan_join_expiry_time: geti(row, "clan_join_expiry_time"),
            power_grade: geti(row, "power_grade") as i32,
            pledge_type: geti(row, "subpledge") as i32,
            race: geti(row, "race") as i32,
            class_id: geti(row, "classid") as i32,
            base_class_id: geti(row, "base_class") as i32,
            delete_time,
            last_access: geti(row, "lastAccess"),
            vitality_points: geti(row, "vitality_points") as i32,
            pccafe_points: geti(row, "pccafe_points") as i32,
            prime_points,
            access_level: geti(row, "accesslevel") as i32,
            noble: geti(row, "nobless") == 1,
            subclasses,
            char_slot: slot as i32,
            items,
            // The active class index is whichever subclass row matches the
            // `characters.classid` we just loaded; base class → 0.
            skills: skills_by_index.get(&active_index).cloned().unwrap_or_default(),
            skills_by_index,
            hennas: hennas_by_index.get(&active_index).cloned().unwrap_or_default(),
            hennas_by_index,
            recipe_book,
            variables,
            shortcuts: shortcuts_by_index.get(&active_index).cloned().unwrap_or_default(),
            shortcuts_by_index,
            macros,
            friends,
            quests,
            skill_reuses,
            skill_buffs,
        });
    }
    // Characters marked for deletion are listed last in the lobby; the stable
    // sort keeps createDate order within each group. Slots are the list
    // positions the client will send back, so renumber after sorting.
    out.sort_by_key(|c| c.delete_time > 0);
    for (slot, c) in out.iter_mut().enumerate() {
        c.char_slot = slot as i32;
    }
    out
}

/// A character's `character_skills` rows (Java: `Player.restoreSkills`,
/// called for every row shown in `CharSelectionInfo` — same treatment as
/// `load_items`).
async fn load_skills(pool: &SqlitePool, owner_id: i32) -> std::collections::HashMap<i32, Vec<(i32, i32)>> {
    let rows = sqlx::query("SELECT skill_id, skill_level, class_index FROM character_skills WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32)>> = std::collections::HashMap::new();
    for r in &rows {
        out.entry(geti(r, "class_index") as i32)
            .or_default()
            .push((geti(r, "skill_id") as i32, geti(r, "skill_level") as i32));
    }
    out
}

/// A character's `character_hennas` rows (Java `Player.restoreHenna`) as
/// `(slot, symbol_id)`. `class_index = 0` — no subclasses on this dist.
async fn load_hennas(pool: &SqlitePool, owner_id: i32) -> std::collections::HashMap<i32, Vec<(i32, i32)>> {
    let rows = sqlx::query("SELECT slot, symbol_id, class_index FROM character_hennas WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<(i32, i32)>> = std::collections::HashMap::new();
    for r in &rows {
        let (slot, sym) = (geti(r, "slot") as i32, geti(r, "symbol_id") as i32);
        if (1..=3).contains(&slot) && sym != 0 {
            out.entry(geti(r, "class_index") as i32).or_default().push((slot, sym));
        }
    }
    out
}

/// A character's `character_recipebook` rows (Java `Player.restoreRecipeBook`)
/// as recipe-*list* ids. The dwarven/common split (the `type` column) is
/// re-derived from `RecipeData` on the game thread, so the DB layer just
/// returns the ids. `classIndex = 0` — no subclasses on this dist.
async fn load_recipe_book(pool: &SqlitePool, owner_id: i32) -> Vec<i32> {
    let rows = sqlx::query("SELECT id FROM character_recipebook WHERE charId=? AND classIndex=0")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter().map(|r| geti(r, "id") as i32).collect()
}

/// A character's `character_variables` rows (Java `PlayerVariables.restoreMe`)
/// as `(var, val)` pairs. Values stay strings — the component parses on read,
/// like Java's `StatSet` getters.
async fn load_variables(pool: &SqlitePool, owner_id: i32) -> Vec<(String, String)> {
    let rows = sqlx::query("SELECT var, val FROM character_variables WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter().map(|r| (gets(r, "var"), gets(r, "val"))).collect()
}

/// A character's `character_skills_save` reuse rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 1` half). Already-expired rows (`systime <= now`) are
/// dropped here; the survivors carry the absolute `systime` and the game side
/// converts it to a game tick when the character enters the world. Buff rows
/// (restore_type 0) are loaded separately by [`load_skill_buffs`].
async fn load_skill_reuses(pool: &SqlitePool, owner_id: i32, class_index: i32) -> Vec<SkillReuseRow> {
    let now = now_millis();
    let rows = sqlx::query(
        "SELECT skill_id, skill_level, reuse_delay, systime FROM character_skills_save \
         WHERE charId=? AND class_index=? AND restore_type=1",
    )
    .bind(owner_id)
    .bind(class_index)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .filter_map(|r| {
            let systime_ms = geti(r, "systime");
            (systime_ms > now).then_some(SkillReuseRow {
                reuse_key: geti(r, "skill_id") as i32,
                skill_level: geti(r, "skill_level") as i32,
                reuse_delay: geti(r, "reuse_delay") as i32,
                systime_ms,
            })
        })
        .collect()
}

/// A character's `character_skills_save` **buff** rows for the **active** class
/// index (Java `restoreEffects`, `restore_type = 0` half), in `buff_index`
/// order so the buff bar comes back in the order it was stored.
///
/// No expiry filtering happens here, unlike [`load_skill_reuses`]: a buff's
/// `remaining_time` is relative and its countdown is frozen while the character
/// is offline, so there is no elapsed time to compare against. Rows with a
/// non-positive remaining time are dropped since they'd restore an
/// already-dead buff.
async fn load_skill_buffs(pool: &SqlitePool, owner_id: i32, class_index: i32) -> Vec<SkillBuffRow> {
    let rows = sqlx::query(
        "SELECT skill_id, skill_level, remaining_time FROM character_skills_save \
         WHERE charId=? AND class_index=? AND restore_type=0 ORDER BY buff_index ASC",
    )
    .bind(owner_id)
    .bind(class_index)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .filter_map(|r| {
            let remaining_time_secs = geti(r, "remaining_time") as i32;
            (remaining_time_secs > 0).then_some(SkillBuffRow {
                skill_id: geti(r, "skill_id") as i32,
                skill_level: geti(r, "skill_level") as i32,
                remaining_time_secs,
            })
        })
        .collect()
}

/// A character's recommendation counters (Java `Player.loadRecommendations`).
/// Returns `(rec_have, rec_left)`; `(0, 0)` when the row is absent, matching
/// Java's field defaults for a character whose `character_reco_bonus` row
/// hasn't been written yet.
async fn load_reco_bonus(pool: &SqlitePool, owner_id: i32) -> (i32, i32) {
    match sqlx::query("SELECT rec_have, rec_left FROM character_reco_bonus WHERE charId=?")
        .bind(owner_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => (geti(&row, "rec_have") as i32, geti(&row, "rec_left") as i32),
        _ => (0, 0),
    }
}

/// A character's `character_shortcuts` rows (Java `ShortCuts.restoreMe` —
/// the inventory verification half runs on the game thread, in
/// `Player::from_char`). `characterType` isn't stored; restore hardcodes 1
/// like Java. `shared_reuse_group` starts at the -1 default; `from_char`
/// fills it for EtcItem shortcuts.
async fn load_shortcuts(
    pool: &SqlitePool,
    owner_id: i32,
) -> std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> {
    let rows = sqlx::query("SELECT slot, page, type, shortcut_id, level, class_index FROM character_shortcuts WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<i32, Vec<crate::model::shortcut::Shortcut>> =
        std::collections::HashMap::new();
    for r in &rows {
        out.entry(geti(r, "class_index") as i32).or_default().push(crate::model::shortcut::Shortcut {
            slot: geti(r, "slot") as i32,
            page: geti(r, "page") as i32,
            kind: crate::model::shortcut::ShortcutType::from_ordinal(geti(r, "type") as i32),
            id: geti(r, "shortcut_id") as i32,
            level: geti(r, "level") as i32,
            character_type: 1,
            shared_reuse_group: -1,
        });
    }
    out
}

/// A character's `character_friends` rows joined with each friend's
/// character row — the name/level/class snapshot Java reads through
/// `CharInfoTable` on demand (`relation`/`memo` unused).
async fn load_friends(pool: &SqlitePool, owner_id: i32) -> Vec<crate::character::FriendInfo> {
    let rows = sqlx::query(
        "SELECT f.friendId, c.char_name, c.level, c.classid FROM character_friends f \
         JOIN characters c ON c.charId = f.friendId WHERE f.charId=?",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .map(|row| crate::character::FriendInfo {
            char_id: geti(row, "friendId") as i32,
            name: gets(row, "char_name"),
            level: geti(row, "level") as i32,
            class_id: geti(row, "classid") as i32,
        })
        .collect()
}

/// A character's `character_quests` rows grouped by quest name (Java
/// `Quest.playerEnter`): the `<state>` rows define which quests exist, the
/// remaining rows fill each one's variable map. Vars for a quest without a
/// state row are orphans — Java warns (or deletes with
/// `AUTODELETE_INVALID_QUEST_DATA`); we drop them from the load.
async fn load_quests(pool: &SqlitePool, owner_id: i32) -> std::collections::HashMap<String, crate::model::quest::QuestState> {
    use crate::model::quest::{state, QuestState, STATE_VAR};
    let rows = sqlx::query("SELECT name, var, value FROM character_quests WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out: std::collections::HashMap<String, QuestState> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| gets(r, "var") == STATE_VAR) {
        out.insert(
            gets(row, "name"),
            QuestState { state: state::from_name(&gets(row, "value")), ..Default::default() },
        );
    }
    for row in rows.iter().filter(|r| gets(r, "var") != STATE_VAR) {
        if let Some(qs) = out.get_mut(&gets(row, "name")) {
            qs.vars.insert(gets(row, "var"), gets(row, "value"));
        }
    }
    out
}

/// `GrandBossManager.init`: every `grandboss_data` row. The NPC-template
/// filter (`NpcData.getTemplate != null`) runs on the game thread, which owns
/// the datapack; here we just read the table.
async fn load_grandboss_data(pool: &SqlitePool) -> Vec<crate::model::grand_boss::GrandBoss> {
    let rows = sqlx::query(
        "SELECT boss_id, loc_x, loc_y, loc_z, heading, respawn_time, currentHP, currentMP, status \
         FROM grandboss_data ORDER BY boss_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .map(|r| crate::model::grand_boss::GrandBoss {
            boss_id: geti(r, "boss_id") as i32,
            loc_x: geti(r, "loc_x") as i32,
            loc_y: geti(r, "loc_y") as i32,
            loc_z: geti(r, "loc_z") as i32,
            heading: geti(r, "heading") as i32,
            respawn_time: geti(r, "respawn_time"),
            current_hp: getf(r, "currentHP"),
            current_mp: getf(r, "currentMP"),
            status: geti(r, "status") as i32,
        })
        .collect()
}

/// `CursedWeaponsManager.restore`: every `cursed_weapons` state row.
async fn load_cursed_weapons(pool: &SqlitePool) -> Vec<CursedWeaponRow> {
    let rows = sqlx::query(
        "SELECT itemId, charId, playerReputation, playerPkKills, nbKills, endTime FROM cursed_weapons",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .map(|r| CursedWeaponRow {
            item_id: geti(r, "itemId") as i32,
            char_id: geti(r, "charId") as i32,
            player_reputation: geti(r, "playerReputation") as i32,
            player_pk_kills: geti(r, "playerPkKills") as i32,
            nb_kills: geti(r, "nbKills") as i32,
            end_time: geti(r, "endTime"),
        })
        .collect()
}

/// `ClanTable`'s boot restore: every `clan_data` row + its member roster
/// from `characters WHERE clanid=?` (Java `Clan.restore`).
/// The stationed siege guards (`castle_siege_guards WHERE isHired=0`) — the
/// non-mercenary garrison spawned at siege start.
async fn load_siege_guards(pool: &SqlitePool) -> Vec<(i32, crate::model::siege::SiegeSpawn)> {
    let rows = sqlx::query("SELECT castleId, npcId, x, y, z, heading FROM castle_siege_guards WHERE isHired=0")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| {
            (
                geti(r, "castleId") as i32,
                crate::model::siege::SiegeSpawn {
                    npc_id: geti(r, "npcId") as i32,
                    x: geti(r, "x") as i32,
                    y: geti(r, "y") as i32,
                    z: geti(r, "z") as i32,
                    heading: geti(r, "heading") as i32,
                },
            )
        })
        .collect()
}

/// `Siege.loadSiegeClan`: every `siege_clans` row.
async fn load_siege_clans(pool: &SqlitePool) -> Vec<SiegeClanRow> {
    let rows = sqlx::query("SELECT castle_id, clan_id, type FROM siege_clans").fetch_all(pool).await.unwrap_or_default();
    rows.iter()
        .map(|r| SiegeClanRow {
            castle_id: geti(r, "castle_id") as i32,
            clan_id: geti(r, "clan_id") as i32,
            kind: geti(r, "type") as i32,
        })
        .collect()
}

/// `CastleManager.load`: every `castle` row (id/name/side).
async fn load_castles(pool: &SqlitePool) -> Vec<crate::model::castle::Castle> {
    let rows = sqlx::query("SELECT id, name, side FROM castle ORDER BY id").fetch_all(pool).await.unwrap_or_default();
    rows.iter()
        .map(|r| crate::model::castle::Castle {
            id: geti(r, "id") as i32,
            name: gets(r, "name"),
            side: crate::model::castle::CastleSide::from_str(&gets(r, "side")).unwrap_or_default(),
        })
        .collect()
}

async fn load_clans(pool: &SqlitePool) -> Vec<crate::model::clan::Clan> {
    let clan_rows = sqlx::query("SELECT clan_id, clan_name, clan_level, reputation_score, hasCastle, leader_id, char_penalty_expiry_time, dissolving_expiry_time, new_leader_id, ally_id, ally_name, ally_penalty_expiry_time, ally_penalty_type FROM clan_data")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out = Vec::with_capacity(clan_rows.len());
    for row in &clan_rows {
        let clan_id = geti(row, "clan_id") as i32;
        let member_rows = sqlx::query("SELECT charId, char_name, level, classid, sex, race, power_grade, title, subpledge FROM characters WHERE clanid=?")
            .bind(clan_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        // Clan warehouse contents (`owner_id = clan_id`, `loc = "CLANWH"`).
        let wh_rows = load_items(pool, clan_id).await;
        // Clan skills (Java `Clan.restoreSkills`) — the main-pledge set
        // (`sub_pledge_id = -2`); sub-unit skills aren't modelled, so other
        // sub_pledge ids are ignored. Missing table → empty (graceful).
        let skill_rows = sqlx::query("SELECT skill_id, skill_level FROM clan_skills WHERE clan_id=? AND (sub_pledge_id=-2 OR sub_pledge_id=0)")
            .bind(clan_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        let skills = skill_rows
            .iter()
            .map(|s| (geti(s, "skill_id") as i32, geti(s, "skill_level") as i32))
            .collect();
        // Rank → privilege-mask rows (Java `restoreRankPrivs`; rank -1 skipped).
        let priv_rows = sqlx::query("SELECT `rank`, privs FROM clan_privs WHERE clan_id=?")
            .bind(clan_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        let rank_privs = priv_rows
            .iter()
            .map(|r| (geti(r, "rank") as i32, geti(r, "privs") as i32))
            .filter(|&(rank, _)| rank != -1)
            .collect();
        // Sub-pledges (Java `Clan.restoreSubPledges`).
        let sub_rows = sqlx::query("SELECT sub_pledge_id, name, leader_id FROM clan_subpledges WHERE clan_id=?")
            .bind(clan_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        let sub_pledges: std::collections::HashMap<i32, crate::model::clan::SubPledge> = sub_rows
            .iter()
            .map(|r| {
                let id = geti(r, "sub_pledge_id") as i32;
                (id, crate::model::clan::SubPledge { id, name: gets(r, "name"), leader_id: geti(r, "leader_id") as i32 })
            })
            .collect();
        out.push(crate::model::clan::Clan {
            id: clan_id,
            name: gets(row, "clan_name"),
            leader_id: geti(row, "leader_id") as i32,
            level: geti(row, "clan_level") as i32,
            reputation_score: geti(row, "reputation_score") as i32,
            castle_id: geti(row, "hasCastle") as i32,
            char_penalty_expiry_time: geti(row, "char_penalty_expiry_time"),
            dissolving_expiry_time: geti(row, "dissolving_expiry_time"),
            rank_privs,
            new_leader_id: geti(row, "new_leader_id") as i32,
            sub_pledges,
            ally_id: geti(row, "ally_id") as i32,
            ally_name: gets(row, "ally_name"),
            ally_penalty_expiry_time: geti(row, "ally_penalty_expiry_time"),
            ally_penalty_type: geti(row, "ally_penalty_type") as i32,
            skills,
            warehouse: crate::model::inventory::Warehouse::from_rows(&wh_rows),
            members: member_rows
                .iter()
                .map(|m| crate::model::clan::ClanMember {
                    char_id: geti(m, "charId") as i32,
                    name: gets(m, "char_name"),
                    level: geti(m, "level") as i32,
                    class_id: geti(m, "classid") as i32,
                    sex: geti(m, "sex") as i32,
                    race: geti(m, "race") as i32,
                    power_grade: geti(m, "power_grade") as i32,
                    title: gets(m, "title"),
                    pledge_type: geti(m, "subpledge") as i32,
                })
                .collect(),
        });
    }
    out
}

/// A character's `character_macroses` rows (Java `MacroList.restoreMe`),
/// commands decoded from the `type,d1,d2[,cmd];…` column encoding.
async fn load_macros(pool: &SqlitePool, owner_id: i32) -> Vec<crate::model::shortcut::Macro> {
    let rows = sqlx::query("SELECT id, icon, name, descr, acronym, commands FROM character_macroses WHERE charId=?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| crate::model::shortcut::Macro {
            id: geti(r, "id") as i32,
            icon: geti(r, "icon") as i32,
            name: gets(r, "name"),
            descr: gets(r, "descr"),
            acronym: gets(r, "acronym"),
            commands: crate::model::shortcut::decode_commands(&gets(r, "commands")),
        })
        .collect()
}

async fn upsert_shortcut(pool: &SqlitePool, char_id: i32, slot: i32, page: i32, kind: i32, shortcut_id: i32, level: i32) {
    exec(
        pool,
        sqlx::query(
            "INSERT INTO character_shortcuts (charId, slot, page, type, shortcut_id, level, sub_level, class_index) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 0) \
             ON CONFLICT(charId, slot, page, class_index) DO UPDATE SET \
             type=excluded.type, shortcut_id=excluded.shortcut_id, level=excluded.level",
        )
        .bind(char_id)
        .bind(slot)
        .bind(page)
        .bind(kind)
        .bind(shortcut_id)
        .bind(level),
    )
    .await;
}

async fn upsert_macro(pool: &SqlitePool, char_id: i32, m: &crate::model::shortcut::Macro) {
    exec(
        pool,
        sqlx::query(
            "INSERT INTO character_macroses (charId, id, icon, name, descr, acronym, commands) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(charId, id) DO UPDATE SET \
             icon=excluded.icon, name=excluded.name, descr=excluded.descr, \
             acronym=excluded.acronym, commands=excluded.commands",
        )
        .bind(char_id)
        .bind(m.id)
        .bind(m.icon)
        .bind(&m.name)
        .bind(&m.descr)
        .bind(&m.acronym)
        .bind(crate::model::shortcut::encode_commands(&m.commands)),
    )
    .await;
}

/// A character's `items` rows (Java: `PlayerInventory.restore`, called for
/// every row shown in `CharSelectionInfo`, not just the entered character).
async fn load_items(pool: &SqlitePool, owner_id: i32) -> Vec<ItemRow> {
    // Java `PlayerInventory.restore` orders by `loc_data` so a client's saved
    // inventory arrangement (`RequestSaveInventoryOrder`) survives relog.
    let rows = sqlx::query("SELECT * FROM items WHERE owner_id=? ORDER BY loc_data")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    // Augmentations (Java `Item.restoreAttributes`): object_id → (mineral, o1, o2).
    let var_rows = sqlx::query(
        "SELECT mineralId, option1, option2, itemId FROM item_variations WHERE itemId IN \
         (SELECT object_id FROM items WHERE owner_id=?)",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let variations: std::collections::HashMap<i32, (i32, i32, i32)> = var_rows
        .iter()
        .map(|r| (geti(r, "itemId") as i32, (geti(r, "mineralId") as i32, geti(r, "option1") as i32, geti(r, "option2") as i32)))
        .collect();
    rows.iter()
        .map(|r| {
            let object_id = geti(r, "object_id") as i32;
            let (augment_mineral, augment_option1, augment_option2) = variations.get(&object_id).copied().unwrap_or((0, 0, 0));
            ItemRow {
                object_id,
                item_id: geti(r, "item_id") as i32,
                count: geti(r, "count"),
                enchant_level: geti(r, "enchant_level") as i32,
                loc: gets(r, "loc"),
                loc_data: geti(r, "loc_data") as i32,
                custom_type1: geti(r, "custom_type1") as i32,
                custom_type2: geti(r, "custom_type2") as i32,
                mana_left: geti(r, "mana_left") as i32,
                time: geti(r, "time") as i32,
                augment_mineral,
                augment_option1,
                augment_option2,
            }
        })
        .collect()
}

/// Case-insensitive character-name existence check (`getIdByName`).
async fn name_exists(pool: &SqlitePool, name: &str) -> bool {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE char_name=? COLLATE NOCASE")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    n > 0
}

async fn create_character(pool: &SqlitePool, next_id: &mut i64, max_characters: i32, data: &NewCharacter) -> CreateResult {
    if name_exists(pool, &data.name).await {
        return CreateResult::NameExists;
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE account_name=?")
        .bind(&data.account)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if max_characters > 0 && count >= max_characters as i64 {
        return CreateResult::TooMany;
    }

    let char_id = *next_id;
    *next_id += 1;
    let res = sqlx::query(
        "INSERT INTO characters \
         (account_name, charId, char_name, level, maxHp, curHp, maxCp, curCp, maxMp, curMp, \
          face, hairStyle, hairColor, sex, heading, x, y, z, exp, sp, reputation, \
          race, classid, base_class, deletetime, title, accesslevel, online, char_slot, lastAccess, createDate, \
          vitality_points) \
         VALUES (?, ?, ?, 1, ?, ?, 0, 0, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, 0, 0, 0, ?, ?, ?, 0, '', 0, 0, ?, ?, date('now'), \
          ?)",
    )
    .bind(&data.account)
    .bind(char_id)
    .bind(&data.name)
    .bind(data.max_hp)
    .bind(data.max_hp) // curHp = maxHp
    .bind(data.max_mp)
    .bind(data.max_mp) // curMp = maxMp
    .bind(data.face)
    .bind(data.hair_style)
    .bind(data.hair_color)
    .bind(data.sex)
    .bind(data.x)
    .bind(data.y)
    .bind(data.z)
    .bind(data.race)
    .bind(data.class_id)
    .bind(data.class_id) // base_class = classid
    .bind(count as i32) // char_slot
    .bind(now_millis())
    .bind(data.vitality_points)
    .execute(pool)
    .await;

    match res {
        Ok(_) => {
            // Seed the recommendation row: Java `Player.create` grants
            // rec_left=20, persisted to `character_reco_bonus` when the
            // freshly-created character disconnects back to the lobby.
            exec(
                pool,
                sqlx::query("INSERT INTO character_reco_bonus (charId, rec_have, rec_left, time_left) VALUES (?, 0, 20, 0)")
                    .bind(char_id),
            )
            .await;
            // Initial skills (character_skills).
            for (skill_id, skill_level) in &data.skills {
                exec(
                    pool,
                    sqlx::query(
                        "INSERT INTO character_skills (charId, skill_id, skill_level, skill_sub_level, class_index) \
                         VALUES (?, ?, ?, 0, 0)",
                    )
                    .bind(char_id)
                    .bind(skill_id)
                    .bind(skill_level),
                )
                .await;
            }
            // Initial equipment + starting adena. The item_id → object_id
            // map feeds ITEM shortcut resolution below (first occurrence
            // wins, like Java `getItemByItemId`).
            let mut item_object_ids: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
            for item in &data.items {
                let item_object_id = *next_id;
                *next_id += 1;
                item_object_ids.entry(item.item_id).or_insert(item_object_id);
                let (loc, loc_data) = match item.paperdoll_index {
                    Some(slot) => ("PAPERDOLL", slot as i32),
                    None => ("INVENTORY", 0),
                };
                exec(
                    pool,
                    sqlx::query(
                        "INSERT INTO items \
                         (owner_id, object_id, item_id, count, enchant_level, loc, loc_data, \
                          custom_type1, custom_type2, mana_left, time) \
                         VALUES (?, ?, ?, ?, 0, ?, ?, 0, 0, -1, 0)",
                    )
                    .bind(char_id)
                    .bind(item_object_id)
                    .bind(item.item_id)
                    .bind(item.count)
                    .bind(loc)
                    .bind(loc_data),
                )
                .await;
            }
            // Initial shortcuts + macro presets (`InitialShortcutData.
            // registerAllShortcuts` — persistence only; there's no in-world
            // session to echo packets to at creation).
            for sc in &data.shortcuts {
                let shortcut_id = if sc.kind == crate::model::shortcut::ShortcutType::Item {
                    // ITEM entries reference an item id; skip ones the new
                    // character didn't actually receive (Java `continue`s).
                    match item_object_ids.get(&sc.id) {
                        Some(&object_id) => object_id as i32,
                        None => continue,
                    }
                } else {
                    sc.id
                };
                upsert_shortcut(pool, char_id as i32, sc.slot, sc.page, sc.kind.ordinal(), shortcut_id, sc.level).await;
            }
            for m in &data.macros {
                upsert_macro(pool, char_id as i32, m).await;
            }
            info!(
                "Created character '{}' ({}) for account {} with {} initial skill(s), {} item(s).",
                data.name,
                char_id,
                data.account,
                data.skills.len(),
                data.items.len()
            );
            CreateResult::Ok
        }
        Err(e) => {
            error!("DB thread: character insert failed: {e}");
            CreateResult::Fail
        }
    }
}

/// Java `Player.storeCharBase` (narrowed to the tracked columns, see
/// [`PlayerSnapshot`]) + `updateOnlineStatus` — the character leaves the world,
/// so `online=0` and `lastAccess=now` in the same write.
/// Flush a whole player to the database in one transaction — the only path that
/// writes character-owned gameplay state (memory-first model). Reconciles the
/// `characters` row plus every child table (items, skills, shortcuts, macros,
/// quests) so a single flush captures pickups, drops, equips, skill changes,
/// shortcut/macro edits and quest progress made since the last flush. Child
/// tables are rewritten wholesale (delete-this-owner + re-insert), which is
/// atomic inside the transaction and doubles as the delete path — anything no
/// longer in memory is gone from the DB after the flush. On any error the
/// transaction is dropped (rolled back) and logged, leaving the last good save
/// intact.
async fn store_player(pool: &SqlitePool, s: &PlayerSaveData) {
    if let Err(e) = store_player_tx(pool, s).await {
        error!("store_player: flush for char {} failed (rolled back): {e}", s.base.object_id);
    }
}

async fn store_player_tx(pool: &SqlitePool, s: &PlayerSaveData) -> Result<(), sqlx::Error> {
    let b = &s.base;
    let char_id = b.object_id;
    let mut tx = pool.begin().await?;

    // characters row (Java storeCharBase). online stays 0: the port never sets
    // it to 1, and char-select doesn't read it — a periodic save of an online
    // player must not diverge from that.
    sqlx::query(
        "UPDATE characters SET level=?, maxHp=?, curHp=?, maxCp=?, curCp=?, maxMp=?, curMp=?, \
         face=?, hairStyle=?, hairColor=?, sex=?, heading=?, x=?, y=?, z=?, exp=?, sp=?, \
         reputation=?, pvpkills=?, pkkills=?, race=?, classid=?, base_class=?, \
         vitality_points=?, pccafe_points=?, nobless=?, online=0, lastAccess=? WHERE charId=?",
    )
    .bind(b.level)
    .bind(b.max_hp)
    .bind(b.cur_hp)
    .bind(b.max_cp)
    .bind(b.cur_cp)
    .bind(b.max_mp)
    .bind(b.cur_mp)
    .bind(b.face)
    .bind(b.hair_style)
    .bind(b.hair_color)
    .bind(b.sex)
    .bind(b.heading)
    .bind(b.x)
    .bind(b.y)
    .bind(b.z)
    .bind(b.exp)
    .bind(b.sp)
    .bind(b.reputation)
    .bind(b.pvp_kills)
    .bind(b.pk_kills)
    .bind(b.race)
    .bind(b.class_id)
    .bind(b.base_class_id)
    .bind(b.vitality_points)
    .bind(b.pccafe_points)
    .bind(if b.noble { 1 } else { 0 })
    .bind(now_millis())
    .bind(char_id)
    .execute(&mut *tx)
    .await?;

    // character_reco_bonus (Java `Player.storeRecommendations`, an
    // insert-or-update on charId). `time_left` is always 0 here — the reco
    // bonus timer (bonusTime/bonusVal/bonusType in ExVoteSystemInfo) isn't
    // used in Interlude Classic. The unique index on charId makes this an
    // upsert.
    sqlx::query(
        "INSERT INTO character_reco_bonus (charId, rec_have, rec_left, time_left) VALUES (?, ?, ?, 0) \
         ON CONFLICT(charId) DO UPDATE SET rec_have=excluded.rec_have, rec_left=excluded.rec_left, time_left=excluded.time_left",
    )
    .bind(char_id)
    .bind(b.rec_have)
    .bind(b.rec_left)
    .execute(&mut *tx)
    .await?;

    // items (inventory + equipped): `Inventory::to_rows` is the whole owned set.
    sqlx::query("DELETE FROM items WHERE owner_id=?").bind(char_id).execute(&mut *tx).await?;
    for it in &s.items {
        sqlx::query(
            "INSERT INTO items \
             (owner_id, object_id, item_id, count, enchant_level, loc, loc_data, \
              custom_type1, custom_type2, mana_left, time) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(char_id)
        .bind(it.object_id)
        .bind(it.item_id)
        .bind(it.count)
        .bind(it.enchant_level)
        .bind(&it.loc)
        .bind(it.loc_data)
        .bind(it.custom_type1)
        .bind(it.custom_type2)
        .bind(it.mana_left)
        .bind(it.time)
        .execute(&mut *tx)
        .await?;
    }

    // Augmentations (`item_variations`, keyed by item object id). Scoped to the
    // just-reinserted owner items, then reinsert the augmented ones.
    sqlx::query("DELETE FROM item_variations WHERE itemId IN (SELECT object_id FROM items WHERE owner_id=?)")
        .bind(char_id)
        .execute(&mut *tx)
        .await?;
    for it in s.items.iter().filter(|it| it.augment_option1 != 0 || it.augment_option2 != 0) {
        sqlx::query("INSERT INTO item_variations (itemId, mineralId, option1, option2) VALUES (?, ?, ?, ?)")
            .bind(it.object_id)
            .bind(it.augment_mineral)
            .bind(it.augment_option1)
            .bind(it.augment_option2)
            .execute(&mut *tx)
            .await?;
    }

    // Learned skills, per class index (G17): every slot is rewritten, so a
    // subclass keeps its own book rather than inheriting the active one.
    sqlx::query("DELETE FROM character_skills WHERE charId=?").bind(char_id).execute(&mut *tx).await?;

    // worn henna dyes (Java stores per add/remove; here delete+reinsert on flush,
    // memory-first like items/skills). Per class index since G17.
    sqlx::query("DELETE FROM character_hennas WHERE charId=?").bind(char_id).execute(&mut *tx).await?;
    let mut henna_idx: Vec<(i32, &Vec<(i32, i32)>)> =
        s.hennas_by_index.iter().map(|(i, v)| (*i, v)).collect();
    henna_idx.push((s.class_index, &s.hennas));
    for (class_index, hennas) in henna_idx {
        for (slot, symbol_id) in hennas {
            sqlx::query(
                "INSERT OR REPLACE INTO character_hennas (charId, symbol_id, slot, class_index) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(char_id)
            .bind(symbol_id)
            .bind(slot)
            .bind(class_index)
            .execute(&mut *tx)
            .await?;
        }
    }
    // The active index's book comes from `skills`; the rest from the banked
    // per-index map.
    let mut per_index: Vec<(i32, &Vec<(i32, i32)>)> =
        s.skills_by_index.iter().map(|(i, v)| (*i, v)).collect();
    per_index.push((s.class_index, &s.skills));
    for (class_index, skills) in per_index {
        for (skill_id, level) in skills {
            sqlx::query(
                "INSERT OR REPLACE INTO character_skills \
                 (charId, skill_id, skill_level, skill_sub_level, class_index) \
                 VALUES (?, ?, ?, 0, ?)",
            )
            .bind(char_id)
            .bind(skill_id)
            .bind(level)
            .bind(class_index)
            .execute(&mut *tx)
            .await?;
        }
    }

    // registered recipes (Java saves per-registration; here delete+reinsert
    // with the persist flush, memory-first like items/skills). `type` = 1
    // dwarven / 0 common; `classIndex` 0.
    sqlx::query("DELETE FROM character_recipebook WHERE charId=? AND classIndex=0").bind(char_id).execute(&mut *tx).await?;
    for (list_id, is_dwarven) in &s.recipe_book {
        sqlx::query(
            "INSERT INTO character_recipebook (charId, id, classIndex, type) VALUES (?, ?, 0, ?)",
        )
        .bind(char_id)
        .bind(list_id)
        .bind(if *is_dwarven { 1 } else { 0 })
        .execute(&mut *tx)
        .await?;
    }

    // character variables (Java `PlayerVariables.storeMe` does exactly this
    // delete-then-reinsert, guarded by a dirty flag we don't need — the flush
    // is already batched).
    sqlx::query("DELETE FROM character_variables WHERE charId=?").bind(char_id).execute(&mut *tx).await?;
    for (var, val) in &s.variables {
        sqlx::query("INSERT INTO character_variables (charId, var, val) VALUES (?, ?, ?)")
            .bind(char_id)
            .bind(var)
            .bind(val)
            .execute(&mut *tx)
            .await?;
    }

    // shortcuts (Java's delete+insert, here scoped to the transaction).
    sqlx::query("DELETE FROM character_shortcuts WHERE charId=?").bind(char_id).execute(&mut *tx).await?;
    let mut sc_idx: Vec<(i32, &Vec<crate::model::shortcut::Shortcut>)> =
        s.shortcuts_by_index.iter().map(|(i, v)| (*i, v)).collect();
    sc_idx.push((s.class_index, &s.shortcuts));
    for (class_index, shortcuts) in sc_idx {
        for sc in shortcuts {
            sqlx::query(
                "INSERT OR REPLACE INTO character_shortcuts \
                 (charId, slot, page, type, shortcut_id, level, sub_level, class_index) \
                 VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
            )
            .bind(char_id)
            .bind(sc.slot)
            .bind(sc.page)
            .bind(sc.kind.ordinal())
            .bind(sc.id)
            .bind(sc.level)
            .bind(class_index)
            .execute(&mut *tx)
            .await?;
        }
    }

    // macros.
    sqlx::query("DELETE FROM character_macroses WHERE charId=?").bind(char_id).execute(&mut *tx).await?;
    for m in &s.macros {
        sqlx::query(
            "INSERT INTO character_macroses (charId, id, icon, name, descr, acronym, commands) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(char_id)
        .bind(m.id)
        .bind(m.icon)
        .bind(&m.name)
        .bind(&m.descr)
        .bind(&m.acronym)
        .bind(crate::model::shortcut::encode_commands(&m.commands))
        .execute(&mut *tx)
        .await?;
    }

    // quests: one `<state>` row per quest + a row per var (the shape
    // `load_quests` reconstructs). Skip freshly-`CREATED` quests with no vars —
    // Java never wrote a row for those, and a touched-but-untouched quest state
    // must not start persisting where Java wouldn't.
    sqlx::query("DELETE FROM character_quests WHERE charId=?").bind(char_id).execute(&mut *tx).await?;
    for (name, qs) in &s.quests {
        use crate::model::quest::{state, STATE_VAR};
        if qs.state == state::CREATED && qs.vars.is_empty() {
            continue;
        }
        sqlx::query("INSERT INTO character_quests (charId, name, var, value) VALUES (?, ?, ?, ?)")
            .bind(char_id)
            .bind(name)
            .bind(STATE_VAR)
            .bind(state::name(qs.state))
            .execute(&mut *tx)
            .await?;
        for (var, value) in &qs.vars {
            sqlx::query("INSERT INTO character_quests (charId, name, var, value) VALUES (?, ?, ?, ?)")
                .bind(char_id)
                .bind(name)
                .bind(var)
                .bind(value)
                .execute(&mut *tx)
                .await?;
        }
    }

    // Active buffs (restore_type 0) + skill reuse cooldowns (restore_type 1),
    // both in `character_skills_save` under the *active* class index (Java
    // `storeEffect` writes one batch for both). Always delete first so an
    // emptied set (or `StoreSkillCooltime` turned off, which makes both vectors
    // empty) clears stale rows.
    //
    // Buff rows carry the relative `remaining_time` and a zero `systime`: a
    // buff's countdown is frozen while offline, so there is no absolute end
    // instant to record. Reuse rows are the mirror image — `remaining_time` is
    // -1 and only `systime` is read back — because cooldowns *do* decay offline.
    // `buff_index` is a single sequence across both kinds, matching Java's
    // shared `++buffIndex` counter.
    sqlx::query("DELETE FROM character_skills_save WHERE charId=? AND class_index=?")
        .bind(char_id)
        .bind(s.class_index)
        .execute(&mut *tx)
        .await?;
    for (i, b) in s.skill_buffs.iter().enumerate() {
        sqlx::query(
            "INSERT INTO character_skills_save \
             (charId, skill_id, skill_level, skill_sub_level, remaining_time, reuse_delay, systime, restore_type, class_index, buff_index) \
             VALUES (?, ?, ?, 0, ?, 0, 0, 0, ?, ?)",
        )
        .bind(char_id)
        .bind(b.skill_id)
        .bind(b.skill_level)
        .bind(b.remaining_time_secs)
        .bind(s.class_index)
        .bind(i as i32 + 1)
        .execute(&mut *tx)
        .await?;
    }
    let buff_rows = s.skill_buffs.len() as i32;
    for (i, r) in s.skill_reuses.iter().enumerate() {
        sqlx::query(
            "INSERT INTO character_skills_save \
             (charId, skill_id, skill_level, skill_sub_level, remaining_time, reuse_delay, systime, restore_type, class_index, buff_index) \
             VALUES (?, ?, ?, 0, -1, ?, ?, 1, ?, ?)",
        )
        .bind(char_id)
        .bind(r.reuse_key)
        .bind(r.skill_level)
        .bind(r.reuse_delay)
        .bind(r.systime_ms)
        .bind(s.class_index)
        .bind(buff_rows + i as i32 + 1)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Character count + pending-deletion timestamps for the login server's
/// `ReplyCharacters` (Java `LoginServerThread.getCharsOnServer`). Mirrors
/// [`load_characters`]: a character whose deletion timer has **expired** is
/// purged and excluded, so the login server-select count never exceeds the
/// char-select list the client sees on entry (the port has no separate global
/// expired-char sweep, so counting raw rows would over-report).
async fn count_characters(pool: &SqlitePool, account: &str) -> (u8, Vec<i64>) {
    let rows = sqlx::query("SELECT charId, deletetime FROM characters WHERE account_name=?")
        .bind(account)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let now = now_millis();
    let mut count: u8 = 0;
    let mut del_times = Vec::new();
    for row in &rows {
        let delete_time = geti(row, "deletetime");
        if delete_time > 0 && now > delete_time {
            delete_char(pool, geti(row, "charId") as i32).await; // restoreChar: purge expired
            continue;
        }
        count += 1;
        if delete_time != 0 {
            del_times.push(delete_time); // still counting down toward deletion
        }
    }
    (count, del_times)
}

async fn delete_char(pool: &SqlitePool, char_id: i32) {
    exec(pool, sqlx::query("DELETE FROM characters WHERE charId=?").bind(char_id)).await;
}

async fn exec<'q>(pool: &SqlitePool, q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>) {
    if let Err(e) = q.execute(pool).await {
        warn!("DB thread: query failed: {e}");
    }
}

// SQLite is dynamically typed; fetch numeric columns leniently.
fn geti(row: &sqlx::sqlite::SqliteRow, col: &str) -> i64 {
    row.try_get::<i64, _>(col).or_else(|_| row.try_get::<f64, _>(col).map(|f| f as i64)).unwrap_or(0)
}
fn getf(row: &sqlx::sqlite::SqliteRow, col: &str) -> f64 {
    row.try_get::<f64, _>(col).or_else(|_| row.try_get::<i64, _>(col).map(|i| i as f64)).unwrap_or(0.0)
}
fn gets(row: &sqlx::sqlite::SqliteRow, col: &str) -> String {
    row.try_get::<String, _>(col).unwrap_or_default()
}

/// `ClanTable.restoreClanWars` — the `clan_wars` table (ids in the varchar
/// columns, as Java writes them).
async fn load_clan_wars(pool: &SqlitePool) -> Vec<crate::model::clan::ClanWar> {
    let rows = sqlx::query("SELECT clan1, clan2, clan1Kill, clan2Kill, winnerClan, startTime, endTime, state FROM clan_wars")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| crate::model::clan::ClanWar {
            attacker_id: geti(r, "clan1") as i32,
            attacked_id: geti(r, "clan2") as i32,
            attacker_kills: geti(r, "clan1Kill") as i32,
            attacked_kills: geti(r, "clan2Kill") as i32,
            winner_id: geti(r, "winnerClan") as i32,
            start_time: geti(r, "startTime"),
            end_time: geti(r, "endTime"),
            state: crate::model::clan::ClanWarState::from_i32(geti(r, "state") as i32),
        })
        .collect()
}
