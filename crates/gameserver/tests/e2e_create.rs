//! Full end-to-end test: a scripted client logs into the real login server,
//! selects the server, connects to the real game server, authenticates, and
//! creates a character — exactly the path the GUI client takes. This reproduces
//! and regression-tests the G3 character-creation flow across both servers.

use migration::MigratorTrait;
use models::sea_orm;
use std::sync::Arc;
use std::time::Duration;

use commons::crypt::NewCrypt;
use commons::network::{PacketWriter, read_frame, write_frame};
use gameserver::network::cipher::Encryption;
use num_bigint_dig::BigUint;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

// ---------- login server + client crypto (mirrors loginserver test harness) ----------

const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

/// The login side of this test gets the real schema, from the migrations.
async fn setup_login_schema(db: &sea_orm::DatabaseConnection) {
    migration::Migrator::up(db, None).await.unwrap();
    models::repo::gameservers::register(db, 1, "-2ad66b3f483c22be097019f55c8abdf0", "")
        .await
        .unwrap();
}

fn login_config() -> loginserver::config::LoginConfig {
    use loginserver::config::LoginConfig;
    LoginConfig {
        // Status channel off in tests: nothing binds a monitoring port.
        internal_status_bind_address: "127.0.0.1".to_string(),
        internal_status_port: 0,
        login_bind_address: "127.0.0.1".into(),
        port_login: 0,
        game_server_login_host: "127.0.0.1".into(),
        game_server_login_port: 0,
        database_url: String::new(),
        database_max_connections: 1,
        login_try_before_ban: 5,
        login_block_after_ban: 900,
        accept_new_gameserver: true,
        enable_flood_protection: false,
        fast_connection_limit: 15,
        normal_connection_time: 700,
        fast_connection_time: 350,
        max_connection_per_ip: 50,
        enable_cmd_line_login: false,
        only_cmd_line_login: false,
        show_licence: true,
        show_pi_agreement: false,
        auto_create_accounts: true,
        datapack_root: ".".into(),
        login_server_schedule_restart: false,
        login_server_schedule_restart_time: 24,
        backup_database: false,
        backup_path: "../backup/".into(),
    }
}

