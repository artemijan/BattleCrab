//! Walks a decrypted `.dat` with its [`crate::dat_schema`] layout and renders
//! it as the tab-separated text L2ClientDat produces.
//!
//! # Why "did it consume the whole file" is the test
//!
//! Nothing in a `.dat` is self-describing past the first field, so a schema
//! that is subtly wrong still reads *something* — it just drifts. The one
//! reliable signal is the end position: a correct walk stops on the final byte
//! (plus the 13-byte `SafePackage` trailer, which is an `ASCF` `"SafePackage"`
//! string), and a drifting one stops early, runs off the end, or reads an
//! absurd length. [`Outcome::exact`] is that check, and it is what makes
//! `--chronicle auto` able to pick the right schema set by itself.
//!
//! # Output shape
//!
//! Ported from `DescriptorReader.parseData`. Strings are bracketed, fields
//! inside a hidden-name group are `;`-separated and wrapped in `{}`, and named
//! cycles bracket their body with `<name>_begin` / `<name>_end`. Line breaks
//! come from `<write name="\r\n"/>` constants in the schema, not from the
//! walker.

use crate::dat_schema::{Count, Field, Layout, Node};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The 13 bytes a `isSafePackage` file ends with: compact length 12 then
/// `"SafePackage\0"`.
pub const SAFE_PACKAGE_LEN: usize = 13;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Str(String),
}

impl Value {
    fn as_count(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Str(s) => s.parse().ok(),
        }
    }

    fn matches(&self, other: &str) -> bool {
        match self {
            Value::Int(i) => i.to_string().eq_ignore_ascii_case(other),
            Value::Str(s) => s.eq_ignore_ascii_case(other),
        }
    }
}

/// One client `.dat`, decrypted and decoded — plus everything needed to write
/// it back.
pub struct Unpacked {
    /// Where it came from, so a caller that saves does not re-derive the path.
    pub path: PathBuf,
    /// The tab-separated text L2ClientDat would produce.
    pub text: String,
    /// The `Lineage2Ver` header, needed to re-encrypt with the same cipher.
    pub version: String,
    /// The layout that consumed the file exactly. Handed back so packing uses
    /// the very same one — unpack and pack cannot disagree about the shape.
    pub layout: Layout,
    pub enums: HashMap<String, HashMap<i64, String>>,
}

