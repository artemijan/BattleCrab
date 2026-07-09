//! M5 acceptance: ReplyCharacters → char counts in ServerList,
//! RequestTempBan → account_data + IP ban, ChangePassword roundtrip,
//! PlayerTracert → accounts columns.

mod common;

use commons::crypt::{hash_password, NewCrypt};
use commons::network::{read_frame, write_frame, PacketReader, PacketWriter};
use common::{login, start_server, test_config};
use loginserver::gs_link::packets::{gs_decrypt, gs_encrypt, GS_STATIC_BLOWFISH_KEY};
use loginserver::gs_table::hexid_from_string;
use num_bigint_dig::BigUint;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

const HEXID: &str = "-2ad66b3f483c22be097019f55c8abdf0";

struct SimGameServer {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    crypt: NewCrypt,
}

impl SimGameServer {
    async fn send(&mut self, body: Vec<u8>) {
        let data = gs_encrypt(&self.crypt, body);
        write_frame(&mut self.write, &data).await.unwrap();
    }

    async fn recv(&mut self) -> Option<Vec<u8>> {
        let mut data = read_frame(&mut self.read, 65533).await.unwrap()?;
        assert!(gs_decrypt(&self.crypt, &mut data), "GS-link checksum");
        Some(data)
    }
}

async fn register_game_server(gs_addr: std::net::SocketAddr) -> SimGameServer {
    let stream = TcpStream::connect(gs_addr).await.unwrap();
    let (read, write) = stream.into_split();
    let mut gs = SimGameServer { read, write, crypt: NewCrypt::new(GS_STATIC_BLOWFISH_KEY) };

    let init = gs.recv().await.expect("no InitLS");
    let mut r = PacketReader::new(&init);
    r.read_u8();
    r.read_i32();
    let key_len = r.read_i32().unwrap() as usize;
    let modulus = BigUint::from_bytes_be(r.read_bytes(key_len).unwrap());

    let mut new_key = [0u8; 16];
    new_key[0] = 0x37;
    for (i, b) in new_key.iter_mut().enumerate().skip(1) {
        *b = (i as u8).wrapping_mul(53).wrapping_add(7);
    }
    let mut plain = [0u8; 64];
    plain[48..].copy_from_slice(&new_key);
    let c = BigUint::from_bytes_be(&plain).modpow(&BigUint::from(65537u32), &modulus);
    let mut block = vec![0u8; 64];
    let cb = c.to_bytes_be();
    block[64 - cb.len()..].copy_from_slice(&cb);
    let mut w = PacketWriter::new();
    w.write_u8(0x00);
    w.write_i32(block.len() as i32);
    w.write_bytes(&block);
    gs.send(w.into_bytes()).await;
    gs.crypt = NewCrypt::new(&new_key);

    let hexid = hexid_from_string(HEXID).unwrap();
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(1);
    w.write_u8(0);
    w.write_u8(0);
    w.write_i16(7777);
    w.write_i32(1000);
    w.write_i32(hexid.len() as i32);
    w.write_bytes(&hexid);
    w.write_i32(1);
    w.write_string("0.0.0.0/0");
    w.write_string("127.0.0.1");
    gs.send(w.into_bytes()).await;
    let resp = gs.recv().await.expect("no AuthResponse");
    assert_eq!(resp[0], 0x02);
    gs
}

#[tokio::test]
async fn reply_characters_lands_in_server_list() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr).await;

    let (mut client, reply) = login(server.addr, "charuser", "pw").await;
    assert_eq!(reply[0], 0x03, "LoginOk");
    let ok1 = i32::from_le_bytes(reply[1..5].try_into().unwrap());
    let ok2 = i32::from_le_bytes(reply[5..9].try_into().unwrap());

    // The LS should have asked us for characters on login (RequestCharacters).
    let req = gs.recv().await.expect("no RequestCharacters");
    let mut r = PacketReader::new(&req);
    assert_eq!(r.read_u8().unwrap(), 0x05, "RequestCharacters opcode");
    assert_eq!(r.read_string().unwrap(), "charuser");

    // Reply: 3 characters, none pending deletion.
    let mut w = PacketWriter::new();
    w.write_u8(0x08);
    w.write_string("charuser");
    w.write_u8(3);
    w.write_u8(0);
    gs.send(w.into_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // RequestServerList → char count block present.
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_i32(ok1);
    w.write_i32(ok2);
    client.send(&w.into_bytes()).await;

    let list = client.recv().await.expect("no ServerList");
    assert_eq!(list[0], 0x04);
    let count = list[1] as usize;
    // layout: 1 opcode + 1 count + 1 lastServer + count*21 + 2 (0xA4) + chars pairs
    let chars_block = 3 + count * 21 + 2;
    assert!(list.len() >= chars_block + 2, "char count block should be appended");
    assert_eq!(list[chars_block], 1, "server id 1");
    assert_eq!(list[chars_block + 1], 3, "3 characters on Bartz");
}

