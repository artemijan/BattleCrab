//! Packs the text [`crate::dat_text`] produces back into a decrypted `.dat`.
//!
//! This is the exact inverse of the reader's walk: the same schema drives both,
//! so packing is a *guided* parse — at every point the layout says what type to
//! expect, which is the only reason the text is parseable at all (fields sit
//! directly against each other, with no delimiter between an int and the string
//! after it).
//!
//! # Counts are recomputed, never trusted
//!
//! A `<for size="#n">`'s count field is not in the text at all (see
//! [`crate::dat_schema::Node::Value::iterator`]). The packer emits a
//! placeholder where that field belongs and backfills it once it has counted
//! the records that actually follow, so a hand edit that adds or deletes rows
//! cannot desynchronise the file.
//!
//! # The one lossy case
//!
//! The reader brackets strings as `[text]` without escaping a `]` inside them,
//! exactly as L2ClientDat does. A string containing `]` therefore cannot be
//! re-read unambiguously; the packer takes the first `]` and the round-trip
//! test reports any file this affects rather than silently writing a wrong
//! `.dat`.

use crate::dat_schema::{Count, Field, Layout, Node};
use std::collections::HashMap;

/// Emitted bytes, with holes for counts that are not known yet.
enum Chunk {
    Bytes(Vec<u8>),
    /// A `<for>`'s count field, filled in once its records have been counted.
    Hole(Field),
}

struct Packer<'a> {
    text: &'a str,
    pos: usize,
    chunks: Vec<Chunk>,
    /// Iterator field name -> index into `chunks` of its hole.
    holes: HashMap<String, usize>,
    /// Values read so far, for `<if>` / `<mask>` / fixed-size lookups.
    vars: HashMap<String, String>,
}

/// Rebuild the `.dat` body for `text` under `layout`.
pub fn pack(text: &str, layout: &Layout) -> Result<Vec<u8>, String> {
    // The reader ends visible cycles with `\r\n`; treat it as a plain separator
    // the way `DescriptorWriter` does.
    let normalised = text.replace("\r\n", "\t");
    let mut packer = Packer {
        text: &normalised,
        pos: 0,
        chunks: Vec::new(),
        holes: HashMap::new(),
        vars: HashMap::new(),
    };
    packer.walk(&layout.nodes, false)?;
    packer.skip_ws();
    if packer.pos < packer.text.len() {
        return Err(format!(
            "{} characters of text left over at offset {}: {:?}",
            packer.text.len() - packer.pos,
            packer.pos,
            &packer.text[packer.pos..(packer.pos + 40).min(packer.text.len())]
        ));
    }

    let mut out = Vec::new();
    for chunk in packer.chunks {
        match chunk {
            Chunk::Bytes(b) => out.extend_from_slice(&b),
            // A cycle that never ran leaves its count unfilled; it is zero.
            Chunk::Hole(field) => out.extend_from_slice(&encode_int(0, field)?),
        }
    }
    if layout.safe_package {
        out.push(12);
        out.extend_from_slice(b"SafePackage\0");
    }
    Ok(out)
}

