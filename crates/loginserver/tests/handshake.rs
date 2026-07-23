//! M2 acceptance (protocol side): Init → AuthGameGuard → GGAuth, plus the
//! wrong-session failure path.

mod common;

use common::{handshake, start_server, test_config};

#[tokio::test]
async fn full_handshake() {
    let server = start_server(test_config()).await;
    let client = handshake(server.addr).await;
    assert_ne!(client.session_id, 0);
}

#[tokio::test]
async fn wrong_session_id_gets_login_fail() {
    let server = start_server(test_config()).await;

    let stream = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let (mut read, mut write) = stream.into_split();

    let mut init = commons::network::read_frame(&mut read, 8192)
        .await
        .unwrap()
        .expect("no Init frame");
    commons::crypt::NewCrypt::new(&common::STATIC_BLOWFISH_KEY).decrypt(&mut init);
    common::dec_xor_pass(&mut init);
    let session_id = i32::from_le_bytes(init[1..5].try_into().unwrap());
    let blowfish_key: [u8; 16] = init[9 + 128 + 16..9 + 128 + 16 + 16].try_into().unwrap();
    let crypt = commons::crypt::NewCrypt::new(&blowfish_key);

    let mut body = vec![0x07u8];
    body.extend_from_slice(&session_id.wrapping_add(1).to_le_bytes()); // wrong id
    body.extend_from_slice(&[0u8; 16]);
    commons::network::write_frame(&mut write, &common::client_encrypt(&crypt, &body))
        .await
        .unwrap();

    let mut fail = commons::network::read_frame(&mut read, 8192)
        .await
        .unwrap()
        .expect("no LoginFail frame");
    crypt.decrypt(&mut fail);
    assert_eq!(fail[0], 0x01, "LoginFail opcode");
    assert_eq!(fail[1], 0x15, "REASON_ACCESS_FAILED");

    // Server closes after LoginFail (Java close(reason) semantics).
    assert!(commons::network::read_frame(&mut read, 8192)
        .await
        .unwrap()
        .is_none());
}
