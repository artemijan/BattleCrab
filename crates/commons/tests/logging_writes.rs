//! End-to-end check on the logging pipeline: config in, files on disk out.
//!
//! The unit tests next to the module only cover config parsing, which would
//! still pass if the layers were wired up wrong and nothing was ever written.
//! This test asserts the part that actually matters — that a line logged after
//! `init` reaches the file, and that the error file really is filtered.
//!
//! **One `init` per process.** Installing the global subscriber twice panics,
//! and the repo runs tests under `cargo nextest`, which is process-per-test —
//! so this file deliberately contains exactly one test.

use std::time::{Duration, Instant};

/// Reads `path` until it contains `needle`, or the deadline passes.
///
/// `WorkerGuard`'s drop joins the writer thread, so the content is normally
/// there immediately; the retry only keeps the test off a knife edge if the
/// flush and the filesystem disagree about timing.
fn wait_for_contents(path: &std::path::Path, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = String::new();
    while Instant::now() < deadline {
        last = std::fs::read_to_string(path).unwrap_or_default();
        if last.contains(needle) {
            return last;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    last
}

fn find_file(dir: &std::path::Path, predicate: impl Fn(&str) -> bool) -> std::path::PathBuf {
    // Symlinks are skipped so the "latest" link cannot stand in for the real
    // file here — it is asserted on separately, and picking it up by read_dir
    // order would make this test pass with a dangling link.
    let listing: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("log dir {} unreadable: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| !e.path().is_symlink())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let found = listing
        .iter()
        .find(|name| predicate(name))
        .unwrap_or_else(|| panic!("no matching log file in {}: {listing:?}", dir.display()));
    dir.join(found)
}

#[test]
fn writes_the_diagnostic_and_error_files() {
    let dir = std::env::temp_dir().join(format!("l2r-logging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");

    // Run from *inside* the temp dir with an empty root, so the log directory
    // the appender receives is the relative `log` — which is what the servers
    // actually pass (`dist/game/log`, `dist/login/log`) and the only condition
    // under which the latest-symlink bug appears. Handing it the absolute temp
    // path instead would make the symlink assertion below vacuous.
    //
    // Changing the process-wide cwd is safe here only because this binary holds
    // exactly one test and nextest runs it in its own process.
    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&dir).expect("enter temp dir");

    let config = commons::logging::LoggingConfig {
        // Keep the test's own output clean, and skip the reporter thread.
        console_enable: false,
        drop_report_seconds: 0,
        ..Default::default()
    };
    let guard = commons::logging::init_with(&config, "", "test_server", false);

    tracing::info!(marker = "info-marker", "a diagnostic line");
    tracing::error!(marker = "error-marker", "an incident");

    // Flushes both writer threads.
    drop(guard);

    let log_dir = std::path::PathBuf::from("log");

    // The main file: JSON by default, and it carries both levels.
    let main = find_file(&log_dir, |n| {
        n.starts_with("test_server.") && n.ends_with(".json")
    });
    let body = wait_for_contents(&main, "info-marker");
    assert!(
        body.contains("info-marker"),
        "info line missing from {}: {body}",
        main.display()
    );
    assert!(
        body.lines()
            .next()
            .is_some_and(|l| l.trim_start().starts_with('{')),
        "FileFormat = json should produce JSON lines, got: {body}"
    );

    // The error file: WARN and above only. An info line leaking in here would
    // mean the level filter is not attached to the layer.
    let err = find_file(&log_dir, |n| {
        n.starts_with("test_server_error.") && n.ends_with(".log")
    });
    let err_body = wait_for_contents(&err, "error-marker");
    assert!(
        err_body.contains("error-marker"),
        "error line missing from {}: {err_body}",
        err.display()
    );
    assert!(
        !err_body.contains("info-marker"),
        "the error file must not receive INFO lines: {err_body}"
    );

    // The stable path an operator tails, since rotation dates the real
    // filenames. `tracing-appender` builds the link target by joining the log
    // directory onto the filename and then places the link *inside* that same
    // directory, so the target only resolves when the directory handed to the
    // appender is absolute. Hand it a relative one and every link dangles —
    // which is exactly what this asserts against.
    #[cfg(unix)]
    {
        let latest = log_dir.join("test_server.json");
        assert!(
            latest.is_symlink(),
            "expected a latest-symlink at {}",
            latest.display()
        );
        let via_link = std::fs::read_to_string(&latest).unwrap_or_else(|e| {
            panic!(
                "the latest-symlink does not resolve ({e}); it points at {:?}",
                std::fs::read_link(&latest)
            )
        });
        assert!(
            via_link.contains("info-marker"),
            "reading through the latest-symlink gave: {via_link}"
        );
    }

    // Leave the cwd as we found it before removing the directory underneath it.
    let _ = std::env::set_current_dir(&original_cwd);
    let _ = std::fs::remove_dir_all(&dir);
}
