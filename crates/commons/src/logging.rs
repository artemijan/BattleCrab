//! The diagnostic logging pipeline — the rough equivalent of the Java server's
//! `log.cfg` (`java.util.logging`), which fans one event stream out to a
//! rotating `java%g.log`, a console handler, and a filtered `error%g.log`.
//!
//! Two properties matter more here than they did in Java:
//!
//! * **The game thread must never pay for a log line.** The file layers write
//!   through [`tracing_appender::non_blocking`]: the line is still *formatted*
//!   on the calling thread, but the finished bytes go to a bounded channel
//!   drained by a dedicated writer thread, so no `write(2)` and no shared
//!   writer lock land on the tick. In lossy mode a full channel **drops** the
//!   line instead of blocking, which is the whole point — a load spike costs
//!   log fidelity rather than tick budget.
//! * **Silent loss must be visible.** Dropping is invisible by construction, so
//!   [`init`] starts a reporter thread that publishes the running drop count.
//!   Otherwise a saturated log and a quiet one look identical.
//!
//! Formatting is *not* moved off-thread by any of this. The lever for genuinely
//! hot paths is the compile-time one: building with
//! `--features tracing/release_max_level_info` makes every `debug!`/`trace!` in
//! the packet path vanish at compile time rather than being filtered at runtime.
//!
//! ## What does not belong here
//!
//! Audit records — chat, GM commands, item transactions, enchants — deliberately
//! do not travel this pipeline. They are *records*, not diagnostics: they are
//! low-volume, high-value, and must never be dropped. Routing them through a
//! lossy sink would discard exactly the evidence a busy server is most likely to
//! be asked for. They get their own append-only sink.

use std::path::Path;
use std::time::Duration;

use tracing_appender::non_blocking::{ErrorCounter, NonBlockingBuilder, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::config::PropertiesParser;

/// Datapack-relative location of the logging config, alongside every other
/// `.ini` the servers read.
pub const LOGGING_CONFIG_FILE: &str = "config/Logging.ini";

/// Everything `Logging.ini` controls. Defaults here are the values used when the
/// file is absent entirely, so a deployment that never ships one still gets
/// rotation and retention rather than an unbounded file.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// `EnvFilter` directives for the file layer, e.g.
    /// `info,gameserver::game_loop::net=warn`. `RUST_LOG` overrides it.
    pub level: String,
    pub console_enable: bool,
    pub console_level: String,
    pub file_enable: bool,
    /// Log directory, resolved against the datapack root.
    pub directory: String,
    /// JSON lines rather than the human-readable format.
    pub json: bool,
    /// `daily`, `hourly` or `never`.
    pub rotation: String,
    /// How many rotated files to keep. `0` keeps them forever.
    pub retention: usize,
    /// Bounded channel capacity, in lines, before the writer starts dropping.
    pub buffered_lines: usize,
    /// Mirror `WARN`+ into a second, never-dropping file.
    pub error_file_enable: bool,
    /// How often to report dropped lines. `0` disables the reporter.
    pub drop_report_seconds: u64,
    /// How often to emit the [`crate::metrics`] snapshot as one structured log
    /// event. `0` disables it.
    pub metrics_interval_seconds: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            console_enable: true,
            console_level: "info".to_string(),
            file_enable: true,
            directory: "log".to_string(),
            json: true,
            rotation: "daily".to_string(),
            retention: 14,
            buffered_lines: 131_072,
            error_file_enable: true,
            drop_report_seconds: 60,
            metrics_interval_seconds: 60,
        }
    }
}

impl LoggingConfig {
    /// Reads `{root}config/Logging.ini`, falling back to [`Default`] per key.
    pub fn load(root: &str) -> Self {
        Self::from_parser(&PropertiesParser::load_rel(root, LOGGING_CONFIG_FILE))
    }

