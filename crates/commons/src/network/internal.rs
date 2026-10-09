//! The bind rule for the internal channels — the login server's status channel
//! and both servers' monitor channel (`docs/MONITORING.md` §4).
//!
//! Those channels have no authentication: the network they listen on is the
//! control. They may span machines, but only over a private network, so an
//! address is accepted only if it can't be an internet-facing interface:
//! loopback or a private range. `0.0.0.0`/`::` (every interface, the public
//! one included) and public addresses are refused, which turns "the channels
//! never face the internet" from a convention into something a typo in an ini
//! can't break.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// `Ok` when every address `host` resolves to is internal. A hostname is
/// resolved once, here, the same way the bind that follows resolves it.
pub fn check_bind(host: &str, port: u16) -> Result<(), String> {
    let addrs: Vec<IpAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {host:?}: {e}"))?
        .map(|a| a.ip())
        .collect();
    if addrs.is_empty() {
        return Err(format!("{host:?} resolves to no address"));
    }
    match addrs.iter().find(|ip| !is_internal(**ip)) {
        Some(ip) if ip.is_unspecified() => Err(format!(
            "{host} listens on every interface, the public one included; \
             bind loopback or a private-network address instead"
        )),
        Some(ip) => Err(format!(
            "{ip} is not a loopback or private-network address; \
             internal channels have no authentication and must not face the internet"
        )),
        None => Ok(()),
    }
}

/// Loopback, RFC 1918, the RFC 6598 shared range (Tailscale's `100.64/10`),
/// and IPv6 unique-local `fc00::/7`.
pub fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_internal_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_internal_v4(v4),
            None => v6.is_loopback() || is_unique_local(v6),
        },
    }
}

fn is_internal_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback() || ip.is_private() || (a == 100 && (64..128).contains(&b))
}

/// `Ipv6Addr::is_unique_local` is still unstable.
fn is_unique_local(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xfe00 == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_private_ranges_are_internal() {
        for ok in [
            "127.0.0.1",
            "10.0.0.2",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.10",
            "100.64.0.1",
            "100.127.255.255",
            "::1",
            "fd12:3456::1",
            "::ffff:10.0.0.2",
        ] {
            assert!(is_internal(ok.parse().unwrap()), "{ok}");
        }
    }

    #[test]
    fn public_and_wildcard_addresses_are_not() {
        for bad in [
            "0.0.0.0",
            "::",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "203.0.113.9",
            "2001:db8::1",
            "::ffff:8.8.8.8",
            "169.254.1.1",
        ] {
            assert!(!is_internal(bad.parse().unwrap()), "{bad}");
        }
    }

    #[test]
    fn the_check_resolves_names_and_explains_a_refusal() {
        assert!(check_bind("127.0.0.1", 7779).is_ok());
        assert!(check_bind("localhost", 7779).is_ok());
        assert!(check_bind("10.1.2.3", 0).is_ok());
        assert!(
            check_bind("0.0.0.0", 7779)
                .unwrap_err()
                .contains("every interface")
        );
        assert!(
            check_bind("8.8.8.8", 7779)
                .unwrap_err()
                .contains("not a loopback or private")
        );
    }
}
