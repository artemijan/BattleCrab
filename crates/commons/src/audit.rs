//! Audit records — the never-dropped half of the logging story.
//!
//! Port of the category loggers in the Java server's `log.cfg`: `accounting`,
//! `chat`, `item`, `enchant.items` / `enchant.skills`, `olympiad`, `audit` and
//! the separate `GMAudit` writer. Java gave each its own rotating file; so do
//! we, as newline-delimited JSON under `log/audit/`.
//!
//! ## Why this is not [`crate::logging`]
//!
//! Diagnostics are droppable: when the server is busy enough to fill the write
//! buffer, losing lines is the correct trade against tick budget. Audit records
//! are the opposite kind of object. They are *records* — low-volume,
//! high-value, and read months later to answer "this player says his +12
//! weapon vanished". Routing them through a lossy sink would discard precisely
//! the evidence a busy server is most likely to be asked for, so this sink
//! never drops:
//!
//! * the queue is bounded, but a full queue makes the caller **wait** rather
//!   than discarding the record (see [`record`]). Audit volume is a rounding
//!   error next to diagnostics, so in practice the queue never fills; if it
//!   ever does, a brief stall is the lesser harm.
//! * the writer thread is joined on shutdown, so nothing queued is lost.
//!
//! ## Why files rather than the game database
//!
//! `interlude_classic.db` is opened by the login server, the game server and
//! the dashboard at once. SQLite allows one writer per *file*, and the
//! connection string sets `busy_timeout=5000` — so an audit insert contending
//! with a player-persistence flush does not fail fast, it parks the caller for
//! up to five seconds. Retention would be worse: pruning rows needs `VACUUM` to
//! return space, and `VACUUM` takes an exclusive lock on the whole database.
//! Separate NDJSON files make retention a file deletion instead.
//!
//! ## Denormalised on purpose
//!
//! Records carry `char_name` / `account` inline rather than an id to join on.
//! An audit record must read as it did *then*: a later rename, or a deleted
//! character, must not rewrite history.

use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;

use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use crate::config::PropertiesParser;

/// The audit categories, one file each. Names match the Java logger names so
/// an operator moving from the Java server finds the same filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Connect, disconnect, login, logout, character create/delete/restore.
    Accounting,
    /// Public and private chat (Java: `Say2`, `RequestSendFriendMsg`).
    Chat,
    /// Item ownership and count changes (Java: `Item.setOwnerId`/`changeCount`).
    Item,
    /// Item and skill enchant attempts (Java: `enchant.items`/`enchant.skills`).
    Enchant,
    /// Olympiad match results.
    Olympiad,
    /// Illegal-action reports (Java: `IllegalPlayerActionTask`, whose logger is
    /// plain `audit`).
    ///
    /// Written by the game server's illegal-action task
    /// (`game_loop::punishment::on_illegal_action_punish`, the port of Java's
    /// `IllegalPlayerActionTask`) 5 seconds after a packet-validation guard
    /// trips, alongside the configured `DefaultPunish` kick/ban/jail.
    Illegal,
    /// GM command usage (Java: `GMAudit.auditGMAction`).
    GmAudit,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Self::Accounting,
        Self::Chat,
        Self::Item,
        Self::Enchant,
        Self::Olympiad,
        Self::Illegal,
        Self::GmAudit,
    ];

    /// Filename stem, matching the Java logger/handler names.
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Accounting => "accounting",
            Self::Chat => "chat",
            Self::Item => "item",
            Self::Enchant => "enchant",
            Self::Olympiad => "olympiad",
            Self::Illegal => "audit",
            Self::GmAudit => "gmaudit",
        }
    }
}