    /// Parses an in-memory ini body — for tests and for callers that assemble
    /// config without a file on disk.
    pub fn from_content(content: &str) -> Self {
        Self::from_parser(&PropertiesParser::from_content(
            LOGGING_CONFIG_FILE,
            content,
        ))
    }

    fn from_parser(p: &PropertiesParser) -> Self {
        let d = Self::default();
        Self {
            level: p.get_string("Level", &d.level),
            console_enable: p.get_bool("ConsoleEnable", d.console_enable),
            console_level: p.get_string("ConsoleLevel", &d.console_level),
            file_enable: p.get_bool("FileEnable", d.file_enable),
            directory: p.get_string("Directory", &d.directory),
            json: p
                .get_string("FileFormat", if d.json { "json" } else { "plain" })
                .eq_ignore_ascii_case("json"),
            rotation: p.get_string("Rotation", &d.rotation),
            retention: p.get_int("Retention", d.retention as i32).max(0) as usize,
            buffered_lines: p.get_int("BufferedLines", d.buffered_lines as i32).max(1) as usize,
            error_file_enable: p.get_bool("ErrorFileEnable", d.error_file_enable),
            drop_report_seconds: p
                .get_int("DropReportSeconds", d.drop_report_seconds as i32)
                .max(0) as u64,
            metrics_interval_seconds: p
                .get_int("MetricsIntervalSeconds", d.metrics_interval_seconds as i32)
                .max(0) as u64,
        }
    }
}

/// Keeps the non-blocking writer threads alive.
///
/// **This must be held for the whole life of the process.** Each [`WorkerGuard`]
/// flushes its channel on drop; drop them early — by not binding the return of
/// [`init`], say — and every log line written afterwards is silently discarded,
/// which most visibly eats the tail of the log at shutdown.
#[must_use = "dropping the guard stops the log writer threads and truncates the log"]
pub struct LogGuard {
    _guards: Vec<WorkerGuard>,
}

/// Installs the global subscriber for `service` (`game_server`, `login_server`,
/// …), reading `{root}config/Logging.ini`.
///
/// Falls back to a console-only subscriber if the log directory or the appender
/// cannot be opened — a server that cannot write logs should still boot.
pub fn init(root: &str, service: &str) -> LogGuard {
    let path = format!("{root}{LOGGING_CONFIG_FILE}");
    // Probe first: `PropertiesParser::load` reports a missing file through
    // `tracing`, and at this point there is no subscriber to receive it. An
    // absent Logging.ini is a supported configuration, not an error.
    let present = Path::new(&path).exists();
    let config = if present {
        LoggingConfig::load(root)
    } else {
        LoggingConfig::default()
    };
    init_with(&config, root, service, present)
}

