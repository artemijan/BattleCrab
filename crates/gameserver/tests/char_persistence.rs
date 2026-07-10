//! G3 persistence test: drive the real DB thread through
//! create → load → mark-delete → restore against a temp SQLite database using
//! the stock `characters` schema. This is the heart of the G3 gate
//! ("create a character, it persists, delete works").

use std::time::Duration;

use gameserver::db::{self, CreateResult, DbCommand, DbEvent, NewCharacter};

fn new_char(name: &str) -> NewCharacter {
    NewCharacter {
        account: "acc".into(),
        name: name.into(),
        race: 0,
        class_id: 0,
        sex: 0,
        face: 1,
        hair_style: 2,
        hair_color: 3,
        x: -71338,
        y: 258271,
        z: -3104,
        max_hp: 80,
        max_mp: 30,
        skills: vec![(1177, 1)],
    }
}

fn recv(rx: &std::sync::mpsc::Receiver<DbEvent>) -> DbEvent {
    rx.recv_timeout(Duration::from_secs(5)).expect("db event")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_persist_delete_restore() {
    // Temp database with the stock characters schema.
    let dir = std::env::temp_dir().join(format!("l2r_g3_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let schema = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/sqlite/game/characters.sql"
    ))
    .unwrap();
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for stmt in schema.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool.close().await;
    }

    // Run the DB thread against it.
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let handle = db::spawn(url.clone(), 1, 7, cmd_rx, event_tx);

    // Create a character.
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Hero") }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharacterCreated { result, .. } => assert_eq!(result, CreateResult::Ok),
        _ => panic!("expected CharacterCreated"),
    }
    let char_id = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars.len(), 1);
            assert_eq!(chars[0].name, "Hero");
            assert_eq!(chars[0].x, -71338);
            assert_eq!(chars[0].max_hp, 80);
            assert!(chars[0].object_id >= 0x10000000);
            chars[0].object_id
        }
        _ => panic!("expected CharactersLoaded"),
    };

    // Duplicate name is rejected.
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Hero") }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharacterCreated { result, .. } => assert_eq!(result, CreateResult::NameExists),
        _ => panic!("expected CharacterCreated(NameExists)"),
    }

    // It persists: a fresh load still finds it.
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert_eq!(chars.len(), 1),
        _ => panic!("expected CharactersLoaded"),
    }

    // Mark for deletion (3-day timer) → still listed, now with a delete time.
    let delete_time = commons::util::now_millis() + 3 * 86_400_000;
    cmd_tx.send(DbCommand::MarkDelete { client_id: 1, account: "acc".into(), char_id, delete_time }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars.len(), 1);
            assert_eq!(chars[0].delete_time, delete_time);
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Restore → delete time cleared.
    cmd_tx.send(DbCommand::RestoreCharacter { client_id: 1, account: "acc".into(), char_id }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars.len(), 1);
            assert_eq!(chars[0].delete_time, 0);
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Char count for the login server's ReplyCharacters.
    cmd_tx.send(DbCommand::CountCharacters { account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharCount { count, del_times, .. } => {
            assert_eq!(count, 1);
            assert!(del_times.is_empty());
        }
        _ => panic!("expected CharCount"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
