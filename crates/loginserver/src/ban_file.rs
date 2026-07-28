//! Port of `LoginServer.loadBanFile` — `banned_ip.cfg`:
//! `address[ duration][ # comment]` per line, duration in ms, 0/absent = permanent.

use tracing::{info, warn};

use crate::controller::ControllerHandle;

pub async fn load(controller: &ControllerHandle) {
    let path = crate::config::BANNED_IP_FILE;
    let Ok(content) = std::fs::read_to_string(path) else {
        warn!("IP Bans file ({path}) is missing or is a directory, skipped.");
        return;
    };

    let mut count = 0u32;
    for (line_number, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let Some(address) = parts.next() else {
            continue;
        };
        let duration = match parts.next() {
            Some(value) => match value.parse::<i64>() {
                Ok(d) => d,
                Err(_) => {
                    warn!(
                        "Skipped: Incorrect ban duration ({value}) on (banned_ip.cfg). Line: {}",
                        line_number + 1
                    );
                    continue;
                }
            },
            None => 0,
        };
        if address.parse::<std::net::Ipv4Addr>().is_err() {
            warn!(
                "Skipped: Invalid address ({address}) on (banned_ip.cfg). Line: {}",
                line_number + 1
            );
            continue;
        }
        controller.add_ban(address, duration).await;
        count += 1;
    }
    info!("Loaded {count} IP Bans.");
}
