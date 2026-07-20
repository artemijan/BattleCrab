//! Shutdown signal handling shared by the login and game server binaries.
//!
//! `tokio::signal::ctrl_c()` alone only catches SIGINT. `systemctl stop` sends
//! SIGTERM, so without a SIGTERM handler a systemd-managed server never gets
//! the chance to shut down gracefully — systemd just waits out
//! `TimeoutStopSec` and then SIGKILLs it.

/// Resolves once either SIGINT (Ctrl+C) or SIGTERM (systemd's default stop
/// signal) is received.
pub async fn wait_for_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("failed to install SIGTERM handler: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