/// Start the login server; returns (client_addr, gs_addr).
async fn start_login() -> (std::net::SocketAddr, std::net::SocketAddr) {
    use loginserver::controller::{ControllerSettings, spawn};
    // One connection: a second one would open its own empty `:memory:`.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    setup_login_schema(&db).await;
    let mut gs_table = loginserver::gs_table::GameServerTable::load(&db).await;
    gs_table.server_names.insert(1, "Bartz".to_string());
    let config = login_config();
    let controller = spawn(
        ControllerSettings {
            auto_create_accounts: true,
            login_try_before_ban: 5,
            login_block_after_ban_ms: 900_000,
            show_licence: true,
            accept_new_gameserver: true,
        },
        db.clone(),
        gs_table,
    );
    let ctx = Arc::new(loginserver::context::LoginContext::new(
        config, db, controller,
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(loginserver::network::client_connection::accept_loop(
        ctx.clone(),
        listener,
    ));
    let gs_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gs_addr = gs_listener.local_addr().unwrap();
    tokio::spawn(loginserver::gs_link::listener::accept_loop(
        ctx,
        gs_listener,
    ));
    (addr, gs_addr)
}

fn dec_xor_pass(data: &mut [u8]) {
    let stop = data.len() - 8;
    let read = |d: &[u8], i: usize| i32::from_le_bytes(d[i..i + 4].try_into().unwrap());
    let mut ecx = read(data, stop);
    let mut pos = stop as isize - 4;
    while pos >= 4 {
        let enc = read(data, pos as usize);
        let edx = enc ^ ecx;
        data[pos as usize..pos as usize + 4].copy_from_slice(&edx.to_le_bytes());
        ecx = ecx.wrapping_sub(edx);
        pos -= 4;
    }
}

fn login_encrypt(crypt: &NewCrypt, body: &[u8]) -> Vec<u8> {
    let mut size = body.len() + 4;
    size += 8 - (size % 8);
    let mut data = body.to_vec();
    data.resize(size, 0);
    NewCrypt::append_checksum(&mut data);
    crypt.crypt(&mut data);
    data
}

fn unscramble_modulus(scrambled: &[u8]) -> BigUint {
    let mut m: Vec<u8> = scrambled.to_vec();
    for i in 0..0x40 {
        m[0x40 + i] ^= m[i];
    }
    for i in 0..4 {
        m[0x0d + i] ^= m[0x34 + i];
    }
    for i in 0..0x40 {
        m[i] ^= m[0x40 + i];
    }
    for i in 0..4 {
        m.swap(i, 0x4d + i);
    }
    BigUint::from_bytes_be(&m)
}

fn encrypt_credentials(modulus: &BigUint, user: &str, password: &str) -> [u8; 0x80] {
    let mut plain = [0u8; 0x80];
    plain[0x5E..0x5E + user.len()].copy_from_slice(user.as_bytes());
    plain[0x6C..0x6C + password.len()].copy_from_slice(password.as_bytes());
    let m = BigUint::from_bytes_be(&plain);
    let c = m.modpow(&BigUint::from(65537u32), modulus);
    let bytes = c.to_bytes_be();
    let mut out = [0u8; 0x80];
    out[0x80 - bytes.len()..].copy_from_slice(&bytes);
    out
}

struct LoginClient {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    crypt: NewCrypt,
    modulus: BigUint,
}

impl LoginClient {
    async fn send(&mut self, body: &[u8]) {
        write_frame(&mut self.write, &login_encrypt(&self.crypt, body))
            .await
            .unwrap();
    }
    async fn recv(&mut self) -> Vec<u8> {
        let mut d = read_frame(&mut self.read, 8192).await.unwrap().unwrap();
        self.crypt.decrypt(&mut d);
        d
    }
}

/// Run the login flow; returns the 4 session-key ints (loginOk1/2, playOk1/2).
async fn do_login(addr: std::net::SocketAddr, user: &str, password: &str) -> (i32, i32, i32, i32) {
    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut read, write) = stream.into_split();
    let mut init = read_frame(&mut read, 8192).await.unwrap().unwrap();
    NewCrypt::new(&STATIC_BLOWFISH_KEY).decrypt(&mut init);
    dec_xor_pass(&mut init);
    assert_eq!(init[0], 0x00, "Init opcode");
    let modulus = unscramble_modulus(&init[9..9 + 128]);
    let blowfish_key: [u8; 16] = init[9 + 128 + 16..9 + 128 + 32].try_into().unwrap();
    let mut c = LoginClient {
        read,
        write,
        crypt: NewCrypt::new(&blowfish_key),
        modulus,
    };

    // GameGuard.
    let mut gg = vec![0x07u8];
    gg.extend_from_slice(&init[1..5]); // session id
    gg.extend_from_slice(&[0u8; 16]);
    c.send(&gg).await;
    assert_eq!(c.recv().await[0], 0x0b, "GGAuth");

    // RequestAuthLogin.
    let mut body = vec![0x00u8];
    body.extend_from_slice(&encrypt_credentials(&c.modulus, user, password));
    c.send(&body).await;
    let reply = c.recv().await;
    assert_eq!(reply[0], 0x03, "LoginOk");
    let login_ok1 = i32::from_le_bytes(reply[1..5].try_into().unwrap());
    let login_ok2 = i32::from_le_bytes(reply[5..9].try_into().unwrap());

    // RequestServerList.
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    c.send(&w.into_bytes()).await;
    assert_eq!(c.recv().await[0], 0x04, "ServerList");

    // RequestServerLogin → PlayOk.
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    w.write_u8(1);
    c.send(&w.into_bytes()).await;
    let play = c.recv().await;
    assert_eq!(play[0], 0x07, "PlayOk");
    let play_ok1 = i32::from_le_bytes(play[1..5].try_into().unwrap());
    let play_ok2 = i32::from_le_bytes(play[5..9].try_into().unwrap());
    (login_ok1, login_ok2, play_ok1, play_ok2)
}

// ---------- game server ----------

async fn start_game(
    gs_login_addr: std::net::SocketAddr,
    db_url: String,
    cfg: gameserver::config::CombatConfig,
) -> std::net::SocketAddr {
    use gameserver::db::{self, DbCommand};
    use gameserver::events::GameEvent;
    use gameserver::game_loop::{self, GameThreadChannels, Shutdown};
    use gameserver::loginlink::{self, LoginLinkConfig};
    use gameserver::network::NetEventTx;
    use gameserver::network::connection::{self, NetworkConfig};

    let (events_tx, events_rx) = std::sync::mpsc::channel::<GameEvent>();
    let net_tx = NetEventTx(events_tx.clone());
    let login_tx = loginlink::EventTx(events_tx.clone());
    let db_event_tx = db::EventTx(events_tx.clone());
    let path_event_tx = gameserver::geo::worker::PathEventTx(events_tx);
    let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, db_cmd_rx) = tokio::sync::mpsc::unbounded_channel::<DbCommand>();

    db::spawn(
        db_url,
        1,
        7,
        false,
        db::GroundItemBootConfig {
            save_dropped_item: false,
            clear_dropped_item_table: false,
            empty_dropped_item_table_after_load: false,
        },
        db_cmd_rx,
        db_event_tx,
    );

    let geo = Arc::new(gameserver::geo::GeoEngine::empty());
    let (path_tx, path_req_rx) = std::sync::mpsc::channel();
    gameserver::geo::worker::spawn(geo.clone(), Default::default(), path_req_rx, path_event_tx);

    let data = gameserver::data::GameData::load();
    let (login_ready_tx, login_ready_rx) = tokio::sync::oneshot::channel::<()>();
    let _game = game_loop::spawn(
        Shutdown::new(),
        GameThreadChannels {
            events_rx,
            link_tx: link_tx.clone(),
            login_ready_tx,
            db_tx,
            data,
            geo,
            path_tx,
            path_finding: 2,
            path_cfg: Default::default(),
            geoedit_path: "saves/".to_string(),
            max_characters_per_account: 7,
            delete_days: 3,
            starting_adena: 100,
            cfg,
        },
    );

    // hexid.txt lives under the cwd (dist/game) we set in the test.
    let hex = gameserver::config::HexId::load(1);
    let link_cfg = LoginLinkConfig {
        host: gs_login_addr.ip().to_string(),
        port: gs_login_addr.port(),
        game_port: 0,
        hex_id: hex.hex_id,
        request_id: hex.server_id,
        accept_alternate: false,
        reserve_host: false,
        max_players: 100,
        hosts: vec![("0.0.0.0/0".into(), "127.0.0.1".into())],
        server_list_type: 0x400,
        server_list_bracket: false,
        server_list_age: 0,
        gmonly: false,
    };
    tokio::spawn(loginlink::run(link_cfg, link_rx, login_tx, login_ready_rx));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let net_cfg = Arc::new(NetworkConfig {
        packet_encryption: true,
        protocol_list: vec![110],
        server_id: 1,
        is_classic: true,
        // The shipped `Security.ini` defaults, so the transport limits are
        // exercised by the end-to-end tests rather than bypassed by them.
        security: gameserver::config::SecurityConfig::default(),
        // The dist `Network.ini` policy, so the e2e path runs with dropping
        // armed (the threshold is far above anything this test queues).
        drop_packets: true,
        drop_packet_threshold: 2500,
    });
    tokio::spawn(connection::accept_loop(listener, net_tx, net_cfg));
    addr
}

