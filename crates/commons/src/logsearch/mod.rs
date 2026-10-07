//! Log search over a service's own files — `docs/MONITORING.md` §6 (P4).
//!
//! Each service searches its own files, on its own machine: the game and
//! login servers answer `logs` requests on their monitor channel
//! (`crate::monitor`), and the dashboard searches its own logs in-process.
//! Nothing reads another machine's disk.
//!
//! **No client-supplied paths, by construction.** A request names a
//! [`Stream`] (a closed enum). The service maps it to a directory and a
//! filename prefix it derived itself at boot, and enumerates that directory. A cursor carries a filename, but
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
use serde::{Deserialize, Serialize};

use crate::audit::Category;
use reverse::ReverseLines;

/// Most results one request may return.
pub const MAX_LIMIT: usize = 500;

/// Longer than any sensible search; a cap so a pattern cannot be a payload.
pub const MAX_QUERY_LEN: usize = 512;

/// Compiled-program and lazy-DFA caps: `regex` is linear-time, and these
/// keep a pathological pattern from costing memory instead.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// Matching line bytes one response may carry. Results stop there as at
/// `limit`, with a cursor, so a page of very long lines can't become a
/// response nobody should be sending over a socket.
pub const MAX_HIT_BYTES: usize = 8 << 20;

/// Ceilings on what a request may ask for. The asker sets its own budget
/// (`LogSearchMaxBytes`, `LogSearchTimeoutMs`); the service that runs the scan
/// still caps it, since the scan costs that service's machine.
pub const MAX_SCAN_BYTES: u64 = 1 << 30;
pub const MAX_DEADLINE: Duration = Duration::from_secs(30);

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
    /// The service's files under `root` (a datapack root, as a path prefix
    /// like `logging::init` takes): the log directory from its
    /// `Logging.ini`, and `audit_dir`, which is where its audit sink writes.
    pub fn resolve(service: &str, root: &str, audit_dir: &str) -> Self {
        let root_slash = if root.is_empty() || root.ends_with('/') {
            root.to_string()
        } else {
            format!("{root}/")
        };
        let logging = crate::logging::LoggingConfig::load(&root_slash);
        Self {
            service: service.to_string(),
            log_dir: PathBuf::from(format!("{root_slash}{}", logging.directory)),
            audit_dir: PathBuf::from(format!("{root_slash}{audit_dir}")),
        }
    }

    /// A server's files, audit directory from its own `Logging.ini`.
    pub fn of_server(service: &str, root: &str) -> Self {
        let audit = crate::audit::AuditConfig::load(root);
        Self::resolve(service, root, &audit.directory)
    }

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

/// A search as it travels: what the dashboard's `/admin/logs/search` resolved
/// its defaults to, sent as-is over a monitor channel or validated in-process.
/// [`SearchRequest::validate`] is the one check both paths run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub stream: String,
    pub from: i64,
    pub to: i64,
    #[serde(default)]
    pub q: String,
    /// `q` is a regular expression rather than literal text.
    #[serde(default)]
    pub regex: bool,
    /// Minimum level (`warn` = WARN and ERROR). Not for audit streams.
    pub level: Option<String>,
    pub limit: usize,
    pub cursor: Option<String>,
    /// The asker's budget; capped at [`MAX_SCAN_BYTES`] and [`MAX_DEADLINE`].
    pub max_bytes: u64,
    pub timeout_ms: u64,
}

impl SearchRequest {
    /// Everything about a request that can be refused before a file is
    /// touched. The message is the client's to read.
    pub fn validate(&self) -> Result<(Query, Bounds), String> {
        let stream = Stream::parse(&self.stream).ok_or_else(|| {
            format!(
                "unknown stream {:?}; valid: diagnostic, error, audit:<category>",
                self.stream
            )
        })?;
        if self.from >= self.to {
            return Err("`from` must be before `to`".into());
        }
        let level = match self.level.as_deref().filter(|l| !l.is_empty()) {
            None => None,
            Some(_) if !stream.has_level() => {
                return Err("audit records have no level; drop `level`".into());
            }
            Some(l) => {
                Some(Level::parse(l).ok_or("`level` must be trace, debug, info, warn or error")?)
            }
        };
        if !(1..=MAX_LIMIT).contains(&self.limit) {
            return Err(format!("`limit` must be between 1 and {MAX_LIMIT}"));
        }
        let cursor = match self.cursor.as_deref().filter(|c| !c.is_empty()) {
            None => None,
            Some(c) => Some(Cursor::decode(c).ok_or("invalid `cursor`")?),
        };
        let query = Query {
            stream,
            from: self.from,
            to: self.to,
            pattern: compile(&self.q, self.regex)?,
            level,
            limit: self.limit,
            cursor,
        };
        let bounds = Bounds {
            max_bytes: self.max_bytes.clamp(1, MAX_SCAN_BYTES),
            deadline: Duration::from_millis(self.timeout_ms.max(1)).min(MAX_DEADLINE),
        };
        Ok((query, bounds))
    }
}

/// A literal `q` matches case-insensitively, as a search box is expected to;
/// a regex is taken as written (`(?i)` opts back in).
pub fn compile(q: &str, regex: bool) -> Result<Option<regex::bytes::Regex>, String> {
    if q.is_empty() {
        return Ok(None);
    }
    if q.len() > MAX_QUERY_LEN {
        return Err(format!("`q` is limited to {MAX_QUERY_LEN} bytes"));
    }
    let pattern = if regex {
        q.to_string()
    } else {
        regex::escape(q)
    };
    regex::bytes::RegexBuilder::new(&pattern)
        .case_insensitive(!regex)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map(Some)
        .map_err(|e| format!("invalid regex: {e}"))
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
    /// `limit` results (or [`MAX_HIT_BYTES`] of them) found; more may follow
    /// from the cursor.
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
    let mut hit_bytes = 0usize;

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
            hit_bytes += bytes.len();
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
            if out.hits.len() >= q.limit || hit_bytes >= MAX_HIT_BYTES {
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

/// What `/admin/logs/streams` lists: a stream that has files, and the dates
/// they cover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
