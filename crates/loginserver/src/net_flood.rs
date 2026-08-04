//! Per-IP accept-time flood protection — port of `FloodProtectedListener`'s
//! rules, shared by both of the login server's listeners.
//!
//! Java wires `FloodProtectedListener` to **`GameServerListener` only**: the
//! player-facing login listener accepts every connection, and the sole defence
//! on it is `LoginTryBeforeBan` (5 wrong passwords → a 15-minute IP ban), which
//! an attacker only reaches *after* a full RSA handshake. Applying the same
//! `LoginServer.ini` rules to the client listener as well closes that gap; the
//! keys are shared, since an operator reading "Flood Protection" in that file
//! reasonably expects it to cover the port players actually connect to.

use std::collections::HashMap;
use std::sync::Arc;

use commons::util;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::LoginConfig;

#[derive(Default)]
struct ForeignConnection {
    connection_number: i32,
    last_connection: i64,
    is_flooding: bool,
}

/// The per-address table for one listener. Cloneable so the accepted
/// connection's task can [`release`](Self::release) its slot when it ends.
#[derive(Clone, Default)]
pub struct ConnectionFloodGuard {
    entries: Arc<Mutex<HashMap<String, ForeignConnection>>>,
}

impl ConnectionFloodGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Java `FloodProtectedListener.run`'s accept test: count the connection
    /// first, then refuse if any of the three rules trips. `true` = accept.
    ///
    /// A disabled `EnableFloodProtection` accepts without recording anything,
    /// exactly as Java's `if (Config.FLOOD_PROTECTION)` guard does — including
    /// its consequence that [`release`](Self::release) then has nothing to do.
    pub async fn accept(&self, ip: &str, cfg: &LoginConfig) -> bool {
        if !cfg.enable_flood_protection {
            return true;
        }
        let mut map = self.entries.lock().await;
        let now = util::now_millis();
        let Some(entry) = map.get_mut(ip) else {
            map.insert(
                ip.to_string(),
                ForeignConnection {
                    connection_number: 1,
                    last_connection: now,
                    is_flooding: false,
                },
            );
            return true;
        };

        entry.connection_number += 1;
        let too_fast = (entry.connection_number > cfg.fast_connection_limit
            && (now - entry.last_connection) < cfg.normal_connection_time as i64)
            || (now - entry.last_connection) < cfg.fast_connection_time as i64
            || entry.connection_number > cfg.max_connection_per_ip;
        if too_fast {
            // Java bumps `lastConnection` on the refused attempt too, so a
            // client hammering the port keeps pushing its own window out.
            entry.last_connection = now;
            entry.connection_number -= 1;
            if !entry.is_flooding {
                warn!("Potential Flood from {ip}");
            }
            entry.is_flooding = true;
            return false;
        }
        if entry.is_flooding {
            entry.is_flooding = false;
            info!("{ip} is not considered as flooding anymore.");
        }
        entry.last_connection = now;
        true
    }

    /// Java `removeFloodProtection`: give the slot back when the connection
    /// ends, dropping the address once it holds none.
    pub async fn release(&self, ip: &str) {
        let mut map = self.entries.lock().await;
        if let Some(entry) = map.get_mut(ip) {
            entry.connection_number -= 1;
            if entry.connection_number <= 0 {
                map.remove(ip);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped `LoginServer.ini` values, through the real key names.
    fn cfg() -> LoginConfig {
        LoginConfig::from_parser(&commons::config::PropertiesParser::from_content(
            "LoginServer.ini",
            "EnableFloodProtection = True\n\
             FastConnectionLimit = 15\n\
             NormalConnectionTime = 700\n\
             FastConnectionTime = 350\n\
             MaxConnectionPerIP = 50\n",
        ))
    }

    #[tokio::test]
    async fn the_first_connection_is_accepted_and_a_burst_is_not() {
        let g = ConnectionFloodGuard::new();
        let c = cfg();
        assert!(g.accept("10.0.0.1", &c).await);
        assert!(
            !g.accept("10.0.0.1", &c).await,
            "a second connection inside FastConnectionTime is refused"
        );
        assert!(
            g.accept("10.0.0.2", &c).await,
            "a different address is unaffected"
        );
    }

    /// The cap counts *live* connections, so releasing frees the slot.
    #[tokio::test]
    async fn releasing_frees_the_slot() {
        let g = ConnectionFloodGuard::new();
        let mut c = cfg();
        c.max_connection_per_ip = 1;
        c.fast_connection_time = 0; // isolate the ceiling from the pacing rule
        assert!(g.accept("10.0.0.1", &c).await);
        assert!(!g.accept("10.0.0.1", &c).await, "at the ceiling");
        g.release("10.0.0.1").await;
        assert!(g.accept("10.0.0.1", &c).await, "the freed slot is reusable");
    }

    /// With protection off nothing is recorded and nothing is refused — the
    /// operator's switch has to be total, not merely lenient.
    #[tokio::test]
    async fn disabled_accepts_everything() {
        let g = ConnectionFloodGuard::new();
        let mut c = cfg();
        c.enable_flood_protection = false;
        for _ in 0..100 {
            assert!(g.accept("10.0.0.1", &c).await);
        }
        assert!(g.entries.lock().await.is_empty());
    }
}
