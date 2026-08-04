//! `config/Security.ini` — transport-level flood protection.
//!
//! **This file has no Java counterpart.** `FloodProtector.ini` (see
//! [`super::flood_protector`]) rate-limits *actions* per logged-in client, which
//! is the only flood defence L2J Mobius has on the game port: its client-facing
//! listener accepts every connection, and its packet reader forwards every
//! frame. That is survivable in Java because the reader thread processes each
//! client's packets inline, so a flooding client mostly throttles itself.
//!
//! This port decouples the two — the connection task forwards decrypted bodies
//! to the game thread over an **unbounded** channel — so a client that never
//! logs in, or one that spams faster than the 100 ms tick drains, has no
//! backpressure at all and the queue is bounded only by memory. These settings
//! close that gap:
//!
//! * a per-connection inbound packet rate, which bounds how much one socket can
//!   put in flight between two ticks, and
//! * per-IP accept-time limits, which are Java's own `FloodProtectedListener`
//!   rules (upstream applies them only to the game-server↔login link, never to
//!   players) with its `LoginServer.ini` values as the defaults.
//!
//! Everything here can be turned off; the defaults are deliberately far above
//! any legitimate client.

use commons::config::PropertiesParser;

pub const SECURITY_CONFIG_FILE: &str = "config/Security.ini";

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Master switch for the per-IP accept-time rules below.
    pub enable_connection_flood_protection: bool,
    /// Live connections allowed from one address (Java `MaxConnectionPerIP`).
    /// Generous on purpose: an internet cafe or any NAT is one address.
    pub max_connections_per_ip: i32,
    /// Above this many live connections from one address, a new one must be at
    /// least `normal_connection_time` after the previous (Java
    /// `FastConnectionLimit`).
    pub fast_connection_limit: i32,
    /// Milliseconds (Java `NormalConnectionTime`).
    pub normal_connection_time: i64,
    /// Any two connections from one address closer together than this are
    /// refused outright (Java `FastConnectionTime`). **This is the setting to
    /// relax first** if legitimate players share an address.
    pub fast_connection_time: i64,

    /// Master switch for the per-connection inbound packet rate.
    pub enable_packet_rate_limit: bool,
    /// Inbound packets one connection may send per second before it is closed.
    /// A busy client peaks in the low tens; the default leaves an order of
    /// magnitude of headroom, because this is a transport backstop and not a
    /// gameplay rule — `FloodProtector.ini` is where per-action limits belong.
    pub max_packets_per_second: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_connection_flood_protection: true,
            max_connections_per_ip: 50,
            fast_connection_limit: 15,
            normal_connection_time: 700,
            fast_connection_time: 350,
            enable_packet_rate_limit: true,
            max_packets_per_second: 300,
        }
    }
}

impl SecurityConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, SECURITY_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enable_connection_flood_protection: p.get_bool(
                "EnableConnectionFloodProtection",
                d.enable_connection_flood_protection,
            ),
            max_connections_per_ip: p.get_int("MaxConnectionsPerIP", d.max_connections_per_ip),
            fast_connection_limit: p.get_int("FastConnectionLimit", d.fast_connection_limit),
            normal_connection_time: p
                .get_int("NormalConnectionTime", d.normal_connection_time as i32)
                as i64,
            fast_connection_time: p.get_int("FastConnectionTime", d.fast_connection_time as i32)
                as i64,
            enable_packet_rate_limit: p
                .get_bool("EnablePacketRateLimit", d.enable_packet_rate_limit),
            max_packets_per_second: p
                .get_int("MaxPacketsPerSecond", d.max_packets_per_second as i32)
                .max(0) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser(body: &str) -> PropertiesParser {
        PropertiesParser::from_content("Security.ini", body)
    }

    #[test]
    fn an_absent_file_leaves_the_protection_on_with_the_java_values() {
        let cfg = SecurityConfig::from_parser(&parser(""));
        assert!(cfg.enable_connection_flood_protection);
        assert!(cfg.enable_packet_rate_limit);
        // The four accept-time numbers are Java's `LoginServer.ini` defaults.
        assert_eq!(cfg.max_connections_per_ip, 50);
        assert_eq!(cfg.fast_connection_limit, 15);
        assert_eq!(cfg.normal_connection_time, 700);
        assert_eq!(cfg.fast_connection_time, 350);
    }

    #[test]
    fn an_operator_can_turn_both_halves_off_independently() {
        let cfg = SecurityConfig::from_parser(&parser(
            "EnableConnectionFloodProtection = False\nMaxPacketsPerSecond = 1000\n",
        ));
        assert!(!cfg.enable_connection_flood_protection);
        assert!(
            cfg.enable_packet_rate_limit,
            "the packet limit has its own switch"
        );
        assert_eq!(cfg.max_packets_per_second, 1000);
    }
}