/// Decrypt `system_dir/name` and decode it with whichever schema candidate
/// consumes the file **exactly**.
///
/// That test is the whole point (see the module header): nothing in a `.dat` is
/// self-describing past the first field, so a subtly wrong schema still reads
/// *something*. The layout that accounts for every byte is the one that
/// describes the file, and keeping it is what lets `--chronicle auto` work.
///
/// Everything stays in memory — no `system_decrypted` is read or written — so
/// an editing or sync run leaves no trace on disk unless its caller saves.
///
/// `progress` is called with a one-word stage name before each slow step; pass
/// `&mut |_| {}` where there is no UI to feed.
pub fn unpack(
    set: &mut crate::dat_schema::SchemaSet,
    system_dir: &Path,
    name: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<Unpacked, String> {
    progress("decrypting");
    let path = system_dir.join(name);
    let raw = std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let version = crate::client_dat::read_version(&raw)
        .ok_or_else(|| format!("{} has no Lineage2Ver header", path.display()))?;
    let plain = crate::client_dat::decrypt(&raw, &version)?;

    progress("reading");
    let enums = set.enums.clone();
    let (text, layout) = set
        .candidates(name)
        .into_iter()
        .find_map(|(_label, layout)| {
            let outcome = read(&plain, &layout, &enums, false);
            outcome.exact().then_some((outcome.text, layout))
        })
        .ok_or_else(|| format!("no schema layout fits {name}"))?;

    Ok(Unpacked {
        path,
        text,
        version,
        layout,
        enums,
    })
}

/// How a walk ended.
pub struct Outcome {
    pub text: String,
    /// Byte offset the walk stopped at.
    pub position: usize,
    pub total: usize,
    /// Bytes the layout says should be left over (the `SafePackage` trailer).
    pub expected_tail: usize,
    pub error: Option<String>,
}

impl Outcome {
    /// Whether the schema accounted for every byte — the correctness test.
    pub fn exact(&self) -> bool {
        self.error.is_none() && self.position + self.expected_tail == self.total
    }

    pub fn summary(&self) -> String {
        match &self.error {
            Some(e) => format!("failed at byte {}/{}: {e}", self.position, self.total),
            None if self.exact() => format!("ok, {} bytes", self.total),
            None => format!(
                "stopped at {}/{} ({} bytes unread)",
                self.position,
                self.total,
                self.total as i64 - self.expected_tail as i64 - self.position as i64
            ),
        }
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err(format!(
                "ran past the end of the file (wanted {n} bytes at {}, {} left)",
                self.pos,
                self.data.len() - self.pos
            ));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    /// Unreal compact index. Byte 0: bit 7 = sign, bit 6 = continue, bits 0-5
    /// are the low payload; later bytes carry 7 bits each.
    fn compact(&mut self) -> Result<i64, String> {
        let mut out: i64 = 0;
        let mut signed = false;
        for i in 0..5 {
            let x = self.u8()? as i64;
            if i == 0 {
                signed = x & 0x80 != 0;
                out |= x & 0x3F;
                if x & 0x40 == 0 {
                    break;
                }
            } else if i == 4 {
                out |= (x & 0x1F) << 27;
            } else {
                out |= (x & 0x7F) << (6 + (i - 1) * 7);
                if x & 0x80 == 0 {
                    break;
                }
            }
        }
        Ok(if signed { -out } else { out })
    }
}

const MAX_STRING: i64 = 1_000_000;

fn read_field(cur: &mut Cursor, field: Field) -> Result<Value, String> {
    Ok(match field {
        Field::UChar => Value::Int(cur.u8()? as i8 as i64),
        Field::UByte => Value::Int(cur.u8()? as i64),
        Field::Cntr => Value::Int(cur.compact()?),
        Field::Short => Value::Int(i16::from_le_bytes(cur.take(2)?.try_into().unwrap()) as i64),
        Field::UShort => Value::Int(u16::from_le_bytes(cur.take(2)?.try_into().unwrap()) as i64),
        Field::Int | Field::UInt | Field::MapInt => {
            Value::Int(i32::from_le_bytes(cur.take(4)?.try_into().unwrap()) as i64)
        }
        Field::Long => Value::Int(i64::from_le_bytes(cur.take(8)?.try_into().unwrap())),
        Field::Float => {
            let f = f32::from_le_bytes(cur.take(4)?.try_into().unwrap());
            Value::Str(format_float(f as f64))
        }
        Field::Double => {
            let f = f64::from_le_bytes(cur.take(8)?.try_into().unwrap());
            Value::Str(format_float(f))
        }
        Field::Hex => Value::Str(format!("{:02X}", cur.u8()?)),
        Field::Rgb => {
            let b = cur.take(3)?;
            Value::Str(format!("{:02X}{:02X}{:02X}", b[0], b[1], b[2]))
        }
        Field::Rgba => {
            let b = cur.take(4)?;
            Value::Str(format!("{:02X}{:02X}{:02X}{:02X}", b[0], b[1], b[2], b[3]))
        }
        Field::Ascf => Value::Str(read_ascf(cur)?),
        Field::Unicode => Value::Str(read_unicode(cur)?),
        // A `<write>` constant is the only STRING, and it consumes nothing.
        Field::Str => Value::Str(String::new()),
    })
}

/// Marks a string the client stored as UTF-16 though it would fit in single
/// bytes. Emitted as `w[…]`.
pub(crate) const WIDE: char = '\u{1}';
/// Marks a `UNICODE` string whose stored byte count does *not* include a
/// terminator. Emitted as `n[…]`.
pub(crate) const NO_TERMINATOR: char = '\u{2}';
/// Marks an `ASCF` stored as length 1 — a terminator and no content, which is
/// not the same file as length 0. Emitted as `z[…]`.
pub(crate) const BARE_TERMINATOR: char = '\u{3}';

/// Compact-length string. A negative length means UTF-16LE and `-2 * len`
/// bytes; the stored length counts the terminator, which is dropped.
///
/// Lengths 0 and 1 both read as the empty string — nothing at all, versus a
/// lone terminator — so length 1 keeps the [`BARE_TERMINATOR`] mark. Without
/// it `EULA-eu.dat`, which is four of these, loses a byte per string every
/// time it is packed.
fn read_ascf(cur: &mut Cursor) -> Result<String, String> {
    let len = cur.compact()?;
    if len == 0 {
        return Ok(String::new());
    }
    let size = if len > 0 { len } else { -2 * len };
    if size > MAX_STRING {
        return Err(format!("implausible string length {size}"));
    }
    let bytes = cur.take(size as usize)?;
    Ok(if len > 0 {
        let body = decode_latin1(&bytes[..bytes.len().saturating_sub(1)]);
        if body.is_empty() {
            BARE_TERMINATOR.to_string()
        } else {
            body
        }
    } else {
        // Prefix retained by the caller so packing can reproduce the width.
        format!(
            "{WIDE}{}",
            decode_utf16le(&bytes[..bytes.len().saturating_sub(2)])
        )
    })
}

/// `int32` byte count, then UTF-16 stored as byte-swapped pairs.
///
/// Whether that count includes a terminator is not fixed by the format: most
/// files store one, `L2GameDataName.dat` stores none, and the two read back as
/// the same text. So the terminator-less form keeps the [`NO_TERMINATOR`]
/// mark, exactly as a wide `ASCF` keeps [`WIDE`] — otherwise packing grows
/// every one of that file's 91,155 names by two bytes.
fn read_unicode(cur: &mut Cursor) -> Result<String, String> {
    let size = i32::from_le_bytes(cur.take(4)?.try_into().unwrap()) as i64;
    if size <= 0 {
        return Ok(String::new());
    }
    if size > MAX_STRING {
        return Err(format!("implausible string length {size}"));
    }
    let raw = cur.take(size as usize)?;
    let mut swapped = raw.to_vec();
    for pair in swapped.chunks_exact_mut(2) {
        pair.swap(0, 1);
    }
    // The pairs were stored big-endian-first, so swapping yields UTF-16BE.
    let units: Vec<u16> = swapped
        .chunks_exact(2)
        .map(|p| u16::from_be_bytes([p[0], p[1]]))
        .collect();
    let s = String::from_utf16_lossy(&units);
    // Exactly one terminator comes off, not every trailing NUL: a name that
    // really ends in one has to keep it to come back the same size.
    Ok(match s.strip_suffix('\0') {
        Some(body) => body.to_string(),
        None => format!("{NO_TERMINATOR}{s}"),
    })
}

fn decode_latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn decode_utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Java prints floats via `BigDecimal(Double.toString(v)).toPlainString()`,
/// i.e. shortest round-trip form without exponent notation.
fn format_float(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{:.1}", v);
    }
    let s = format!("{v}");
    if s.contains('e') || s.contains('E') {
        return format!("{v:.10}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
    }
    s
}

struct Walker<'a> {
    cur: Cursor<'a>,
    /// Nesting depth of the cycle being walked, so only the outermost one
    /// breaks the line — a record stays on one line however deep it nests.
    depth: usize,
    vars: HashMap<String, Value>,
    enums: &'a HashMap<String, HashMap<i64, String>>,
    use_enums: bool,
}

/// Render `data` using `layout` as a tab-separated token stream.
pub fn read(
    data: &[u8],
    layout: &Layout,
    enums: &HashMap<String, HashMap<i64, String>>,
    use_enums: bool,
) -> Outcome {
    let mut walker = Walker {
        cur: Cursor { data, pos: 0 },
        depth: 0,
        vars: HashMap::new(),
        enums,
        use_enums,
    };
    let mut out = String::new();
    let error = walker.walk(&layout.nodes, 1, None, &mut out).err();
    Outcome {
        text: out.trim_end().to_string(),
        position: walker.cur.pos,
        total: data.len(),
        expected_tail: if layout.safe_package {
            SAFE_PACKAGE_LEN
        } else {
            0
        },
        error,
    }
}

/// Append one token, tab-separated from whatever came before.
fn emit(out: &mut String, token: &str) {
    if !out.is_empty() && !out.ends_with('\t') && !out.ends_with('\n') {
        out.push('\t');
    }
    out.push_str(token);
}

impl Walker<'_> {
    fn walk(
        &mut self,
        nodes: &[Node],
        cycles: i64,
        cycle_name: Option<&str>,
        out: &mut String,
    ) -> Result<(), String> {
        if cycles <= 0 {
            return Ok(());
        }
        if cycle_name.is_some() {
            self.depth += 1;
        }
        for _ in 0..cycles {
            if let Some(name) = cycle_name {
                emit(out, &format!("{name}_begin"));
            }
            for node in nodes {
                self.walk_node(node, out)?;
            }
            if let Some(name) = cycle_name {
                emit(out, &format!("{name}_end"));
                if self.depth == 1 {
                    out.push_str("\r\n");
                }
            }
        }
        if cycle_name.is_some() {
            self.depth -= 1;
        }
        Ok(())
    }

    fn walk_node(&mut self, node: &Node, out: &mut String) -> Result<(), String> {
        match node {
            // Schema-supplied literal text. It carries no data, so it is a
            // comment as far as the round trip is concerned; the newlines it
            // asks for are the only part worth keeping.
            Node::Constant { text } => {
                if text.contains('\n') {
                    out.push_str("\r\n");
                }
            }

            Node::If {
                param,
                val,
                negate,
                children,
            } => {
                let hit = self.vars.get(param).is_some_and(|v| v.matches(val));
                if hit != *negate {
                    self.walk(children, 1, None, out)?;
                }
            }

            Node::Mask {
                param,
                val,
                children,
            } => {
                let hit = self
                    .vars
                    .get(param)
                    .and_then(Value::as_count)
                    .is_some_and(|v| v & val == *val);
                if hit {
                    self.walk(children, 1, None, out)?;
                }
            }

            // A wrapper only groups fields; the schema knows its shape, so it
            // needs no marker of its own.
            Node::Wrapper { children, .. } => self.walk(children, 1, None, out)?,

            Node::Cycle {
                name,
                count,
                children,
                ..
            } => {
                let times = match count {
                    Count::Fixed(n) => *n,
                    Count::Var(var) => self
                        .vars
                        .get(var)
                        .and_then(Value::as_count)
                        .ok_or_else(|| format!("cycle `{name}` needs unread variable `{var}`"))?,
                };
                if times > 10_000_000 {
                    return Err(format!("cycle `{name}` wants {times} iterations"));
                }
                // Every cycle is labelled, hidden or not: the label is what
                // makes the record count recoverable when packing.
                self.walk(children, times, Some(name), out)?;
            }

            Node::Value {
                name,
                field,
                enum_name,
                iterator,
                ..
            } => {
                let value = read_field(&mut self.cur, *field)
                    .map_err(|e| format!("field `{name}` ({field:?}): {e}"))?;
                if *iterator {
                    // Redundant with the record count that follows; packing
                    // recomputes it, so it never reaches the text.
                    self.vars.insert(name.clone(), value);
                    return Ok(());
                }
                let token = match (&value, field) {
                    // The leading mark, when there is one, says how the client
                    // stored this string; the text alone cannot.
                    (Value::Str(s), Field::Ascf | Field::Unicode) => {
                        let (prefix, body) = match s.chars().next() {
                            Some(WIDE) => ("w", &s[WIDE.len_utf8()..]),
                            Some(NO_TERMINATOR) => ("n", &s[NO_TERMINATOR.len_utf8()..]),
                            Some(BARE_TERMINATOR) => ("z", &s[BARE_TERMINATOR.len_utf8()..]),
                            _ => ("", s.as_str()),
                        };
                        format!("{prefix}[{}]", escape_string(body))
                    }
                    (Value::Str(s), _) => s.clone(),
                    (Value::Int(i), _) => enum_name
                        .as_ref()
                        .filter(|_| self.use_enums)
                        .and_then(|e| self.enums.get(e))
                        .and_then(|m| m.get(i))
                        .cloned()
                        .unwrap_or_else(|| i.to_string()),
                };
                emit(out, &token);
                self.vars.insert(name.clone(), value);
            }
        }
        Ok(())
    }
}

