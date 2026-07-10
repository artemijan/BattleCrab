//! Port of the transport-relevant parts of `gameserver/network/GameClient`.
//!
//! The connection task owns one of these. It holds the per-connection cipher and
//! protocol state; the gameplay-facing parts of the Java `GameClient` (the bound
//! `Player`, account, etc.) live on the game thread and are added from G2 on.

use rand::RngCore;

use super::cipher::Encryption;
use super::ConnectionState;

pub struct GameClient {
    pub client_id: u32,
    pub state: ConnectionState,
    pub protocol_version: i32,
    pub protocol_ok: bool,
    /// Config.PACKET_ENCRYPTION — when false the cipher is never installed and
    /// packets go in the clear (the client still receives a key).
    packet_encryption: bool,
    encryption: Option<Encryption>,
}

impl GameClient {
    pub fn new(client_id: u32, packet_encryption: bool) -> Self {
        Self {
            client_id,
            state: ConnectionState::Connected,
            protocol_version: 0,
            protocol_ok: false,
            packet_encryption,
            encryption: None,
        }
    }

    /// Java `enableCrypt`: pick a random key (`[8 random | 8 static]`), install
    /// the cipher (only if encryption is enabled), and return the full 16-byte
    /// key. The caller sends the first 8 bytes in `KeyPacket`.
    pub fn enable_crypt(&mut self) -> [u8; 16] {
        let mut random8 = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut random8);
        let key = Encryption::key_from_random(&random8);
        if self.packet_encryption {
            let mut enc = Encryption::new();
            enc.set_key(&key);
            self.encryption = Some(enc);
        }
        key
    }

    /// Java `encrypt`: transform in place when the cipher is installed
    /// (the first call is a pass-through — see [`Encryption`]).
    pub fn encrypt(&mut self, data: &mut [u8]) {
        if self.packet_encryption {
            if let Some(e) = &mut self.encryption {
                e.encrypt(data);
            }
        }
    }

    /// Java `decrypt`: no-op until the cipher is installed.
    pub fn decrypt(&mut self, data: &mut [u8]) {
        if self.packet_encryption {
            if let Some(e) = &mut self.encryption {
                e.decrypt(data);
            }
        }
    }
}
