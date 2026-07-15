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
    pub race: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub vitality_points: i32,
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
            race: p.race,
            class_id: p.class_id,
            base_class_id: p.base_class_id,
            vitality_points: p.vitality_points,
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
    /// Learned skills as `(skill_id, skill_level)` (class_index 0).
    pub skills: Vec<(i32, i32)>,
    /// Panel/hotbar shortcuts (`Shortcuts` component).
    pub shortcuts: Vec<crate::model::shortcut::Shortcut>,
    /// Macro definitions (`Macros` component).
    pub macros: Vec<crate::model::shortcut::Macro>,
    /// Quest states + vars (`Quests` component), keyed by quest name.
    pub quests: std::collections::HashMap<String, crate::model::quest::QuestState>,
    /// Live skill reuse cooldowns (`Reuses` component) as `character_skills_save`
    /// rows — empty when `StoreSkillCooltime` is off. See [`SkillReuseRow`].
    pub skill_reuses: Vec<SkillReuseRow>,
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
    /// Fire-and-forget clan-warehouse flush — delete every `owner_id = clan_id`
    /// item row (`loc = "CLANWH"`) and reinsert the current set (the same
    /// delete-then-reinsert the player item save uses).
    StoreClanWarehouse { clan_id: i32, items: Vec<ItemRow> },
    /// Java `Player.setAccessLevel(updateInDb=true)` — persist a GM access-level
    /// change immediately (the memory-first autosave doesn't carry accesslevel).
    SetAccessLevel { char_id: i32, level: i32 },
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
    ClansLoaded { clans: Vec<crate::model::clan::Clan> },
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

    // `ClanTable`'s boot restore, likewise unprompted.
    let _ = event_tx.send(DbEvent::ClansLoaded { clans: load_clans(&pool).await });

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
        let skills = load_skills(pool, object_id).await;
        let shortcuts = load_shortcuts(pool, object_id).await;
        let macros = load_macros(pool, object_id).await;
        let friends = load_friends(pool, object_id).await;
        let quests = load_quests(pool, object_id).await;
        let skill_reuses = load_skill_reuses(pool, object_id).await;
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
            clan_id: geti(row, "clanid") as i32,
            clan_privs: geti(row, "clan_privs") as i32,
            clan_create_expiry_time: geti(row, "clan_create_expiry_time"),
            race: geti(row, "race") as i32,
            class_id: geti(row, "classid") as i32,
            base_class_id: geti(row, "base_class") as i32,
            delete_time,
            last_access: geti(row, "lastAccess"),
            vitality_points: geti(row, "vitality_points") as i32,
            access_level: geti(row, "accesslevel") as i32,
            noble: geti(row, "nobless") == 1,
            char_slot: slot as i32,
            items,
            skills,
            shortcuts,
            macros,
            friends,
            quests,
            skill_reuses,
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
async fn load_skills(pool: &SqlitePool, owner_id: i32) -> Vec<(i32, i32)> {
    let rows = sqlx::query("SELECT skill_id, skill_level FROM character_skills WHERE charId=? AND class_index=0")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter().map(|r| (geti(r, "skill_id") as i32, geti(r, "skill_level") as i32)).collect()
}

