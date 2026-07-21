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
        vitality_points: 0,
    }
}

/// Build a full `PlayerSaveData` from a loaded character, mirroring what the
/// game thread's `build_save_data` gathers — the memory-first flush unit. Tests
/// mutate the child vecs, then send one `StorePlayer` to exercise the DB
/// thread's transactional reconcile (adds, in-place updates, and deletions).
fn save_from(c: &gameserver::character::CharData) -> db::PlayerSaveData {
    db::PlayerSaveData {
        base: db::PlayerSnapshot {
            object_id: c.object_id,
            level: c.level,
            max_hp: c.max_hp,
            cur_hp: c.cur_hp,
            max_cp: 0,
            cur_cp: 0.0,
            max_mp: c.max_mp,
            cur_mp: c.cur_mp,
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            sex: c.sex,
            heading: 0,
            x: c.x,
            y: c.y,
            z: c.z,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pvp_kills: c.pvp_kills,
            pk_kills: c.pk_kills,
            rec_have: c.rec_have,
            rec_left: c.rec_left,
            race: c.race,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            vitality_points: c.vitality_points,
            pccafe_points: c.pccafe_points,
            noble: false,
        },
        items: c.items.clone(),
        skills: c.skills.clone(),
        skills_by_index: Default::default(),
        hennas_by_index: Default::default(),
        shortcuts_by_index: Default::default(),
        class_index: 0,
        hennas: c.hennas.clone(),
        recipe_book: c.recipe_book.iter().map(|&id| (id, true)).collect(),
        variables: c.variables.clone(),
        pets: c.pets.clone(),
        shortcuts: c.shortcuts.clone(),
        macros: c.macros.clone(),
        quests: c.quests.clone(),
        skill_reuses: c.skill_reuses.clone(),
        skill_buffs: c.skill_buffs.clone(),
    }
}

