//! Lines of a file, newest first — `docs/MONITORING.md` §6, step 2.
//!
//! Reads fixed-size chunks backwards from an end offset. A line that
//! straddles a chunk boundary is reassembled before it is returned, so every
//! line comes back whole and exactly once, and a search pattern can never be
//! missed for having been split across two reads.

use std::io::{Read, Seek, SeekFrom};

/// Production chunk size. Big enough that a typical day's file is a few
/// hundred reads; small enough that stopping early wastes little.
pub const CHUNK: usize = 256 * 1024;

/// A line longer than this is skipped (and counted), never buffered whole: a
/// file with no newlines in it must not become one giant allocation.
pub const MAX_LINE: usize = 1024 * 1024;

pub struct ReverseLines<R> {
    reader: R,
    chunk: usize,
    /// Bytes before this offset have not been read yet.
    pos: u64,
    /// The head of the oldest line seen so far, whose start lies before
    /// `pos` — the next chunk read completes it.
    carry: Vec<u8>,
    /// Inside a line already over `MAX_LINE`: discard until its start.
    oversized: bool,
    /// Complete lines from the last chunk, oldest first; popped from the end.
    pending: Vec<(u64, Vec<u8>)>,
    pub bytes_read: u64,
    pub skipped_oversized: u64,
    /// Offset of the oldest line the caller has finished with — where a
    /// search that stops now resumes. Starts at `end`: nothing consumed yet.
    pub consumed_to: u64,
}

impl<R: Read + Seek> ReverseLines<R> {
    /// Lines that *end* at or before `end`. For a whole file, `end` is its
    /// length; to resume, it is the offset of the oldest line already
    /// returned, which always sits just after a `\n`.
    pub fn new(reader: R, end: u64, chunk: usize) -> Self {
        Self {
            reader,
            chunk: chunk.max(1),
            pos: end,
            carry: Vec::new(),
            oversized: false,
            pending: Vec::new(),
            bytes_read: 0,
            skipped_oversized: 0,
            consumed_to: end,
        }
    }

    /// The next-older line and the byte offset it starts at. The trailing
    /// `\n` is not included; empty lines are skipped.
    pub fn next_line(&mut self) -> std::io::Result<Option<(u64, Vec<u8>)>> {
        loop {
            if let Some(line) = self.pending.pop() {
                return Ok(Some(line));
            }
            if self.pos == 0 {
                if self.oversized {
                    self.oversized = false;
                    self.skipped_oversized += 1;
                }
                let carry = std::mem::take(&mut self.carry);
                return Ok((!carry.is_empty()).then_some((0, carry)));
            }
            let start = self.pos.saturating_sub(self.chunk as u64);
            let mut buf = vec![0u8; (self.pos - start) as usize];
            self.reader.seek(SeekFrom::Start(start))?;
            self.reader.read_exact(&mut buf)?;
            self.bytes_read += buf.len() as u64;
            self.pos = start;

            let Some(first_nl) = memchr::memchr(b'\n', &buf) else {
                // The whole chunk is the middle of one line.
                self.grow_carry(buf);
                continue;
            };
            // Everything after the first newline is whole lines; the last of
            // them is completed at its end by the carry from the previous
            // (later) chunk.
            let tail = buf.split_off(first_nl + 1);
            buf.truncate(first_nl);
            let tail_start = start + first_nl as u64 + 1;
            let mut line_start = 0;
            for nl in memchr::memchr_iter(b'\n', &tail) {
                self.push(tail_start + line_start as u64, &tail[line_start..nl]);
                line_start = nl + 1;
            }
            if self.oversized {
                // The last segment is the head of the oversized line, which
                // starts here: drop it along with the discarded rest.
                self.oversized = false;
                self.skipped_oversized += 1;
            } else {
                let mut last = tail[line_start..].to_vec();
                last.append(&mut self.carry);
                self.push(tail_start + line_start as u64, &last);
            }
            // The bytes before the first newline end a line that starts
            // further back; they wait for the next chunk.
            self.carry.clear();
            self.grow_carry(buf);
        }
    }