impl Packer<'_> {
    fn rest(&self) -> &str {
        &self.text[self.pos..]
    }

    fn skip_ws(&mut self) {
        while self.rest().starts_with(['\t', '\r', '\n']) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, token: &str) -> bool {
        if self.rest().starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    fn push(&mut self, bytes: Vec<u8>) {
        self.chunks.push(Chunk::Bytes(bytes));
    }

    fn walk(&mut self, nodes: &[Node], name_hidden: bool) -> Result<(), String> {
        for node in nodes {
            self.walk_node(node, name_hidden)?;
            if name_hidden {
                self.eat(";");
            }
        }
        Ok(())
    }

    fn walk_node(&mut self, node: &Node, name_hidden: bool) -> Result<(), String> {
        match node {
            // Constants are literal text the reader emitted; skip them.
            Node::Constant { text } => {
                let normalised = text.replace("\r\n", "\t");
                self.eat(&normalised);
            }

            Node::If {
                param,
                val,
                negate,
                children,
            } => {
                let hit = self
                    .vars
                    .get(param)
                    .is_some_and(|v| v.eq_ignore_ascii_case(val));
                if hit != *negate {
                    self.walk(children, name_hidden)?;
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
                    .and_then(|v| v.parse::<i64>().ok())
                    .is_some_and(|v| v & val == *val);
                if hit {
                    self.walk(children, name_hidden)?;
                }
            }

            Node::Wrapper { name, children } => {
                if !name_hidden {
                    self.skip_ws();
                    self.eat(&format!("{name}="));
                }
                self.walk(children, true)?;
            }

            Node::Cycle {
                name,
                count,
                hidden,
                children,
            } => {
                let iterations = if *hidden || name_hidden {
                    self.pack_inline_cycle(children, name_hidden)?
                } else {
                    self.pack_labelled_cycle(name, children)?
                };
                if let Count::Var(var) = count {
                    self.fill_count(var, iterations)?;
                } else if let Count::Fixed(n) = count
                    && *n >= 0
                    && *n != iterations
                {
                    return Err(format!(
                        "cycle `{name}` is fixed at {n} records but the text has {iterations}"
                    ));
                }
            }

            Node::Value {
                name,
                field,
                hidden,
                iterator,
                ..
            } => {
                if *iterator {
                    // Reserve the slot; the matching cycle fills it.
                    self.holes.insert(name.clone(), self.chunks.len());
                    self.chunks.push(Chunk::Hole(*field));
                    return Ok(());
                }
                if !name_hidden && *hidden {
                    self.skip_ws();
                    self.eat(&format!("{name}="));
                }
                let raw = self.read_token(*field)?;
                let bytes = encode(&raw, *field)
                    .map_err(|e| format!("field `{name}` ({field:?}) from {raw:?}: {e}"))?;
                self.push(bytes);
                self.vars.insert(name.clone(), raw);
            }
        }
        Ok(())
    }

    /// `name_begin ... name_end` per record, repeated.
    fn pack_labelled_cycle(&mut self, name: &str, children: &[Node]) -> Result<i64, String> {
        let begin = format!("{name}_begin");
        let end = format!("{name}_end");
        let mut count = 0i64;
        loop {
            self.skip_ws();
            if !self.eat(&begin) {
                break;
            }
            self.walk(children, false)?;
            self.skip_ws();
            if !self.eat(&end) {
                return Err(format!(
                    "record {count} of `{name}` has no `{end}` at offset {}: {:?}",
                    self.pos,
                    &self.rest()[..40.min(self.rest().len())]
                ));
            }
            count += 1;
        }
        Ok(count)
    }

    /// `{a;b;c}` — one `{}` group holding `;`-separated records.
    fn pack_inline_cycle(&mut self, children: &[Node], outer_hidden: bool) -> Result<i64, String> {
        if !self.eat("{") {
            // A hidden cycle with no records prints nothing at all.
            return Ok(0);
        }
        if self.eat("}") {
            return Ok(0);
        }
        let printing = children.iter().filter(|n| prints(n)).count();
        let mut count = 0i64;
        loop {
            // Each record is itself `{...}` when it has more than one field.
            let braced = printing > 1 && self.eat("{");
            self.walk(children, true)?;
            if braced {
                self.eat("}");
            }
            count += 1;
            if self.eat(";") {
                continue;
            }
            break;
        }
        if !self.eat("}") {
            return Err(format!(
                "inline cycle not closed at offset {} (outer_hidden={outer_hidden}): {:?}",
                self.pos,
                &self.rest()[..40.min(self.rest().len())]
            ));
        }
        Ok(count)
    }

    fn fill_count(&mut self, var: &str, count: i64) -> Result<(), String> {
        let index = self
            .holes
            .get(var)
            .copied()
            .ok_or_else(|| format!("no count field was reserved for `{var}`"))?;
        let field = match self.chunks[index] {
            Chunk::Hole(f) => f,
            Chunk::Bytes(_) => return Err(format!("count field `{var}` was already written")),
        };
        self.chunks[index] = Chunk::Bytes(encode_int(count, field)?);
        self.vars.insert(var.to_string(), count.to_string());
        Ok(())
    }

    /// Pull one value's text, using the expected type to know where it ends —
    /// fields sit directly against one another with no separator.
    fn read_token(&mut self, field: Field) -> Result<String, String> {
        self.skip_ws();
        match field {
            Field::Ascf | Field::Unicode => {
                if !self.eat("[") {
                    return Err(format!(
                        "expected a bracketed string at offset {}: {:?}",
                        self.pos,
                        &self.rest()[..40.min(self.rest().len())]
                    ));
                }
                // Unescaped `]` inside a string is genuinely ambiguous; take
                // the first one, and let the round-trip check catch it.
                let end = self
                    .rest()
                    .find(']')
                    .ok_or_else(|| format!("unterminated string at offset {}", self.pos))?;
                let raw = self.rest()[..end].to_string();
                self.pos += end + 1;
                Ok(raw)
            }
            // Fixed-width hex.
            Field::Rgba => self.take_fixed(8),
            Field::Rgb => self.take_fixed(6),
            Field::Hex => self.take_fixed(2),
            Field::Str => Ok(String::new()),
            // Numbers: a maximal run of characters that can belong to one.
            _ => {
                let len = self
                    .rest()
                    .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '+' || c == '.'))
                    .unwrap_or(self.rest().len());
                if len == 0 {
                    return Err(format!(
                        "expected a number at offset {}: {:?}",
                        self.pos,
                        &self.rest()[..40.min(self.rest().len())]
                    ));
                }
                let raw = self.rest()[..len].to_string();
                self.pos += len;
                Ok(raw)
            }
        }
    }

    fn take_fixed(&mut self, n: usize) -> Result<String, String> {
        if self.rest().len() < n {
            return Err(format!("expected {n} hex digits at offset {}", self.pos));
        }
        let raw = self.rest()[..n].to_string();
        self.pos += n;
        Ok(raw)
    }
}