fn recv(rx: &std::sync::mpsc::Receiver<DbEvent>) -> DbEvent {
    loop {
        match rx.recv_timeout(Duration::from_secs(5)).expect("db event") {
            // Boot-time pushes (id reservation, premium/grand-boss/cursed-weapon/
            // clan tables), not part of any exchange.
            DbEvent::IdBlock { .. }
            | DbEvent::ClansLoaded { .. }
            | DbEvent::PremiumLoaded { .. }
            | DbEvent::BufferSchemesLoaded { .. }
            | DbEvent::FavoritesLoaded { .. }
            | DbEvent::GrandBossesLoaded { .. }
            | DbEvent::CursedWeaponsLoaded { .. }
            | DbEvent::CastlesLoaded { .. }
            | DbEvent::SiegesLoaded { .. }
            | DbEvent::SiegeGuardsLoaded { .. }
            | DbEvent::NpcRespawnsLoaded { .. } => continue,
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

/// The login server-select char count (`CountCharacters` → `ReplyCharacters`)
/// must match the char-select list the client sees: a character whose deletion
/// timer has expired is purged and excluded from the count, while one still
/// counting down is kept. Regression for the login screen over-reporting when
/// an expired-deletion row lingered (no GS load had purged it yet).
#[tokio::test]
async fn login_char_count_excludes_expired_deletions() {
    let dir = std::env::temp_dir().join(format!("l2r_charcount_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "character_skills", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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

    // Three characters on the account.
    for name in ["Alive", "Pending", "Doomed"] {
        cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char(name) }).unwrap();
        recv(&event_rx); // CharacterCreated
        recv(&event_rx); // CharactersLoaded
    }
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    let chars = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars,
        _ => panic!("expected CharactersLoaded"),
    };
    let id_of = |n: &str| chars.iter().find(|c| c.name == n).unwrap().object_id;
    let now = commons::util::now_millis();

    // "Pending" is counting down (future); "Doomed" already expired — stamped
    // directly so no `LoadCharacters` purge intervenes before the count.
    cmd_tx
        .send(DbCommand::MarkDelete { client_id: 1, account: "acc".into(), char_id: id_of("Pending"), delete_time: now + 3 * 86_400_000 })
        .unwrap();
    recv(&event_rx); // CharactersLoaded (Pending kept — still counting down)
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        sqlx::query("UPDATE characters SET deletetime=? WHERE charId=?").bind(1_i64).bind(id_of("Doomed")).execute(&pool).await.unwrap();
        pool.close().await;
    }

    // The login count: 3 rows → 2 (Doomed purged), 1 pending-deletion timestamp.
    cmd_tx.send(DbCommand::CountCharacters { account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharCount { count, del_times, .. } => {
            assert_eq!(count, 2, "expired deletion excluded from the login count");
            assert_eq!(del_times.len(), 1, "only the still-counting-down deletion is reported");
        }
        _ => panic!("expected CharCount"),
    }

    // …and the expired row is actually purged, so the char-select list agrees.
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars.len(), 2, "two characters remain");
            assert!(!chars.iter().any(|c| c.name == "Doomed"), "expired char purged");
        }
        _ => panic!("expected CharactersLoaded"),
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
        // `character_quests` is needed too: the memory-first flush reconciles
        // every child table, so `store_player` always touches it (even with no
        // quests, to delete any that were abandoned).
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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
    let loaded = match recv(&event_rx) {
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
            c.clone()
        }
        _ => panic!("expected CharactersLoaded"),
    };

    // Memory-first flush: the game thread mutates its in-memory copy (overwrite
    // slot 0, delete slot 3, replace the preset macro with a user macro) and
    // sends one `StorePlayer`. The DB thread's `store_player` reconciles every
    // child table in a transaction — in-place updates, deletions, and untouched
    // rows (the item + skill + surviving shortcuts) all in one write.
    let mut save = save_from(&loaded);
    // A base `characters` column (pccafe_points) round-trips through the flush.
    save.base.pccafe_points = 4200;
    for sc in save.shortcuts.iter_mut().filter(|s| s.slot == 0 && s.page == 0) {
        sc.kind = ShortcutType::Skill;
        sc.id = 1177;
        sc.level = 2;
    }
    save.shortcuts.retain(|s| !(s.slot == 3 && s.page == 0));
    save.macros = vec![Macro { id: 1000, icon: 0, name: "mine".into(), descr: String::new(), acronym: String::new(), commands: vec![] }];
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
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
            // Untouched child rows survive the reconcile.
            assert_eq!(c.items.len(), 1, "item preserved");
            assert!(c.skills.iter().any(|&(id, lvl)| id == 1177 && lvl == 1), "skill preserved");
            assert_eq!(c.pccafe_points, 4200, "pccafe points persisted");
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
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_friends", "character_reco_bonus", "character_hennas", "character_recipebook", "character_variables"] {
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
    use gameserver::model::quest::{state, QuestState};

    let dir = std::env::temp_dir().join(format!("l2r_g11_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_friends", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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
    let loaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].clone(),
        _ => panic!("expected CharactersLoaded"),
    };

    let quest = "Q00258_BringWolfPelts";
    // Flush the quest STARTED at cond 2 as one memory-first `StorePlayer`. The DB
    // thread writes one `<state>` row + a `cond` row inside the reconcile
    // transaction; the load path rebuilds the `QuestState`.
    {
        let mut save = save_from(&loaded);
        let mut qs = QuestState { state: state::STARTED, ..Default::default() };
        qs.vars.insert("cond".into(), "2".into());
        save.quests.insert(quest.into(), qs);
        cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    }
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let quests = &chars[0].quests;
            assert_eq!(quests.len(), 1);
            let qs = &quests[quest];
            assert_eq!(qs.state, state::STARTED);
            assert_eq!(qs.cond(), 2);
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Non-repeatable exit (COMPLETED, vars cleared): flushing the quest with an
    // empty var set makes the reconcile drop the old `cond` row and keep only
    // the `<state>` row — Java's `keep_state` delete, now a consequence of the
    // full-state rewrite.
    {
        let mut save = save_from(&loaded);
        save.quests.insert(quest.into(), QuestState { state: state::COMPLETED, ..Default::default() });
        cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    }
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let qs = &chars[0].quests[quest];
            assert_eq!(qs.state, state::COMPLETED);
            assert!(qs.vars.is_empty(), "COMPLETED keeps only <state>");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // Repeatable exit (quest forgotten): flushing with no quest at all makes the
    // reconcile delete every row for it.
    cmd_tx.send(DbCommand::StorePlayer { save: save_from(&loaded) }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert!(chars[0].quests.is_empty()),
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Recommendations round-trip through `character_reco_bonus` (Java
/// `Player.create` seed → `loadRecommendations` → `storeRecommendations`): a new
/// character loads with rec_left=20/rec_have=0, and a flush persists updated
/// counts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recommendations_persist() {
    let dir = std::env::temp_dir().join(format!("l2r_reco_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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

    // Create → the seed row grants 20 recommendations to give.
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Recruit") }).unwrap();
    assert!(matches!(recv(&event_rx), DbEvent::CharacterCreated { result: CreateResult::Ok, .. }));
    let loaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!((chars[0].rec_have, chars[0].rec_left), (0, 20), "new character seed");
            chars[0].clone()
        }
        _ => panic!("expected CharactersLoaded"),
    };

    // Flush updated counts (recommended a few players, received one).
    let mut save = save_from(&loaded);
    save.base.rec_have = 7;
    save.base.rec_left = 13;
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();

    // Reload → the flushed counts survive.
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!((chars[0].rec_have, chars[0].rec_left), (7, 13), "flushed counts persist");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // The daily reset command zeroes rec_left and decays rec_have for the
    // offline row (rec_have 7 <= 20 → 0).
    cmd_tx.send(DbCommand::ResetRecommends).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!((chars[0].rec_have, chars[0].rec_left), (0, 0), "daily reset");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// G13.9: skill reuse cooldowns round-trip through `character_skills_save`
/// (Java `storeEffect`/`restoreEffects`, reuse half). A cooldown ending in the
/// future survives the reload; one that already elapsed while offline is
/// filtered out; and a flush with no cooldowns clears the table.
#[tokio::test]
async fn skill_reuse_cooldowns_persist() {
    let dir = std::env::temp_dir().join(format!("l2r_g139_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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

    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Cooldown") }).unwrap();
    recv(&event_rx); // CharacterCreated
    let loaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].clone(),
        _ => panic!("expected CharactersLoaded"),
    };

    let now = commons::util::now_millis();
    let mut save = save_from(&loaded);
    save.skill_reuses = vec![
        db::SkillReuseRow { reuse_key: 1177, skill_level: 3, reuse_delay: 300_000, systime_ms: now + 120_000 },
        db::SkillReuseRow { reuse_key: 1178, skill_level: 1, reuse_delay: 10_000, systime_ms: now - 5_000 },
    ];
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let r = &chars[0].skill_reuses;
            assert_eq!(r.len(), 1, "the already-elapsed cooldown is filtered on load");
            assert_eq!((r[0].reuse_key, r[0].skill_level, r[0].reuse_delay), (1177, 3, 300_000));
            assert!(r[0].systime_ms >= now + 120_000 - 5_000, "future systime preserved");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // A flush with no live cooldowns clears the table (the reconcile always deletes).
    cmd_tx.send(DbCommand::StorePlayer { save: save_from(&loaded) }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert!(chars[0].skill_reuses.is_empty()),
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Active buffs round-trip through `character_skills_save` (Java
/// `storeEffect`/`restoreEffects`, buff half). The stored `remaining_time` is
/// relative and comes back **verbatim** however much wall-clock time passed —
/// a buff's countdown is frozen while the character is offline, which is the
/// whole difference between this and `skill_reuse_cooldowns_persist` above.
/// Buff and reuse rows also have to coexist in the one table without either
/// kind bleeding into the other's load.
#[tokio::test]
async fn active_buffs_persist_with_frozen_countdown() {
    let dir = std::env::temp_dir().join(format!("l2r_buffsave_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables"] {
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

    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data: new_char("Buffed") }).unwrap();
    recv(&event_rx); // CharacterCreated
    let loaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => chars[0].clone(),
        _ => panic!("expected CharactersLoaded"),
    };

    let now = commons::util::now_millis();
    let mut save = save_from(&loaded);
    // Wind Walk with 20 min left, Might with 5 s left, and an already-dead row.
    save.skill_buffs = vec![
        db::SkillBuffRow { skill_id: 1204, skill_level: 2, remaining_time_secs: 1200 },
        db::SkillBuffRow { skill_id: 1068, skill_level: 3, remaining_time_secs: 5 },
        db::SkillBuffRow { skill_id: 1085, skill_level: 1, remaining_time_secs: 0 },
    ];
    // A cooldown shares the table; it must not come back as a buff.
    save.skill_reuses = vec![db::SkillReuseRow { reuse_key: 1177, skill_level: 3, reuse_delay: 300_000, systime_ms: now + 120_000 }];
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let b = &chars[0].skill_buffs;
            assert_eq!(b.len(), 2, "the zero-remaining buff is filtered on load");
            // `buff_index` order is preserved, so the bar comes back as stored.
            assert_eq!((b[0].skill_id, b[0].skill_level, b[0].remaining_time_secs), (1204, 2, 1200));
            assert_eq!((b[1].skill_id, b[1].skill_level, b[1].remaining_time_secs), (1068, 3, 5));
            assert_eq!(chars[0].skill_reuses.len(), 1, "the reuse row loads as a reuse, not a buff");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    // A flush with no live buffs clears them (the reconcile always deletes).
    cmd_tx.send(DbCommand::StorePlayer { save: save_from(&loaded) }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => assert!(chars[0].skill_buffs.is_empty()),
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// G29: a pet row round-trips through the real `pets` table — written by the
/// character flush, keyed by its collar's object id, and reloaded at login.
///
/// This is the gate's "and it persists": the in-memory half is covered by
/// `servitor_tests`, but only this exercises the actual schema, so a column
/// name or bind-order mistake shows up here rather than in production.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pets_persist() {
    let dir = std::env::temp_dir().join(format!("l2r_g29pets_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("jdbc:sqlite:{}", db_path.display());

    let sql_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/db_installer/sql/sqlite/game");
    {
        let pool = commons::db::init(&url, 1).await.unwrap();
        for table in ["characters", "items", "item_variations", "character_skills", "character_skills_save", "character_shortcuts", "character_macroses", "character_reco_bonus", "character_quests", "character_hennas", "character_recipebook", "character_variables", "pets"] {
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

    let mut data = new_char("Beastmaster");
    // The Wolf Collar this pet is bound to.
    data.items = vec![db::NewItem { item_id: 2375, count: 1, paperdoll_index: None }];
    cmd_tx.send(DbCommand::CreateCharacter { client_id: 1, data }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharacterCreated { result, .. } => assert_eq!(result, CreateResult::Ok),
        _ => panic!("expected CharacterCreated"),
    }
    let loaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert!(chars[0].pets.is_empty(), "a new character owns no pets");
            chars[0].clone()
        }
        _ => panic!("expected CharactersLoaded"),
    };
    let collar_oid = loaded.items[0].object_id;

    // Summon + wound the pet, then flush the character.
    let mut save = save_from(&loaded);
    save.pets = vec![db::PetRow {
        collar_object_id: collar_oid,
        name: "Wolf".into(),
        level: 5,
        cur_hp: 91.5,
        cur_mp: 20.0,
        exp: 12_345,
        sp: 67,
        fed: 140,
        // Java stores this as the literal string "true"/"false"; the reconnect
        // resummon reads it back to decide whether the pet was out at logout.
        restore: true,
    }];
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    let reloaded = match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            let c = &chars[0];
            assert_eq!(c.pets.len(), 1, "the pet row came back");
            let p = &c.pets[0];
            assert_eq!(p.collar_object_id, collar_oid, "keyed by the collar's object id");
            assert_eq!(p.level, 5);
            assert_eq!(p.exp, 12_345);
            assert_eq!(p.sp, 67);
            assert_eq!(p.fed, 140);
            assert_eq!(p.cur_hp, 91.5, "fractional HP survives the column type");
            assert!(p.restore, "the 'was out at logout' flag round-trips as a string column");
            assert_eq!(c.items.len(), 1, "the collar itself is untouched");
            c.clone()
        }
        _ => panic!("expected CharactersLoaded"),
    };

    // A second flush must *update* the row, not duplicate it — the upsert runs
    // on every save, so a wrong conflict target would grow the table forever.
    let mut save = save_from(&reloaded);
    save.pets[0].fed = 30;
    save.pets[0].level = 6;
    save.pets[0].restore = false;
    cmd_tx.send(DbCommand::StorePlayer { save }).unwrap();
    cmd_tx.send(DbCommand::LoadCharacters { client_id: 1, account: "acc".into() }).unwrap();
    match recv(&event_rx) {
        DbEvent::CharactersLoaded { chars, .. } => {
            assert_eq!(chars[0].pets.len(), 1, "re-saving updates in place, no duplicate row");
            assert_eq!(chars[0].pets[0].fed, 30);
            assert_eq!(chars[0].pets[0].level, 6);
            assert!(!chars[0].pets[0].restore, "and it can be cleared again");
        }
        _ => panic!("expected CharactersLoaded"),
    }

    cmd_tx.send(DbCommand::Shutdown).unwrap();
    tokio::task::spawn_blocking(move || handle.join()).await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
