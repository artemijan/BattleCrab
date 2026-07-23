//! G2 protocol-parity tests for the GS↔LS link, cross-checked against the
//! **login server's** own packet code (added as a dev-dependency). Proves the
//! game side and login side agree byte-for-byte without spinning up sockets.

use commons::crypt::{
    gs_decrypt, gs_encrypt, NewCrypt, RawRsaKeyPair, RsaPublicModulus, GS_STATIC_BLOWFISH_KEY,
};
use commons::network::PacketReader;
use gameserver::loginlink::packets as gs;
use gameserver::session::SessionKey;
use loginserver::gs_link::packets as ls;

/// LS builders → GS parsers (the packets the game server receives).
#[test]
fn ls_to_gs_packets_parse() {
    // InitLS: revision + modulus.
    let pair = RawRsaKeyPair::generate(512);
    let modulus = pair.modulus_java_bytes();
    let init_bytes = ls::init_ls(0x0106, &modulus);
    let init = gs::InitLs::read(&init_bytes[1..]).unwrap();
    assert_eq!(init.revision, 0x0106);
    assert_eq!(init.rsa_key, modulus);

    // AuthResponse: server id + name.
    let ar = gs::AuthResponse::read(&ls::auth_response(1, "Bartz")[1..]).unwrap();
    assert_eq!(ar.server_id, 1);
    assert_eq!(ar.server_name, "Bartz");

    // PlayerAuthResponse: account + authed flag.
    let par = gs::PlayerAuthResponse::read(&ls::player_auth_response("acc", true)[1..]).unwrap();
    assert_eq!(par.account, "acc");
    assert!(par.authed);
    let par = gs::PlayerAuthResponse::read(&ls::player_auth_response("acc", false)[1..]).unwrap();
    assert!(!par.authed);

    // KickPlayer / RequestCharacters: a single account string.
    assert_eq!(
        gs::read_account(&ls::kick_player("bob")[1..]).unwrap(),
        "bob"
    );
    assert_eq!(
        gs::read_account(&ls::request_characters("bob")[1..]).unwrap(),
        "bob"
    );

    // LoginServerFail: reason code.
    assert_eq!(
        gs::read_login_server_fail(&ls::login_server_fail(7)[1..]).unwrap(),
        7
    );
}

/// GS→LS `BlowFishKey` RSA block decrypts on the LS side to the original key.
#[test]
fn blowfish_key_rsa_roundtrips_to_ls() {
    let pair = RawRsaKeyPair::generate(512);
    let modulus = RsaPublicModulus::from_java_bytes(&pair.modulus_java_bytes());
    let key = commons::util::generate_hex(40);

    let packet = gs::blowfish_key(&key, &modulus);
    // Decode the wire layout: opcode 0x00, int length, encrypted block.
    let mut r = PacketReader::new(&packet);
    assert_eq!(r.read_u8().unwrap(), 0x00);
    let len = r.read_i32().unwrap() as usize;
    let block = r.read_bytes(len).unwrap();

    // LS decrypts with its private key and strips leading zeros (as it does live).
    let decrypted = pair.decrypt_raw(block);
    let first = decrypted
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(decrypted.len());
    assert_eq!(&decrypted[first..], &key[..]);
}

/// GS→LS `AuthRequest` has the exact field layout the LS reader consumes.
#[test]
fn auth_request_layout() {
    let hexid = commons::util::generate_hex(16);
    let hosts = vec![("127.0.0.0/8".to_string(), "127.0.0.1".to_string())];
    let bytes = gs::auth_request(1, false, false, 7777, 2000, &hexid, &hosts);

    let mut r = PacketReader::new(&bytes);
    assert_eq!(r.read_u8().unwrap(), 0x01); // opcode
    assert_eq!(r.read_u8().unwrap(), 1); // desired id
    assert_eq!(r.read_u8().unwrap(), 0); // accept alternate
    assert_eq!(r.read_u8().unwrap(), 0); // reserve host
    assert_eq!(r.read_i16().unwrap(), 7777); // port
    assert_eq!(r.read_i32().unwrap(), 2000); // max players
    assert_eq!(r.read_i32().unwrap() as usize, hexid.len());
    assert_eq!(r.read_bytes(hexid.len()).unwrap(), &hexid[..]);
    assert_eq!(r.read_i32().unwrap(), 1); // host pair count
    assert_eq!(r.read_string().unwrap(), "127.0.0.0/8");
    assert_eq!(r.read_string().unwrap(), "127.0.0.1");
}

/// GS→LS `PlayerAuthRequest` field order: play keys first, then login keys.
#[test]
fn player_auth_request_key_order() {
    let key = SessionKey::new(11, 12, 21, 22); // login1,login2,play1,play2
    let bytes = gs::player_auth_request("acc", &key);
    let mut r = PacketReader::new(&bytes);
    assert_eq!(r.read_u8().unwrap(), 0x05);
    assert_eq!(r.read_string().unwrap(), "acc");
    assert_eq!(r.read_i32().unwrap(), 21); // playOk1
    assert_eq!(r.read_i32().unwrap(), 22); // playOk2
    assert_eq!(r.read_i32().unwrap(), 11); // loginOk1
    assert_eq!(r.read_i32().unwrap(), 12); // loginOk2
}

/// The shared payload crypto round-trips a GS→LS packet.
#[test]
fn gs_link_payload_crypto_roundtrips() {
    let crypt = NewCrypt::new(GS_STATIC_BLOWFISH_KEY);
    let body = gs::player_logout("someone");
    let mut wire = gs_encrypt(&crypt, body.clone());
    assert!(gs_decrypt(&crypt, &mut wire));
    assert_eq!(&wire[..body.len()], &body[..]);
}
