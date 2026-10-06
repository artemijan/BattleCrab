//! Log search over the servers' own files — `docs/MONITORING.md` §6 (P4).
//!
//! **No client-supplied paths, by construction.** A request names a service
//! (one of `LogSearchRoots`' keys) and a [`Stream`] (a closed enum). The
//! server maps those to a directory and a filename prefix it derived itself
//! at boot, and enumerates that directory. A cursor carries a filename, but
//! it is only ever *compared* against that enumeration, never joined onto a
//! path — so traversal is unrepresentable rather than defended against.
//!
//! The scan is bounded every way it can run away: bytes read, wall-clock
//! time, results returned, and concurrent searches. Hitting a bound returns
//! what was found so far, `truncated`, and a cursor that resumes exactly
//! where it stopped — never a silently partial answer.

pub mod reverse;
pub mod time;

use std::fs::File;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Serialize;

use commons::audit::Category;
use reverse::ReverseLines;

/// Where the dashboard writes its own audit records. Shared with `main`,
/// which sets it, so search finds them where they are.
pub const DASHBOARD_AUDIT_DIR: &str = "log/audit-dashboard";

/// Lines a little older than `from` may sit after newer ones: lines are
/// stamped on the emitting thread and written by one writer thread, so the
/// file is ordered to within a few ms. The scan stops only once lines are
/// this far before `from`.
const ORDER_SLACK_MS: i64 = 5_000;

/// How often the deadline is checked, in lines.
const DEADLINE_EVERY: usize = 256;

/// What a service writes, by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// `<service>.<date>.json` — the diagnostic log, JSON lines.
    Diagnostic,
    /// `<service>_error.<date>.log` — WARN+ only, plain text.
    Error,
    /// `<stem>.<date>.ndjson` in the audit directory.
    Audit(Category),
}

impl Stream {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "diagnostic" => Some(Self::Diagnostic),
            "error" => Some(Self::Error),
            _ => {
                let stem = s.strip_prefix("audit:")?;
                Category::ALL
                    .into_iter()
                    .find(|c| c.file_stem() == stem)
                    .map(Self::Audit)
            }
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Diagnostic => "diagnostic".into(),
            Self::Error => "error".into(),
            Self::Audit(c) => format!("audit:{}", c.file_stem()),
        }
    }

    /// Lines are JSON (and returned parsed) rather than plain text.
    fn is_json(self) -> bool {
        !matches!(self, Self::Error)
    }

    /// Lines carry a level a search can filter on.
    pub fn has_level(self) -> bool {
        !matches!(self, Self::Audit(_))
    }

    fn all() -> impl Iterator<Item = Stream> {
        [Self::Diagnostic, Self::Error]
            .into_iter()
            .chain(Category::ALL.into_iter().map(Self::Audit))
    }
}

/// One service's log locations, resolved at boot from its datapack's own
/// `Logging.ini` — the same file the service reads to decide where to write.
#[derive(Debug, Clone)]
pub struct Source {
    pub service: String,
    pub log_dir: PathBuf,
    pub audit_dir: PathBuf,
}

impl Source {
    fn locate(&self, stream: Stream) -> (PathBuf, String, &'static str) {
        match stream {
            Stream::Diagnostic => (self.log_dir.clone(), self.service.clone(), "json"),
            Stream::Error => (
                self.log_dir.clone(),
                format!("{}_error", self.service),
                "log",
            ),
            Stream::Audit(c) => (self.audit_dir.clone(), c.file_stem().to_string(), "ndjson"),
        }
    }

    /// The stream's files, newest first: `(filename, covered span)`. Dated
    /// files are `<prefix>.<date>.<suffix>`. The undated `<prefix>.<suffix>`
    /// is the "latest" symlink to one of them and is skipped — unless it is a
    /// real file, which is what `Rotation = never` writes, covering all time.
    pub fn files(&self, stream: Stream) -> Vec<LogFile> {
        let (dir, prefix, suffix) = self.locate(stream);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out: Vec<LogFile> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let middle = name
                    .strip_prefix(prefix.as_str())?
                    .strip_suffix(suffix)?
                    .strip_suffix('.')?;
                let span = if middle.is_empty() {
                    let is_symlink = e.file_type().ok()?.is_symlink();
                    if is_symlink {
                        return None;
                    }
                    (i64::MIN, i64::MAX)
                } else {
                    time::file_date_span(middle.strip_prefix('.')?)?
                };
                e.file_type().ok().filter(|t| t.is_file())?;
                Some(LogFile {
                    path: e.path(),
                    name,
                    span,
                })
            })
            .collect();
        out.sort_by(|a, b| b.span.cmp(&a.span).then_with(|| b.name.cmp(&a.name)));
        out
    }
}

