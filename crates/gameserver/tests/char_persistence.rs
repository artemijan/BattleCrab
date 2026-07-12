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
        items: vec![],
        shortcuts: vec![],
        macros: vec![],
    }
}

fn recv(rx: &std::sync::mpsc::Receiver<DbEvent>) -> DbEvent {
    loop {
        match rx.recv_timeout(Duration::from_secs(5)).expect("db event") {
            // Boot-time pushes (id reservation, clan table), not part of
            // any exchange.
            DbEvent::IdBlock { .. } | DbEvent::ClansLoaded { .. } => continue,
            other => return other,
        }
    }
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

/// G9.6: initial shortcuts/macros persist at creation (ITEM entries resolved
/// to the created items' object ids, missing items dropped), the runtime
/// upsert/delete commands round-trip, and macro commands survive the
/// `type,d1,d2[,cmd];` column encoding.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shortcuts_and_macros_persist() {
    use gameserver::model::shortcut::{Macro, MacroCmd, MacroType, ShortcutType};

    let dir = std::env::temp_dir().join(format!("l2r_g96_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "character_skills", "character_shortcuts", "character_macroses"] {
            let schema = std::fs::read_to_string(format!("{sql_root}/{table}.sql")).unwrap();
            for stmt in schema.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(stmt).execute(&pool).await.unwrap();
            }
        }
        pool.close().await;
    }

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let handle = db::spawn(url.clone(), 1, 7, cmd_rx, event_tx);

    let sc = |slot: i32, kind: ShortcutType, id: i32, level: i32| db::NewShortcut { slot, page: 0, kind, id, level };
    let preset = Macro {
        id: 10000,
        icon: 1,
        name: "preset".into(),
        descr: "d".into(),
        acronym: "p".into(),
        commands: vec![
            MacroCmd { entry: 0, kind: MacroType::Skill, d1: 1177, d2: 1, cmd: String::new() },
            MacroCmd { entry: 1, kind: MacroType::Text, d1: 0, d2: 0, cmd: "/loc".into() },
        ],
    };
    let mut data = new_char("Shorty");
    data.items = vec![db::NewItem { item_id: 2369, count: 1, paperdoll_index: Some(5) }];
    data.shortcuts = vec![
        sc(0, ShortcutType::Action, 2, 0),
        sc(1, ShortcutType::Item, 2369, 0),
        sc(2, ShortcutType::Item, 999, 0), // item the class didn't get — dropped
        sc(3, ShortcutType::Skill, 1177, 1),
        sc(4, ShortcutType::Macro, 10000, 0),
    ];
    data.macros = vec![preset.clone()];

    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharacterCreated { result, .. } => assert_eq!(result, CreateResult::Ok),
        _ => panic!("expected CharacterCreated"),
    }
    let (char_id, item_object_id) = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let c = &chars[0];
            assert_eq!(c.items.len(), 1);
            let item_oid = c.items[0].object_id;
            assert_eq!(c.shortcuts.len(), 4, "missing-item shortcut dropped");
            let item_sc = c.shortcuts.iter().find(|s| s.kind == ShortcutType::Item).unwrap();
            assert_eq!(item_sc.id, item_oid, "ITEM shortcut resolved to the created object id");
            assert!(c.shortcuts.iter().any(|s| s.kind == ShortcutType::Skill && s.id == 1177 && s.level == 1));
            assert!(c.shortcuts.iter().any(|s| s.kind == ShortcutType::Macro && s.id == 10000));
            assert_eq!(c.macros.len(), 1);
            assert_eq!(c.macros[0].commands, preset.commands, "commands column round-trips");
            (c.object_id, item_oid)
        }
        _ => panic!("expected CharactersLoaded"),
    };
    let _ = item_object_id;

    // Runtime traffic: overwrite a slot, delete one, add + delete a macro.
    cmd_tx.send(DbCommand::UpsertShortcut { char_id, slot: 0, page: 0, kind: ShortcutType::Skill.ordinal(), shortcut_id: 1177, level: 2 }).unwrap();
    cmd_tx.send(DbCommand::DeleteShortcut { char_id, slot: 3, page: 0 }).unwrap();
    let user_macro = Macro { id: 1000, icon: 0, name: "mine".into(), descr: String::new(), acronym: String::new(), commands: vec![] };
    cmd_tx.send(DbCommand::UpsertMacro { char_id, macro_: user_macro }).unwrap();
    cmd_tx.send(DbCommand::DeleteMacro { char_id, macro_id: 10000 }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let c = &chars[0];
            assert_eq!(c.shortcuts.len(), 3);
            let slot0 = c.shortcuts.iter().find(|s| s.slot == 0 && s.page == 0).unwrap();
            assert_eq!((slot0.kind, slot0.id, slot0.level), (ShortcutType::Skill, 1177, 2), "slot overwritten in place");
            assert!(!c.shortcuts.iter().any(|s| s.slot == 3), "deleted slot gone");
            assert_eq!(c.macros.len(), 1);
            assert_eq!(c.macros[0].id, 1000, "preset deleted, user macro kept");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// G10: friendship rows round-trip — the pair insert writes both directions,
/// the reload joins the friend's name/level/class, and the pair delete
/// removes both rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn friendships_persist() {
    let dir = std::env::temp_dir().join(format!("l2r_g10_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "character_skills", "character_shortcuts", "character_macroses", "character_friends"] {
            let schema = std::fs::read_to_string(format!("{sql_root}/{table}.sql")).unwrap();
            for stmt in schema.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(stmt).execute(&pool).await.unwrap();
            }
        }
        pool.close().await;
    }

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let handle = db::spawn(url.clone(), 1, 7, cmd_rx, event_tx);

    // Two characters on separate accounts.
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Aria") }).unwrap();
    recv(&event_rx); // CharacterCreated
    let aria = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].object_id,
        _ => panic!("expected CharactersLoaded"),
    };
    let mut second = new_char("Boro");
    second.account = "acc2".into();
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 2, data: second }).unwrap();
    recv(&event_rx);
    let boro = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].object_id,
        _ => panic!("expected CharactersLoaded"),
    };

    // Befriend + reload: both sides see each other with joined columns.
    cmd_tx.send(DbCommand::InsertFriendPair { a: aria, b: boro }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars[0].friends.len(), 1);
            let f = &chars[0].friends[0];
            assert_eq!((f.char_id, f.name.as_str(), f.level), (boro, "Boro", 1));
        }
        _ => panic!("expected CharactersLoaded"),
    }
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 2, account: "acc2".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars[0].friends.len(), 1);
            assert_eq!(chars[0].friends[0].char_id, aria);
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Unfriend removes both rows.
    cmd_tx.send(DbCommand::DeleteFriendPair { a: boro, b: aria }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert!(chars[0].friends.is_empty()),
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// G11: quest-state rows persist and reload through the real DB thread —
/// var upserts, the repeatable/non-repeatable delete split, and the
/// orphan-var filter (vars without a `<state>` row don't load).
#[tokio::test]
async fn quest_states_persist() {
    use gameserver::model::quest::{state, STATE_VAR};

    let dir = std::env::temp_dir().join(format!("l2r_g11_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "character_skills", "character_shortcuts", "character_macroses", "character_friends", "character_quests"] {
            let schema = std::fs::read_to_string(format!("{sql_root}/{table}.sql")).unwrap();
            for stmt in schema.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(stmt).execute(&pool).await.unwrap();
            }
        }
        pool.close().await;
    }

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let handle = db::spawn(url.clone(), 1, 7, cmd_rx, event_tx);

    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Ques") }).unwrap();
    recv(&event_rx); // CharacterCreated
    let oid = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].object_id,
        _ => panic!("expected CharactersLoaded"),
    };

    let quest = "Q00258_BringWolfPelts";
    let up = |var: &str, value: &str| DbCommand::UpsertQuestVar {
        char_id: oid,
        quest: quest.into(),
        var: var.into(),
        value: value.into(),
    };
    cmd_tx.send(up(STATE_VAR, "Started")).unwrap();
    cmd_tx.send(up("cond", "1")).unwrap();
    cmd_tx.send(up("cond", "2")).unwrap(); // upsert path: same PK, new value
    // Orphan rows (no <state>) must not surface as a quest.
    cmd_tx
        .send(DbCommand::UpsertQuestVar {
            char_id: oid,
            quest: "Q99999_Ghost".into(),
            var: "cond".into(),
            value: "5".into(),
        })
        .unwrap();

    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let quests = &chars[0].quests;
            assert_eq!(quests.len(), 1, "orphan vars must not create a quest");
            let qs = &quests[quest];
            assert_eq!(qs.state, state::STARTED);
            assert_eq!(qs.cond(), 2);
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Non-repeatable exit: vars deleted, <state> row kept (COMPLETED write
    // comes through a normal UpsertQuestVar from the engine).
    cmd_tx.send(up(STATE_VAR, "Completed")).unwrap();
    cmd_tx.send(DbCommand::DeleteQuest { char_id: oid, quest: quest.into(), keep_state: true }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let qs = &chars[0].quests[quest];
            assert_eq!(qs.state, state::COMPLETED);
            assert!(qs.vars.is_empty(), "non-repeatable delete keeps only <state>");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Repeatable exit: everything gone.
    cmd_tx.send(DbCommand::DeleteQuest { char_id: oid, quest: quest.into(), keep_state: false }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert!(chars[0].quests.is_empty()),
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
