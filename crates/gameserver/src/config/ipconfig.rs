//! Port of `Config.IPConfigData` — the game server's network configuration:
//! the (subnet, host) pairs advertised to the login server so it can hand each
//! client the right game-server address for its network (`ServerList`).
//!
//! Manual mode: `config/ipconfig.xml` (copy of the shipped `ipconfig-default.xml`)
//! lists `<define subnet="a.b.c.d/n" address="x.x.x.x"/>` entries plus a default
//! `<gameserver address="...">`. Automatic mode (no file): enumerate the host's
//! network interfaces into subnet→address pairs and add `0.0.0.0/0` → external IP.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::{info, warn};

pub const IPCONFIG_FILE: &str = "config/ipconfig.xml";

/// Parallel subnet/host lists, exactly like Java's `_subnets`/`_hosts`.
pub struct IpConfig {
    subnets: Vec<String>,
    hosts: Vec<String>,
}

impl IpConfig {
    pub fn load() -> Self {
        let mut cfg = IpConfig { subnets: Vec::new(), hosts: Vec::new() };
        if std::path::Path::new(IPCONFIG_FILE).exists() {
            info!("Network Config: ipconfig.xml exists using manual configuration...");
            cfg.parse_file();
        } else {
            info!("Network Config: ipconfig.xml doesn't exist using automatic configuration...");
            cfg.auto_ip_config();
        }
        cfg
    }

    /// (subnet, host) pairs for `AuthRequest`; Java's empty-list fallbacks apply.
    pub fn pairs(&self) -> Vec<(String, String)> {
        if self.subnets.is_empty() {
            return vec![("0.0.0.0/0".to_string(), "127.0.0.1".to_string())];
        }
        self.subnets.iter().cloned().zip(self.hosts.iter().cloned()).collect()
    }

    /// `parseDocument`: `<gameserver address="..."><define subnet address/></gameserver>`.
    fn parse_file(&mut self) {
        let content = match std::fs::read_to_string(IPCONFIG_FILE) {
            Ok(c) => c,
            Err(e) => {
                warn!("Network Config: cannot read {IPCONFIG_FILE}: {e}");
                return;
            }
        };
        let mut reader = Reader::from_str(&content);
        let mut default_address: Option<String> = None;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = e.name();
                    if name.as_ref().eq_ignore_ascii_case(b"gameserver") {
                        default_address =
                            attr(&e, b"address").or(default_address);
                    } else if name.as_ref().eq_ignore_ascii_case(b"define") {
                        if let (Some(subnet), Some(address)) = (attr(&e, b"subnet"), attr(&e, b"address")) {
                            self.subnets.push(subnet);
                            self.hosts.push(address);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn!("Network Config: error parsing {IPCONFIG_FILE}: {e}");
                    break;
                }
                _ => {}
            }
        }
        // The default (catch-all) server address.
        match default_address {
            Some(addr) => self.hosts.push(addr),
            None => {
                warn!("Failed to load {IPCONFIG_FILE} file - default server address is missing.");
                self.hosts.push("127.0.0.1".to_string());
            }
        }
        self.subnets.push("0.0.0.0/0".to_string());
    }

    /// `autoIpConfig`: derive subnets from local interfaces + external IP.
    fn auto_ip_config(&mut self) {
        let external_ip = fetch_external_ip().unwrap_or_else(|| "127.0.0.1".to_string());

        match if_addrs::get_if_addrs() {
            Ok(interfaces) => {
                for iface in interfaces {
                    let if_addrs::IfAddr::V4(v4) = iface.addr else { continue };
                    let prefix = mask_to_prefix(v4.netmask);
                    let network = mask_network(v4.ip, v4.netmask);
                    let subnet = format!("{network}/{prefix}");
                    let host = v4.ip.to_string();
                    if !self.subnets.contains(&subnet) {
                        self.subnets.push(subnet.clone());
                        self.hosts.push(host.clone());
                        info!("Network Config: Adding new subnet: {subnet} address: {host}");
                    }
                }
            }
            Err(e) => warn!("Network Config: failed to enumerate interfaces: {e}"),
        }

        self.hosts.push(external_ip.clone());
        self.subnets.push("0.0.0.0/0".to_string());
        info!("Network Config: Adding new subnet: 0.0.0.0/0 address: {external_ip}");
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref().eq_ignore_ascii_case(key))
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// Count the leading one-bits of an IPv4 netmask → prefix length.
fn mask_to_prefix(mask: Ipv4Addr) -> u8 {
    u32::from(mask).count_ones() as u8
}

/// `ip & mask` as a dotted-quad string.
fn mask_network(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) & u32::from(mask))
}

/// Best-effort external IP (Java uses `https://checkip.amazonaws.com/`). We use
/// plain HTTP with a short timeout so boot never hangs; `None` on any failure.
fn fetch_external_ip() -> Option<String> {
    let timeout = Duration::from_millis(1500);
    let addr = ("checkip.amazonaws.com", 80).to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    stream
        .write_all(b"GET / HTTP/1.0\r\nHost: checkip.amazonaws.com\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let body = response.split("\r\n\r\n").nth(1)?.trim();
    // Validate it parses as an IPv4 address before trusting it.
    body.parse::<Ipv4Addr>().ok().map(|ip| ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_and_network() {
        assert_eq!(mask_to_prefix("255.255.255.0".parse().unwrap()), 24);
        assert_eq!(mask_to_prefix("255.0.0.0".parse().unwrap()), 8);
        assert_eq!(mask_to_prefix("255.255.255.255".parse().unwrap()), 32);
        assert_eq!(
            mask_network("192.168.1.42".parse().unwrap(), "255.255.255.0".parse().unwrap()),
            "192.168.1.0".parse::<Ipv4Addr>().unwrap()
        );
    }

    #[test]
    fn empty_falls_back_to_localhost() {
        let cfg = IpConfig { subnets: Vec::new(), hosts: Vec::new() };
        assert_eq!(cfg.pairs(), vec![("0.0.0.0/0".to_string(), "127.0.0.1".to_string())]);
    }

    /// Parse the shipped `ipconfig.xml` format (defines + default address).
    fn parse_str(xml: &str) -> IpConfig {
        // Exercise the same parsing logic via a temp file.
        let dir = std::env::temp_dir().join(format!("l2r_ipcfg_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("config")).unwrap();
        let path = dir.join(IPCONFIG_FILE);
        std::fs::write(&path, xml).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let cfg = IpConfig::load();
        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        cfg
    }

    #[test]
    fn manual_config_resolves_per_subnet() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gameserver address="192.168.1.17" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <define subnet="127.0.0.0/8" address="127.0.0.1" />
    <define subnet="192.168.1.0/24" address="192.168.1.17" />
    <define subnet="0.0.0.0/0" address="192.168.1.17" />
</gameserver>"#;
        let pairs = parse_str(xml).pairs();
        // Each define, plus the gameserver default appended as another 0.0.0.0/0.
        assert!(pairs.contains(&("127.0.0.0/8".into(), "127.0.0.1".into())));
        assert!(pairs.contains(&("192.168.1.0/24".into(), "192.168.1.17".into())));
        assert_eq!(pairs.last().unwrap(), &("0.0.0.0/0".to_string(), "192.168.1.17".to_string()));
        // A LAN client (192.168.1.x) must resolve to the LAN address, not localhost.
        let lan = pairs.iter().find(|(s, _)| s == "192.168.1.0/24").map(|(_, h)| h.as_str());
        assert_eq!(lan, Some("192.168.1.17"));
    }
}
