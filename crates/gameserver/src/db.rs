//! The DB thread (CONCURRENCY_MODEL §2.4). A dedicated OS thread owns the SQLite
//! pool; the game thread never blocks on the database — it sends [`DbCommand`]s
//! and drains [`DbEvent`]s each tick. Character id allocation lives here too
//! (a minimal `IdManager`).

use std::thread::JoinHandle;

use sqlx::{Row, SqlitePool};
use tracing::{error, info, warn};

use crate::character::CharData;
use commons::util::now_millis;

/// First object id handed out by `IdManager` (Java `FIRST_OID`).
const FIRST_OID: i64 = 0x10000000;

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

async fn load_next_id(pool: &SqlitePool) -> i64 {
    let max: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(charId), 0) FROM characters")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    (max + 1).max(FIRST_OID)
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
            race: geti(row, "race") as i32,
            class_id: geti(row, "classid") as i32,
            base_class_id: geti(row, "base_class") as i32,
            delete_time,
            last_access: geti(row, "lastAccess"),
            vitality_points: geti(row, "vitality_points") as i32,
            access_level: geti(row, "accesslevel") as i32,
            noble: geti(row, "nobless") == 1,
            char_slot: slot as i32,
        });
    }
    out
}

async fn create_character(pool: &SqlitePool, next_id: &mut i64, max_characters: i32, data: &NewCharacter) -> CreateResult {
    // Name uniqueness (case-insensitive, like the client enforces).
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE char_name=? COLLATE NOCASE")
        .bind(&data.name)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if exists > 0 {
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
            info!("Created character '{}' ({}) for account {}.", data.name, char_id, data.account);
            CreateResult::Ok
        }
        Err(e) => {
            error!("DB thread: character insert failed: {e}");
            CreateResult::Fail
        }
    }
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
