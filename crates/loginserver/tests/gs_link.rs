//! M4 acceptance (protocol side): a simulated Java game server registers over
//! the GS link (InitLS → BlowFishKey → GameServerAuth → AuthResponse), the
//! client sees it in ServerList, gets PlayOk, and the GS validates the
//! session via PlayerAuthRequest — the full end-to-end handoff.

mod common;

use commons::crypt::NewCrypt;
use commons::network::{read_frame, write_frame, PacketReader, PacketWriter};
use common::{login, start_server, test_config};
use loginserver::gs_link::packets::{gs_decrypt, gs_encrypt, GS_STATIC_BLOWFISH_KEY};
use loginserver::gs_table::hexid_from_string;
use num_bigint_dig::BigUint;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// The stock hexid seeded in the test `gameservers` table (same as dist).
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

/// Connect + key exchange + GameServerAuth, exactly like `LoginServerThread`.
async fn register_game_server(gs_addr: std::net::SocketAddr, desired_id: u8, port: u16) -> SimGameServer {
    let stream = TcpStream::connect(gs_addr).await.unwrap();
    let (read, write) = stream.into_split();
    let mut gs = SimGameServer { read, write, crypt: NewCrypt::new(GS_STATIC_BLOWFISH_KEY) };

    // InitLS: protocol rev + RSA modulus (BigInteger.toByteArray()).
    let init = gs.recv().await.expect("no InitLS");
    let mut r = PacketReader::new(&init);
    assert_eq!(r.read_u8().unwrap(), 0x00, "InitLS opcode");
    assert_eq!(r.read_i32().unwrap(), 0x0106, "protocol rev");
    let key_len = r.read_i32().unwrap() as usize;
    let modulus = BigUint::from_bytes_be(r.read_bytes(key_len).unwrap());

    // BlowFishKey: RSA-encrypt a fresh 16-byte key into a 64-byte block.
    let mut new_key = [0u8; 16];
    new_key[0] = 0x42;
    for (i, b) in new_key.iter_mut().enumerate().skip(1) {
        *b = (i as u8).wrapping_mul(29).wrapping_add(3);
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

    // GameServerAuth (AuthRequest sender layout).
    let hexid = hexid_from_string(HEXID).unwrap();
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(desired_id);
    w.write_u8(0); // acceptAlternate
    w.write_u8(0); // reserveHost
    w.write_i16(port as i16);
    w.write_i32(5000); // maxPlayers
    w.write_i32(hexid.len() as i32);
    w.write_bytes(&hexid);
    w.write_i32(1); // subnet/host pair count
    w.write_string("0.0.0.0/0");
    w.write_string("127.0.0.1");
    gs.send(w.into_bytes()).await;

    // AuthResponse.
    let resp = gs.recv().await.expect("no AuthResponse");
    let mut r = PacketReader::new(&resp);
    assert_eq!(r.read_u8().unwrap(), 0x02, "AuthResponse opcode");
    assert_eq!(r.read_u8().unwrap(), desired_id, "assigned server id");
    assert_eq!(r.read_string().unwrap(), "Bartz");

    gs
}

async fn set_status_normal(gs: &mut SimGameServer) {
    let mut w = PacketWriter::new();
    w.write_u8(0x06);
    w.write_i32(1);
    w.write_i32(0x01); // SERVER_LIST_STATUS
    w.write_i32(0x02); // STATUS_NORMAL
    gs.send(w.into_bytes()).await;
}

#[tokio::test]
async fn full_end_to_end_login_through_gs() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr, 1, 7777).await;
    set_status_normal(&mut gs).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await; // let status apply

    // Client logs in and requests the server list.
    let (mut client, reply) = login(server.addr, "player1", "pw").await;
    assert_eq!(reply[0], 0x03, "LoginOk");
    let login_ok1 = i32::from_le_bytes(reply[1..5].try_into().unwrap());
    let login_ok2 = i32::from_le_bytes(reply[5..9].try_into().unwrap());

    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    client.send(&w.into_bytes()).await;

    let list = client.recv().await.expect("no ServerList");
    assert_eq!(list[0], 0x04, "ServerList opcode");
    assert_eq!(list[1], 1, "one server");
    let mut r = PacketReader::new(&list[3..]);
    assert_eq!(r.read_u8().unwrap(), 1, "server id");
    assert_eq!(r.read_bytes(4).unwrap(), &[127, 0, 0, 1], "resolved ip");
    assert_eq!(r.read_i32().unwrap(), 7777, "port");
    let _age = r.read_u8().unwrap();
    let _pvp = r.read_u8().unwrap();
    let _current = r.read_i16().unwrap();
    assert_eq!(r.read_i16().unwrap(), 5000, "max players");
    assert_eq!(r.read_u8().unwrap(), 0x01, "server is up");

    // RequestServerLogin → PlayOk.
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    w.write_u8(1);
    client.send(&w.into_bytes()).await;

    let play = client.recv().await.expect("no PlayOk");
    assert_eq!(play[0], 0x07, "PlayOk opcode");
    let play_ok1 = i32::from_le_bytes(play[1..5].try_into().unwrap());
    let play_ok2 = i32::from_le_bytes(play[5..9].try_into().unwrap());

    // The game server validates the session key with the login server.
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_string("player1");
    w.write_i32(play_ok1);
    w.write_i32(play_ok2);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    gs.send(w.into_bytes()).await;

    let auth = gs.recv().await.expect("no PlayerAuthResponse");
    let mut r = PacketReader::new(&auth);
    assert_eq!(r.read_u8().unwrap(), 0x03, "PlayerAuthResponse opcode");
    assert_eq!(r.read_string().unwrap(), "player1");
    assert_eq!(r.read_u8().unwrap(), 1, "session accepted");
}