#[derive(Debug, Clone)]
pub struct LogFile {
    pub path: PathBuf,
    pub name: String,
    /// `[start, end)` epoch ms the filename's date covers.
    pub span: (i64, i64),
}

/// Parses `LogSearchRoots` (`service=datapack_root,…`) into sources. Each
/// service's directories come from its root's `Logging.ini`; the dashboard's
/// own audit directory is the override `main` applies, not the ini's.
pub fn sources(raw: &str) -> Result<Vec<Source>, String> {
    let mut out: Vec<Source> = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (service, root) = entry
            .split_once('=')
            .map(|(s, r)| (s.trim(), r.trim()))
            .filter(|(s, r)| {
                !s.is_empty()
                    && !r.is_empty()
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
            .ok_or_else(|| format!("LogSearchRoots entry {entry:?} is not service=path"))?;
        if out.iter().any(|s| s.service == service) {
            return Err(format!("LogSearchRoots names {service:?} twice"));
        }
        let root_slash = if root.ends_with('/') {
            root.to_string()
        } else {
            format!("{root}/")
        };
        let logging = commons::logging::LoggingConfig::load(&root_slash);
        let audit_dir = if service == "dashboard_api" {
            DASHBOARD_AUDIT_DIR.to_string()
        } else {
            commons::audit::AuditConfig::load(&root_slash).directory
        };
        out.push(Source {
            service: service.to_string(),
            log_dir: PathBuf::from(root).join(logging.directory),
            audit_dir: PathBuf::from(root).join(audit_dir),
        });
    }
    Ok(out)
}

/// Minimum-level filter. Ordered so `>=` means "at least as severe".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Where a search stopped: the file, and the offset of the oldest line it
/// consumed. Opaque to clients (base64), so nobody is tempted to build one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub file: String,
    pub offset: u64,
}

impl Cursor {
    pub fn encode(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(format!("{}\n{}", self.offset, self.file))
    }

    pub fn decode(s: &str) -> Option<Self> {
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .ok()?;
        let raw = String::from_utf8(raw).ok()?;
        let (offset, file) = raw.split_once('\n')?;
        Some(Self {
            file: file.to_string(),
            offset: offset.parse().ok()?,
        })
    }
}

pub struct Query {
    pub stream: Stream,
    pub from: i64,
    pub to: i64,
    /// Compiled from `q`; `None` matches every line.
    pub pattern: Option<regex::bytes::Regex>,
    pub level: Option<Level>,
    pub limit: usize,
    pub cursor: Option<Cursor>,
}

pub struct Bounds {
    pub max_bytes: u64,
    pub deadline: Duration,
}