/// A character's `character_skills_save` reuse rows (Java `restoreEffects`,
/// `restore_type = 1` half). Already-expired rows (`systime <= now`) are
/// dropped here; the survivors carry the absolute `systime` and the game side
/// converts it to a game tick when the character enters the world. Buff rows
/// (restore_type 0) are ignored — buff restore is a later milestone.
async fn load_skill_reuses(pool: &SqlitePool, owner_id: i32) -> Vec<SkillReuseRow> {
    let now = now_millis();
    let rows = sqlx::query(
        "SELECT skill_id, skill_level, reuse_delay, systime FROM character_skills_save \
         WHERE charId=? AND class_index=0 AND restore_type=1",
    )
    .bind(owner_id)
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

/// A character's `character_shortcuts` rows (Java `ShortCuts.restoreMe` —
/// the inventory verification half runs on the game thread, in
/// `Player::from_char`). `characterType` isn't stored; restore hardcodes 1
/// like Java. `shared_reuse_group` starts at the -1 default; `from_char`
/// fills it for EtcItem shortcuts.
async fn load_shortcuts(pool: &SqlitePool, owner_id: i32) -> Vec<crate::model::shortcut::Shortcut> {
    let rows = sqlx::query("SELECT slot, page, type, shortcut_id, level FROM character_shortcuts WHERE charId=? AND class_index=0")
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| crate::model::shortcut::Shortcut {
            slot: geti(r, "slot") as i32,
            page: geti(r, "page") as i32,
            kind: crate::model::shortcut::ShortcutType::from_ordinal(geti(r, "type") as i32),
            id: geti(r, "shortcut_id") as i32,
            level: geti(r, "level") as i32,
            character_type: 1,
            shared_reuse_group: -1,
        })
        .collect()
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

/// `ClanTable`'s boot restore: every `clan_data` row + its member roster
/// from `characters WHERE clanid=?` (Java `Clan.restore`).
async fn load_clans(pool: &SqlitePool) -> Vec<crate::model::clan::Clan> {
    let clan_rows = sqlx::query("SELECT clan_id, clan_name, clan_level, leader_id FROM clan_data")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut out = Vec::with_capacity(clan_rows.len());
    for row in &clan_rows {
        let clan_id = geti(row, "clan_id") as i32;
        let member_rows = sqlx::query("SELECT charId, char_name, level, classid, sex, race FROM characters WHERE clanid=?")
            .bind(clan_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        // Clan warehouse contents (`owner_id = clan_id`, `loc = "CLANWH"`).
        let wh_rows = load_items(pool, clan_id).await;
        out.push(crate::model::clan::Clan {
            id: clan_id,
            name: gets(row, "clan_name"),
            leader_id: geti(row, "leader_id") as i32,
            level: geti(row, "clan_level") as i32,
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
          race, classid, base_class, deletetime, title, accesslevel, online, char_slot, lastAccess, createDate) \
         VALUES (?, ?, ?, 1, ?, ?, 0, 0, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, 0, 0, 0, ?, ?, ?, 0, '', 0, 0, ?, ?, date('now'))",
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
    .execute(pool)
    .await;

    match res {
        Ok(_) => {
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
         vitality_points=?, online=0, lastAccess=? WHERE charId=?",
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
    .bind(now_millis())
    .bind(char_id)
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

    // learned skills (class_index 0 — no subclasses on this dist).
    sqlx::query("DELETE FROM character_skills WHERE charId=? AND class_index=0").bind(char_id).execute(&mut *tx).await?;
    for (skill_id, level) in &s.skills {
        sqlx::query(
            "INSERT INTO character_skills (charId, skill_id, skill_level, skill_sub_level, class_index) \
             VALUES (?, ?, ?, 0, 0)",
        )
        .bind(char_id)
        .bind(skill_id)
        .bind(level)
        .execute(&mut *tx)
        .await?;
    }

    // shortcuts (Java's delete+insert, here scoped to the transaction).
    sqlx::query("DELETE FROM character_shortcuts WHERE charId=? AND class_index=0").bind(char_id).execute(&mut *tx).await?;
    for sc in &s.shortcuts {
        sqlx::query(
            "INSERT INTO character_shortcuts (charId, slot, page, type, shortcut_id, level, sub_level, class_index) \
             VALUES (?, ?, ?, ?, ?, ?, 0, 0)",
        )
        .bind(char_id)
        .bind(sc.slot)
        .bind(sc.page)
        .bind(sc.kind.ordinal())
        .bind(sc.id)
        .bind(sc.level)
        .execute(&mut *tx)
        .await?;
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

    // skill reuse cooldowns (`character_skills_save`, restore_type 1). Always
    // delete first so an emptied set (or `StoreSkillCooltime` turned off, which
    // makes `skill_reuses` empty) clears stale rows; `remaining_time` is -1 like
    // Java's reuse rows (only `systime` is read back). buff rows (restore_type 0)
    // are a later milestone and never written here.
    sqlx::query("DELETE FROM character_skills_save WHERE charId=? AND class_index=0")
        .bind(char_id)
        .execute(&mut *tx)
        .await?;
    for (i, r) in s.skill_reuses.iter().enumerate() {
        sqlx::query(
            "INSERT INTO character_skills_save \
             (charId, skill_id, skill_level, skill_sub_level, remaining_time, reuse_delay, systime, restore_type, class_index, buff_index) \
             VALUES (?, ?, ?, 0, -1, ?, ?, 1, 0, ?)",
        )
        .bind(char_id)
        .bind(r.reuse_key)
        .bind(r.skill_level)
        .bind(r.reuse_delay)
        .bind(r.systime_ms)
        .bind(i as i32 + 1)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn count_characters(pool: &SqlitePool, account: &str) -> (u8, Vec<i64>) {
    let rows = sqlx::query("SELECT deletetime FROM characters WHERE account_name=?")
        .bind(account)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let count = rows.len() as u8;
    let del_times = rows.iter().map(|r| geti(r, "deletetime")).filter(|&t| t != 0).collect();
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
