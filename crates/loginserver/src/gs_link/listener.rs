//! Port of `FloodProtectedListener`/`GameServerListener`: accepts GS
//! connections with per-IP flood protection.

use std::collections::HashMap;
use std::sync::Arc;

use crate::context::LoginContext;
use crate::gs_link::connection;
use commons::util;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Default)]
struct ForeignConnection {
    connection_number: i32,
    last_connection: i64,
    is_flooding: bool,
}

pub async fn accept_loop(ctx: Arc<LoginContext>, listener: tokio::net::TcpListener) {
    let flood: Arc<Mutex<HashMap<String, ForeignConnection>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        let Ok((stream, addr)) = listener.accept().await else {
            continue;
        };
        let ip = addr.ip().to_string();

        if ctx.config.enable_flood_protection {
            let mut map = flood.lock().await;
            let now = util::now_millis();
            match map.get_mut(&ip) {
                Some(entry) => {
                    entry.connection_number += 1;
                    let too_fast = (entry.connection_number > ctx.config.fast_connection_limit
                        && (now - entry.last_connection)
                            < ctx.config.normal_connection_time as i64)
                        || (now - entry.last_connection) < ctx.config.fast_connection_time as i64
                        || entry.connection_number > ctx.config.max_connection_per_ip;
                    if too_fast {
                        entry.last_connection = now;
                        entry.connection_number -= 1;
                        if !entry.is_flooding {
                            warn!("Potential Flood from {ip}");
                        }
                        entry.is_flooding = true;
                        drop(stream);
                        continue;
                    }
                    if entry.is_flooding {
                        entry.is_flooding = false;
                        info!("{ip} is not considered as flooding anymore.");
                    }
                    entry.last_connection = now;
                }
                None => {
                    map.insert(
                        ip.clone(),
                        ForeignConnection {
                            connection_number: 1,
                            last_connection: now,
                            is_flooding: false,
                        },
                    );
                }
            }
        }

        let ctx = ctx.clone();
        let flood = flood.clone();
        let conn_ip = ip.clone();
        tokio::spawn(async move {
            connection::handle(ctx, stream, conn_ip.clone()).await;
            // removeFloodProtection on disconnect.
            let mut map = flood.lock().await;
            if let Some(entry) = map.get_mut(&conn_ip) {
                entry.connection_number -= 1;
                if entry.connection_number <= 0 {
                    map.remove(&conn_ip);
                }
            }
        });
    }
}
