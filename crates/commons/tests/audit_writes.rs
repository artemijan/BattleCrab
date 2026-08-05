//! End-to-end check on the audit sink: records in, NDJSON on disk out.
//!
//! **One `init` per process.** The sink lives in a `OnceLock`, and the repo
//! runs tests under `cargo nextest`, which is process-per-test — so this file
//! deliberately contains exactly one test.

use std::time::{Duration, Instant};

use commons::audit::{self, AuditConfig, Category};
use serde_json::Value;

fn wait_for(path: &std::path::Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = String::new();
    while Instant::now() < deadline {
        last = std::fs::read_to_string(path).unwrap_or_default();
        if !last.trim().is_empty() {
            return last;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    last
}

fn find(dir: &std::path::Path, stem: &str) -> std::path::PathBuf {
    let listing: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("audit dir {} unreadable: {e}", dir.display()))
        .filter_map(|e| e.ok())
        // Skip the "latest" symlinks so a dangling one cannot stand in for the
        // real file; the symlink is asserted on separately below.
        .filter(|e| !e.path().is_symlink())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let found = listing
        .iter()
        .find(|n| n.starts_with(&format!("{stem}.")))
        .unwrap_or_else(|| panic!("no {stem} file in {}: {listing:?}", dir.display()));
    dir.join(found)
}

#[test]
fn writes_ndjson_per_category() {
    let dir = std::env::temp_dir().join(format!("l2r-audit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    // Run from inside the temp dir with a relative audit directory — the same
    // shape the servers use (`dist/game/log/audit`), and the only condition
    // under which a bad symlink target shows up.
    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&dir).expect("enter temp dir");

    let config = AuditConfig {
        directory: "log/audit".to_string(),
        ..Default::default()
    };
    let guard = audit::init("", &config);

    audit::record(
        Category::Chat,
        serde_json::json!({"char_name": "Tester", "text": "hello"}),
    );
    audit::record(
        Category::GmAudit,
        serde_json::json!({"gm": "Admin", "command": "//help"}),
    );

    // Joins the writer, so everything queued is on disk after this.
    drop(guard);

    let audit_dir = std::path::PathBuf::from("log/audit");

    // Each category gets its own file — a chat record must not land in the GM
    // file, which is the whole point of separate categories.
    let chat = wait_for(&find(&audit_dir, "chat"));
    assert!(chat.contains("Tester"), "chat record missing: {chat}");
    assert!(
        !chat.contains("//help"),
        "the GM record leaked into the chat file: {chat}"
    );

    let gm = wait_for(&find(&audit_dir, "gmaudit"));
    assert!(gm.contains("//help"), "gm record missing: {gm}");

    // NDJSON: one complete JSON object per line, with the timestamp the sink
    // adds so no call site has to remember one.
    let first = chat.lines().next().expect("at least one line");
    let parsed: Value =
        serde_json::from_str(first).unwrap_or_else(|e| panic!("not valid JSON ({e}): {first}"));
    assert_eq!(parsed["char_name"], "Tester");
    assert!(
        parsed
            .get("ts")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t.contains('T')),
        "record has no RFC3339 ts: {first}"
    );

    // The stable path an operator tails. `tracing-appender` builds the target
    // by joining the directory onto the filename and puts the link in that same
    // directory, so this only resolves because the sink canonicalises first.
    #[cfg(unix)]
    {
        let latest = audit_dir.join("chat.ndjson");
        assert!(
            latest.is_symlink(),
            "no latest-symlink at {}",
            latest.display()
        );
        let via_link = std::fs::read_to_string(&latest).unwrap_or_else(|e| {
            panic!(
                "the latest-symlink dangles ({e}); it points at {:?}",
                std::fs::read_link(&latest)
            )
        });
        assert!(via_link.contains("Tester"));
    }

    let _ = std::env::set_current_dir(&original_cwd);
    let _ = std::fs::remove_dir_all(&dir);
}
