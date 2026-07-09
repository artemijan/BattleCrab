//! Immutable process-wide state: config, the RSA keypair cache, and the
//! Blowfish key cache (`LoginController` constructor, minus mutable state —
//! that part becomes the controller actor in M3).

use std::sync::Arc;

use commons::crypt::ScrambledKeyPair;
use commons::util::rnd;
use sqlx::SqlitePool;
use tracing::info;

use crate::config::LoginConfig;

const KEYPAIRS: usize = 10;
const BLOWFISH_KEYS: usize = 20;

pub struct LoginContext {
    pub config: LoginConfig,
    pub pool: SqlitePool,
    keypairs: Vec<Arc<ScrambledKeyPair>>,
    blowfish_keys: Vec<[u8; 16]>,
}

impl LoginContext {
    pub fn new(config: LoginConfig, pool: SqlitePool) -> Self {
        let keypairs: Vec<_> = (0..KEYPAIRS).map(|_| Arc::new(ScrambledKeyPair::generate())).collect();
        info!("Cached {KEYPAIRS} KeyPairs for RSA communication.");

        let blowfish_keys: Vec<[u8; 16]> = (0..BLOWFISH_KEYS)
            .map(|_| {
                let mut key = [0u8; 16];
                rnd::fill_bytes(&mut key);
                key
            })
            .collect();
        info!("Stored {BLOWFISH_KEYS} keys for Blowfish communication.");

        Self { config, pool, keypairs, blowfish_keys }
    }

    pub fn random_keypair(&self) -> Arc<ScrambledKeyPair> {
        self.keypairs[rnd::get(KEYPAIRS as i32) as usize].clone()
    }

    pub fn random_blowfish_key(&self) -> [u8; 16] {
        self.blowfish_keys[rnd::get(BLOWFISH_KEYS as i32) as usize]
    }
}
