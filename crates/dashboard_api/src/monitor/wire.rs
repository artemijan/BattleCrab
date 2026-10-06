//! The monitor channel's line format, as the dashboard reads it — the other
//! half of `commons::monitor::Sample::to_json_line` (`docs/MONITORING.md` §4).

use std::collections::BTreeMap;

use serde::Deserialize;

/// One sample line. Fields the sampler always writes are required, so a line
/// from something that is not a monitor channel fails to parse instead of
/// being stored as zeros.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WireSample {
    pub service: String,
    pub ts: i64,
    pub started: i64,
    pub interval_ms: i64,
    pub cpu_micros: i64,
    pub rss_bytes: Option<i64>,
    pub heap_bytes: Option<i64>,
    #[serde(default)]
    pub metrics: BTreeMap<String, i64>,
}

/// What one poll's response body held.
#[derive(Debug, Default)]
pub struct Parsed {
    pub samples: Vec<WireSample>,
    /// The channel's own `{"error":…}` line, if it sent one.
    pub error: Option<String>,
    /// Lines that were neither — counted, not fatal: one bad line must not
    /// throw away the good ones around it.
    pub malformed: usize,
}

pub fn parse_body(body: &str) -> Parsed {
    let mut out = Parsed::default();
    for line in body.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<WireSample>(line) {
            Ok(s) => out.samples.push(s),
            Err(_) => match serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            {
                Some(e) => out.error = Some(e),
                None => out.malformed += 1,
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire contract, checked against the real producer: if
    /// `commons::monitor` renames or drops a field, this fails here instead of
    /// the poller silently storing nothing.
    #[test]
    fn parses_what_the_sampler_actually_writes() {
        let mut metrics = BTreeMap::new();
        metrics.insert("packets_in".to_string(), 1204u64);
        let line = commons::monitor::Sample {
            ts_ms: 1_759_000_005_000,
            started_ms: 1_758_990_000_000,
            interval_ms: 5000,
            cpu_micros: 8120,
            rss_bytes: None,
            heap_bytes: Some(48 << 20),
            metrics,
        }
        .to_json_line("game_server");

        let parsed = parse_body(&line);
        assert_eq!(parsed.malformed, 0);
        let s = &parsed.samples[0];
        assert_eq!(s.service, "game_server");
        assert_eq!(s.ts, 1_759_000_005_000);
        assert_eq!(s.started, 1_758_990_000_000);
        assert_eq!(s.rss_bytes, None);
        assert_eq!(s.heap_bytes, Some(48 << 20));
        assert_eq!(s.metrics["packets_in"], 1204);
    }

    #[test]
    fn keeps_good_lines_around_bad_ones_and_surfaces_channel_errors() {
        let good = r#"{"service":"login_server","ts":5000,"started":1,"interval_ms":5000,"cpu_micros":1,"rss_bytes":null,"heap_bytes":null,"metrics":{}}"#;
        let body = format!(
            "{good}\nnot json\n{{\"ts\":1}}\n{good}\n{{\"error\":\"expected `since <epoch_ms>`\"}}\n"
        );
        let parsed = parse_body(&body);
        assert_eq!(parsed.samples.len(), 2);
        assert_eq!(parsed.malformed, 2);
        assert_eq!(parsed.error.as_deref(), Some("expected `since <epoch_ms>`"));
    }
}
