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
//! Counting is by label: a cycle's records are the `<name>_begin` blocks that
//! follow. Two *sibling* cycles can share both a name and a count field, though
//! — `MTX3_NEW` reads N meshes and then N value pairs, both under
//! `for_mtx3_size_1_` — and then the labels alone cannot say where one cycle's
//! records stop and the next one's start. Such a run is counted as a whole and
//! split evenly between its cycles, which is exactly what sharing one `#size`
//! means. See [`Packer::walk_shared_run`].
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
    tokens: Vec<&'a str>,
    pos: usize,
    chunks: Vec<Chunk>,
    /// Iterator field name -> index into `chunks` of its hole.
    holes: HashMap<String, usize>,
    /// Values read so far, for `<if>` and `<mask>`.
    vars: HashMap<String, String>,
    /// Field type of each filled hole, so a second cycle sharing that count
    /// can re-encode its own tally and compare.
    hole_fields: HashMap<usize, Field>,
}

/// Rebuild the `.dat` body for `text` under `layout`.
pub fn pack(text: &str, layout: &Layout) -> Result<Vec<u8>, String> {
    let tokens: Vec<&str> = text
        .split(['\t', '\r', '\n'])
        .filter(|s| !s.is_empty())
        .collect();
    let mut packer = Packer {
        tokens,
        pos: 0,
        chunks: Vec::new(),
        holes: HashMap::new(),
        vars: HashMap::new(),
        hole_fields: HashMap::new(),
    };
    packer.walk(&layout.nodes, 1, None)?;
    if packer.pos < packer.tokens.len() {
        return Err(format!(
            "{} tokens left over at {}: {:?}",
            packer.tokens.len() - packer.pos,
            packer.pos,
            &packer.tokens[packer.pos..(packer.pos + 6).min(packer.tokens.len())]
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
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).copied()
    }

    fn next_token(&mut self) -> Result<&str, String> {
        let token = self
            .tokens
            .get(self.pos)
            .copied()
            .ok_or_else(|| "ran out of text".to_string())?;
        self.pos += 1;
        Ok(token)
    }

    fn walk(
        &mut self,
        nodes: &[Node],
        cycles: i64,
        cycle_name: Option<&str>,
    ) -> Result<(), String> {
        for _ in 0..cycles {
            if let Some(name) = cycle_name {
                let want = format!("{name}_begin");
                let got = self.next_token()?;
                if got != want {
                    return Err(format!("expected `{want}`, found `{got}`"));
                }
            }
            let mut i = 0;
            while i < nodes.len() {
                // Sibling cycles that share a label are indistinguishable in
                // the text, so they are counted together rather than one by one.
                let run = shared_label_run(nodes, i);
                if run > 1 {
                    self.walk_shared_run(&nodes[i..i + run])?;
                } else {
                    self.walk_node(&nodes[i])?;
                }
                i += run;
            }
            if let Some(name) = cycle_name {
                let want = format!("{name}_end");
                let got = self.next_token()?;
                if got != want {
                    return Err(format!("expected `{want}`, found `{got}`"));
                }
            }
        }
        Ok(())
    }

    fn walk_node(&mut self, node: &Node) -> Result<(), String> {
        match node {
            // Constants are schema-supplied punctuation; they carry no data and
            // the reader does not tokenise them.
            Node::Constant { .. } => {}

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
                    self.walk(children, 1, None)?;
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
                    self.walk(children, 1, None)?;
                }
            }

            Node::Wrapper { children, .. } => self.walk(children, 1, None)?,

            Node::Cycle {
                name,
                count,
                children,
                ..
            } => {
                // Every cycle is labelled, so the record count is simply how
                // many `<name>_begin` tokens follow.
                let begin = format!("{name}_begin");
                let mut iterations = 0i64;
                while self.peek() == Some(begin.as_str()) {
                    self.walk(children, 1, Some(name))?;
                    iterations += 1;
                }
                match count {
                    Count::Var(var) => self.fill_count(var, iterations)?,
                    Count::Fixed(n) if *n >= 0 && *n != iterations => {
                        return Err(format!(
                            "cycle `{name}` is fixed at {n} records but the text has {iterations}"
                        ));
                    }
                    Count::Fixed(_) => {}
                }
            }

            Node::Value {
                name,
                field,
                iterator,
                ..
            } => {
                if *iterator {
                    // Reserve the slot; the matching cycle fills it.
                    self.holes.insert(name.clone(), self.chunks.len());
                    self.chunks.push(Chunk::Hole(*field));
                    return Ok(());
                }
                let raw = self.next_token()?.to_string();
                // `w[...]` marks an ASCF the client stored as UTF-16 even
                // though it would fit in single bytes. Nothing in the text
                // implies that, so the reader records it and we honour it —
                // otherwise every such string would shrink on repack.
                let wide = raw.starts_with("w[");
                let inner = match field {
                    Field::Ascf | Field::Unicode => raw
                        .strip_prefix("w[")
                        .or_else(|| raw.strip_prefix('['))
                        .and_then(|s| s.strip_suffix(']'))
                        .map(crate::dat_text::unescape_string)
                        .ok_or_else(|| {
                            format!("field `{name}` expected a bracketed string, found {raw:?}")
                        })?,
                    _ => raw.clone(),
                };
                let bytes = if wide && *field == Field::Ascf {
                    encode_ascf_utf16(&inner)
                } else {
                    encode(&inner, *field)
                        .map_err(|e| format!("field `{name}` ({field:?}) from {raw:?}: {e}"))?
                };
                self.chunks.push(Chunk::Bytes(bytes));
                self.vars.insert(name.clone(), inner);
            }
        }
        Ok(())
    }

    /// How many balanced `<name>_begin … <name>_end` blocks sit at the cursor.
    ///
    /// Matched by depth rather than by counting `_begin` tokens, so a cycle
    /// nested inside a record under the same name cannot inflate the tally.
    fn count_blocks(&self, name: &str) -> i64 {
        let begin = format!("{name}_begin");
        let end = format!("{name}_end");
        let mut pos = self.pos;
        let mut blocks = 0;
        while self.tokens.get(pos).is_some_and(|t| *t == begin) {
            let mut depth = 0usize;
            loop {
                match self.tokens.get(pos) {
                    Some(t) if *t == begin => depth += 1,
                    Some(t) if *t == end => {
                        depth -= 1;
                        if depth == 0 {
                            pos += 1;
                            break;
                        }
                    }
                    Some(_) => {}
                    // Unbalanced text. Stop here and let the walk itself report
                    // it against the field that actually goes wrong.
                    None => return blocks,
                }
                pos += 1;
            }
            blocks += 1;
        }
        blocks
    }

    /// Walk a run of sibling cycles that share one label and one count field.
    ///
    /// Sharing `size="#n"` means every cycle in the run has the same length, so
    /// the run's blocks divide evenly between them — that division is the only
    /// thing that can tell the cycles apart, since their labels cannot.
    fn walk_shared_run(&mut self, run: &[Node]) -> Result<(), String> {
        let (name, var) = match &run[0] {
            Node::Cycle {
                name,
                count: Count::Var(var),
                ..
            } => (name, var),
            _ => unreachable!("shared_label_run only groups Count::Var cycles"),
        };
        let total = self.count_blocks(name);
        let parts = run.len() as i64;
        if total % parts != 0 {
            return Err(format!(
                "{parts} cycles named `{name}` share the count field `{var}`, so they must \
                 have equal lengths, but the text has {total} records to divide between them"
            ));
        }
        let each = total / parts;
        for node in run {
            let Node::Cycle { children, .. } = node else {
                unreachable!("shared_label_run only groups cycles")
            };
            self.walk(children, each, Some(name))?;
        }
        self.fill_count(var, each)
    }

    fn fill_count(&mut self, var: &str, count: i64) -> Result<(), String> {
        let index = self
            .holes
            .get(var)
            .copied()
            .ok_or_else(|| format!("no count field was reserved for `{var}`"))?;
        // Two cycles can share one count field. That is fine as long as they
        // agree; if they disagree the file is unrepresentable and saying so
        // beats writing a length that matches only one of them.
        let field = match &self.chunks[index] {
            Chunk::Hole(f) => *f,
            Chunk::Bytes(existing) => {
                let field = self
                    .hole_fields
                    .get(&index)
                    .copied()
                    .ok_or_else(|| format!("count field `{var}` was filled without a type"))?;
                if *existing != encode_int(count, field)? {
                    return Err(format!(
                        "count field `{var}` is shared by cycles of different lengths"
                    ));
                }
                return Ok(());
            }
        };
        self.hole_fields.insert(index, field);
        self.chunks[index] = Chunk::Bytes(encode_int(count, field)?);
        self.vars.insert(var.to_string(), count.to_string());
        Ok(())
    }
}