fn prints(node: &Node) -> bool {
    !matches!(
        node,
        Node::Constant { .. } | Node::Value { iterator: true, .. }
    )
}

fn encode_int(value: i64, field: Field) -> Result<Vec<u8>, String> {
    encode(&value.to_string(), field)
}

fn encode(raw: &str, field: Field) -> Result<Vec<u8>, String> {
    let int = || -> Result<i64, String> {
        raw.parse::<i64>()
            .or_else(|_| raw.parse::<f64>().map(|f| f as i64))
            .map_err(|e| e.to_string())
    };
    Ok(match field {
        Field::UChar | Field::UByte => vec![int()? as u8],
        Field::Cntr => encode_compact(int()?),
        Field::Short | Field::UShort => (int()? as i16).to_le_bytes().to_vec(),
        Field::Int | Field::UInt | Field::MapInt => (int()? as i32).to_le_bytes().to_vec(),
        Field::Long => int()?.to_le_bytes().to_vec(),
        Field::Float => (raw.parse::<f64>().map_err(|e| e.to_string())? as f32)
            .to_le_bytes()
            .to_vec(),
        Field::Double => raw
            .parse::<f64>()
            .map_err(|e| e.to_string())?
            .to_le_bytes()
            .to_vec(),
        Field::Hex | Field::Rgb | Field::Rgba => {
            let mut out = Vec::with_capacity(raw.len() / 2);
            for pair in raw.as_bytes().chunks(2) {
                let s = std::str::from_utf8(pair).map_err(|e| e.to_string())?;
                out.push(u8::from_str_radix(s, 16).map_err(|e| e.to_string())?);
            }
            out
        }
        Field::Ascf => encode_ascf(raw),
        Field::Unicode => encode_unicode(raw),
        Field::Str => Vec::new(),
    })
}

/// Inverse of `read_ascf`: single-byte when every char fits, UTF-16LE with a
/// negative length otherwise. The stored length counts the terminator.
fn encode_ascf(s: &str) -> Vec<u8> {
    let text = s.replace("\\r\\n", "\r\n");
    if text.is_empty() {
        return vec![0];
    }
    if text.chars().all(|c| (c as u32) < 0x100) {
        let mut out = encode_compact(text.chars().count() as i64 + 1);
        out.extend(text.chars().map(|c| c as u8));
        out.push(0);
        out
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out = encode_compact(-(units.len() as i64 + 1));
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
        out
    }
}

/// Inverse of `read_unicode`: `int32` byte count, then byte-swapped UTF-16.
fn encode_unicode(s: &str) -> Vec<u8> {
    let text = s.replace("\\r\\n", "\r\n");
    if text.is_empty() {
        return 0i32.to_le_bytes().to_vec();
    }
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    let mut out = ((units.len() * 2) as i32).to_le_bytes().to_vec();
    for u in units {
        // The reader swaps each pair and then reads it big-endian, which nets
        // out to plain UTF-16LE on disk.
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Inverse of the Unreal compact index.
fn encode_compact(value: i64) -> Vec<u8> {
    let negative = value < 0;
    let mut v = value.unsigned_abs();
    let mut first = (v & 0x3F) as u8;
    if negative {
        first |= 0x80;
    }
    v >>= 6;
    if v == 0 {
        return vec![first];
    }
    first |= 0x40;
    let mut out = vec![first];
    // Four more bytes are available; bytes 1-3 carry 7 bits and a continue
    // flag, byte 4 carries the last 5.
    for i in 1..5 {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if i < 4 && v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip against the reader's own primitives.
    #[test]
    fn compact_int_round_trips_through_both_directions() {
        for v in [
            0i64, 1, 10, 63, 64, 100, 8191, 8192, 1_000_000, -1, -10, -64, -100, -100_000,
        ] {
            let encoded = encode_compact(v);
            let decoded = crate::dat_text::decode_compact_for_test(&encoded);
            assert_eq!(decoded, Some((v, encoded.len())), "value {v}");
        }
    }

    #[test]
    fn ascf_round_trips_for_ascii_and_wide_text() {
        for s in [
            "",
            "Equipment",
            "Select the area to watch.",
            "Ma\u{00e7}\u{00e3}",
        ] {
            let bytes = encode_ascf(s);
            assert_eq!(
                crate::dat_text::decode_ascf_for_test(&bytes).as_deref(),
                Some(s),
                "{s:?}"
            );
        }
        // Beyond Latin-1 the encoder must switch to the UTF-16 form.
        let wide = "\u{4f60}\u{597d}";
        let bytes = encode_ascf(wide);
        assert_eq!(bytes[0] & 0x80, 0x80, "wide text needs a negative length");
        assert_eq!(
            crate::dat_text::decode_ascf_for_test(&bytes).as_deref(),
            Some(wide)
        );
    }

    #[test]
    fn unicode_round_trips() {
        for s in ["", "Gremlin", "\u{4f60}\u{597d}"] {
            let bytes = encode_unicode(s);
            assert_eq!(
                crate::dat_text::decode_unicode_for_test(&bytes).as_deref(),
                Some(s),
                "{s:?}"
            );
        }
    }
}