#[tokio::test]
async fn wrong_session_key_rejected_by_player_auth() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr, 1, 7777).await;

    let (_client, reply) = login(server.addr, "player2", "pw").await;
    assert_eq!(reply[0], 0x03);

    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_string("player2");
    w.write_i32(111);
    w.write_i32(222);
    w.write_i32(333);
    w.write_i32(444);
    gs.send(w.into_bytes()).await;

    let auth = gs.recv().await.expect("no PlayerAuthResponse");
    let mut r = PacketReader::new(&auth);
    assert_eq!(r.read_u8().unwrap(), 0x03);
    assert_eq!(r.read_string().unwrap(), "player2");
    assert_eq!(r.read_u8().unwrap(), 0, "session rejected");
}

#[tokio::test]
async fn account_on_gs_gets_kicked_on_relogin() {
    let server = start_server(test_config()).await;
    let mut gs = register_game_server(server.gs_addr, 1, 7777).await;

    // GS reports the account as in-game (PlayerInGame).
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i16(1);
    w.write_string("dualgs");
    gs.send(w.into_bytes()).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Account exists (create via first login on another name path): create directly.
    let (_c, reply) = login(server.addr, "dualgs", "pw").await;
    // ALREADY_ON_GS → ACCOUNT_IN_USE for the client...
    assert_eq!(reply[0], 0x01, "LoginFail opcode");
    assert_eq!(reply[1], 0x07, "REASON_ACCOUNT_IN_USE");

    // ...and a KickPlayer command for the game server.
    let kick = gs.recv().await.expect("no KickPlayer");
    let mut r = PacketReader::new(&kick);
    assert_eq!(r.read_u8().unwrap(), 0x04, "KickPlayer opcode");
    assert_eq!(r.read_string().unwrap(), "dualgs");
}

#[tokio::test]
async fn wrong_hexid_rejected() {
    let mut config = test_config();
    config.accept_new_gameserver = false;
    let server = start_server(config).await;

    let stream = TcpStream::connect(server.gs_addr).await.unwrap();
    let (read, write) = stream.into_split();
    let mut gs = SimGameServer { read, write, crypt: NewCrypt::new(GS_STATIC_BLOWFISH_KEY) };

    let init = gs.recv().await.expect("no InitLS");
    let mut r = PacketReader::new(&init);
    r.read_u8();
    r.read_i32();
    let key_len = r.read_i32().unwrap() as usize;
    let modulus = BigUint::from_bytes_be(r.read_bytes(key_len).unwrap());

    let new_key = [0x11u8; 16];
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

    // Auth with a hexid that doesn't match server 1's registration.
    let bad_hexid = hexid_from_string("1234abcd").unwrap();
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(1);
    w.write_u8(0);
    w.write_u8(0);
    w.write_i16(7777);
    w.write_i32(100);
    w.write_i32(bad_hexid.len() as i32);
    w.write_bytes(&bad_hexid);
    w.write_i32(0);
    gs.send(w.into_bytes()).await;

    let fail = gs.recv().await.expect("no LoginServerFail");
    assert_eq!(fail[0], 0x01, "LoginServerFail opcode");
    assert_eq!(fail[1], 3, "REASON_WRONG_HEXID");
}
