//! Port of `GameServerTable.java`: server names from `servername.xml`,
//! registered servers from the `gameservers` table, per-server live state.
//! The mutable state lives inside the controller actor; this module holds the
//! data types and loading.

use std::collections::{HashMap, HashSet};

use num_bigint_dig::BigInt;
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// `ServerStatus` constants.
pub mod server_status {
    pub const STATUS_AUTO: i32 = 0x00;
    pub const STATUS_NORMAL: i32 = 0x02;
    pub const STATUS_DOWN: i32 = 0x04;
    pub const STATUS_GM_ONLY: i32 = 0x05;

    pub const SERVER_LIST_STATUS: i32 = 0x01;
    pub const SERVER_TYPE: i32 = 0x02;
    pub const SERVER_LIST_SQUARE_BRACKET: i32 = 0x03;
    pub const MAX_PLAYERS: i32 = 0x04;
    pub const SERVER_AGE: i32 = 0x06;
}

/// `LoginServerFail` reason codes (LS → GS).
pub mod login_server_fail {
    pub const REASON_IP_BANNED: u8 = 1;
    pub const REASON_WRONG_HEXID: u8 = 3;
    pub const REASON_NO_FREE_ID: u8 = 5;
    pub const NOT_AUTHED: u8 = 6;
    pub const REASON_ALREADY_LOGGED_IN: u8 = 7;
}

/// Commands the controller sends to a connected game server's link task.
#[derive(Debug)]
pub enum GsCommand {
    KickPlayer { account: String },
    RequestCharacters { account: String },
}

/// IPv4 subnet ("a.b.c.d/n" or bare address), port of `IPSubnet`.
#[derive(Debug, Clone)]
pub struct Subnet {
    addr: u32,
    mask: u32,
}

impl Subnet {
    pub fn parse(s: &str) -> Option<Self> {
        let (ip, bits) = match s.split_once('/') {
            Some((ip, bits)) => (ip, bits.parse::<u32>().ok()?),
            None => (s, 32),
        };
        let addr: std::net::Ipv4Addr = ip.parse().ok()?;
        let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits.min(32)) };
        Some(Self { addr: u32::from(addr) & mask, mask })
    }

    pub fn matches(&self, ip: std::net::Ipv4Addr) -> bool {
        (u32::from(ip) & self.mask) == self.addr
    }
}

/// One registered game server (`GameServerInfo`).
pub struct GameServerEntry {
    pub id: i32,
    pub hex_id: Vec<u8>,
    pub authed: bool,
    pub status: i32,
    pub port: u16,
    /// (subnet, host) pairs in the order the GS sent them.
    pub addresses: Vec<(Subnet, String)>,
    pub max_players: i32,
    pub server_type: i32,
    pub age_limit: i32,
    pub showing_brackets: bool,
    pub accounts: HashSet<String>,
    /// Command channel to the live link task; None while the server is down.
    pub link: Option<mpsc::Sender<GsCommand>>,
}

impl GameServerEntry {
    pub fn new(id: i32, hex_id: Vec<u8>) -> Self {
        Self {
            id,
            hex_id,
            authed: false,
            status: server_status::STATUS_DOWN,
            port: 0,
            addresses: Vec::new(),
            max_players: 0,
            server_type: 0,
            age_limit: 0,
            showing_brackets: false,
            accounts: HashSet::new(),
            link: None,
        }
    }

    /// `GameServerInfo.setDown()` + thread teardown.
    pub fn set_down(&mut self) {
        self.authed = false;
        self.status = server_status::STATUS_DOWN;
        self.link = None;
        self.accounts.clear();
    }

    /// `getServerAddress(clientAddr)`: first matching subnet's host.
    pub fn address_for(&self, client_ip: std::net::Ipv4Addr) -> Option<&str> {
        self.addresses.iter().find(|(subnet, _)| subnet.matches(client_ip)).map(|(_, host)| host.as_str())
    }

    /// `canLogin`.
    pub fn can_login(&self, access_level: i32) -> bool {
        if self.status == server_status::STATUS_DOWN {
            return false;
        }
        if self.status == server_status::STATUS_GM_ONLY || (self.accounts.len() as i32) >= self.max_players {
            return access_level > 0;
        }
        access_level >= 0
    }
}

/// `stringToHex`: signed BigInteger hex string → two's-complement bytes.
pub fn hexid_from_string(s: &str) -> Option<Vec<u8>> {
    let big = BigInt::parse_bytes(s.trim().as_bytes(), 16)?;
    Some(big.to_signed_bytes_be())
}

/// `hexToString`: two's-complement bytes → signed BigInteger hex string.
pub fn hexid_to_string(bytes: &[u8]) -> String {
    BigInt::from_signed_bytes_be(bytes).to_str_radix(16)
}

pub struct GameServerTable {
    pub server_names: HashMap<i32, String>,
    pub servers: HashMap<i32, GameServerEntry>,
}

pub const SERVER_NAME_FILE: &str = "dist/login/data/servername.xml";

impl GameServerTable {
    pub async fn load(pool: &SqlitePool) -> Self {
        let server_names = load_server_names();
        info!("Loaded {} server names.", server_names.len());

        let mut servers = HashMap::new();
        let rows: Vec<(i32, String)> =
            sqlx::query_as("SELECT server_id, hexid FROM gameservers").fetch_all(pool).await.unwrap_or_default();
        for (id, hexid) in rows {
            match hexid_from_string(&hexid) {
                Some(hex) => {
                    servers.insert(id, GameServerEntry::new(id, hex));
                }
                None => warn!("Invalid hexid for server {id} in gameservers table."),
            }
        }
        info!("Loaded {} registered Game Servers.", servers.len());

        Self { server_names, servers }
    }
}

/// `data/servername.xml`: `<server id="1" name="Bartz"/>` entries.
fn load_server_names() -> HashMap<i32, String> {
    let mut names = HashMap::new();
    let content = match std::fs::read_to_string(SERVER_NAME_FILE) {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not read {SERVER_NAME_FILE}: {e}");
            return names;
        }
    };
    let mut reader = quick_xml::Reader::from_str(&content);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                if e.name().as_ref() == b"server" =>
            {
                let mut id = None;
                let mut name = None;
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => id = String::from_utf8_lossy(&attr.value).parse::<i32>().ok(),
                        b"name" => name = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                        _ => {}
                    }
                }
                if let (Some(id), Some(name)) = (id, name) {
                    names.insert(id, name);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing {SERVER_NAME_FILE}: {e}");
                break;
            }
            _ => {}
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexid_roundtrip_negative() {
        // The stock gameservers row: negative BigInteger.
        let s = "-2ad66b3f483c22be097019f55c8abdf0";
        let bytes = hexid_from_string(s).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(hexid_to_string(&bytes), s);
    }

    #[test]
    fn subnet_matching() {
        let all = Subnet::parse("0.0.0.0/0").unwrap();
        assert!(all.matches("203.0.113.7".parse().unwrap()));
        let lan = Subnet::parse("192.168.1.0/24").unwrap();
        assert!(lan.matches("192.168.1.42".parse().unwrap()));
        assert!(!lan.matches("192.168.2.42".parse().unwrap()));
        let exact = Subnet::parse("127.0.0.1").unwrap();
        assert!(exact.matches("127.0.0.1".parse().unwrap()));
        assert!(!exact.matches("127.0.0.2".parse().unwrap()));
    }
}
