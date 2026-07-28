//! Byte-for-byte parity tests against vectors dumped from the Java
//! `loginserver/crypt` classes (`tools/vector-dump/VectorDump.java`).

use commons::crypt::{NewCrypt, ScrambledKeyPair, hash_password};
use num_bigint_dig::BigUint;

fn vectors() -> serde_json::Value {
    let raw = include_str!("vectors.json");
    serde_json::from_str(raw).expect("invalid vectors.json")
}

fn hex(v: &serde_json::Value, key: &str) -> Vec<u8> {
    let s = v[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing vector {key}"));
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn blowfish_matches_java() {
    let v = vectors();
    let plain = hex(&v, "blowfish_plain");

    for (key_name, expected_name) in [
        ("blowfish_static_key", "blowfish_static_encrypted"),
        ("blowfish_runtime_key", "blowfish_runtime_encrypted"),
    ] {
        let crypt = NewCrypt::new(&hex(&v, key_name));
        let mut work = plain.clone();
        crypt.crypt(&mut work);
        assert_eq!(
            work,
            hex(&v, expected_name),
            "encrypt mismatch for {key_name}"
        );
        crypt.decrypt(&mut work);
        assert_eq!(work, plain, "decrypt roundtrip failed for {key_name}");
    }
}

#[test]
fn checksum_matches_java() {
    let v = vectors();
    let mut work = hex(&v, "checksum_input");
    NewCrypt::append_checksum(&mut work);
    assert_eq!(work, hex(&v, "checksum_output"));
    assert!(NewCrypt::verify_checksum(&work));
}

#[test]
fn xor_pass_matches_java() {
    let v = vectors();
    let mut work = hex(&v, "xor_input");
    let key = v["xor_key"].as_i64().unwrap() as i32;
    NewCrypt::enc_xor_pass(&mut work, key);
    assert_eq!(work, hex(&v, "xor_output"));
}

#[test]
fn modulus_scramble_matches_java() {
    let v = vectors();
    let n = BigUint::parse_bytes(v["rsa_modulus"].as_str().unwrap().as_bytes(), 16).unwrap();
    let d = BigUint::parse_bytes(v["rsa_d"].as_str().unwrap().as_bytes(), 16).unwrap();
    let pair = ScrambledKeyPair::from_parts(n, d);
    assert_eq!(
        pair.scrambled_modulus().as_slice(),
        hex(&v, "rsa_scrambled_modulus")
    );
}

#[test]
fn raw_rsa_decrypt_matches_java() {
    let v = vectors();
    let n = BigUint::parse_bytes(v["rsa_modulus"].as_str().unwrap().as_bytes(), 16).unwrap();
    let d = BigUint::parse_bytes(v["rsa_d"].as_str().unwrap().as_bytes(), 16).unwrap();
    let pair = ScrambledKeyPair::from_parts(n, d);

    let encrypted: [u8; 0x80] = hex(&v, "rsa_encrypted_block").try_into().unwrap();
    let decrypted = pair.decrypt_raw(&encrypted);
    assert_eq!(decrypted.as_slice(), hex(&v, "rsa_plain_block"));
}

#[test]
fn password_hash_matches_java() {
    let v = vectors();
    assert_eq!(
        hash_password(v["password_plain"].as_str().unwrap()),
        v["password_hash"].as_str().unwrap()
    );
}