/// How many cycles starting at `nodes[start]` are siblings sharing one label
/// and one count field. Anything else — including a lone cycle — is 1.
///
/// Only a shared *name* makes the text ambiguous; the many `skipWriteSize`
/// cycles in `weapongrp` share `#Enchanted` under distinct names, so each still
/// counts its own blocks and [`Packer::fill_count`] just checks they agree.
fn shared_label_run(nodes: &[Node], start: usize) -> usize {
    let Some(Node::Cycle {
        name,
        count: Count::Var(var),
        ..
    }) = nodes.get(start)
    else {
        return 1;
    };
    nodes[start..]
        .iter()
        .take_while(|n| {
            matches!(n, Node::Cycle { name: n2, count: Count::Var(v2), .. }
                if n2 == name && v2 == var)
        })
        .count()
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
    let text = s;
    if text.is_empty() {
        return vec![0];
    }
    if text.chars().all(|c| (c as u32) < 0x100) {
        let mut out = encode_compact(text.chars().count() as i64 + 1);
        out.extend(text.chars().map(|c| c as u8));
        out.push(0);
        out
    } else {
        encode_ascf_utf16(text)
    }
}

/// The UTF-16 form of ASCF: a negative length, then the units.
fn encode_ascf_utf16(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return vec![0];
    }
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut out = encode_compact(-(units.len() as i64 + 1));
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    out
}

