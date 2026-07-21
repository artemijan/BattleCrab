//! In-memory fixed-window limiter, keyed by an arbitrary string.
//!
//! This is load-bearing, not decorative: web login verifies against the game's
//! unsalted SHA-1 hash (PLAN_DASHBOARD.md §5.2), so throttling is the primary
//! defence against online password guessing. Login is limited per-IP *and*
//! per-account, so one attacker cannot spread a spray across many IPs against
//! one account, nor walk many accounts from one IP.
//!
//! Deliberately in-process: a single dashboard instance is the deployment shape
//! (it must share a host with the game server anyway — §9). If this ever runs
//! multi-instance, this becomes per-instance and needs rethinking.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    limit: u32,
    window: Duration,
    hits: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window_secs: u64) -> Self {
        Self {
            limit,
            window: Duration::from_secs(window_secs),
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Records an attempt. Returns false when the caller should be rejected.
    pub fn check(&self, key: &str) -> bool {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> bool {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());

        // Opportunistic sweep so a stream of unique keys (every login attempt
        // carries a fresh IP, say) can't grow this map without bound.
        if hits.len() > 10_000 {
            hits.retain(|_, (started, _)| now.duration_since(*started) < self.window);
        }

        let entry = hits.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.limit
    }

    /// Clears a key's budget — called after a *successful* login so a legitimate
    /// user who fumbled their password a few times isn't left throttled.
    pub fn reset(&self, key: &str) {
        self.hits.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit_then_rejects() {
        let rl = RateLimiter::new(3, 60);
        assert!(rl.check("a"));
        assert!(rl.check("a"));
        assert!(rl.check("a"));
        assert!(!rl.check("a"));
    }

    #[test]
    fn keys_are_independent() {
        let rl = RateLimiter::new(1, 60);
        assert!(rl.check("a"));
        assert!(!rl.check("a"));
        assert!(rl.check("b"));
    }

    #[test]
    fn the_window_rolls_over() {
        let rl = RateLimiter::new(1, 60);
        let t0 = Instant::now();
        assert!(rl.check_at("a", t0));
        assert!(!rl.check_at("a", t0 + Duration::from_secs(30)));
        assert!(rl.check_at("a", t0 + Duration::from_secs(61)));
    }

    #[test]
    fn reset_restores_the_budget() {
        let rl = RateLimiter::new(1, 60);
        assert!(rl.check("a"));
        assert!(!rl.check("a"));
        rl.reset("a");
        assert!(rl.check("a"));
    }
}
