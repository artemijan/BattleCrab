//! Port of `FloodProtectedListener`/`GameServerListener`: accepts GS
//! connections with per-IP flood protection. The rules themselves live in
//! [`crate::net_flood`], shared with the client listener.

use std::sync::Arc;

use crate::context::LoginContext;
use crate::gs_link::connection;
use crate::net_flood::ConnectionFloodGuard;

pub async fn accept_loop(ctx: Arc<LoginContext>, listener: tokio::net::TcpListener) {
    let guard = ConnectionFloodGuard::new();

    loop {
        let Ok((stream, addr)) = listener.accept().await else {
            continue;
        };
        let ip = addr.ip().to_string();

        if !guard.accept(&ip, &ctx.config).await {
            drop(stream);
            continue;
        }

        let ctx = ctx.clone();
        let guard = guard.clone();
        tokio::spawn(async move {
            connection::handle(ctx, stream, ip.clone()).await;
            guard.release(&ip).await;
        });
    }
}