/// Everything the `Audit*` keys in `Logging.ini` control.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub enable: bool,
    /// Directory for the NDJSON files, relative to the datapack root.
    pub directory: String,
    /// How many rotated files to keep. Rotation is daily, so this is a window
    /// in days — deliberately far longer than the diagnostic retention, since
    /// these are the records a support question reaches for.
    pub retention: usize,
    /// Bounded queue depth before a writer stall starts blocking callers.
    pub queue_capacity: usize,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable: true,
            directory: "log/audit".to_string(),
            retention: 180,
            queue_capacity: 16_384,
        }
    }
}

impl AuditConfig {
    /// Reads the `Audit*` keys out of the same `Logging.ini` the diagnostic
    /// pipeline uses — one file for the whole logging story.
    pub fn load(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(
            root,
            crate::logging::LOGGING_CONFIG_FILE,
        ))
    }

    pub fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            enable: p.get_bool("AuditEnable", d.enable),
            directory: p.get_string("AuditDirectory", &d.directory),
            retention: p.get_int("AuditRetention", d.retention as i32).max(0) as usize,
            queue_capacity: p
                .get_int("AuditQueueCapacity", d.queue_capacity as i32)
                .max(1) as usize,
        }
    }
}

enum Msg {
    Line(Category, String),
    Shutdown,
}

struct Sink {
    tx: SyncSender<Msg>,
}

static SINK: OnceLock<Sink> = OnceLock::new();

/// Counts how many times a caller had to wait for the writer. Non-zero means
/// the queue filled — worth surfacing, because the cost of never dropping is
/// paid here rather than in lost records.
static BLOCKED: AtomicU64 = AtomicU64::new(0);

/// Number of records written since start, for the metrics endpoint.
static WRITTEN: AtomicU64 = AtomicU64::new(0);

pub fn blocked_count() -> u64 {
    BLOCKED.load(Ordering::Relaxed)
}

pub fn written_count() -> u64 {
    WRITTEN.load(Ordering::Relaxed)
}

/// Joins the writer thread on drop so queued records reach disk.
///
/// Hold it for the life of the process, exactly like the logging guard.
#[must_use = "dropping the guard stops the audit writer and loses queued records"]
pub struct AuditGuard {
    handle: Option<JoinHandle<()>>,
}