/// [`init`] with the config supplied directly, for callers that build it
/// themselves.
pub fn init_with(config: &LoggingConfig, root: &str, service: &str, from_file: bool) -> LogGuard {
    let mut guards: Vec<WorkerGuard> = Vec::new();
    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();
    // Anything worth saying about the logging setup itself has to wait: the
    // subscriber that would carry it does not exist yet.
    let mut notes: Vec<String> = Vec::new();

    // `RUST_LOG` beats the ini so a running server can be debugged without
    // editing the datapack.
    let (level, level_source) = match std::env::var("RUST_LOG") {
        Ok(v) if !v.trim().is_empty() => (v, "RUST_LOG"),
        _ => (config.level.clone(), "Logging.ini"),
    };

    if config.console_enable {
        layers.push(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_filter(env_filter(&config.console_level))
                .boxed(),
        );
    }

    let relative_dir = format!("{root}{}", config.directory);
    // Resolved to an absolute path on purpose. `tracing-appender` builds the
    // "latest" symlink's target by joining the log directory onto the filename,
    // then places the link *inside* that same directory — so with a relative
    // directory the link points at `log/log/<file>` and dangles. An absolute
    // directory makes the target absolute and the link correct.
    let dir = match std::fs::create_dir_all(&relative_dir) {
        Ok(()) => std::fs::canonicalize(&relative_dir).ok(),
        Err(e) => {
            notes.push(format!(
                "could not create log directory {relative_dir}: {e} — file logging disabled"
            ));
            None
        }
    };
    let dir = dir.as_ref().map(|p| p.display().to_string());

    if config.file_enable
        && let Some(dir) = dir.as_deref()
    {
        let suffix = if config.json { "json" } else { "log" };
        match build_appender(dir, service, suffix, config) {
            Ok(appender) => {
                let (writer, guard) = NonBlockingBuilder::default()
                    // The load-shedding switch. A full channel drops the line
                    // rather than parking the caller — which, on the game
                    // thread, would be the tick.
                    .lossy(true)
                    .buffered_lines_limit(config.buffered_lines)
                    .thread_name(&format!("{service}-log"))
                    .finish(appender);
                guards.push(guard);
                spawn_drop_reporter(writer.error_counter(), config.drop_report_seconds);

                let filter = env_filter(&level);
                layers.push(if config.json {
                    fmt::layer()
                        .json()
                        // Span fields are the point of the JSON format here: a
                        // connection span carrying account/char/oid turns "what
                        // happened to this player" into one query.
                        .flatten_event(true)
                        .with_current_span(true)
                        .with_span_list(true)
                        .with_writer(writer)
                        .with_ansi(false)
                        .with_filter(filter)
                        .boxed()
                } else {
                    fmt::layer()
                        .with_writer(writer)
                        .with_ansi(false)
                        .with_filter(filter)
                        .boxed()
                });
            }
            Err(e) => notes.push(format!("could not open the log file in {dir}: {e}")),
        }
    }

    if config.error_file_enable
        && let Some(dir) = dir.as_deref()
    {
        match build_appender(dir, &format!("{service}_error"), "log", config) {
            Ok(appender) => {
                // Deliberately NOT lossy, and deliberately not subject to the
                // per-target directives above: incidents are rare, cheap to
                // keep, and the one thing that must survive a saturated buffer.
                let (writer, guard) = NonBlockingBuilder::default()
                    .lossy(false)
                    .buffered_lines_limit(config.buffered_lines)
                    .thread_name(&format!("{service}-errlog"))
                    .finish(appender);
                guards.push(guard);
                layers.push(
                    fmt::layer()
                        .with_writer(writer)
                        .with_ansi(false)
                        .with_filter(LevelFilter::WARN)
                        .boxed(),
                );
            }
            Err(e) => notes.push(format!("could not open the error log in {dir}: {e}")),
        }
    }

    tracing_subscriber::registry().with(layers).init();

    // The subscriber exists now, so the deferred notes finally have somewhere
    // to go.
    for note in notes {
        tracing::error!("logging: {note}");
    }
    // The field is `filter`, not `level`: the JSON formatter emits the event's
    // own severity as `level`, and a second `level` field would put a duplicate
    // key in every line — valid-ish JSON that parsers resolve inconsistently.
    tracing::info!(
        service,
        filter = %level,
        filter_source = level_source,
        config = if from_file { "Logging.ini" } else { "defaults (no Logging.ini)" },
        json = config.json,
        rotation = %config.rotation,
        retention = config.retention,
        buffered_lines = config.buffered_lines,
        "logging initialised"
    );

    LogGuard { _guards: guards }
}

/// Routes panics through `tracing` so they land in the log files rather than
/// only on stderr, then chains to the previous hook so the default stderr
/// message (and any backtrace) still appears.
///
/// This matters most for the packet path: `game_loop/net.rs` catches unwinds
/// per packet, so without this a recovered panic would leave nothing in the log
/// file to explain the dropped packet.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let thread = std::thread::current()
            .name()
            .unwrap_or("<unnamed>")
            .to_string();
        tracing::error!(location = %location, thread = %thread, "panic: {payload}");
        previous(info);
    }));
}

