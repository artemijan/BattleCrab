//! The login server's `accounting` audit: every authentication attempt is
//! recorded with its outcome.
//!
//! This exists because the sink was once started here and wired to nothing —
//! `dist/login/log/audit/` was created, the boot line said "audit records
//! enabled", and not a single record was ever written. An empty audit directory
//! that implies coverage is worse than no audit at all, so the guarantee is
//! pinned by driving real logins through the real server.
//!
//! **One `init` per process.** The sink lives in a `OnceLock` and the repo runs
//! tests under `cargo nextest` (process-per-test), so this file holds exactly
//! one test.

mod common;

use std::time::{Duration, Instant};

use common::{login, start_server, test_config};
use commons::audit::{self, AuditConfig};

fn accounting_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| !e.path().is_symlink())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("accounting."))
        })
}

#[tokio::test]
async fn every_login_attempt_is_recorded_with_its_outcome() {
    let tmp = std::env::temp_dir().join(format!("l2r-login-audit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("temp dir");

    // An absolute root, so the audit files land in the temp dir without
    // disturbing the cwd the server harness runs from.
    let root = format!("{}/", tmp.display());
    let guard = audit::init(&root, &AuditConfig::default());

    let server = start_server(test_config()).await;

    // Auto-created, so this one succeeds.
    let (_c, reply) = login(server.addr, "audituser", "secret").await;
    assert_eq!(reply[0], 0x03, "expected LoginOk for the first attempt");

    // Same account, wrong password — the failure that matters most here.
    let (_c2, reply) = login(server.addr, "audituser", "wrong").await;
    assert_eq!(reply[0], 0x01, "expected LoginFail for the bad password");

    // Joins the writer so everything queued is on disk.
    drop(guard);

    let audit_dir = tmp.join("log/audit");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut body = String::new();
    while Instant::now() < deadline {
        if let Some(path) = accounting_file(&audit_dir) {
            body = std::fs::read_to_string(&path).unwrap_or_default();
            if body.lines().count() >= 2 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    assert!(
        !body.is_empty(),
        "no accounting records at {} — the sink is wired to nothing again",
        audit_dir.display()
    );
    assert!(
        body.contains("\"result\":\"success\""),
        "the successful login was not recorded: {body}"
    );
    assert!(
        body.contains("\"result\":\"access_failed\""),
        "the failed login was not recorded — a failed-login pattern is the \
         main thing this file exists to reconstruct: {body}"
    );
    assert!(
        body.contains("audituser"),
        "records must name the account: {body}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
