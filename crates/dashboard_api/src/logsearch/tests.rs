use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// A scratch datapack root with `log/` and `log/audit/`, removed on drop.
struct Root(PathBuf);

impl Root {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "dashboard-logsearch-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("log/audit")).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::write(&p, body).unwrap();
        p
    }

    fn source(&self) -> Source {
        Source {
            service: "game_server".into(),
            log_dir: self.0.join("log"),
            audit_dir: self.0.join("log/audit"),
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ms(s: &str) -> i64 {
    time::parse_rfc3339(s.as_bytes()).unwrap()
}

fn diag(ts: &str, level: &str, msg: &str) -> String {
    format!(
        "{{\"timestamp\":\"{ts}\",\"level\":\"{level}\",\"message\":\"{msg}\",\"target\":\"t\",\"span\":{{\"account\":\"bob\"}}}}\n"
    )
}

fn query(stream: Stream, from: &str, to: &str) -> Query {
    Query {
        stream,
        from: ms(from),
        to: ms(to),
        pattern: None,
        level: None,
        limit: 100,
        cursor: None,
    }
}

fn wide() -> Bounds {
    Bounds {
        max_bytes: u64::MAX,
        deadline: Duration::from_secs(30),
    }
}

fn messages(o: &Outcome) -> Vec<String> {
    o.hits
        .iter()
        .map(|h| match &h.line {
            Line::Structured(v) => v["message"].as_str().unwrap_or("?").to_string(),
            Line::Raw(s) => s.clone(),
        })
        .collect()
}

/// Two days of diagnostic log, plus the noise a real directory holds.
fn two_days(root: &Root) {
    root.write(
        "log/game_server.2026-08-13.json",
        &[
            diag("2026-08-13T10:00:00Z", "INFO", "d1 boot"),
            diag("2026-08-13T23:59:00Z", "WARN", "d1 late warning"),
        ]
        .concat(),
    );
    let today = root.write(
        "log/game_server.2026-08-14.json",
        &[
            diag("2026-08-14T00:00:01Z", "INFO", "d2 first"),
            diag("2026-08-14T08:00:00Z", "ERROR", "d2 Kaboom"),
            diag("2026-08-14T09:00:00Z", "DEBUG", "d2 chatter"),
        ]
        .concat(),
    );
    // The latest symlink to today's file: must not be scanned twice.
    #[cfg(unix)]
    std::os::unix::fs::symlink(&today, root.0.join("log/game_server.json")).unwrap();
    // Same directory, other streams and services: never matched.
    root.write("log/game_server_error.2026-08-14.log", "x\n");
    root.write(
        "log/login_server.2026-08-14.json",
        &diag("2026-08-14T01:00:00Z", "INFO", "other service"),
    );
}

#[test]
fn files_are_enumerated_newest_first_without_the_latest_symlink() {
    let root = Root::new();
    two_days(&root);
    let names: Vec<String> = root
        .source()
        .files(Stream::Diagnostic)
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(
        names,
        vec!["game_server.2026-08-14.json", "game_server.2026-08-13.json"]
    );
    let errors: Vec<String> = root
        .source()
        .files(Stream::Error)
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(errors, vec!["game_server_error.2026-08-14.log"]);
}

#[test]
fn an_undated_real_file_is_what_rotation_never_writes_and_is_searched() {
    let root = Root::new();
    root.write(
        "log/game_server.json",
        &diag("2026-08-14T00:00:01Z", "INFO", "never rotated"),
    );
    let o = search(
        &root.source(),
        &query(
            Stream::Diagnostic,
            "2026-08-14T00:00:00Z",
            "2026-08-15T00:00:00Z",
        ),
        &wide(),
    );
    assert_eq!(messages(&o), vec!["never rotated"]);
}

#[test]
fn results_come_newest_first_across_files_within_the_range() {
    let root = Root::new();
    two_days(&root);
    let o = search(
        &root.source(),
        &query(
            Stream::Diagnostic,
            "2026-08-13T12:00:00Z",
            "2026-08-14T08:30:00Z",
        ),
        &wide(),
    );
    assert_eq!(
        messages(&o),
        vec!["d2 Kaboom", "d2 first", "d1 late warning"]
    );
    assert_eq!(o.stopped, None);
    assert!(o.cursor.is_none());
    assert!(!o.truncated);
    // Span fields survive: the line is passed through, not mapped to a DTO.
    let Line::Structured(v) = &o.hits[0].line else {
        panic!("diagnostic lines are JSON")
    };
    assert_eq!(v["span"]["account"], "bob");
    assert_eq!(o.hits[0].ts, Some(ms("2026-08-14T08:00:00Z")));
}

#[test]
fn a_range_wholly_inside_today_never_opens_yesterday() {
    let root = Root::new();
    two_days(&root);
    let o = search(
        &root.source(),
        &query(
            Stream::Diagnostic,
            "2026-08-14T07:00:00Z",
            "2026-08-14T23:00:00Z",
        ),
        &wide(),
    );
    assert_eq!(o.files_scanned, vec!["game_server.2026-08-14.json"]);
}

#[test]
fn text_and_regex_patterns_and_the_level_floor() {
    let root = Root::new();
    two_days(&root);
    let day = ("2026-08-13T00:00:00Z", "2026-08-15T00:00:00Z");

    let mut q = query(Stream::Diagnostic, day.0, day.1);
    q.pattern = Some(
        regex::bytes::RegexBuilder::new(&regex::escape("kaboom"))
            .case_insensitive(true)
            .build()
            .unwrap(),
    );
    assert_eq!(
        messages(&search(&root.source(), &q, &wide())),
        vec!["d2 Kaboom"]
    );

    q.pattern = Some(regex::bytes::Regex::new(r"d[12] (boot|first)").unwrap());
    assert_eq!(
        messages(&search(&root.source(), &q, &wide())),
        vec!["d2 first", "d1 boot"]
    );

    let mut q = query(Stream::Diagnostic, day.0, day.1);
    q.level = Some(Level::Warn);
    assert_eq!(
        messages(&search(&root.source(), &q, &wide())),
        vec!["d2 Kaboom", "d1 late warning"]
    );
}

#[test]
fn error_log_lines_are_raw_and_filter_on_their_level_token() {
    let root = Root::new();
    root.write(
        "log/game_server_error.2026-08-14.log",
        "2026-08-14T01:00:00.000001Z  WARN gameserver::x: slow tick\n\
         2026-08-14T02:00:00.000001Z ERROR gameserver::y: panicked at foo\n\
         stack backtrace line without a stamp\n",
    );
    let mut q = query(
        Stream::Error,
        "2026-08-14T00:00:00Z",
        "2026-08-15T00:00:00Z",
    );
    q.level = Some(Level::Error);
    let o = search(&root.source(), &q, &wide());
    let got = messages(&o);
    // The unstamped continuation is kept with its entry.
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0], "stack backtrace line without a stamp");
    assert!(got[1].contains("panicked"));
    assert!(matches!(o.hits[0].line, Line::Raw(_)));
}