/// Inverse of `read_unicode`: `int32` byte count, then byte-swapped UTF-16.
fn encode_unicode(s: &str) -> Vec<u8> {
    let text = s;
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

// --- directory driver -------------------------------------------------------

/// Pack every `<name>.dat.txt` under `in_dir` back into `<out_dir>/<name>`.
///
/// Each file is verified before it is written: the packed bytes are re-read
/// with the same layout and must reproduce the text they came from. A `.dat`
/// that fails is reported, never written — a corrupt one crashes the client.
pub fn pack_dir(
    set: &mut crate::dat_schema::SchemaSet,
    in_dir: &std::path::Path,
    out_dir: &std::path::Path,
    chronicle: Option<&str>,
) -> Result<Vec<(String, Option<String>)>, String> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(in_dir)
        .map_err(|e| format!("cannot read {}: {e}", in_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".dat.txt"))
        .collect();
    files.sort();
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;

    let enums = set.enums.clone();
    let mut results = Vec::new();
    for path in files {
        let stem = path.file_name().unwrap_or_default().to_string_lossy();
        let name = stem.trim_end_matches(".txt").to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                results.push((name, Some(format!("read: {e}"))));
                continue;
            }
        };
        let candidates = match chronicle {
            Some(c) => set
                .layout_for(c, &name)
                .map(|l| vec![(c.to_string(), l)])
                .unwrap_or_default(),
            None => set.candidates(&name),
        };

        let mut outcome = Some("no layout packed this file".to_string());
        for (_label, layout) in candidates {
            let Ok(bytes) = pack(&text, &layout) else {
                continue;
            };
            // Prove it before writing: re-reading must give back the same text.
            let back = crate::dat_text::read(&bytes, &layout, &enums, false);
            if !back.exact() || back.text.trim_end() != text.trim_end() {
                continue;
            }
            outcome = match std::fs::write(out_dir.join(&name), &bytes) {
                Ok(()) => None,
                Err(e) => Some(format!("write: {e}")),
            };
            break;
        }
        results.push((name, outcome));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(name: &str, field: Field, iterator: bool) -> Node {
        Node::Value {
            name: name.to_string(),
            field,
            hidden: false,
            enum_name: None,
            iterator,
        }
    }

    fn cycle(name: &str, var: &str, children: Vec<Node>) -> Node {
        Node::Cycle {
            name: name.to_string(),
            count: Count::Var(var.to_string()),
            hidden: false,
            children,
        }
    }

    /// The shape `MTX3_NEW2` has: one count, then two cycles under one label —
    /// N meshes and then N value pairs.
    fn mtx3_shaped_layout() -> Layout {
        Layout {
            version: "test".to_string(),
            safe_package: false,
            nodes: vec![
                value("size", Field::UChar, true),
                cycle("grp", "size", vec![value("mesh", Field::MapInt, false)]),
                cycle(
                    "grp",
                    "size",
                    vec![
                        value("v1", Field::UChar, false),
                        value("v2", Field::UChar, false),
                    ],
                ),
            ],
        }
    }

    /// Both cycles print `grp_begin`/`grp_end`, so the labels alone cannot say
    /// where the first cycle's records stop. Counting the run as a whole and
    /// halving it is what keeps such a file packable at all — greedily giving
    /// every block to the first cycle derails on the second cycle's extra
    /// field.
    #[test]
    fn sibling_cycles_sharing_a_label_split_their_records() {
        let layout = mtx3_shaped_layout();
        // Exactly what the reader emits: one record per line.
        let text = "grp_begin\t11\tgrp_end\r\ngrp_begin\t12\tgrp_end\r\n\
                    grp_begin\t1\t2\tgrp_end\r\ngrp_begin\t3\t4\tgrp_end";
        let bytes = pack(text, &layout).expect("packs");

        let mut want = vec![2u8]; // the recomputed count, shared by both cycles
        want.extend_from_slice(&11i32.to_le_bytes());
        want.extend_from_slice(&12i32.to_le_bytes());
        want.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(bytes, want);

        // And it is a genuine round trip: the reader gives the text back.
        let back = crate::dat_text::read(&bytes, &layout, &HashMap::new(), false);
        assert!(back.exact(), "{}", back.summary());
        assert_eq!(back.text, text);
    }

    /// An empty run still has to fill the shared count with zero.
    #[test]
    fn sibling_cycles_sharing_a_label_handle_no_records() {
        let bytes = pack("", &mtx3_shaped_layout()).expect("packs");
        assert_eq!(bytes, vec![0u8]);
    }

    /// One `#size` cannot describe cycles of different lengths, so an edit that
    /// leaves an odd number of records is refused rather than silently split.
    #[test]
    fn sibling_cycles_reject_records_that_do_not_divide_evenly() {
        let text = "grp_begin\t11\tgrp_end\tgrp_begin\t1\t2\tgrp_end\tgrp_begin\t3\t4\tgrp_end";
        let err = pack(text, &mtx3_shaped_layout()).expect_err("3 records, 2 cycles");
        assert!(err.contains("equal lengths"), "{err}");
    }

    /// Cycles under *distinct* labels are unambiguous even when they share a
    /// count — `weapongrp`'s `skipWriteSize` blocks all hang off `#Enchanted` —
    /// so each still counts its own blocks and they are checked against each
    /// other.
    #[test]
    fn distinctly_labelled_cycles_still_share_one_count() {
        let layout = Layout {
            version: "test".to_string(),
            safe_package: false,
            nodes: vec![
                value("size", Field::UChar, true),
                cycle("a", "size", vec![value("x", Field::UChar, false)]),
                cycle("b", "size", vec![value("y", Field::UChar, false)]),
            ],
        };
        let text = "a_begin\t7\ta_end\tb_begin\t8\tb_end";
        assert_eq!(pack(text, &layout).expect("packs"), vec![1u8, 7, 8]);

        let lopsided = "a_begin\t7\ta_end\tb_begin\t8\tb_end\tb_begin\t9\tb_end";
        let err = pack(lopsided, &layout).expect_err("1 vs 2 records");
        assert!(err.contains("different lengths"), "{err}");
    }

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
        // Beyond Latin-1 the encoder must switch to the UTF-16 form, and the
        // decoder marks it so packing reproduces the same width.
        let wide = "\u{4f60}\u{597d}";
        let bytes = encode_ascf(wide);
        assert_eq!(bytes[0] & 0x80, 0x80, "wide text needs a negative length");
        assert_eq!(
            crate::dat_text::decode_ascf_for_test(&bytes).as_deref(),
            Some(format!("\u{1}{wide}").as_str())
        );
        // Forcing the wide form for text that would fit narrow is what keeps a
        // repacked file byte-identical.
        let forced = encode_ascf_utf16("Equipment");
        assert_eq!(
            crate::dat_text::decode_ascf_for_test(&forced).as_deref(),
            Some("\u{1}Equipment")
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