/// A matching line. JSON streams come back parsed, as a pass-through value —
/// span fields are the point of the format and a fixed DTO would drop them.
#[derive(Debug, Serialize)]
pub struct Hit {
    pub file: String,
    pub offset: u64,
    pub ts: Option<i64>,
    pub line: Line,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Line {
    Structured(serde_json::Value),
    Raw(String),
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// `limit` results found; more may follow from the cursor.
    Limit,
    /// `LogSearchMaxBytes` read.
    MaxBytes,
    /// `LogSearchTimeoutMs` elapsed.
    Deadline,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub hits: Vec<Hit>,
    /// Present whenever the scan stopped before exhausting the range.
    pub cursor: Option<String>,
    pub stopped: Option<StopReason>,
    /// The scan hit a resource bound rather than the result limit: the range
    /// was not fully searched, and `cursor` resumes it.
    pub truncated: bool,
    pub scanned_bytes: u64,
    pub files_scanned: Vec<String>,
    /// Lines over `reverse::MAX_LINE`, skipped rather than buffered.
    pub skipped_oversized: u64,
}

/// The cheap timestamp: a byte search for the stamp field rather than a JSON
/// parse, because it runs on every line the scan touches, matching or not.
/// An escaped quote inside a string value is `\"`, so `"ts":"` cannot match
/// inside one.
fn line_ts(line: &[u8], json: bool) -> Option<i64> {
    if !json {
        let end = memchr::memchr(b' ', line).unwrap_or(line.len());
        return time::parse_rfc3339(&line[..end]);
    }
    for key in [&b"\"timestamp\":\""[..], b"\"ts\":\""] {
        if let Some(at) = memchr::memmem::find(line, key) {
            let rest = &line[at + key.len()..];
            let end = memchr::memchr(b'"', rest)?;
            return time::parse_rfc3339(&rest[..end]);
        }
    }
    None
}

/// The cheap level, for the same reason as [`line_ts`]: a level filter must
/// not cost a JSON parse per line. Plain-text `_error.log` lines carry it as
/// the token after the stamp.
fn line_level(line: &[u8], json: bool) -> Option<Level> {
    let token = if json {
        let key = b"\"level\":\"";
        let at = memchr::memmem::find(line, key)?;
        let rest = &line[at + key.len()..];
        &rest[..memchr::memchr(b'"', rest)?]
    } else {
        line.split(|b| *b == b' ')
            .filter(|t| !t.is_empty())
            .nth(1)?
    };
    Level::parse(std::str::from_utf8(token).ok()?)
}

/// Run one search. Blocking file I/O — callers run it on `spawn_blocking`.
pub fn search(source: &Source, q: &Query, bounds: &Bounds) -> Outcome {
    let started = Instant::now();
    let mut out = Outcome {
        hits: Vec::new(),
        cursor: None,
        stopped: None,
        truncated: false,
        scanned_bytes: 0,
        files_scanned: Vec::new(),
        skipped_oversized: 0,
    };
    let files = source.files(q.stream);
    // With a cursor, start at its file; a cursor naming no current file
    // (rotated away since) starts nowhere — the range it pointed into is
    // gone, and answering "no more results" is the truth.
    let start_index = match &q.cursor {
        Some(c) => match files.iter().position(|f| f.name == c.file) {
            Some(i) => i,
            None => return out,
        },
        None => 0,
    };
    let json = q.stream.is_json();

    'files: for file in &files[start_index..] {
        // Rotation dates the filenames: skip files wholly outside the range
        // without opening them.
        if file.span.0 >= q.to {
            continue;
        }
        if file.span.1 <= q.from.saturating_sub(ORDER_SLACK_MS) {
            break;
        }
        let Ok(handle) = File::open(&file.path) else {
            continue;
        };
        // The length at open: a live file keeps growing, and lines appended
        // during the scan are newer than anything this search is paging
        // backwards through.
        let Ok(len) = handle.metadata().map(|m| m.len()) else {
            continue;
        };
        let end = match &q.cursor {
            Some(c) if c.file == file.name => c.offset.min(len),
            _ => len,
        };
        out.files_scanned.push(file.name.clone());
        let mut lines = ReverseLines::new(handle, end, reverse::CHUNK);
        let mut n = 0usize;
        loop {
            let budget_left = bounds.max_bytes.saturating_sub(out.scanned_bytes);
            if lines.bytes_read >= budget_left {
                out.scanned_bytes += lines.bytes_read;
                out.skipped_oversized += lines.skipped_oversized;
                out.stopped = Some(StopReason::MaxBytes);
                out.truncated = true;
                out.cursor = Some(resume(file, &lines, end));
                break 'files;
            }
            n += 1;
            if n.is_multiple_of(DEADLINE_EVERY) && started.elapsed() >= bounds.deadline {
                out.scanned_bytes += lines.bytes_read;
                out.skipped_oversized += lines.skipped_oversized;
                out.stopped = Some(StopReason::Deadline);
                out.truncated = true;
                out.cursor = Some(resume(file, &lines, end));
                break 'files;
            }
            let Ok(Some((offset, bytes))) = lines.next_line() else {
                break;
            };
            lines.consumed_to = offset;
            let ts = line_ts(&bytes, json);
            if let Some(ts) = ts {
                if ts >= q.to {
                    continue;
                }
                if ts < q.from {
                    if ts < q.from - ORDER_SLACK_MS {
                        out.scanned_bytes += lines.bytes_read;
                        out.skipped_oversized += lines.skipped_oversized;
                        break 'files;
                    }
                    continue;
                }
            }
            // A line with no readable level (a multi-line panic's
            // continuation in `_error.log`) is kept: it belongs to the entry
            // above it, which passed.
            if let Some(min) = q.level
                && line_level(&bytes, json).is_some_and(|l| l < min)
            {
                continue;
            }
            if let Some(p) = &q.pattern
                && !p.is_match(&bytes)
            {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let line = if json {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => Line::Structured(v),
                    Err(_) => Line::Raw(text),
                }
            } else {
                Line::Raw(text)
            };
            out.hits.push(Hit {
                file: file.name.clone(),
                offset,
                ts,
                line,
            });
            if out.hits.len() >= q.limit {
                out.scanned_bytes += lines.bytes_read;
                out.skipped_oversized += lines.skipped_oversized;
                out.stopped = Some(StopReason::Limit);
                out.cursor = Some(
                    Cursor {
                        file: file.name.clone(),
                        offset,
                    }
                    .encode(),
                );
                break 'files;
            }
        }
        out.scanned_bytes += lines.bytes_read;
        out.skipped_oversized += lines.skipped_oversized;
    }
    out
}