fn build_appender(
    dir: &str,
    prefix: &str,
    suffix: &str,
    config: &LoggingConfig,
) -> Result<RollingFileAppender, tracing_appender::rolling::InitError> {
    let builder = RollingFileAppender::builder()
        .rotation(rotation(&config.rotation))
        .filename_prefix(prefix)
        .filename_suffix(suffix)
        .max_log_files(config.retention);
    // Rotation dates the filenames, so keep a stable path to tail — this is
    // what `deploy.sh` used to get from the systemd `append:` redirect.
    #[cfg(unix)]
    let builder = builder.latest_symlink(format!("{prefix}.{suffix}"));
    builder.build(dir)
}

/// Reports lines lost to a saturated buffer. Without this, lossy mode is a
/// silent failure: the log simply looks calm during the exact incident anyone
/// would go to it to understand.
fn spawn_drop_reporter(counter: ErrorCounter, period_seconds: u64) {
    if period_seconds == 0 {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("log-drop-reporter".to_string())
        .spawn(move || {
            let mut reported = 0usize;
            loop {
                std::thread::sleep(Duration::from_secs(period_seconds));
                let total = counter.dropped_lines();
                if total > reported {
                    tracing::warn!(
                        dropped = total - reported,
                        dropped_total = total,
                        "log buffer saturated — diagnostic lines were dropped"
                    );
                    reported = total;
                }
            }
        });
}

fn env_filter(directives: &str) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse(directives)
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

fn rotation(name: &str) -> Rotation {
    match name.trim().to_ascii_lowercase().as_str() {
        "hourly" => Rotation::HOURLY,
        "never" => Rotation::NEVER,
        _ => Rotation::DAILY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_when_keys_are_absent() {
        let config = LoggingConfig::from_content("");
        let d = LoggingConfig::default();
        assert_eq!(config.level, d.level);
        assert_eq!(config.retention, d.retention);
        assert!(config.json);
        assert!(config.error_file_enable);
    }

    #[test]
    fn parses_a_full_file() {
        let config = LoggingConfig::from_content(
            "Level = info,gameserver::game_loop::net=warn\n\
             ConsoleEnable = false\n\
             FileFormat = plain\n\
             Rotation = hourly\n\
             Retention = 3\n\
             BufferedLines = 4096\n\
             ErrorFileEnable = false\n\
             DropReportSeconds = 0\n",
        );
        assert_eq!(config.level, "info,gameserver::game_loop::net=warn");
        assert!(!config.console_enable);
        assert!(!config.json);
        assert_eq!(config.retention, 3);
        assert_eq!(config.buffered_lines, 4096);
        assert!(!config.error_file_enable);
        assert_eq!(config.drop_report_seconds, 0);
    }

    /// `Retention = 0` means "keep everything" to the appender, so it has to
    /// survive the parse rather than being clamped to a 1-file window.
    #[test]
    fn zero_retention_is_preserved() {
        assert_eq!(LoggingConfig::from_content("Retention = 0\n").retention, 0);
    }

    #[test]
    fn negative_values_do_not_wrap_around() {
        let config = LoggingConfig::from_content("Retention = -5\nBufferedLines = -1\n");
        assert_eq!(config.retention, 0);
        assert_eq!(config.buffered_lines, 1);
    }

    #[test]
    fn rotation_names_map_and_default_to_daily() {
        assert_eq!(rotation("hourly"), Rotation::HOURLY);
        assert_eq!(rotation("HOURLY"), Rotation::HOURLY);
        assert_eq!(rotation("never"), Rotation::NEVER);
        assert_eq!(rotation("daily"), Rotation::DAILY);
        assert_eq!(rotation("nonsense"), Rotation::DAILY);
    }

    /// A malformed directive string must not take the server down at boot.
    #[test]
    fn a_broken_filter_falls_back_instead_of_panicking() {
        let _ = env_filter("this is not=a=valid=filter");
    }
}
