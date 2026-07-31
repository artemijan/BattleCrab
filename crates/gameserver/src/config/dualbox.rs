//! `Custom/DualboxCheck.ini` — port of the `DUALBOX_CHECK_CONFIG_FILE` block of
//! `Config.java`, read by Java's `AntiFeedManager` to cap how many characters
//! from one IP may take part in a thing at once.
//!
//! Only the **event** cap has a consumer in this port so far (the TvT
//! registration gate); the login and olympiad caps are carried because the
//! whitelist below is shared by all three and the keys are cheap to read, and
//! because a missing key silently falling back to a code default is the
//! failure mode that bit the deploy once already.

use std::collections::HashMap;

use commons::config::PropertiesParser;

pub const DUALBOX_CHECK_CONFIG_FILE: &str = "config/Custom/DualboxCheck.ini";

#[derive(Debug, Clone, Default)]
pub struct DualboxConfig {
    /// `DualboxCheckMaxPlayersPerIP` — characters per IP allowed in game at
    /// once (2 here). No consumer yet: the port does not gate login on it.
    pub max_players_per_ip: i32,
    /// `DualboxCheckMaxOlympiadParticipantsPerIP` (1 here). No consumer yet.
    pub max_olympiad_participants_per_ip: i32,
    /// `DualboxCheckMaxL2EventParticipantsPerIP` (**1** here) — participants per
    /// IP in an event, read by the TvT registration gate. `0` means unlimited,
    /// and Java skips the check entirely at 0 rather than treating it as a cap.
    pub max_event_participants_per_ip: i32,
    /// `DualboxCountOfflineTraders` (False here) — whether an offline shop still
    /// counts against its IP. No consumer yet.
    pub count_offline_traders: bool,
    /// `DualboxCheckWhitelist` — `address,extra;address,extra…`, an *additional*
    /// allowance per address on top of every cap above. Java hashes the resolved
    /// `InetAddress`; the port keys by the literal string, which is equivalent
    /// for the numeric forms this file uses and avoids a DNS lookup at boot.
    pub whitelist: HashMap<String, i32>,
}

impl DualboxConfig {
    pub fn load_from(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, DUALBOX_CHECK_CONFIG_FILE))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let whitelist = parse_whitelist(&p.get_string("DualboxCheckWhitelist", ""));
        Self {
            max_players_per_ip: p.get_int("DualboxCheckMaxPlayersPerIP", 0),
            max_olympiad_participants_per_ip: p
                .get_int("DualboxCheckMaxOlympiadParticipantsPerIP", 0),
            max_event_participants_per_ip: p.get_int("DualboxCheckMaxL2EventParticipantsPerIP", 0),
            count_offline_traders: p.get_bool("DualboxCountOfflineTraders", false),
            whitelist,
        }
    }

    /// The event cap for one address: the global limit plus the whitelist's
    /// extra allowance (Java `max + DUALBOX_CHECK_WHITELIST.getOrDefault(…, 0)`).
    pub fn event_limit_for(&self, ip: &str) -> i32 {
        self.max_event_participants_per_ip + self.whitelist.get(ip).copied().unwrap_or(0)
    }
}

/// `address,extra;address,extra…` — Java splits on `;` then `,`, warns on a
/// malformed entry and skips it rather than failing the whole load.
fn parse_whitelist(raw: &str) -> HashMap<String, i32> {
    let mut out = HashMap::new();
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        match entry.split_once(',') {
            Some((addr, num)) => match num.trim().parse::<i32>() {
                Ok(extra) => {
                    out.insert(addr.trim().to_string(), extra);
                }
                Err(_) => tracing::warn!("DualboxCheck: invalid whitelist count in '{entry}'."),
            },
            None => tracing::warn!("DualboxCheck: invalid whitelist entry '{entry}'."),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_parses_pairs_and_skips_junk() {
        let wl = parse_whitelist("127.0.0.1,2;10.0.0.5,1;bogus;10.0.0.6,x");
        assert_eq!(wl.get("127.0.0.1"), Some(&2));
        assert_eq!(wl.get("10.0.0.5"), Some(&1));
        assert!(!wl.contains_key("bogus"), "no separator → skipped");
        assert!(!wl.contains_key("10.0.0.6"), "non-numeric count → skipped");
    }

    /// The whitelist adds *on top of* the global cap (Java `max + extra`).
    #[test]
    fn the_whitelist_raises_the_event_cap_for_one_address() {
        let cfg = DualboxConfig {
            max_event_participants_per_ip: 1,
            whitelist: parse_whitelist("127.0.0.1,2"),
            ..Default::default()
        };
        assert_eq!(cfg.event_limit_for("127.0.0.1"), 3);
        assert_eq!(cfg.event_limit_for("8.8.8.8"), 1);
    }
}