fn resume<R>(file: &LogFile, lines: &ReverseLines<R>, end: u64) -> String {
    Cursor {
        file: file.name.clone(),
        offset: lines.consumed_to.min(end),
    }
    .encode()
}

/// Everything a search request needs that outlives it: the sources resolved
/// at boot, the per-request bounds, and the concurrency cap.
pub struct LogSearch {
    pub sources: Vec<Source>,
    pub bounds: Bounds,
    /// One permit per running search. Scans are cheap per line but not per
    /// request, and each one holds a blocking-pool thread.
    pub permits: std::sync::Arc<tokio::sync::Semaphore>,
}

impl LogSearch {
    pub fn new(sources: Vec<Source>, bounds: Bounds, concurrency: usize) -> Self {
        Self {
            sources,
            bounds,
            permits: std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1))),
        }
    }

    /// `None` (with a log line saying why) when `LogSearchRoots` is empty or
    /// malformed. Never fatal: the rest of the dashboard serves either way.
    pub fn from_config(config: &crate::config::DashboardConfig) -> Option<std::sync::Arc<Self>> {
        let sources = match sources(&config.log_search_roots) {
            Ok(s) if s.is_empty() => {
                tracing::info!("log search: disabled (LogSearchRoots is empty)");
                return None;
            }
            Ok(s) => s,
            Err(e) => {
                tracing::error!("log search: disabled — {e}");
                return None;
            }
        };
        tracing::info!(
            "log search: {}",
            sources
                .iter()
                .map(|s| format!("{} in {}", s.service, s.log_dir.display()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        Some(std::sync::Arc::new(Self::new(
            sources,
            Bounds {
                max_bytes: config.log_search_max_bytes,
                deadline: Duration::from_millis(config.log_search_timeout_ms),
            },
            config.log_search_concurrency,
        )))
    }

    pub fn source(&self, service: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.service == service)
    }
}

/// What `/admin/logs/streams` lists: a stream that has files, and the dates
/// they cover.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub service: String,
    pub stream: String,
    pub files: usize,
    /// Start of the oldest file's span, epoch ms. `None` for an undated
    /// (never-rotated) file, which covers all time.
    pub oldest: Option<i64>,
    pub newest_end: Option<i64>,
}

impl Source {
    pub fn streams(&self) -> Vec<StreamInfo> {
        Stream::all()
            .filter_map(|stream| {
                let files = self.files(stream);
                let dated = || files.iter().filter(|f| f.span.0 != i64::MIN);
                (!files.is_empty()).then(|| StreamInfo {
                    service: self.service.clone(),
                    stream: stream.name(),
                    files: files.len(),
                    oldest: dated().map(|f| f.span.0).min(),
                    newest_end: dated().map(|f| f.span.1).max(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
