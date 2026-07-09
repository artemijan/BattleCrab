//! LS→GS packets (`loginserverpackets/*`) and the GS-link payload
//! encryption: [data + 4-byte checksum + pad-to-8], Blowfish ECB — no XOR
//! pass and no static/session switch (`GameServerThread.sendPacket`).

use commons::crypt::NewCrypt;
use commons::network::PacketWriter;

/// Initial GS-link Blowfish key (`_;v.]05-31!|+-%xT!^[$` + NUL).
pub const GS_STATIC_BLOWFISH_KEY: &[u8] = b"_;v.]05-31!|+-%xT!^[$\x00";

pub fn gs_encrypt(crypt: &NewCrypt, mut body: Vec<u8>) -> Vec<u8> {
    body.extend_from_slice(&[0u8; 4]); // reserved for checksum
    while body.len() % 8 != 0 {
        body.push(0);
    }
    NewCrypt::append_checksum(&mut body);
    crypt.crypt(&mut body);
    body
}

/// Decrypt + checksum-verify an inbound GS payload in place.
pub fn gs_decrypt(crypt: &NewCrypt, data: &mut [u8]) -> bool {
    if data.len() % 8 != 0 {
        return false;
    }
    crypt.decrypt(data);
    NewCrypt::verify_checksum(data)
}

/// `InitLS`: protocol revision + RSA modulus (Java `BigInteger.toByteArray()`).
pub fn init_ls(protocol_rev: i32, modulus_java_bytes: &[u8]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x00);
    w.write_i32(protocol_rev);
    w.write_i32(modulus_java_bytes.len() as i32);
    w.write_bytes(modulus_java_bytes);
    w.into_bytes()
}

/// `LoginServerFail`.
pub fn login_server_fail(reason: u8) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x01);
    w.write_u8(reason);
    w.into_bytes()
}

/// `AuthResponse`: assigned id + server name.
pub fn auth_response(server_id: i32, server_name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x02);
    w.write_u8(server_id as u8);
    w.write_string(server_name);
    w.into_bytes()
}

/// `PlayerAuthResponse`.
pub fn player_auth_response(account: &str, ok: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x03);
    w.write_string(account);
    w.write_u8(if ok { 1 } else { 0 });
    w.into_bytes()
}

/// `KickPlayer`.
pub fn kick_player(account: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x04);
    w.write_string(account);
    w.into_bytes()
}

/// `RequestCharacters`.
pub fn request_characters(account: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0x05);
    w.write_string(account);
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs_roundtrip() {
        let crypt = NewCrypt::new(GS_STATIC_BLOWFISH_KEY);
        let body = init_ls(0x0106, &[1, 2, 3]);
        let mut encrypted = gs_encrypt(&crypt, body.clone());
        assert_eq!(encrypted.len() % 8, 0);
        assert!(gs_decrypt(&crypt, &mut encrypted));
        assert_eq!(&encrypted[..body.len()], &body[..]);
    }
}