struct GameClient {
    stream: TcpStream,
    // One cipher for both directions (shared enabled flag), like the server's.
    crypt: Encryption,
}

impl GameClient {
    async fn connect(addr: std::net::SocketAddr) -> Self {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        // ProtocolVersion (plaintext).
        let mut w = PacketWriter::new();
        w.write_u8(0x0E);
        w.write_i32(110);
        write_frame(&mut stream, &w.into_bytes()).await.unwrap();
        let kp = read_frame(&mut stream, 65535).await.unwrap().unwrap();
        assert_eq!(kp[0], 0x2E, "KeyPacket");
        let key8: [u8; 8] = kp[2..10].try_into().unwrap();
        let key = Encryption::key_from_random(&key8);
        let mut crypt = Encryption::new();
        crypt.set_key(&key);
        crypt.encrypt(&mut Vec::new()); // enable the cipher (both directions)
        Self { stream, crypt }
    }
    async fn send(&mut self, body: &[u8]) {
        let mut b = body.to_vec();
        self.crypt.encrypt(&mut b);
        write_frame(&mut self.stream, &b).await.unwrap();
    }
    async fn recv(&mut self) -> Vec<u8> {
        let mut d = read_frame(&mut self.stream, 65535).await.unwrap().unwrap();
        self.crypt.decrypt(&mut d);
        d
    }
    /// Like `recv`, but skips unsolicited packets a live world can push at any
    /// time, so a reply-then-assert exchange isn't derailed by background world
    /// activity that happens to land on the wire first:
    /// - `StatusUpdate` (0x18) — the 3 s passive-regen tick since G6.
    /// - the starting village's NPCs, described and *animated* around the
    ///   mage-start spawn: `NpcInfo` (0x0C) as they enter the region grid,
    ///   `DeleteObject` (0x08) as they leave it, and — since G9 wander AI —
    ///   `MoveToLocation` (0x2F) / `StopMove` (0x47) as a nearby NPC walks and
    ///   arrives. (These are for *NPC* object ids; the player is idle here.)
    /// - `ExSetCompassZoneCode` (FE:0x33) — G12 zone revalidation reporting the
    ///   spawn point's peace-zone compass icon.
    async fn recv_skip_status_update(&mut self) -> Vec<u8> {
        // Ambient broadcasts that can land between a request and its reply.
        // 0x27 SocialAction is an NPC *idle animation* — it fires on a random
        // timer for any NPC near the player, so it can interleave anywhere.
        // This is the most likely cause of the intermittent failures this test
        // showed before minions existed; adding escorts near the spawn made it
        // near-deterministic rather than introducing it.
        const SKIP: &[u8] = &[0x18, 0x0C, 0x08, 0x2F, 0x47, 0x27];
        loop {
            let pkt = self.recv().await;
            let compass =
                pkt[0] == 0xFE && pkt.len() >= 3 && u16::from_le_bytes([pkt[1], pkt[2]]) == 0x33;
            if !SKIP.contains(&pkt[0]) && !compass {
                return pkt;
            }
        }
    }
}

/// Walk the UserInfo mask blocks to MAX_HPCPMP and return (maxHp, maxMp).
fn parse_userinfo_hpmp(ui: &[u8]) -> (i32, i32) {
    // opcode(1) + objectId(4) + initSize(4) + maskCount(2) + mask(3) = 14.
    let mut pos = 14;
    pos += 4; // RELATION block: a bare int, no length prefix.
    let block_len = |b: &[u8], p: usize| u16::from_le_bytes([b[p], b[p + 1]]) as usize;
    pos += block_len(ui, pos); // BASIC_INFO
    pos += block_len(ui, pos); // BASE_STATS
    // MAX_HPCPMP: short(len) then maxHp, maxMp, maxCp.
    let hp = i32::from_le_bytes(ui[pos + 2..pos + 6].try_into().unwrap());
    let mp = i32::from_le_bytes(ui[pos + 6..pos + 10].try_into().unwrap());
    (hp, mp)
}

/// Walk the UserInfo mask blocks to the STATS block and return
/// `(p_atk, p_def, m_atk_spd)`. STATS layout: len(2) + i16(20 no-weapon) + 13
/// i32s (p_atk[0], p_atk_spd[1], p_def[2], …, m_atk_spd[7], …).
fn parse_userinfo_stats(ui: &[u8]) -> (i32, i32, i32) {
    let block_len = |b: &[u8], p: usize| u16::from_le_bytes([b[p], b[p + 1]]) as usize;
    let mut pos = 14;
    pos += 4; // RELATION
    pos += block_len(ui, pos); // BASIC_INFO
    pos += block_len(ui, pos); // BASE_STATS
    pos += block_len(ui, pos); // MAX_HPCPMP
    pos += block_len(ui, pos); // CURRENT_HPMPCP_EXP_SP
    pos += block_len(ui, pos); // ENCHANTLEVEL
    pos += block_len(ui, pos); // APPEARANCE
    pos += block_len(ui, pos); // STATUS
    let field =
        |i: usize| i32::from_le_bytes(ui[pos + 4 + i * 4..pos + 8 + i * 4].try_into().unwrap());
    (field(0), field(2), field(7)) // p_atk, p_def, m_atk_spd
}

