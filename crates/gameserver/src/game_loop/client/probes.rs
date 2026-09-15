//! The unknown-input log: what to do when a client sends something no handler
//! claims — an unrecognised opcode, ex-opcode or `bypass -h` command.
//!
//! # Why this is not just a `warn!` at the dispatch site
//!
//! The inbound side of dispatch is reachable by anything that can complete the
//! handshake, so "the client sent something I don't know" is a statement about
//! *the client*, not about the server. Two consequences the plain log line at
//! each `_ =>` arm got wrong:
//!
//! 1. **Severity.** `error!` puts it in `game_server_error.log`, next to the
//!    failures an operator is paged for. A port scanner sending opcode `0xff`
//!    — not a Interlude client opcode at all — is not a server fault, and
//!    training operators to skip that file is worse than logging nothing.
//!    `warn!` is what the bypass router already chose for the same situation.
//!
//! 2. **Rate.** One line per packet, formatted **on the game thread**
//!    (`docs/LOGGING.md`: the non-blocking appender moves the `write(2)` off
//!    thread, not the formatting), with no cap. A socket looping on one
//!    unknown opcode turns the tick budget into log formatting and buries
//!    every real warning in the same file.
//!
//! # What it does instead
//!
//! One line per *distinct* unknown input per connection, up to
//! [`PROBE_LOG_CAP`] distinct inputs, then one closing line and silence.
//!
//! An opcode line carries a hex preview of the body that followed it
//! ([`payload`]). An opcode alone does not say what a client sent — `0xff` is
//! not an opcode this chronicle defines, so the only way to tell a port
//! scanner from a mis-framed real packet is to see the bytes. The preview is
//! *not* part of the dedupe key: the same opcode with a different body stays
//! one line, or a client could spend the whole cap on one opcode.
//!
//! The distinct-key part is deliberate and is what keeps this useful in
//! development: an unported bypass still announces itself exactly once per
//! session, which is how an unported corner of the datapack gets found. The cap is what keeps it useless as an amplifier — a client that
//! wants to write to the log gets [`PROBE_LOG_CAP`] lines and no more, for a
//! bounded `PROBE_LOG_CAP * 8` bytes of state.
//!
//! The state rides on `Session` next to the flood protectors, and for the same
//! reason they do: it must survive the connection's state transitions, or the
//! cap resets every time a client bounces to character select and back.

use std::collections::BTreeSet;

use tracing::warn;

/// How many *distinct* unknown inputs one connection may put in the log.
///
/// Sized for the development case rather than the abuse case: a client
/// exercising an unported NPC dialog trips a handful of distinct bypasses in
/// one session and all of them should be visible. Past that the connection has
/// stopped being informative — the same 32 names are already on the record,
/// and a 33rd tells an operator nothing the first 32 did not.
pub const PROBE_LOG_CAP: usize = 32;

/// Longest client-supplied command echoed into a log line.
///
/// The bypass string is attacker-controlled and reaches a plain-text sink
/// (`game_server_error.log`), so it is bounded here rather than trusted to be
/// dialog-sized.
const MAX_ECHO: usize = 80;

/// How many bytes of an unhandled packet's body are hex-dumped into its line.
///
/// Enough to identify what the packet actually was — an extended packet's
/// sub-opcode, a string's length prefix, a leading int — without putting a
/// client-sized frame in the log. The line already carries the full length, so
/// what is lost to truncation is the tail, not the fact that there was one.
const MAX_PAYLOAD_PREVIEW: usize = 16;