/// Make a string safe to sit inside `[...]` in a tab-separated stream. Unlike
/// L2ClientDat we escape the terminators, so a `]` or a tab in game text can
/// survive the round trip.
///
/// Public because the sync tools have to write text that reads back through
/// [`unescape_string`] byte for byte; they each carried a private copy of this
/// before, which is a round-trip bug waiting for someone to add an escape here
/// and not there.
pub fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ']' => out.push_str("\\]"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Wrap a string the way [`read`] would have emitted it, so a repack
/// round-trips.
///
/// Text that will not fit in single bytes packs as UTF-16, which the reader
/// marks `w[...]` — 40 rows of the shipped `NpcName` are such names, and 53
/// retail system messages end in a `…`. Writing a plain `[...]` for those makes
/// the file fail its own round-trip check, which is how the case was caught
/// rather than shipped.
pub fn bracket(text: &str) -> String {
    let escaped = escape_string(text);
    if text.chars().any(|c| (c as u32) >= 0x100) {
        format!("w[{escaped}]")
    } else {
        format!("[{escaped}]")
    }
}

/// Inverse of [`escape_string`].
pub fn unescape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(']') => out.push(']'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

// --- directory driver -------------------------------------------------------

/// What to unpack. Nothing here prints or reads the environment.
pub struct Config<'a> {
    pub in_dir: &'a Path,
    pub out_dir: &'a Path,
    /// A chronicle name, or `None` to try every layout and keep the one that
    /// consumes the file exactly.
    pub chronicle: Option<&'a str>,
    /// Replace enum-valued integers with their labels from `enums/base.xml`.
    pub use_enums: bool,
}

pub struct Entry {
    pub file: String,
    /// `chronicle/version` of the layout that won, when one did.
    pub layout: Option<String>,
    pub detail: String,
    pub ok: bool,
}

pub struct Report {
    pub entries: Vec<Entry>,
}

impl Report {
    pub fn ok_count(&self) -> usize {
        self.entries.iter().filter(|e| e.ok).count()
    }

    pub fn failures(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| !e.ok)
    }
}

