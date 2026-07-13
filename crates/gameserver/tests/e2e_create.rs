//! Full end-to-end test: a scripted client logs into the real login server,
//! selects the server, connects to the real game server, authenticates, and
//! creates a character — exactly the path the GUI client takes. This reproduces
//! and regression-tests the G3 character-creation flow across both servers.

use std::sync::Arc;
use std::time::Duration;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame, PacketWriter};
use gameserver::network::cipher::Encryption;
use num_bigint_dig::BigUint;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

// ---------- login server + client crypto (mirrors loginserver test harness) ----------

const STATIC_BLOWFISH_KEY: [u8; 16] =
    [0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c];

async fn setup_login_schema(pool: &sqlx::SqlitePool) {
    for stmt in [
        "CREATE TABLE accounts (login VARCHAR(45) NOT NULL DEFAULT '', password VARCHAR(45), email VARCHAR(255) DEFAULT NULL, \
         created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, lastactive BIGINT NOT NULL DEFAULT 0, \
         accessLevel TINYINT NOT NULL DEFAULT 0, lastIP CHAR(15) NULL DEFAULT NULL, lastServer TINYINT DEFAULT 1, \
         pcIp CHAR(15) DEFAULT NULL, hop1 CHAR(15) DEFAULT NULL, hop2 CHAR(15) DEFAULT NULL, hop3 CHAR(15) DEFAULT NULL, \
         hop4 CHAR(15) DEFAULT NULL, PRIMARY KEY (login))",
        "CREATE TABLE account_data (account_name VARCHAR(45) NOT NULL DEFAULT '', var VARCHAR(20) NOT NULL DEFAULT '', \
         value VARCHAR(255), PRIMARY KEY (account_name, var))",
        "CREATE TABLE accounts_ipauth (login VARCHAR(45) NOT NULL, ip CHAR(15) NOT NULL, type VARCHAR(10) NULL DEFAULT 'allow')",
        "CREATE TABLE gameservers (server_id INT NOT NULL DEFAULT 0, hexid VARCHAR(50) NOT NULL DEFAULT '', host VARCHAR(50) NOT NULL DEFAULT '', PRIMARY KEY (server_id))",
    ] {
        sqlx::query(stmt).execute(pool).await.unwrap();
    }
    sqlx::query("INSERT INTO gameservers VALUES (1, '-2ad66b3f483c22be097019f55c8abdf0', '')")
        .execute(pool)
        .await
        .unwrap();
}