#[tokio::test]
async fn temp_ban_writes_account_data_and_bans_ip() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr).await;

    // Create the account first.
    let (_c, reply) = login(server.addr, "hacker", "pw").await;
    assert_eq!(reply[0], 0x03);

    let ban_until = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64)
        + 3_600_000;
    let mut w = PacketWriter::new();
    w.write_u8(0x0A);
    w.write_string("hacker");
    w.write_string("127.0.0.1");
    w.write_i64(ban_until);
    w.write_u8(1);
    w.write_string("cheating");
    gs.send(w.into_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let (value,): (String,) =
        sqlx::query_as("SELECT value FROM account_data WHERE account_name='hacker' AND var='ban_temp'")
            .fetch_one(&server.pool)
            .await
            .expect("ban_temp row written");
    assert_eq!(value, ban_until.to_string());

    // RequestTempBan also IP-bans the reported address (Java behavior), so a
    // new connection is refused before Init: LoginFail(REASON_NOT_AUTHED)
    // under the static first-packet encryption.
    let stream = TcpStream::connect(server.addr).await.unwrap();
    let (mut read, _write) = stream.into_split();
    let mut first = read_frame(&mut read, 8192).await.unwrap().expect("no packet");
    NewCrypt::new(&common::STATIC_BLOWFISH_KEY).decrypt(&mut first);
    common::dec_xor_pass(&mut first);
    assert_eq!(first[0], 0x01, "LoginFail opcode");
    assert_eq!(first[1], 0x06, "REASON_NOT_AUTHED — IP banned by temp ban");
}

#[tokio::test]
async fn change_password_roundtrip() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr).await;

    let (_c, reply) = login(server.addr, "pwuser", "oldpass").await;
    assert_eq!(reply[0], 0x03);
    // Drain the RequestCharacters the LS sent on login.
    let req = gs.recv().await.expect("no RequestCharacters");
    assert_eq!(req[0], 0x05);

    // GS reports the account in-game (ChangePassword requires that).
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i16(1);
    w.write_string("pwuser");
    gs.send(w.into_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Wrong current password → error message.
    let mut w = PacketWriter::new();
    w.write_u8(0x0B);
    w.write_string("pwuser");
    w.write_string("MyChar");
    w.write_string("wrongpass");
    w.write_string("newpass");
    gs.send(w.into_bytes()).await;
    let resp = gs.recv().await.expect("no ChangePasswordResponse");
    let mut r = PacketReader::new(&resp);
    assert_eq!(r.read_u8().unwrap(), 0x06, "ChangePasswordResponse opcode");
    assert_eq!(r.read_string().unwrap(), "MyChar");
    assert!(r.read_string().unwrap().contains("doesn't match"));

    // Correct current password → success + DB updated.
    let mut w = PacketWriter::new();
    w.write_u8(0x0B);
    w.write_string("pwuser");
    w.write_string("MyChar");
    w.write_string("oldpass");
    w.write_string("newpass");
    gs.send(w.into_bytes()).await;
    let resp = gs.recv().await.expect("no ChangePasswordResponse");
    let mut r = PacketReader::new(&resp);
    assert_eq!(r.read_u8().unwrap(), 0x06);
    assert_eq!(r.read_string().unwrap(), "MyChar");
    assert!(r.read_string().unwrap().contains("successfully"));

    let (password,): (String,) = sqlx::query_as("SELECT password FROM accounts WHERE login='pwuser'")
        .fetch_one(&server.pool)
        .await
        .unwrap();
    assert_eq!(password, hash_password("newpass"));
}

#[tokio::test]
async fn player_tracert_updates_accounts() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr).await;

    let (_c, reply) = login(server.addr, "traceuser", "pw").await;
    assert_eq!(reply[0], 0x03);

    let mut w = PacketWriter::new();
    w.write_u8(0x07);
    w.write_string("traceuser");
    w.write_string("10.0.0.5");
    w.write_string("10.0.0.1");
    w.write_string("192.168.0.1");
    w.write_string("0.0.0.0");
    w.write_string("0.0.0.0");
    gs.send(w.into_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let (pc_ip, hop1): (String, String) =
        sqlx::query_as("SELECT pcIp, hop1 FROM accounts WHERE login='traceuser'")
            .fetch_one(&server.pool)
            .await
            .unwrap();
    assert_eq!(pc_ip, "10.0.0.5");
    assert_eq!(hop1, "10.0.0.1");
}
