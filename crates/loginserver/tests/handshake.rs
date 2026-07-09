//! M2 acceptance (protocol side): a simulated Interlude client performs the
//! full handshake — decrypts `Init` exactly as the real client does (static
//! Blowfish, then XOR-pass unwrap), replies with `AuthGameGuard` under the
//! session Blowfish key, and expects `GGAuth`.

use std::sync::Arc;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame};
use loginserver::config::LoginConfig;
use loginserver::context::LoginContext;
use loginserver::network::client_connection;
use tokio::net::TcpStream;

const STATIC_BLOWFISH_KEY: [u8; 16] = [
    0x6b, 0x60, 0xcb, 0x5b, 0x82, 0xce, 0x90, 0xb1, 0xcc, 0x2b, 0x6c, 0x55, 0x6c, 0x6c, 0x6c, 0x6c,
];

fn test_config() -> LoginConfig {
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
    }
}

/// Client-side inverse of `NewCrypt.encXORPass` (as the real client decodes `Init`).
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

/// Client-side packet encryption: checksum + pad + Blowfish (what the real
/// client does after receiving the session key).
fn client_encrypt(crypt: &NewCrypt, body: &[u8]) -> Vec<u8> {
    let mut size = body.len() + 4;
    size += 8 - (size % 8);
    let mut data = body.to_vec();
    data.resize(size, 0);
    NewCrypt::append_checksum(&mut data);
    crypt.crypt(&mut data);
    data
}

#[tokio::test]
async fn full_handshake() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let ctx = Arc::new(LoginContext::new(test_config(), pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(client_connection::accept_loop(ctx, listener));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut read, mut write) = stream.into_split();

    // --- Receive and decode Init.
    let mut init = read_frame(&mut read, 8192).await.unwrap().expect("no Init frame");
    assert_eq!(init.len() % 8, 0);
    NewCrypt::new(&STATIC_BLOWFISH_KEY).decrypt(&mut init);
    dec_xor_pass(&mut init);

    assert_eq!(init[0], 0x00, "Init opcode");
    let session_id = i32::from_le_bytes(init[1..5].try_into().unwrap());
    let protocol = i32::from_le_bytes(init[5..9].try_into().unwrap());
    assert_eq!(protocol, 0x0000c621, "protocol revision");
    let _scrambled_modulus = &init[9..9 + 128];
    let blowfish_key: [u8; 16] = init[9 + 128 + 16..9 + 128 + 16 + 16].try_into().unwrap();

    let session_crypt = NewCrypt::new(&blowfish_key);

    // --- Send AuthGameGuard (opcode 0x07, session id + 4 data ints).
    let mut body = vec![0x07u8];
    body.extend_from_slice(&session_id.to_le_bytes());
    body.extend_from_slice(&[0u8; 16]);
    write_frame(&mut write, &client_encrypt(&session_crypt, &body)).await.unwrap();

    // --- Expect GGAuth echoing the session id.
    let mut gg = read_frame(&mut read, 8192).await.unwrap().expect("no GGAuth frame");
    session_crypt.decrypt(&mut gg);
    assert!(NewCrypt::verify_checksum(&gg), "GGAuth checksum");
    assert_eq!(gg[0], 0x0b, "GGAuth opcode");
    assert_eq!(i32::from_le_bytes(gg[1..5].try_into().unwrap()), session_id);
}

#[tokio::test]
async fn wrong_session_id_gets_login_fail() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let ctx = Arc::new(LoginContext::new(test_config(), pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(client_connection::accept_loop(ctx, listener));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut read, mut write) = stream.into_split();

    let mut init = read_frame(&mut read, 8192).await.unwrap().expect("no Init frame");
    NewCrypt::new(&STATIC_BLOWFISH_KEY).decrypt(&mut init);
    dec_xor_pass(&mut init);
    let session_id = i32::from_le_bytes(init[1..5].try_into().unwrap());
    let blowfish_key: [u8; 16] = init[9 + 128 + 16..9 + 128 + 16 + 16].try_into().unwrap();
    let session_crypt = NewCrypt::new(&blowfish_key);

    let mut body = vec![0x07u8];
    body.extend_from_slice(&session_id.wrapping_add(1).to_le_bytes()); // wrong id
    body.extend_from_slice(&[0u8; 16]);
    write_frame(&mut write, &client_encrypt(&session_crypt, &body)).await.unwrap();

    let mut fail = read_frame(&mut read, 8192).await.unwrap().expect("no LoginFail frame");
    session_crypt.decrypt(&mut fail);
    assert!(NewCrypt::verify_checksum(&fail));
    assert_eq!(fail[0], 0x01, "LoginFail opcode");
    assert_eq!(fail[1], 0x15, "REASON_ACCESS_FAILED");

    // Server closes after LoginFail (Java close(reason) semantics).
    assert!(read_frame(&mut read, 8192).await.unwrap().is_none());
}