#[test]
fn audit_streams_read_the_ts_field() {
    let root = Root::new();
    root.write(
        "log/audit/chat.2026-08-14.ndjson",
        "{\"event\":\"say\",\"text\":\"he said \\\"ts\\\":\\\"1999\\\" lol\",\"ts\":\"2026-08-14T05:00:00Z\"}\n",
    );
    let o = search(
        &root.source(),
        &query(
            Stream::Audit(Category::Chat),
            "2026-08-14T04:00:00Z",
            "2026-08-14T06:00:00Z",
        ),
        &wide(),
    );
    assert_eq!(o.hits.len(), 1);
    // The escaped `"ts":"` inside the chat text is not mistaken for the stamp.
    assert_eq!(o.hits[0].ts, Some(ms("2026-08-14T05:00:00Z")));
}

#[test]
fn paging_by_cursor_has_no_gaps_and_no_repeats() {
    let root = Root::new();
    two_days(&root);
    let mut q = query(
        Stream::Diagnostic,
        "2026-08-13T00:00:00Z",
        "2026-08-15T00:00:00Z",
    );
    q.limit = 2;
    let mut seen = Vec::new();
    let mut pages = 0;
    loop {
        let o = search(&root.source(), &q, &wide());
        seen.extend(messages(&o));
        pages += 1;
        match o.cursor {
            Some(c) => q.cursor = Some(Cursor::decode(&c).unwrap()),
            None => break,
        }
        assert!(pages < 10, "cursor is not advancing");
    }
    assert_eq!(
        seen,
        vec![
            "d2 chatter",
            "d2 Kaboom",
            "d2 first",
            "d1 late warning",
            "d1 boot"
        ]
    );
}

