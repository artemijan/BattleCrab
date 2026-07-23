//! Byte-for-byte parity test for the game XOR cipher against vectors dumped from
//! the real Java `gameserver/network/Encryption` (`tools/game-cipher-vectors`).
//!
//! Replays the exact scripted encrypt/decrypt sequence and asserts identical
//! output at every step — covering the first-call pass-through, both directions,
//! and independent per-direction key rolling across varying packet sizes.

use gameserver::network::cipher::Encryption;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn game_cipher_matches_java() {
    let v: serde_json::Value = serde_json::from_str(include_str!("cipher_vectors.json"))
        .expect("invalid cipher_vectors.json");

    let key: [u8; 16] = hex(v["key"].as_str().unwrap()).try_into().unwrap();
    let mut e = Encryption::new();
    e.set_key(&key);

    for (i, step) in v["steps"].as_array().unwrap().iter().enumerate() {
        let op = step["op"].as_str().unwrap();
        let mut work = hex(step["in"].as_str().unwrap());
        match op {
            "e" => e.encrypt(&mut work),
            "d" => e.decrypt(&mut work),
            other => panic!("unknown op {other}"),
        }
        assert_eq!(
            work,
            hex(step["out"].as_str().unwrap()),
            "step {i} ({op}) mismatch"
        );
    }
}

#[test]
fn encrypt_first_call_is_passthrough_then_roundtrips() {
    // Two peers sharing a key: one enabled by a pass-through "KeyPacket", the
    // other likewise, then a body encrypts on one side and decrypts on the other.
    let key = Encryption::key_from_random(&[1, 2, 3, 4, 5, 6, 7, 8]);

    let mut server = Encryption::new();
    server.set_key(&key);
    let mut client = Encryption::new();
    client.set_key(&key);

    // First call each side: pass-through (KeyPacket in the clear), enables cipher.
    let mut warm = vec![9u8; 4];
    server.encrypt(&mut warm);
    assert_eq!(warm, vec![9u8; 4], "first encrypt must be pass-through");
    client.encrypt(&mut warm);

    // Client encrypts a body; server decrypts it back.
    let plain = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];
    let mut msg = plain.clone();
    client.encrypt(&mut msg);
    assert_ne!(msg, plain, "body must actually be transformed");
    server.decrypt(&mut msg);
    assert_eq!(msg, plain, "server must recover the client's plaintext");
}
