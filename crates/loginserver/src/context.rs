//! Immutable process-wide state: config, the RSA keypair cache, and the
//! Blowfish key cache (`LoginController` constructor, minus mutable state —
//! that part becomes the controller actor in M3).

use std::sync::Arc;

use commons::crypt::{RawRsaKeyPair, ScrambledKeyPair};
use commons::util::rnd;
use sqlx::SqlitePool;
use tracing::info;

use crate::config::LoginConfig;
use crate::controller::ControllerHandle;

const KEYPAIRS: usize = 10;
const BLOWFISH_KEYS: usize = 20;
const GS_KEYPAIRS: usize = 10;

pub struct LoginContext {
    pub config: LoginConfig,
    pub pool: SqlitePool,
    pub controller: ControllerHandle,
    keypairs: Vec<Arc<ScrambledKeyPair>>,
    blowfish_keys: Vec<[u8; 16]>,
    gs_keypairs: Vec<Arc<RawRsaKeyPair>>,
}

impl LoginContext {
    pub fn new(config: LoginConfig, pool: SqlitePool, controller: ControllerHandle) -> Self {
        // Keygen is the dominant boot cost (prime hunting), so generate all
        // pairs in parallel — one thread per key, wall time ≈ the slowest
        // single key (Java generates the same caches sequentially).
        let (keypairs, gs_keypairs) = std::thread::scope(|s| {
            let client: Vec<_> =
                (0..KEYPAIRS).map(|_| s.spawn(|| Arc::new(ScrambledKeyPair::generate()))).collect();
            // GameServerTable.initRSAKeys: 512-bit pairs for the GS link.
            let gs: Vec<_> =
                (0..GS_KEYPAIRS).map(|_| s.spawn(|| Arc::new(RawRsaKeyPair::generate(512)))).collect();
            (
                client.into_iter().map(|h| h.join().expect("RSA keygen thread panicked")).collect::<Vec<_>>(),
                gs.into_iter().map(|h| h.join().expect("RSA keygen thread panicked")).collect::<Vec<_>>(),
            )
        });
        info!("Cached {KEYPAIRS} KeyPairs for RSA communication.");

        let blowfish_keys: Vec<[u8; 16]> = (0..BLOWFISH_KEYS)
            .map(|_| {
                let mut key = [0u8; 16];
                rnd::fill_bytes(&mut key);
                key
            })
            .collect();
        info!("Stored {BLOWFISH_KEYS} keys for Blowfish communication.");

        info!("Cached {GS_KEYPAIRS} RSA keys for Game Server communication.");

        Self { config, pool, controller, keypairs, blowfish_keys, gs_keypairs }
    }

    pub fn random_keypair(&self) -> Arc<ScrambledKeyPair> {
        self.keypairs[rnd::get(KEYPAIRS as i32) as usize].clone()
    }

    pub fn random_gs_keypair(&self) -> Arc<RawRsaKeyPair> {
        self.gs_keypairs[rnd::get(GS_KEYPAIRS as i32) as usize].clone()
    }

    pub fn random_blowfish_key(&self) -> [u8; 16] {
        self.blowfish_keys[rnd::get(BLOWFISH_KEYS as i32) as usize]
    }
}