fn login_config() -> loginserver::config::LoginConfig {
    use loginserver::config::LoginConfig;
    LoginConfig {
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
    use loginserver::controller::{spawn, ControllerSettings};
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    setup_login_schema(&pool).await;
    let mut gs_table = loginserver::gs_table::GameServerTable::load(&pool).await;
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
        pool.clone(),
        gs_table,
    );
    let ctx = Arc::new(loginserver::context::LoginContext::new(config, pool, controller));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(loginserver::network::client_connection::accept_loop(ctx.clone(), listener));
    let gs_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gs_addr = gs_listener.local_addr().unwrap();
    tokio::spawn(loginserver::gs_link::listener::accept_loop(ctx, gs_listener));
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
        write_frame(&mut self.write, &login_encrypt(&self.crypt, body)).await.unwrap();
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
    let mut c = LoginClient { read, write, crypt: NewCrypt::new(&blowfish_key), modulus };

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

async fn start_game(gs_login_addr: std::net::SocketAddr, db_url: String) -> std::net::SocketAddr {
    use gameserver::db::{self, DbCommand, DbEvent};
    use gameserver::game_loop::{self, GameThreadChannels, Shutdown};
    use gameserver::loginlink::{self, LoginLinkConfig, LoginLinkEvent};
    use gameserver::network::connection::{self, NetworkConfig};
    use gameserver::network::NetEvent;

    let (net_tx, net_rx) = std::sync::mpsc::channel::<NetEvent>();
    let (login_tx, login_rx) = std::sync::mpsc::channel::<LoginLinkEvent>();
    let (link_tx, link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, db_cmd_rx) = tokio::sync::mpsc::unbounded_channel::<DbCommand>();
    let (db_event_tx, db_rx) = std::sync::mpsc::channel::<DbEvent>();

    db::spawn(db_url, 1, 7, db_cmd_rx, db_event_tx);

    let geo = std::sync::Arc::new(gameserver::geo::GeoEngine::empty());
    let (path_tx, path_req_rx) = std::sync::mpsc::channel();
    let (path_event_tx, path_rx) = std::sync::mpsc::channel();
    gameserver::geo::worker::spawn(geo.clone(), Default::default(), path_req_rx, path_event_tx);

    let data = gameserver::data::GameData::load();
    let _game = game_loop::spawn(
        Shutdown::new(),
        GameThreadChannels {
            net_rx,
            login_rx,
            link_tx: link_tx.clone(),
            db_rx,
            db_tx,
            data,
            geo,
            path_tx,
            path_rx,
            path_finding: 2,
            max_characters_per_account: 7,
            delete_days: 3,
            starting_adena: 100,
            cfg: gameserver::config::CombatConfig::default(),
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
    tokio::spawn(loginlink::run(link_cfg, link_rx, login_tx));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let net_cfg = Arc::new(NetworkConfig {
        packet_encryption: true,
        protocol_list: vec![110],
        server_id: 1,
        is_classic: true,
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
    /// Like `recv`, but skips unsolicited packets a live world can push at
    /// any time: `StatusUpdate` (0x18, the 3 s passive-regen tick since G6 —
    /// e.g. this character's CP regenerating from its post-creation 0),
    /// `NpcInfo` (0x0C, since G8 the starting village's NPCs are described
    /// on/after enter-world), and `ExSetCompassZoneCode` (FE:0x33, since
    /// G12 zone revalidation reports the peace-zone compass icon — the
    /// mage-start spawn point lies in a peace zone). A reply-then-assert
    /// exchange isn't guaranteed to be the very next frame on the wire.
    async fn recv_skip_status_update(&mut self) -> Vec<u8> {
        loop {
            let pkt = self.recv().await;
            let compass =
                pkt[0] == 0xFE && pkt.len() >= 3 && u16::from_le_bytes([pkt[1], pkt[2]]) == 0x33;
            if pkt[0] != 0x18 && pkt[0] != 0x0C && !compass {
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

fn u16str(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_login_to_character_create() {
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")).unwrap();

    // Fresh characters DB copied from the real one.
    let dir = std::env::temp_dir().join(format!("l2r_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("c.db");
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../interlude_classic.db"), &db_path).unwrap();
    let db_url = format!("jdbc:sqlite:{}", db_path.display());

    let (login_addr, gs_addr) = start_login().await;
    let game_addr = start_game(gs_addr, db_url.clone()).await;
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
    assert_eq!(i32::from_le_bytes(sel[1..5].try_into().unwrap()), 0, "no characters yet");

    // NewCharacter → NewCharacterSuccess (0x0D).
    g.send(&[0x13]).await;
    let templates = g.recv().await;
    assert_eq!(templates[0], 0x0D, "expected NewCharacterSuccess");
    assert!(i32::from_le_bytes(templates[1..5].try_into().unwrap()) >= 9, "creatable templates offered");

    // The client checks name availability first (extended 0xD0/0xA9 packet).
    let name = format!("Hero{}", std::process::id() % 10000);
    let mut w = PacketWriter::new();
    w.write_u8(0xD0);
    w.write_i16(0xA9); // REQUEST_CHARACTER_NAME_CREATABLE
    w.write_bytes(&u16str(&name));
    g.send(&w.into_bytes()).await;
    let creatable = g.recv().await;
    assert_eq!(creatable[0], 0xFE, "ExIsCharNameCreatable (EX opcode)");
    assert_eq!(i16::from_le_bytes([creatable[1], creatable[2]]), 0x10B, "EX_IS_CHAR_NAME_CREATABLE");
    assert_eq!(i32::from_le_bytes(creatable[3..7].try_into().unwrap()), -1, "name should be creatable");

    // CharacterCreate: Human Mystic (class 10) — has 5 level-1 skills.
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
    assert_eq!(created[0], 0x0F, "expected CharCreateOk, got opcode 0x{:02x}", created[0]);
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
    assert_eq!(i32::from_le_bytes(sel2[1..5].try_into().unwrap()), 1, "the created character persisted");

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
    assert_eq!(i16::from_le_bytes([ui_setting[1], ui_setting[2]]), 0x71, "EX_UI_SETTING");

    g2.send(&[0x11]).await; // EnterWorld
    let ui = g2.recv().await;
    assert_eq!(ui[0], 0x32, "UserInfo");
    // Walk the masked blocks to MAX_HPCPMP and check the computed HP/MP match the
    // model (base level-table HP/MP × CON/MEN bonus, truncated like Java).
    let (max_hp, max_mp) = parse_userinfo_hpmp(&ui);
    let data = gameserver::data::GameData::load(); // cwd is dist/game
    let mystic = data.player_templates.get(10).unwrap();
    let expected_hp = gameserver::model::calc_max_hp(&data, mystic, 1) as i32;
    let expected_mp = gameserver::model::calc_max_mp(&data, mystic, 1) as i32;
    assert_eq!(max_hp, expected_hp, "UserInfo max HP matches the calc ({expected_hp})");
    assert_eq!(max_mp, expected_mp, "UserInfo max MP matches the calc ({expected_mp})");
    assert!(max_hp > 90 && max_hp < 110, "Human Mystic level 1 HP is ~99");

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
    assert!(opcodes.iter().filter(|&&o| o == 0x32).count() >= 2, "UserInfo sent twice");

    // G9.6: a fresh Human Mystic gets the initialShortcuts.xml panel — the 4
    // global actions plus the class page's Wind Strike/Self Heal, minus one:
    // Self Heal lands on slot 10, the same slot as the global Sit/Stand, and
    // the class list overwrites the global entry (Java map-put order). The
    // stock MACRO example slot is dropped too (its preset ships
    // enabled="false").
    let shortcut_init = shortcut_init_pkt.expect("ShortCutInit packet");
    let shortcut_count = i32::from_le_bytes([shortcut_init[1], shortcut_init[2], shortcut_init[3], shortcut_init[4]]);
    assert_eq!(shortcut_count, 5, "Human Mystic starting panel");

    // The Human Mystic's starting gear (initialEquipment.xml classId=10: wand,
    // tunic, stockings, all equipped) shows up in ItemList and the paperdoll.
    let item_list = item_list_pkt.expect("ItemList packet");
    let item_count = i16::from_le_bytes([item_list[3], item_list[4]]);
    assert!(item_count >= 3, "Human Mystic should start with at least 3 items, got {item_count}");

    let equip_slot = equip_slot_pkt.expect("ExUserInfoEquipSlot packet");
    let rhand_wire_index = gameserver::enums::InventorySlot::VALUES
        .iter()
        .position(|s| matches!(s, gameserver::enums::InventorySlot::RHand))
        .unwrap();
    let block_start = 14 + rhand_wire_index * 22; // marker+sub+objectId+count+mask(5) header
    let rhand_item_id = i32::from_le_bytes(equip_slot[block_start + 6..block_start + 10].try_into().unwrap());
    assert_eq!(rhand_item_id, 6, "Apprentice's Wand equipped in RHand");

    // In-game, the client requests the manor list (0xD0 / 0x01) → ExSendManorList.
    g2.send(&[0xD0, 0x01, 0x00]).await;
    let manor = g2.recv_skip_status_update().await;
    assert_eq!(manor[0], 0xFE, "ExSendManorList EX opcode");
    assert_eq!(i16::from_le_bytes([manor[1], manor[2]]), 0x22, "EX_SEND_MANOR_LIST");

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
    assert_eq!(i32::from_le_bytes(restart[1..5].try_into().unwrap()), 1, "RestartResponse.TRUE");
    let sel3 = g2.recv_skip_status_update().await;
    assert_eq!(sel3[0], 0x09, "CharSelectionInfo re-sent after restart");
    assert_eq!(i32::from_le_bytes(sel3[1..5].try_into().unwrap()), 1, "character still listed");

    // Relogin without reconnecting (the original bug): select → enter again.
    let mut w = PacketWriter::new();
    w.write_u8(0x12); // CharacterSelect
    w.write_i32(0); // slot
    w.write_i16(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    g2.send(&w.into_bytes()).await;
    assert_eq!(g2.recv_skip_status_update().await[0], 0x0B, "CharSelected after restart");
    g2.send(&[0x11]).await; // EnterWorld
    assert_eq!(g2.recv_skip_status_update().await[0], 0x32, "UserInfo after re-enter");
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

    // The Mystic's 5 initial skills were written to character_skills.
    let check = commons::db::init(&db_url, 1).await.unwrap();
    let skill_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_skills WHERE charId = (SELECT charId FROM characters WHERE char_name = ?)",
    )
    .bind(&name)
    .fetch_one(&check)
    .await
    .unwrap();
    assert_eq!(skill_count, 5, "Human Mystic should start with 5 skills");

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
