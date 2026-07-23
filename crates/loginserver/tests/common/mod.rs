//! Shared test client: performs the protocol exactly as a real Interlude
//! client — Init decode (static Blowfish + XOR unwrap), session-key packet
//! encryption, RSA modulus unscramble and credential-block encryption.

use std::sync::Arc;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame};
use loginserver::config::LoginConfig;
use loginserver::context::LoginContext;
use loginserver::controller::{spawn, ControllerSettings};
use loginserver::network::client_connection;
use num_bigint_dig::BigUint;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

pub fn test_config() -> LoginConfig {
    LoginConfig {
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

pub async fn setup_schema(pool: &sqlx::SqlitePool) {
    sqlx::query(
        "CREATE TABLE accounts (login VARCHAR(45) NOT NULL DEFAULT '', password VARCHAR(45), email VARCHAR(255) DEFAULT NULL, \
         created_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, lastactive BIGINT NOT NULL DEFAULT 0, \
         accessLevel TINYINT NOT NULL DEFAULT 0, lastIP CHAR(15) NULL DEFAULT NULL, lastServer TINYINT DEFAULT 1, \
         pcIp CHAR(15) DEFAULT NULL, hop1 CHAR(15) DEFAULT NULL, hop2 CHAR(15) DEFAULT NULL, hop3 CHAR(15) DEFAULT NULL, \
         hop4 CHAR(15) DEFAULT NULL, PRIMARY KEY (login))",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE account_data (account_name VARCHAR(45) NOT NULL DEFAULT '', var VARCHAR(20) NOT NULL DEFAULT '', \
         value VARCHAR(255), PRIMARY KEY (account_name, var))",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE accounts_ipauth (login VARCHAR(45) NOT NULL, ip CHAR(15) NOT NULL, type VARCHAR(10) NULL DEFAULT 'allow')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE gameservers (server_id INT NOT NULL DEFAULT 0, hexid VARCHAR(50) NOT NULL DEFAULT '', \
         host VARCHAR(50) NOT NULL DEFAULT '', PRIMARY KEY (server_id))",
    )
    .execute(pool)
    .await
    .unwrap();
}

pub struct TestServer {
    pub addr: std::net::SocketAddr,
    pub gs_addr: std::net::SocketAddr,
    pub pool: sqlx::SqlitePool,
}

pub async fn start_server(config: LoginConfig) -> TestServer {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    setup_schema(&pool).await;
    // Seed the stock gameservers row + server names like dist data.
    sqlx::query("INSERT INTO gameservers VALUES (1, '-2ad66b3f483c22be097019f55c8abdf0', '')")
        .execute(&pool)
        .await
        .unwrap();
    let mut gs_table = loginserver::gs_table::GameServerTable::load(&pool).await;
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
        pool.clone(),
        gs_table,
    );
    let ctx = Arc::new(LoginContext::new(config, pool.clone(), controller));

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
        pool,
    }
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
    let (mut read, mut write) = stream.into_split();

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