/// Unpack every `.dat` under `in_dir` into `<out_dir>/<name>.txt`.
///
/// A file is only written when some layout consumed it exactly; a partial parse
/// is reported instead, because a `.txt` from a drifting walk would repack into
/// a corrupt `.dat`.
pub fn unpack_dir(set: &mut crate::dat_schema::SchemaSet, cfg: &Config) -> Result<Report, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(cfg.in_dir)
        .map_err(|e| format!("cannot read {}: {e}", cfg.in_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dat")))
        .collect();
    files.sort();
    std::fs::create_dir_all(cfg.out_dir)
        .map_err(|e| format!("cannot create {}: {e}", cfg.out_dir.display()))?;

    let enums = set.enums.clone();
    let mut entries = Vec::new();
    for path in files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                entries.push(Entry {
                    file: name,
                    layout: None,
                    detail: format!("read: {e}"),
                    ok: false,
                });
                continue;
            }
        };

        let candidates = match cfg.chronicle {
            Some(c) => match set.layout_for(c, &name) {
                Ok(l) => vec![(format!("{c}/{}", l.version), l)],
                Err(e) => {
                    entries.push(Entry {
                        file: name,
                        layout: None,
                        detail: e,
                        ok: false,
                    });
                    continue;
                }
            },
            None => set.candidates(&name),
        };
        if candidates.is_empty() {
            entries.push(Entry {
                file: name,
                layout: None,
                detail: "no schema links this file".to_string(),
                ok: false,
            });
            continue;
        }

        // Keep the best near-miss so a failure can say how far it got.
        let mut best: Option<(String, Outcome)> = None;
        for (label, layout) in candidates {
            let outcome = read(&data, &layout, &enums, cfg.use_enums);
            if outcome.exact() {
                best = Some((label, outcome));
                break;
            }
            if best
                .as_ref()
                .is_none_or(|(_, o)| outcome.position > o.position)
            {
                best = Some((label, outcome));
            }
        }

        let (label, outcome) = best.expect("candidates was non-empty");
        if outcome.exact() {
            let dst = cfg.out_dir.join(format!("{name}.txt"));
            match std::fs::write(&dst, &outcome.text) {
                Ok(()) => entries.push(Entry {
                    file: name,
                    layout: Some(label),
                    detail: outcome.summary(),
                    ok: true,
                }),
                Err(e) => entries.push(Entry {
                    file: name,
                    layout: Some(label),
                    detail: format!("write: {e}"),
                    ok: false,
                }),
            }
        } else {
            entries.push(Entry {
                file: name,
                layout: Some(label),
                detail: outcome.summary(),
                ok: false,
            });
        }
    }
    Ok(Report { entries })
}

