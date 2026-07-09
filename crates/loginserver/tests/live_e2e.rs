//! Live end-to-end check against a running Rust LS (port 2106) with the real
//! Java GS registered. Ignored by default — run explicitly:
//! `cargo test -p loginserver --test live_e2e -- --ignored`

mod common;

use commons::network::{PacketReader, PacketWriter};

#[tokio::test]
#[ignore = "needs live Rust LS on 127.0.0.1:2106 with a registered GS"]
async fn live_login_to_play_ok() {
    let addr: std::net::SocketAddr = "127.0.0.1:2106".parse().unwrap();
    let (mut client, reply) = common::login(addr, "livetest", "livetest").await;
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
    let count = list[1];
    assert!(count >= 1, "at least one game server listed");
    let mut r = PacketReader::new(&list[3..]);
    let server_id = r.read_u8().unwrap();
    let ip = r.read_bytes(4).unwrap().to_vec();
    let port = r.read_i32().unwrap();
    let _age = r.read_u8();
    let _pvp = r.read_u8();
    let current = r.read_i16().unwrap();
    let max = r.read_i16().unwrap();
    let up = r.read_u8().unwrap();
    println!(
        "ServerList: id={server_id} addr={}.{}.{}.{}:{port} players={current}/{max} up={up}",
        ip[0], ip[1], ip[2], ip[3]
    );
    assert_eq!(up, 0x01, "Bartz reports as up");

    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_i32(login_ok1);
    w.write_i32(login_ok2);
    w.write_u8(server_id);
    client.send(&w.into_bytes()).await;

    let play = client.recv().await.expect("no PlayOk");
    assert_eq!(play[0], 0x07, "PlayOk opcode");
    println!("PlayOk received — session handed to game server.");
}