/// What kind of unknown input this was — the log line's subject, and part of
/// the dedupe key so opcode `0x32` and ex-opcode `0x0032` do not collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe<'a> {
    /// An opcode with no arm in `dispatch::on_packet`, and the packet body
    /// that followed it (logged as hex — see [`payload`]).
    Opcode(u8, &'a [u8]),
    /// A `0xD0` sub-opcode with no arm in `dispatch::on_ex_packet`, and the
    /// body that followed the sub-opcode.
    ExOpcode(u16, &'a [u8]),
    /// A `bypass -h` command the router has no branch for. Deduped on the
    /// leading verb, so `Foo 1` and `Foo 2` are one entry; the first sighting
    /// logs the whole command.
    Bypass(&'a str),
}

impl Probe<'_> {
    /// The dedupe key. Tagged per variant, and for a bypass a hash of the verb
    /// rather than the string itself — the set is bounded state, so it must not
    /// store client-sized keys.
    fn key(&self) -> u64 {
        match *self {
            Probe::Opcode(op, _) => u64::from(op),
            Probe::ExOpcode(sub, _) => 1 << 32 | u64::from(sub),
            Probe::Bypass(cmd) => {
                let verb = cmd.split_whitespace().next().unwrap_or("");
                2 << 32 | u64::from(fnv1a32(verb))
            }
        }
    }
}

/// FNV-1a over the verb's bytes. A hash and not the string because the set is
/// the memory bound: a collision costs one missed log line, a stored key costs
/// whatever the client chose to send.
fn fnv1a32(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Render a client-supplied string for a log line: control characters escaped
/// so it cannot forge a line in the plain-text sink, and truncated so it cannot
/// flood one.
fn echo(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_ECHO) + 3);
    for c in s.chars().take(MAX_ECHO) {
        match c {
            '\n' | '\r' | '\t' => out.push_str(&c.escape_default().to_string()),
            c if c.is_control() => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    if s.chars().nth(MAX_ECHO).is_some() {
        out.push('…');
    }
    out
}

/// Render an unhandled packet's body for a log line: its length, then the
/// first [`MAX_PAYLOAD_PREVIEW`] bytes in hex.
///
/// **Hex and not raw bytes** for the reason [`echo`] escapes its string — this
/// is client-chosen content reaching a plain-text sink, and hex cannot forge a
/// line. The length is outside the preview because it is the part that stays
/// true when the preview is truncated: a one-byte packet and a 300-byte packet
/// carrying the same opening bytes are different things.
fn payload(body: &[u8]) -> String {
    if body.is_empty() {
        return "empty".to_string();
    }
    let mut out = String::with_capacity(MAX_PAYLOAD_PREVIEW * 3 + 16);
    out.push_str(&format!("{} B [", body.len()));
    for (i, b) in body.iter().take(MAX_PAYLOAD_PREVIEW).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    if body.len() > MAX_PAYLOAD_PREVIEW {
        out.push_str(" …");
    }
    out.push(']');
    out
}

/// Per-connection record of which unknown inputs have already been logged.
///
/// Lives on `Session` (see the module docs) and is empty for every well-behaved
/// client, which is all of them — the `BTreeSet` allocates only once a
/// connection actually sends something unrecognised.
#[derive(Debug, Clone, Default)]
pub struct ProbeLog {
    seen: BTreeSet<u64>,
    /// Set once the cap is announced, so the closing line is logged once and
    /// not on every further probe.
    capped: bool,
}

impl ProbeLog {
    /// Log `probe` if this connection has not been told about it yet.
    ///
    /// Returns whether a line was written, which is what the tests assert on —
    /// the point of this type is the *second* call being silent.
    pub fn report(&mut self, client_id: u32, probe: Probe<'_>) -> bool {
        if self.capped {
            return false;
        }
        if !self.seen.insert(probe.key()) {
            return false;
        }
        if self.seen.len() > PROBE_LOG_CAP {
            self.capped = true;
            warn!(
                "GameLoop: client {client_id} has sent {PROBE_LOG_CAP} distinct unrecognised \
                 requests; not logging further ones for this connection."
            );
            return true;
        }
        match probe {
            Probe::Opcode(op, body) => warn!(
                "GameLoop: client {client_id} sent opcode 0x{op:02x}, unhandled (no dispatch \
                 arm). Payload {}.",
                payload(body)
            ),
            Probe::ExOpcode(sub, body) => warn!(
                "GameLoop: client {client_id} sent ex-opcode 0x{sub:04x}, unhandled (no dispatch \
                 arm). Payload {}.",
                payload(body)
            ),
            Probe::Bypass(cmd) => warn!(
                "Bypass: client {client_id} sent unhandled bypass [{}].",
                echo(cmd)
            ),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The second sighting is silent.** The whole reason this type exists: a
    /// socket looping on one unknown opcode must cost one log line, not one per
    /// packet.
    #[test]
    fn a_repeated_probe_is_logged_once() {
        let mut log = ProbeLog::default();
        assert!(log.report(1, Probe::Opcode(0xff, &[])));
        assert!(!log.report(1, Probe::Opcode(0xff, &[])));
        assert!(!log.report(1, Probe::Opcode(0xff, &[])));
    }

    /// A *different* unknown input is still news — that is what keeps the
    /// porting-gap signal alive.
    #[test]
    fn distinct_probes_each_get_a_line() {
        let mut log = ProbeLog::default();
        assert!(log.report(1, Probe::Opcode(0xff, &[])));
        assert!(log.report(1, Probe::Opcode(0xfe, &[])));
        assert!(log.report(1, Probe::ExOpcode(0x0032, &[])));
    }

    /// Opcode `0x32` is `AttackRequest`; ex-opcode `0x0032` is unassigned in
    /// this chronicle. The keys are tagged so one cannot mask the other.
    #[test]
    fn an_opcode_and_an_ex_opcode_with_the_same_number_are_different_probes() {
        let mut log = ProbeLog::default();
        assert!(log.report(1, Probe::Opcode(0x32, &[])));
        assert!(log.report(1, Probe::ExOpcode(0x0032, &[])));
    }

    /// Bypasses dedupe on the verb, so a dialog looping `Foo 1`, `Foo 2`, …
    /// cannot spend the cap one argument at a time.
    #[test]
    fn a_bypass_dedupes_on_its_verb() {
        let mut log = ProbeLog::default();
        assert!(log.report(1, Probe::Bypass("item_auction_withdraw")));
        assert!(!log.report(1, Probe::Bypass("item_auction_withdraw")));
        assert!(log.report(1, Probe::Bypass("something_else 1")));
        assert!(!log.report(1, Probe::Bypass("something_else 2")));
    }

    /// Past the cap the connection goes quiet — after exactly one line saying
    /// so, because silence that is not announced is indistinguishable from a
    /// client that stopped probing.
    #[test]
    fn the_cap_is_announced_once_and_then_the_connection_is_silent() {
        let mut log = ProbeLog::default();
        for op in 0..PROBE_LOG_CAP {
            assert!(
                log.report(1, Probe::ExOpcode(op as u16, &[])),
                "probe {op} is within the cap"
            );
        }
        // The one past the cap is the closing line…
        assert!(log.report(1, Probe::ExOpcode(PROBE_LOG_CAP as u16, &[])));
        // …and everything after it, new or repeated, is silent.
        assert!(!log.report(1, Probe::ExOpcode(PROBE_LOG_CAP as u16 + 1, &[])));
        assert!(!log.report(1, Probe::Opcode(0xff, &[])));
    }

    /// **The whole point of the payload preview.** An opcode with no arm says
    /// only that the client sent something unknown; the bytes say *what*. This
    /// is the shape an extended packet would have if one ever arrived under a
    /// prefix we do not treat as one.
    #[test]
    fn the_payload_is_rendered_with_its_length_and_bytes() {
        assert_eq!(payload(&[0x03, 0x00, 0x5c]), "3 B [03 00 5c]");
    }

    /// An empty body is named rather than rendered as an empty bracket pair —
    /// "the client sent a bare opcode" is itself the diagnosis.
    #[test]
    fn an_empty_payload_says_so() {
        assert_eq!(payload(&[]), "empty");
    }

    /// The preview is bounded like every other client-chosen string here, and
    /// the *length* survives the truncation — that is what says how much was
    /// cut.
    #[test]
    fn a_long_payload_is_truncated_but_still_reports_its_full_length() {
        let body = vec![0xabu8; 300];
        let shown = payload(&body);
        assert!(shown.starts_with("300 B ["), "{shown}");
        assert!(shown.ends_with(" …]"), "{shown}");
        assert_eq!(shown.matches("ab").count(), MAX_PAYLOAD_PREVIEW);
        // A body exactly at the limit is not marked as truncated.
        assert!(!payload(&[0xabu8; MAX_PAYLOAD_PREVIEW]).contains('…'));
    }

    /// **The preview must not become a cap-spending lever.** A socket looping
    /// one unknown opcode with a different body each time is still one line —
    /// the body is logged, but it is not part of the dedupe key.
    #[test]
    fn the_payload_does_not_widen_the_dedupe_key() {
        let mut log = ProbeLog::default();
        assert!(log.report(1, Probe::Opcode(0xff, &[0x01])));
        assert!(!log.report(1, Probe::Opcode(0xff, &[0x02])));
        assert!(!log.report(1, Probe::Opcode(0xff, &[0x03, 0x04])));
    }

    /// The echoed command is client-controlled and reaches a plain-text sink,
    /// so a newline in it must not be able to forge a log line.
    #[test]
    fn a_newline_in_a_bypass_cannot_forge_a_log_line() {
        let forged = "x\nERROR fake line";
        assert_eq!(echo(forged), "x\\nERROR fake line");
        assert!(!echo(forged).contains('\n'));
        assert!(!echo("bell\u{7}").contains('\u{7}'));
    }

    /// …and a long one must not be able to flood it.
    #[test]
    fn a_long_bypass_is_truncated() {
        let long = "a".repeat(4096);
        let shown = echo(&long);
        assert!(shown.starts_with(&"a".repeat(MAX_ECHO)));
        assert!(shown.ends_with('…'));
        assert_eq!(shown.chars().count(), MAX_ECHO + 1);
        // A command exactly at the limit is not marked as truncated.
        assert!(!echo(&"a".repeat(MAX_ECHO)).ends_with('…'));
    }
}