#[test]
fn hitting_the_byte_bound_is_truncated_and_resumes_to_completion() {
    let root = Root::new();
    let body: String = (0..200)
        .map(|i| {
            diag(
                &format!("2026-08-14T00:{:02}:{:02}Z", i / 60, i % 60),
                "INFO",
                &format!("m{i}"),
            )
        })
        .collect();
    root.write("log/game_server.2026-08-14.json", &body);
    let mut q = query(
        Stream::Diagnostic,
        "2026-08-14T00:00:00Z",
        "2026-08-15T00:00:00Z",
    );
    q.limit = 500;
    let tight = Bounds {
        max_bytes: 1,
        deadline: Duration::from_secs(30),
    };
    let first = search(&root.source(), &q, &tight);
    assert!(first.truncated);
    assert_eq!(first.stopped, Some(StopReason::MaxBytes));
    let mut seen = messages(&first);
    let mut cursor = first.cursor;
    while let Some(c) = cursor {
        q.cursor = Some(Cursor::decode(&c).unwrap());
        let o = search(&root.source(), &q, &wide());
        seen.extend(messages(&o));
        cursor = o.cursor;
    }
    let expected: Vec<String> = (0..200).rev().map(|i| format!("m{i}")).collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_cursor_into_a_rotated_away_file_ends_the_search() {
    let root = Root::new();
    two_days(&root);
    let mut q = query(
        Stream::Diagnostic,
        "2026-08-13T00:00:00Z",
        "2026-08-15T00:00:00Z",
    );
    q.cursor = Some(Cursor {
        file: "game_server.2026-07-01.json".into(),
        offset: 10,
    });
    let o = search(&root.source(), &q, &wide());
    assert!(o.hits.is_empty() && o.cursor.is_none());
}

#[test]
fn a_cursor_names_a_file_only_by_comparison_never_by_path() {
    let root = Root::new();
    two_days(&root);
    let outside = root.write(
        "secret.json",
        &diag("2026-08-14T00:00:01Z", "INFO", "leaked"),
    );
    let mut q = query(
        Stream::Diagnostic,
        "2026-08-13T00:00:00Z",
        "2026-08-15T00:00:00Z",
    );
    for name in [
        "../secret.json".to_string(),
        outside.display().to_string(),
        "game_server.2026-08-14.json/../../secret.json".to_string(),
    ] {
        q.cursor = Some(Cursor {
            file: name.clone(),
            offset: 0,
        });
        assert!(
            search(&root.source(), &q, &wide()).hits.is_empty(),
            "{name}"
        );
    }
}

#[test]
fn cursors_round_trip_and_garbage_does_not_decode() {
    let c = Cursor {
        file: "game_server.2026-08-14.json".into(),
        offset: 123_456,
    };
    assert_eq!(Cursor::decode(&c.encode()), Some(c));
    assert_eq!(Cursor::decode("!!!"), None);
    assert_eq!(Cursor::decode(""), None);
}

#[test]
fn streams_are_a_closed_set() {
    assert_eq!(Stream::parse("diagnostic"), Some(Stream::Diagnostic));
    assert_eq!(
        Stream::parse("audit:chat"),
        Some(Stream::Audit(Category::Chat))
    );
    assert_eq!(
        Stream::parse("audit:gmaudit"),
        Some(Stream::Audit(Category::GmAudit))
    );
    for bad in ["audit:", "audit:../../etc/passwd", "Diagnostic", "chat", ""] {
        assert_eq!(Stream::parse(bad), None, "{bad}");
    }
    for s in Stream::all() {
        assert_eq!(Stream::parse(&s.name()), Some(s));
    }
}

#[test]
fn sources_derive_directories_from_each_root() {
    let s = sources("game_server=dist/game, dashboard_api=dist/game").unwrap();
    assert_eq!(s[0].log_dir, Path::new("dist/game").join("log"));
    assert_eq!(s[0].audit_dir, Path::new("dist/game").join("log/audit"));
    assert_eq!(
        s[1].audit_dir,
        Path::new("dist/game").join(DASHBOARD_AUDIT_DIR)
    );
    assert!(sources("").unwrap().is_empty());
    assert!(sources("game_server").is_err());
    assert!(sources("../x=dist/game").is_err());
    assert!(sources("a=x,a=y").is_err());
}