// --- test hooks -------------------------------------------------------------
//
// `dat_pack`'s encoders are only correct if they invert these exactly, so its
// tests decode through the very same code the reader uses rather than a copy.

/// Decode a compact int, returning the value and how many bytes it used.
pub fn decode_compact_for_test(bytes: &[u8]) -> Option<(i64, usize)> {
    let mut cur = Cursor {
        data: bytes,
        pos: 0,
    };
    let value = cur.compact().ok()?;
    Some((value, cur.pos))
}

pub fn decode_ascf_for_test(bytes: &[u8]) -> Option<String> {
    read_ascf(&mut Cursor {
        data: bytes,
        pos: 0,
    })
    .ok()
}

pub fn decode_unicode_for_test(bytes: &[u8]) -> Option<String> {
    read_unicode(&mut Cursor {
        data: bytes,
        pos: 0,
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cur(data: &[u8]) -> Cursor<'_> {
        Cursor { data, pos: 0 }
    }

    #[test]
    fn compact_int_matches_the_java_encoding() {
        // Single byte, no continuation.
        assert_eq!(cur(&[0x0A]).compact().unwrap(), 10);
        // Sign bit set on the first byte.
        assert_eq!(cur(&[0x8A]).compact().unwrap(), -10);
        // Continuation: 6 low bits, then 7 more.
        assert_eq!(cur(&[0x52, 0x01]).compact().unwrap(), 0x12 | (1 << 6));
        assert_eq!(cur(&[0x40, 0x00]).compact().unwrap(), 0);
    }

    #[test]
    fn ascf_reads_both_the_ascii_and_the_utf16_form() {
        // len 10 (positive) -> 9 chars plus the terminator.
        let mut d = vec![0x0A];
        d.extend_from_slice(b"Equipment\0");
        assert_eq!(read_ascf(&mut cur(&d)).unwrap(), "Equipment");

        // A negative length selects UTF-16LE, -2*len bytes. The decoder keeps
        // a \u{1} sentinel so the emitter can record that the client stored
        // this string wide even when it would fit in single bytes.
        let mut d = vec![0x83]; // -3
        d.extend_from_slice(&[b'h', 0, b'i', 0, 0, 0]);
        assert_eq!(read_ascf(&mut cur(&d)).unwrap(), "\u{1}hi");
    }

    #[test]
    fn safe_package_trailer_is_thirteen_bytes() {
        let mut d = vec![0x0C];
        d.extend_from_slice(b"SafePackage\0");
        assert_eq!(d.len(), SAFE_PACKAGE_LEN);
        assert_eq!(read_ascf(&mut cur(&d)).unwrap(), "SafePackage");
    }

    #[test]
    fn floats_print_without_exponents() {
        assert_eq!(format_float(1.0), "1.0");
        assert_eq!(format_float(-0.5), "-0.5");
        assert_eq!(format_float(0.0), "0.0");
    }

    /// The oracle itself: a walk that stops short is not `exact`.
    #[test]
    fn exact_requires_every_byte_including_the_trailer() {
        let o = Outcome {
            text: String::new(),
            position: 100,
            total: 113,
            expected_tail: SAFE_PACKAGE_LEN,
            error: None,
        };
        assert!(o.exact());
        let short = Outcome { position: 90, ..o };
        assert!(!short.exact());
    }
}
