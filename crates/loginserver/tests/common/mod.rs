//! Shared test client: performs the protocol exactly as a real Interlude
//! client — Init decode (static Blowfish + XOR unwrap), session-key packet
//! encryption, RSA modulus unscramble and credential-block encryption.

// Each test binary compiles this module separately and uses a different
// subset of it, so per-binary dead_code warnings are false positives.
#![allow(dead_code)]

use std::sync::Arc;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame};
use loginserver::config::LoginConfig;
use loginserver::context::LoginContext;
use loginserver::controller::{ControllerSettings, spawn};
use loginserver::network::client_connection;
use migration::MigratorTrait;
use models::entity::{account_data, accounts, ip_bans};
use models::sea_orm::ActiveValue::Set;
use models::sea_orm::sea_query::Expr;
use models::sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use num_bigint_dig::BigUint;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

pub fn test_config() -> LoginConfig {
    LoginConfig {
        // Status channel off in tests: nothing binds a monitoring port.
        internal_status_bind_address: "127.0.0.1".into(),
        internal_status_port: 0,
        login_bind_address: "127.0.0.1".into(),
        port_login: 0,
        game_server_login_host: "127.0.0.1".into(),
        game_server_login_port: 0,
        database_url: "sqlite::memory:".into(),
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

/// Builds the real schema, the same way a deployment does: `Migrator::up`.
///
/// Hand-written `CREATE TABLE`s used to live here and drifted from dist —
/// which is the failure mode the migrations exist to end.
pub async fn setup_schema(db: &DatabaseConnection) {
    migration::Migrator::up(db, None).await.unwrap();
}

pub struct TestServer {
    pub addr: std::net::SocketAddr,
    pub gs_addr: std::net::SocketAddr,
    /// The server's database, for tests that set up fixtures or assert on rows.
    pub db: DatabaseConnection,
    /// SQLite, or PostgreSQL under `L2R_TEST_DATABASE_URL`; removed on drop.
    _test_db: commons::db::testing::TestDb,
}

pub async fn start_server(config: LoginConfig) -> TestServer {
    let test_db = commons::db::testing::TestDb::new("login").await;
    let db = commons::db::connect(&test_db.url, 2).await.unwrap();
    setup_schema(&db).await;
    // Seed the stock gameservers row + server names like dist data.
    models::repo::gameservers::register(&db, 1, "-2ad66b3f483c22be097019f55c8abdf0", "")
        .await
        .unwrap();
    let mut gs_table = loginserver::gs_table::GameServerTable::load(&db).await;
    gs_table.server_names.insert(1, "Bartz".to_string());
    gs_table.server_names.insert(2, "Sieghardt".to_string());

    let controller = spawn(
        ControllerSettings {
            auto_create_accounts: config.auto_create_accounts,
            login_try_before_ban: config.login_try_before_ban,
            login_block_after_ban_ms: config.login_block_after_ban as i64 * 1000,
            show_licence: config.show_licence,
            accept_new_gameserver: config.accept_new_gameserver,
        },
        db.clone(),
        gs_table,
    );
    let ctx = Arc::new(LoginContext::new(config, db.clone(), controller));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(client_connection::accept_loop(ctx.clone(), listener));

    let gs_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gs_addr = gs_listener.local_addr().unwrap();
    tokio::spawn(loginserver::gs_link::listener::accept_loop(
        ctx,
        gs_listener,
    ));

    TestServer {
        addr,
        gs_addr,
        db,
        _test_db: test_db,
    }
}

/// The game account `login`.
pub async fn account(db: &DatabaseConnection, login: &str) -> accounts::Model {
    accounts::Entity::find()
        .filter(accounts::Column::Login.eq(login))
        .one(db)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("no account {login}"))
}

/// A game account with `password_hash` at `access_level`; every other column
/// takes its schema default, as an auto-created one does.
pub async fn insert_account(
    db: &DatabaseConnection,
    login: &str,
    password_hash: &str,
    access_level: i32,
) {
    accounts::Entity::insert(accounts::ActiveModel {
        login: Set(Some(login.to_string())),
        password: Set(Some(password_hash.to_string())),
        access_level: Set(access_level),
        ..Default::default()
    })
    .exec_without_returning(db)
    .await
    .unwrap();
}

pub async fn insert_account_data(db: &DatabaseConnection, account: &str, var: &str, value: &str) {
    account_data::Entity::insert(account_data::ActiveModel {
        account_name: Set(account.to_string()),
        var: Set(var.to_string()),
        value: Set(Some(value.to_string())),
    })
    .exec_without_returning(db)
    .await
    .unwrap();
}

pub async fn account_data_value(
    db: &DatabaseConnection,
    account: &str,
    var: &str,
) -> Option<String> {
    account_data::Entity::find_by_id((account.to_string(), var.to_string()))
        .one(db)
        .await
        .unwrap()
        .and_then(|row| row.value)
}

/// The one row on the ban list.
pub async fn only_ip_ban(db: &DatabaseConnection) -> ip_bans::Model {
    ip_bans::Entity::find()
        .one(db)
        .await
        .unwrap()
        .expect("an IP ban is stored")
}

pub async fn insert_ip_ban(db: &DatabaseConnection, ip: &str, expires_at: i64) {
    ip_bans::Entity::insert(ip_bans::ActiveModel {
        ip: Set(ip.to_string()),
        expires_at: Set(expires_at),
        ..Default::default()
    })
    .exec_without_returning(db)
    .await
    .unwrap();
}

/// Moves every ban's expiry to `expires_at` (`0` = permanent).
pub async fn set_ip_ban_expiry(db: &DatabaseConnection, expires_at: i64) {
    ip_bans::Entity::update_many()
        .col_expr(ip_bans::Column::ExpiresAt, Expr::value(expires_at))
        .exec(db)
        .await
        .unwrap();
}

/// Client-side inverse of `NewCrypt.encXORPass` (as the real client decodes `Init`).
pub fn dec_xor_pass(data: &mut [u8]) {
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

/// Client-side packet encryption: checksum + pad + Blowfish.
pub fn client_encrypt(crypt: &NewCrypt, body: &[u8]) -> Vec<u8> {
    let mut size = body.len() + 4;
    size += 8 - (size % 8);
    let mut data = body.to_vec();
    data.resize(size, 0);
    NewCrypt::append_checksum(&mut data);
    crypt.crypt(&mut data);
    data
}

/// Client-side inverse of the server's modulus scramble.
pub fn unscramble_modulus(scrambled: &[u8]) -> BigUint {
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

/// Old-method (128-byte) credential block: user at 0x5E, password at 0x6C,
/// raw-RSA encrypted with the server's public key.
pub fn encrypt_credentials(modulus: &BigUint, user: &str, password: &str) -> [u8; 0x80] {
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

pub struct HandshakedClient {
    pub read: OwnedReadHalf,
    pub write: OwnedWriteHalf,
    pub session_id: i32,
    pub crypt: NewCrypt,
    pub modulus: BigUint,
}

impl HandshakedClient {
    pub async fn send(&mut self, body: &[u8]) {
        let data = client_encrypt(&self.crypt, body);
        write_frame(&mut self.write, &data).await.unwrap();
    }

    /// Reads and decrypts the next server packet; None on connection close.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let mut data = read_frame(&mut self.read, 8192).await.unwrap()?;
        self.crypt.decrypt(&mut data);
        assert!(NewCrypt::verify_checksum(&data), "server packet checksum");
        Some(data)
    }
}

/// Connect + decode Init + complete the GameGuard exchange.
pub async fn handshake(addr: std::net::SocketAddr) -> HandshakedClient {
    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut read, write) = stream.into_split();

    let mut init = read_frame(&mut read, 8192)
        .await
        .unwrap()
        .expect("no Init frame");
    NewCrypt::new(&STATIC_BLOWFISH_KEY).decrypt(&mut init);
    dec_xor_pass(&mut init);
    assert_eq!(init[0], 0x00, "Init opcode");
    let session_id = i32::from_le_bytes(init[1..5].try_into().unwrap());
    assert_eq!(
        i32::from_le_bytes(init[5..9].try_into().unwrap()),
        0x0000c621
    );
    let modulus = unscramble_modulus(&init[9..9 + 128]);
    let blowfish_key: [u8; 16] = init[9 + 128 + 16..9 + 128 + 16 + 16].try_into().unwrap();
    let crypt = NewCrypt::new(&blowfish_key);

    let mut client = HandshakedClient {
        read,
        write,
        session_id,
        crypt,
        modulus,
    };

    let mut gg = vec![0x07u8];
    gg.extend_from_slice(&session_id.to_le_bytes());
    gg.extend_from_slice(&[0u8; 16]);
    client.send(&gg).await;

    let reply = client.recv().await.expect("no GGAuth");
    assert_eq!(reply[0], 0x0b, "GGAuth opcode");
    client
}

/// Handshake + RequestAuthLogin; returns the first auth reply packet.
pub async fn login(
    addr: std::net::SocketAddr,
    user: &str,
    password: &str,
) -> (HandshakedClient, Vec<u8>) {
    let mut client = handshake(addr).await;
    let block = encrypt_credentials(&client.modulus, user, password);
    let mut body = vec![0x00u8];
    body.extend_from_slice(&block);
    client.send(&body).await;
    let reply = client.recv().await.expect("no auth reply");
    (client, reply)
}