impl Drop for AuditGuard {
    fn drop(&mut self) {
        if let Some(sink) = SINK.get() {
            // Best-effort: if the writer already died the send fails and the
            // join below returns immediately.
            let _ = sink.tx.send(Msg::Shutdown);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Starts the audit writer. Call once, after [`crate::logging::init`] so that
/// any complaint here has somewhere to go.
pub fn init(root: &str, config: &AuditConfig) -> AuditGuard {
    if !config.enable {
        tracing::info!("audit: disabled by AuditEnable");
        return AuditGuard { handle: None };
    }

    let relative_dir = format!("{root}{}", config.directory);
    if let Err(e) = std::fs::create_dir_all(&relative_dir) {
        tracing::error!("audit: could not create {relative_dir}: {e} — audit records are OFF");
        return AuditGuard { handle: None };
    }
    // Absolute, for the same reason as the diagnostic log: `tracing-appender`
    // joins the directory onto the symlink target, so a relative directory
    // yields a dangling link.
    let dir = std::fs::canonicalize(&relative_dir)
        .map(|p| p.display().to_string())
        .unwrap_or(relative_dir);

    let (tx, rx) = sync_channel::<Msg>(config.queue_capacity);
    let retention = config.retention;
    let writer_dir = dir.clone();

    let handle = std::thread::Builder::new()
        .name("audit-writer".to_string())
        .spawn(move || {
            let mut files: HashMap<Category, RollingFileAppender> = HashMap::new();
            while let Ok(msg) = rx.recv() {
                let (category, line) = match msg {
                    Msg::Line(c, l) => (c, l),
                    Msg::Shutdown => break,
                };
                let file = match files.entry(category) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        match open_appender(&writer_dir, category, retention) {
                            Ok(f) => e.insert(f),
                            Err(err) => {
                                tracing::error!(
                                    "audit: cannot open the {} file: {err}",
                                    category.file_stem()
                                );
                                continue;
                            }
                        }
                    }
                };
                if let Err(e) = writeln!(file, "{line}") {
                    tracing::error!("audit: write to {} failed: {e}", category.file_stem());
                } else {
                    WRITTEN.fetch_add(1, Ordering::Relaxed);
                }
            }
            // Drain whatever is still queued behind the shutdown marker.
            while let Ok(Msg::Line(category, line)) = rx.try_recv() {
                if let Some(file) = files.get_mut(&category) {
                    let _ = writeln!(file, "{line}");
                }
            }
        })
        .ok();

    if handle.is_some() {
        let _ = SINK.set(Sink { tx });
        tracing::info!(dir = %dir, retention, "audit records enabled");
    } else {
        tracing::error!("audit: could not spawn the writer thread — audit records are OFF");
    }
    AuditGuard { handle }
}

/// Writes one record. A no-op when the sink was never started, so tests and
/// tools can call audited code paths without wiring anything up.
///
/// `value` should be a JSON object; a `ts` field is added here so no caller has
/// to remember one.
///
/// **This can block.** A full queue waits for the writer instead of discarding
/// the record — the whole point of the category. See the module docs.
pub fn record(category: Category, mut value: Value) {
    let Some(sink) = SINK.get() else {
        return;
    };
    if let Value::Object(map) = &mut value {
        map.insert("ts".to_string(), Value::String(timestamp()));
    }
    let line = value.to_string();
    match sink.tx.try_send(Msg::Line(category, line)) {
        Ok(()) => {}
        Err(TrySendError::Full(msg)) => {
            BLOCKED.fetch_add(1, Ordering::Relaxed);
            // Deliberately blocking. Dropping here would silently lose exactly
            // the record someone will ask for later.
            let _ = sink.tx.send(msg);
        }
        // The writer is gone; nothing useful left to do.
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn open_appender(
    dir: &str,
    category: Category,
    retention: usize,
) -> Result<RollingFileAppender, tracing_appender::rolling::InitError> {
    let stem = category.file_stem();
    let builder = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(stem)
        .filename_suffix("ndjson")
        .max_log_files(retention);
    #[cfg(unix)]
    let builder = builder.latest_symlink(format!("{stem}.ndjson"));
    builder.build(dir)
}

fn timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_have_distinct_java_matching_stems() {
        let mut stems: Vec<&str> = Category::ALL.iter().map(|c| c.file_stem()).collect();
        stems.sort_unstable();
        let count = stems.len();
        stems.dedup();
        assert_eq!(stems.len(), count, "two categories share a filename");
        // The Java logger names, which operators will be looking for.
        assert!(stems.contains(&"accounting"));
        assert!(stems.contains(&"gmaudit"));
        assert!(stems.contains(&"audit"));
    }

    #[test]
    fn record_without_init_is_a_no_op() {
        // Must not panic: most tests never start the sink.
        record(Category::Chat, serde_json::json!({"msg": "hello"}));
    }

    #[test]
    fn config_defaults_keep_audit_far_longer_than_diagnostics() {
        let config = AuditConfig::default();
        assert!(config.enable);
        assert!(
            config.retention > 30,
            "audit retention is a support window, not a debugging one"
        );
    }

    #[test]
    fn config_parses_and_clamps() {
        let p = PropertiesParser::from_content(
            "Logging.ini",
            "AuditEnable = False\nAuditRetention = -1\nAuditQueueCapacity = 0\n",
        );
        let config = AuditConfig::from_parser(&p);
        assert!(!config.enable);
        assert_eq!(config.retention, 0);
        assert_eq!(config.queue_capacity, 1);
    }

    #[test]
    fn timestamps_are_rfc3339() {
        let ts = timestamp();
        assert!(ts.contains('T'), "not RFC3339: {ts}");
        assert!(ts.ends_with('Z'), "should be UTC: {ts}");
    }
}
