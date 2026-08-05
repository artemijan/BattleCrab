//! `config/Network.ini` — the outbound drop policy (Java `ConnectionConfig`).
//!
//! Only the two keys with a portable meaning are read. The rest of the file
//! tunes Async-mmocore internals that have no equivalent here:
//! `ShutdownWaitTime` (its send-finalize grace), `FairnessBuckets` (its
//! shared-writer scheduler; each connection has its own task in this port) and
//! `AutoReading` (its read-scheduling switch).

use commons::config::PropertiesParser;

pub const NETWORK_CONFIG_FILE: &str = "config/Network.ini";

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Java `DropPackets`: under outbound pressure, disposable packets
    /// (see `network::can_be_dropped`) are dropped instead of queued.
    /// Java's code default is off; the dist config turns it on.
    pub drop_packets: bool,
    /// Java `DropPacketThreshold`: the per-connection outbound queue depth
    /// above which disposable packets start being dropped.
    pub drop_packet_threshold: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            drop_packets: false,
            drop_packet_threshold: 250,
        }
    }
}

impl NetworkConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, NETWORK_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            drop_packets: p.get_bool("DropPackets", d.drop_packets),
            drop_packet_threshold: p
                .get_int("DropPacketThreshold", d.drop_packet_threshold as i32)
                .max(0) as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser(body: &str) -> PropertiesParser {
        PropertiesParser::from_content("Network.ini", body)
    }

    /// An absent file behaves like Java's unset `ConnectionConfig`: the
    /// policy is off and the threshold is the code default.
    #[test]
    fn an_absent_file_mirrors_javas_code_defaults() {
        let cfg = NetworkConfig::from_parser(&parser(""));
        assert!(!cfg.drop_packets);
        assert_eq!(cfg.drop_packet_threshold, 250);
    }

    /// The dist `Network.ini` values are read.
    #[test]
    fn the_dist_values_are_read() {
        let cfg =
            NetworkConfig::from_parser(&parser("DropPackets = True\nDropPacketThreshold = 2500\n"));
        assert!(cfg.drop_packets);
        assert_eq!(cfg.drop_packet_threshold, 2500);
    }
}