fn u16str(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

// This test was broken (a 120 s timeout, not an assertion failure) from its
// introduction until 2026-08-06, and **both causes recorded over that time
// were wrong** — a cautionary tale worth keeping. "RequestServerLogin answers
// PlayFail" (recorded until 2026-08-05) was false: the login half completes
// cleanly on both logins. "A spurious 0x57 triggers an unprompted restart on
// the relogin" (recorded until 2026-08-06) misread the trace: the 0x57 the
// server dispatches is the RequestRestart *this test sends* in its restart
// phase, and everything up to and including that restart works.
//
// The real cause, from a full per-packet trace of both sides: the
// post-restart CharacterSelect was silently swallowed by the CharacterSelect
// flood protector (`FloodProtector.ini` interval 30 ticks = 3 s — Java's
// `CharacterSelect.runImpl` returns without any reply inside the window, and
// the port mirrors it). The scripted client re-selected within ~2 s of its
// first select and then blocked forever on a CharSelected that was never
// coming. Server behavior was retail-faithful all along; the fix was to stop
// re-selecting inside the window — first by sleeping it out, now by turning
// that one protector slot off in `start_game` (the sleep was 3.2 s on the
// suite's longest test).
//
// Note the test self-skips unless the untracked `interlude_classic.db` is
// present, so a fresh checkout / CI runs it as a no-op.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_login_to_character_create() {
    // The runtime DB is an untracked working-tree file. A fixed `../../` finds
    // it only from the main checkout — from a git worktree (this repo's normal
    // workflow) it resolves to the worktree root and the test silently skipped.
    // Walking up the ancestors finds the checkout's copy, so a worktree run
    // gets the same coverage; only a fresh clone skips. Guard before the global
    // cwd change so a skipped run leaves the process cwd untouched.
    let Some(src) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|d| d.join("interlude_classic.db"))
        .find(|p| p.exists())
    else {
        eprintln!("skipping full_login_to_character_create: no interlude_classic.db found");
        return;
    };
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")).unwrap();

    // Fresh characters DB copied from the real one.
    let dir = std::env::temp_dir().join(format!("l2r_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("c.db");
    std::fs::copy(src, &db_path).unwrap();
    let db_url = format!("jdbc:sqlite:{}", db_path.display());

    let (login_addr, gs_addr) = start_login().await;
    let game_addr = start_game(gs_addr, db_url.clone(), {
        // This dist runs `EnableVitality = True`, and `EnterWorld` gates the
        // `ExVitalityEffectInfo` block on it — the burst this test walks
        // packet-by-packet only matches with it on.
        let mut cfg = gameserver::config::CombatConfig::default();
        cfg.character.enable_vitality = true;
        // The one protector this scripted client cannot satisfy: it re-selects
        // its character milliseconds after the first select, and the 3-second
        // `CharacterSelect` window silently swallows that (see the re-select
        // below). Only this slot is turned off; the other fourteen still gate
        // the flow.
        cfg.flood_protector
            .disable(gameserver::config::flood_protector::FloodAction::CharacterSelect);
        cfg
    })
    .await;
    // Give the game server time to register with the login server.
    tokio::time::sleep(Duration::from_millis(600)).await;

    let account = format!("e2e{}", std::process::id() % 100000);
    let (lo1, lo2, po1, po2) = do_login(login_addr, &account, "pw").await;

    // Connect to the game server and authenticate.
    let mut g = GameClient::connect(game_addr).await;
    let mut w = PacketWriter::new();
    w.write_u8(0x2B); // AuthLogin
    w.write_bytes(&u16str(&account));
    w.write_i32(po2);
    w.write_i32(po1);
    w.write_i32(lo1);
    w.write_i32(lo2);
    g.send(&w.into_bytes()).await;

    // LoginFail.LOGIN_SUCCESS (opcode 0x0A), then CharSelectionInfo (0x09).
    let ok = g.recv().await;
    assert_eq!(ok[0], 0x0A, "expected LOGIN_SUCCESS");
    let sel = g.recv().await;
    assert_eq!(sel[0], 0x09, "expected CharSelectionInfo");
    assert_eq!(
        i32::from_le_bytes(sel[1..5].try_into().unwrap()),
        0,
        "no characters yet"
    );

    // NewCharacter → NewCharacterSuccess (0x0D).
    g.send(&[0x13]).await;
    let templates = g.recv().await;
    assert_eq!(templates[0], 0x0D, "expected NewCharacterSuccess");
    assert!(
        i32::from_le_bytes(templates[1..5].try_into().unwrap()) >= 9,
        "creatable templates offered"
    );

    // The client checks name availability first (extended 0xD0/0xA9 packet).
    let name = format!("Hero{}", std::process::id() % 10000);
    let mut w = PacketWriter::new();
    w.write_u8(0xD0);
    w.write_i16(0xA9); // REQUEST_CHARACTER_NAME_CREATABLE
    w.write_bytes(&u16str(&name));
    g.send(&w.into_bytes()).await;
    let creatable = g.recv().await;
    assert_eq!(creatable[0], 0xFE, "ExIsCharNameCreatable (EX opcode)");
    assert_eq!(
        i16::from_le_bytes([creatable[1], creatable[2]]),
        0x10B,
        "EX_IS_CHAR_NAME_CREATABLE"
    );
    assert_eq!(
        i32::from_le_bytes(creatable[3..7].try_into().unwrap()),
        -1,
        "name should be creatable"
    );

    // CharacterCreate: Human Mystic (class 10) — has 7 level-1 skills (the 5
    // from the class tree plus Lucky + Common Craft from the common tree).
    let mut w = PacketWriter::new();
    w.write_u8(0x0C);
    w.write_bytes(&u16str(&name));
    w.write_i32(1); // race (Human)
    w.write_i32(0); // isFemale
    w.write_i32(10); // classId Human Mystic
    for _ in 0..6 {
        w.write_i32(0);
    }
    w.write_i32(0); // hairStyle
    w.write_i32(0); // hairColor
    w.write_i32(0); // face
    g.send(&w.into_bytes()).await;

    // CharCreateOk (0x0F, int 1). Java does NOT re-send CharSelectionInfo after
    // creation, so this is the only reply.
    let created = g.recv().await;
    assert_eq!(
        created[0], 0x0F,
        "expected CharCreateOk, got opcode 0x{:02x}",
        created[0]
    );
    assert_eq!(i32::from_le_bytes(created[1..5].try_into().unwrap()), 1);

    // Reconnecting (fresh session) shows the persisted character in the list.
    drop(g);
    tokio::time::sleep(Duration::from_millis(300)).await; // let the logout propagate
    let (lo1, lo2, po1, po2) = do_login(login_addr, &account, "pw").await;
    let mut g2 = GameClient::connect(game_addr).await;
    let mut w = PacketWriter::new();
    w.write_u8(0x2B);
    w.write_bytes(&u16str(&account));
    w.write_i32(po2);
    w.write_i32(po1);
    w.write_i32(lo1);
    w.write_i32(lo2);
    g2.send(&w.into_bytes()).await;
    assert_eq!(g2.recv().await[0], 0x0A, "LOGIN_SUCCESS on relogin");
    let sel2 = g2.recv().await;
    assert_eq!(sel2[0], 0x09, "CharSelectionInfo on relogin");
    assert_eq!(
        i32::from_le_bytes(sel2[1..5].try_into().unwrap()),
        1,
        "the created character persisted"
    );

    // Select the character (slot 0) → CharSelected → enter the world → UserInfo.
    let mut w = PacketWriter::new();
    w.write_u8(0x12); // CharacterSelect
    w.write_i32(0); // slot
    w.write_i16(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    g2.send(&w.into_bytes()).await;
    let selected = g2.recv().await;
    assert_eq!(selected[0], 0x0B, "CharSelected");

    // The client requests its key mapping while entering (0xD0 / 0x21) → ExUISetting.
    g2.send(&[0xD0, 0x21, 0x00]).await;
    let ui_setting = g2.recv().await;
    assert_eq!(ui_setting[0], 0xFE, "ExUISetting EX opcode");
    assert_eq!(
        i16::from_le_bytes([ui_setting[1], ui_setting[2]]),
        0x71,
        "EX_UI_SETTING"
    );

    g2.send(&[0x11]).await; // EnterWorld
    let ui = g2.recv().await;
    assert_eq!(ui[0], 0x32, "UserInfo");
    // The real enter-world UserInfo STATS block must match the Java client:
    // p_atk 2, p_def 52, and casting speed 499 (333 × Spellcraft 1.5 in a robe).
    let (p_atk, p_def, cast_speed) = parse_userinfo_stats(&ui);
    assert_eq!(
        (p_atk, p_def, cast_speed),
        (2, 52, 499),
        "enter-world UserInfo STATS block"
    );
    // Walk the masked blocks to MAX_HPCPMP and check the computed HP/MP match the
    // model (base level-table HP/MP × CON/MEN bonus, truncated like Java).
    let (max_hp, max_mp) = parse_userinfo_hpmp(&ui);
    let data = gameserver::data::GameData::load(); // cwd is dist/game
    let mystic = data.player_templates.get(10).unwrap();
    // The Human Mystic enters world wearing its initial gear (Apprentice's
    // Tunic +19 MP, Stockings +10 MP), so the enter-world UserInfo Max MP is
    // the base finalizer value *plus* those equipped bonuses. Sum them the same
    // way the server does rather than hardcoding, so the check tracks the data.
    let gear_bonus = |stat: gameserver::model::stats::Stat| -> f64 {
        data.initial_equipment
            .get(10)
            .iter()
            .filter(|i| i.equipped)
            .filter_map(|i| data.item_data.item_stats(i.item_id))
            .flat_map(|s| s.bonuses.iter())
            .filter(|(st, _)| *st == stat)
            .map(|(_, v)| *v)
            .sum()
    };
    let expected_hp = (gameserver::model::calc_max_hp(
        &data,
        mystic,
        1,
        None,
        &gameserver::model::components::StatModifiers::default(),
    ) + gear_bonus(gameserver::model::stats::Stat::MaxHp)) as i32;
    let expected_mp = (gameserver::model::calc_max_mp(
        &data,
        mystic,
        1,
        None,
        &gameserver::model::components::StatModifiers::default(),
    ) + gear_bonus(gameserver::model::stats::Stat::MaxMp)) as i32;
    assert_eq!(
        max_hp, expected_hp,
        "UserInfo max HP matches the calc ({expected_hp})"
    );
    assert_eq!(
        max_mp, expected_mp,
        "UserInfo max MP matches the gear-inclusive calc ({expected_mp})"
    );
    assert!(
        max_hp > 90 && max_hp < 110,
        "Human Mystic level 1 HP is ~99"
    );
    assert!(
        expected_mp
            > gameserver::model::calc_max_mp(
                &data,
                mystic,
                1,
                None,
                &gameserver::model::components::StatModifiers::default()
            ) as i32,
        "equipped gear raised Max MP above the naked value"
    );

    // Drain the rest of the enter-world burst, collecting opcodes, until the
    // welcome SystemMessage (0x62) — the last packet Java sends on enter.
    let mut opcodes = vec![ui[0]];
    let mut item_list_pkt = None;
    let mut equip_slot_pkt = None;
    let mut shortcut_init_pkt = None;
    loop {
        let pkt = g2.recv().await;
        if pkt[0] == 0x0C {
            continue; // NpcInfo for the starting village's NPCs (G8) — unbounded, uncounted
        }
        opcodes.push(pkt[0]);
        if pkt[0] == 0x11 {
            item_list_pkt = Some(pkt.clone());
        }
        if pkt[0] == 0x45 {
            shortcut_init_pkt = Some(pkt.clone());
        }
        if pkt[0] == 0xFE && pkt.len() >= 3 && i16::from_le_bytes([pkt[1], pkt[2]]) == 0x156 {
            equip_slot_pkt = Some(pkt.clone());
        }
        if pkt[0] == 0x62 {
            break; // welcome SystemMessage
        }
        assert!(opcodes.len() < 60, "enter-world burst did not terminate");
    }
    // Key packets the client needs must be present.
    assert!(opcodes.contains(&0x11), "ItemList");
    assert!(opcodes.contains(&0x45), "ShortCutInit");
    assert!(opcodes.contains(&0xE8), "SendMacroList");
    assert!(opcodes.contains(&0x86), "QuestList");
    assert!(opcodes.contains(&0x5F), "SkillList");
    assert!(opcodes.contains(&0x75), "L2FriendList");
    assert!(
        opcodes.iter().filter(|&&o| o == 0x32).count() >= 2,
        "UserInfo sent twice"
    );

    // G9.6: a fresh Human Mystic gets the initialShortcuts.xml panel — the 4
    // global actions plus the class page's Wind Strike/Self Heal, minus one:
    // Self Heal lands on slot 10, the same slot as the global Sit/Stand, and
    // the class list overwrites the global entry (Java map-put order). The
    // stock MACRO example slot is dropped too (its preset ships
    // enabled="false").
    let shortcut_init = shortcut_init_pkt.expect("ShortCutInit packet");
    let shortcut_count = i32::from_le_bytes([
        shortcut_init[1],
        shortcut_init[2],
        shortcut_init[3],
        shortcut_init[4],
    ]);
    assert_eq!(shortcut_count, 5, "Human Mystic starting panel");

    // The Human Mystic's starting gear (initialEquipment.xml classId=10: wand,
    // tunic, stockings, all equipped) shows up in ItemList and the paperdoll.
    let item_list = item_list_pkt.expect("ItemList packet");
    let item_count = i16::from_le_bytes([item_list[3], item_list[4]]);
    assert!(
        item_count >= 3,
        "Human Mystic should start with at least 3 items, got {item_count}"
    );

    let equip_slot = equip_slot_pkt.expect("ExUserInfoEquipSlot packet");
    let rhand_wire_index = gameserver::enums::InventorySlot::VALUES
        .iter()
        .position(|s| matches!(s, gameserver::enums::InventorySlot::RHand))
        .unwrap();
    let block_start = 14 + rhand_wire_index * 22; // marker+sub+objectId+count+mask(5) header
    let rhand_item_id = i32::from_le_bytes(
        equip_slot[block_start + 6..block_start + 10]
            .try_into()
            .unwrap(),
    );
    assert_eq!(rhand_item_id, 6, "Apprentice's Wand equipped in RHand");

    // In-game, the client requests the manor list (0xD0 / 0x01) → ExSendManorList.
    g2.send(&[0xD0, 0x01, 0x00]).await;
    let manor = g2.recv_skip_status_update().await;
    assert_eq!(manor[0], 0xFE, "ExSendManorList EX opcode");
    assert_eq!(
        i16::from_le_bytes([manor[1], manor[2]]),
        0x22,
        "EX_SEND_MANOR_LIST"
    );

    // RequestUserBanInfo (0xD0 / 0x138) is consumed with no reply; RequestSkillCoolTime
    // (0xA6) → SkillCoolTime. Sending both proves the ban-info was swallowed cleanly
    // (the stream stays aligned) and the cooltime request is answered.
    g2.send(&[0xD0, 0x38, 0x01]).await;
    g2.send(&[0xA6]).await;
    let cool = g2.recv_skip_status_update().await;
    assert_eq!(cool[0], 0xC7, "SkillCoolTime reply to RequestSkillCoolTime");

    // --- RequestRestart (0x57): back to character selection on the SAME
    // connection — RestartResponse.TRUE, then a fresh CharSelectionInfo.
    g2.send(&[0x57]).await;
    let restart = g2.recv_skip_status_update().await;
    assert_eq!(restart[0], 0x71, "RestartResponse");
    assert_eq!(
        i32::from_le_bytes(restart[1..5].try_into().unwrap()),
        1,
        "RestartResponse.TRUE"
    );
    let sel3 = g2.recv_skip_status_update().await;
    assert_eq!(sel3[0], 0x09, "CharSelectionInfo re-sent after restart");
    assert_eq!(
        i32::from_le_bytes(sel3[1..5].try_into().unwrap()),
        1,
        "character still listed"
    );

    // Relogin without reconnecting (the original bug): select → enter again.
    //
    // CharacterSelect is flood-gated (Java `CharacterSelectFloodProtector`,
    // `FloodProtector.ini` interval 30 ticks = 3 s): a re-select inside the
    // window of the previous one is *silently* swallowed — Java's `runImpl`
    // returns before building a reply, and the port mirrors it. A real client
    // spends longer than that on the character screen; this scripted one
    // re-selects in milliseconds, so it would block forever on a CharSelected
    // that never comes (which is exactly what this test did for months).
    // `start_game` turns *this one slot* off rather than sleeping the window
    // out, which used to cost the suite 3.2 s of wall clock on its longest
    // test; the protector's own behaviour is covered by
    // `game_loop::tests::flood_tests`.
    let mut w = PacketWriter::new();
    w.write_u8(0x12); // CharacterSelect
    w.write_i32(0); // slot
    w.write_i16(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    g2.send(&w.into_bytes()).await;
    assert_eq!(
        g2.recv_skip_status_update().await[0],
        0x0B,
        "CharSelected after restart"
    );
    g2.send(&[0x11]).await; // EnterWorld
    assert_eq!(
        g2.recv_skip_status_update().await[0],
        0x32,
        "UserInfo after re-enter"
    );
    let mut n = 0;
    loop {
        // Drain the second enter-world burst up to the welcome SystemMessage.
        let op = g2.recv().await[0];
        if op == 0x62 {
            break;
        }
        if op == 0x0C {
            continue; // NpcInfo burst, uncounted (see above)
        }
        n += 1;
        assert!(n < 60, "re-enter burst did not terminate");
    }

    // --- Logout (0x00): LeaveWorld, then the server closes the connection.
    g2.send(&[0x00]).await;
    assert_eq!(g2.recv_skip_status_update().await[0], 0x84, "LeaveWorld");
    tokio::time::sleep(Duration::from_millis(300)).await; // let the store land

    // The Mystic's 7 initial skills were written to character_skills: the class
    // tree's 5 level-1 autoGet skills plus the common tree's Lucky + Common Craft
    // (Java getCompleteClassSkillTree unions in Commons.xml at creation).
    let check = commons::db::init(&db_url, 1).await.unwrap();
    let skill_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_skills WHERE charId = (SELECT charId FROM characters WHERE char_name = ?)",
    )
    .bind(&name)
    .fetch_one(&check)
    .await
    .unwrap();
    assert_eq!(skill_count, 7, "Human Mystic should start with 7 skills");

    // The logout stored the character (storeCharBase + updateOnlineStatus):
    // marked offline with a fresh lastAccess.
    let (online, last_access): (i64, i64) =
        sqlx::query_as("SELECT online, lastAccess FROM characters WHERE char_name = ?")
            .bind(&name)
            .fetch_one(&check)
            .await
            .unwrap();
    assert_eq!(online, 0, "character marked offline after logout");
    assert!(last_access > 0, "lastAccess written on logout");
    check.close().await;

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// In-game check of the drop algorithm.
//
// **A tool, not a regression test** — it prints, it does not assert, and it is
// `#[ignore]`d so nothing runs it by accident. It exists because the drop
// algorithm is the one piece of reward maths whose output a player sees
// directly, and a unit test can only show that the port agrees with a
// transcription of Java, not that the loot actually reaches the chat window.
//
// It boots the real login and game servers on the real datapack **and the dist
// config**, drives a scripted GM client through login → character create →
// enter world, then repeatedly spawns a monster, nukes it down and prints what
// the client is told it picked up. Run it with:
//
//   cargo test -p gameserver --test e2e_create drop_check -- --ignored --nocapture
//
// (`cargo test` rather than nextest: nextest's 120 s per-test timeout kills it
// mid-run, and there is nothing to parallelise.)
// ---------------------------------------------------------------------------

/// Decode a `SystemMessage` (0x62) into `(id, item ids, longs)`.
fn parse_sm(pkt: &[u8]) -> (i16, Vec<i32>, Vec<i64>, Vec<String>) {
    let id = i16::from_le_bytes([pkt[1], pkt[2]]);
    let count = pkt[3];
    let (mut items, mut longs, mut texts) = (Vec::new(), Vec::new(), Vec::new());
    let mut p = 4;
    for _ in 0..count {
        let ty = pkt[p];
        p += 1;
        match ty {
            0 | 12 => {
                let start = p;
                while p + 1 < pkt.len() && !(pkt[p] == 0 && pkt[p + 1] == 0) {
                    p += 2;
                }
                let units: Vec<u16> = pkt[start..p]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                texts.push(String::from_utf16_lossy(&units));
                p += 2;
            }
            1 | 2 | 5 => p += 4,
            3 => {
                items.push(i32::from_le_bytes(pkt[p..p + 4].try_into().unwrap()));
                p += 4;
            }
            4 => p += 8,
            6 => {
                longs.push(i64::from_le_bytes(pkt[p..p + 8].try_into().unwrap()));
                p += 8;
            }
            7 => p += 12,
            _ => break,
        }
    }
    (id, items, longs, texts)
}

#[tokio::test]
#[ignore = "scratch in-game harness, run explicitly"]
async fn drop_check() {
    let Some(src) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|d| d.join("interlude_classic.db"))
        .find(|p| p.exists())
    else {
        eprintln!("skipping drop_check: no interlude_classic.db found");
        return;
    };
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")).unwrap();

    let dir = std::env::temp_dir().join(format!("l2r_drop_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("c.db");
    std::fs::copy(&src, &db_path).unwrap();
    let db_url = format!("jdbc:sqlite:{}", db_path.display());

    let (login_addr, gs_addr) = start_login().await;
    // The **dist** config: DropMaxOccurrences = 1, adena's per-id ×50/×30, and
    // AutoLoot = True, which is what puts the drops in the chat window.
    let mut cfg = gameserver::config::Config::load_from("").combat();
    cfg.flood_protector
        .disable(gameserver::config::flood_protector::FloodAction::CharacterSelect);
    println!(
        "config: occurrences={} adena chance x{:?} amount x{:?} autoloot={}",
        cfg.rates.drop_max_occurrences_normal,
        cfg.rates.drop_chance_by_id.get(&57),
        cfg.rates.drop_amount_by_id.get(&57),
        cfg.character.auto_loot,
    );
    let game_addr = start_game(gs_addr, db_url.clone(), cfg).await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    let account = format!("drop{}", std::process::id() % 100000);
    let (lo1, lo2, po1, po2) = do_login(login_addr, &account, "pw").await;
    let mut g = GameClient::connect(game_addr).await;
    let mut w = PacketWriter::new();
    w.write_u8(0x2B);
    w.write_bytes(&u16str(&account));
    w.write_i32(po2);
    w.write_i32(po1);
    w.write_i32(lo1);
    w.write_i32(lo2);
    g.send(&w.into_bytes()).await;
    assert_eq!(g.recv().await[0], 0x0A, "LOGIN_SUCCESS");
    assert_eq!(g.recv().await[0], 0x09, "CharSelectionInfo");

    g.send(&[0x13]).await;
    assert_eq!(g.recv().await[0], 0x0D, "NewCharacterSuccess");
    let name = format!("Drop{}", std::process::id() % 10000);
    let mut w = PacketWriter::new();
    w.write_u8(0x0C);
    w.write_bytes(&u16str(&name));
    w.write_i32(1);
    w.write_i32(0);
    w.write_i32(10); // Human Mystic
    for _ in 0..6 {
        w.write_i32(0);
    }
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    g.send(&w.into_bytes()).await;
    assert_eq!(g.recv().await[0], 0x0F, "CharCreateOk");
    drop(g);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // GM rights, so the scripted client can spawn and kill on command.
    {
        let pool = commons::db::init(&db_url, 1).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(
            "UPDATE characters SET accesslevel = 100".to_string(),
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    let (lo1, lo2, po1, po2) = do_login(login_addr, &account, "pw").await;
    let mut g = GameClient::connect(game_addr).await;
    let mut w = PacketWriter::new();
    w.write_u8(0x2B);
    w.write_bytes(&u16str(&account));
    w.write_i32(po2);
    w.write_i32(po1);
    w.write_i32(lo1);
    w.write_i32(lo2);
    g.send(&w.into_bytes()).await;
    assert_eq!(g.recv().await[0], 0x0A, "LOGIN_SUCCESS on relogin");
    assert_eq!(g.recv().await[0], 0x09, "CharSelectionInfo");
    let mut w = PacketWriter::new();
    w.write_u8(0x12); // CharacterSelect
    w.write_i32(0);
    w.write_i16(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    g.send(&w.into_bytes()).await;
    assert_eq!(g.recv().await[0], 0x0B, "CharSelected");
    g.send(&[0x11]).await; // EnterWorld
    assert_eq!(g.recv().await[0], 0x32, "UserInfo");
    tokio::time::sleep(Duration::from_millis(400)).await;

    // A GM bypass, the way the admin panel's buttons send commands.
    async fn admin(g: &mut GameClient, command: &str) {
        let mut w = PacketWriter::new();
        w.write_u8(0x23); // RequestBypassToServer
        w.write_bytes(&u16str(command));
        g.send(&w.into_bytes()).await;
    }

    // Goblin (20003), level 5, 84 HP: the dist's usual drop shape — three rare
    // items, adena mid-list at 70 %, then the commons — and low enough level
    // that a fresh character is inside the drop level-gap window.
    const MOB: i32 = 20003;
    const KILLS: usize = 15;
    let mut adena_kills = 0usize;
    let mut item_kills = 0usize;
    let mut both = 0usize;
    let mut nothing = 0usize;
    let mut items_seen: std::collections::BTreeMap<i32, usize> = Default::default();

    // Invulnerable, so the goblin's own swings cannot interrupt the casts
    // (`calcAtkBreak`) and turn this into a test of cast interruption.
    admin(&mut g, "admin_invul").await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    for round in 0..KILLS {
        // Full MP, so the nuke below is always affordable.
        admin(&mut g, "admin_heal").await;
        admin(&mut g, &format!("admin_spawn {MOB}")).await;

        // The spawn announces itself to the client as `NpcInfo` (0x0C); its
        // object id is the first int, which is how a real client learns what to
        // click on.
        let mut mob_oid = None;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(800);
        while mob_oid.is_none() {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                break;
            }
            let Ok(pkt) = tokio::time::timeout(left, g.recv()).await else {
                break;
            };
            if pkt[0] == 0x0C {
                mob_oid = Some(i32::from_le_bytes(pkt[1..5].try_into().unwrap()));
            }
        }
        let Some(mob_oid) = mob_oid else {
            println!("kill {round:2}: no NpcInfo for the spawn");
            continue;
        };

        // Kill it the way a GM checking a drop table would: `//kill_monster`,
        // which deals `maxHp + 1` **as damage** and so registers the GM as the
        // damage dealer the reward split reads. (Before that fix this printed
        // nothing at all, and the harness had to nuke each mob down by hand.)
        admin(&mut g, "admin_kill_monster 600").await;

        let (mut adena, mut items) = (0i64, Vec::new());
        let mut seen_ids: Vec<i16> = Vec::new();
        let mut messages: Vec<String> = Vec::new();
        let mut died = false;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(1200);
        while tokio::time::Instant::now() < deadline {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let Ok(pkt) = tokio::time::timeout(left, g.recv()).await else {
                break;
            };
            // `Die` (0x00) for the mob we just killed.
            if pkt[0] == 0x00 && i32::from_le_bytes(pkt[1..5].try_into().unwrap()) == mob_oid {
                died = true;
                continue;
            }
            if pkt[0] != 0x62 {
                continue;
            }
            let (id, item_ids, longs, texts) = parse_sm(&pkt);
            seen_ids.push(id);
            messages.extend(texts);
            match id {
                28 => adena += longs.first().copied().unwrap_or(0),
                29 | 30 => {
                    if let Some(&item) = item_ids.first() {
                        items.push(item);
                    }
                }
                _ => {}
            }
        }

        for item in &items {
            *items_seen.entry(*item).or_default() += 1;
        }
        match (adena > 0, !items.is_empty()) {
            (true, true) => {
                both += 1;
                adena_kills += 1;
                item_kills += 1;
            }
            (true, false) => adena_kills += 1,
            (false, true) => item_kills += 1,
            (false, false) => nothing += 1,
        }
        println!("kill {round:2}: died={died} adena {adena:6}  items {items:?}  sm {seen_ids:?}");
    }

    println!("\n--- {KILLS} kills of npc {MOB} ---");
    println!("adena dropped in {adena_kills}");
    println!("an item dropped in {item_kills}");
    println!("both together in  {both}");
    println!("nothing at all in {nothing}");
    println!("items: {items_seen:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