    fn push(&mut self, offset: u64, line: &[u8]) {
        if line.len() > MAX_LINE {
            self.skipped_oversized += 1;
        } else if !line.is_empty() {
            self.pending.push((offset, line.to_vec()));
        }
    }

    /// Prepend `head` to the carry (it precedes it in the file).
    fn grow_carry(&mut self, mut head: Vec<u8>) {
        if self.oversized {
            return;
        }
        head.append(&mut self.carry);
        if head.len() > MAX_LINE {
            self.oversized = true;
        } else {
            self.carry = head;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn all(data: &[u8], end: u64, chunk: usize) -> Vec<(u64, String)> {
        let mut r = ReverseLines::new(Cursor::new(data.to_vec()), end, chunk);
        let mut out = Vec::new();
        while let Some((off, line)) = r.next_line().unwrap() {
            out.push((off, String::from_utf8(line).unwrap()));
        }
        out
    }

    /// Forward reference: what a plain split would say, newest first.
    fn expected(data: &[u8]) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        let mut start = 0;
        for (i, b) in data.iter().enumerate() {
            if *b == b'\n' {
                if i > start {
                    out.push((
                        start as u64,
                        String::from_utf8(data[start..i].to_vec()).unwrap(),
                    ));
                }
                start = i + 1;
            }
        }
        if start < data.len() {
            out.push((
                start as u64,
                String::from_utf8(data[start..].to_vec()).unwrap(),
            ));
        }
        out.reverse();
        out
    }

    #[test]
    fn every_chunk_size_yields_every_line_whole_and_once() {
        let data = b"alpha\nbravo charlie\n\ndelta\nechoechoechoecho\nf\n";
        for chunk in 1..=data.len() + 3 {
            assert_eq!(
                all(data, data.len() as u64, chunk),
                expected(data),
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn a_last_line_without_a_newline_is_still_a_line() {
        let data = b"one\ntwo\nthree";
        for chunk in [1, 2, 4, 64] {
            assert_eq!(
                all(data, data.len() as u64, chunk),
                expected(data),
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn resuming_from_a_line_offset_continues_with_no_gap_or_repeat() {
        let data = b"l1\nline2\nl3\nline4\nl5\n";
        let mut r = ReverseLines::new(Cursor::new(data.to_vec()), data.len() as u64, 3);
        let (off5, _) = r.next_line().unwrap().unwrap();
        let (off4, l4) = r.next_line().unwrap().unwrap();
        assert_eq!(l4, b"line4");
        // Stop here; resume from the oldest line returned.
        let resumed = all(data, off4, 3);
        let texts: Vec<&str> = resumed.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(texts, vec!["l3", "line2", "l1"]);
        assert!(off5 > off4);
    }

    #[test]
    fn oversized_lines_are_skipped_without_losing_neighbours() {
        let mut data = b"before\n".to_vec();
        data.extend(std::iter::repeat_n(b'x', MAX_LINE + 10));
        data.extend(b"\nafter\n");
        for chunk in [4096, MAX_LINE / 3, CHUNK] {
            let mut r = ReverseLines::new(Cursor::new(data.clone()), data.len() as u64, chunk);
            let mut lines = Vec::new();
            while let Some((_, l)) = r.next_line().unwrap() {
                lines.push(String::from_utf8(l).unwrap());
            }
            assert_eq!(lines, vec!["after", "before"], "chunk {chunk}");
            assert_eq!(r.skipped_oversized, 1, "chunk {chunk}");
        }
    }

    #[test]
    fn bytes_read_counts_what_was_scanned() {
        let data = b"aaaa\nbbbb\ncccc\n";
        let mut r = ReverseLines::new(Cursor::new(data.to_vec()), data.len() as u64, 5);
        r.next_line().unwrap();
        assert_eq!(
            r.bytes_read, 10,
            "two 5-byte chunks to complete the newest line"
        );
    }
}
