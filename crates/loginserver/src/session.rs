//! Ports of `SessionKey.java` and `model/data/AccountInfo.java`.

use commons::util::rnd;

/// The two key pairs handed to the client: `loginOk` at `LoginOk`,
/// `playOk` at `PlayOk`; the game server verifies all four on enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub login_ok1: i32,
    pub login_ok2: i32,
    pub play_ok1: i32,
    pub play_ok2: i32,
}

impl SessionKey {
    pub fn random() -> Self {
        Self {
            login_ok1: rnd::next_int(),
            login_ok2: rnd::next_int(),
            play_ok1: rnd::next_int(),
            play_ok2: rnd::next_int(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub login: String,
    pub pass_hash: String,
    pub access_level: i32,
    pub last_server: i32,
}

impl AccountInfo {
    pub fn check_pass_hash(&self, hash: &str) -> bool {
        !self.pass_hash.is_empty() && self.pass_hash == hash
    }
}
