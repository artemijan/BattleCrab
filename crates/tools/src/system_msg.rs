//! The client's system-message table, as an editable model.
//!
//! `SystemMsg*-eu.dat` is what the client prints for every server message —
//! the text and the colour it appears in. A session owns the whole thing in
//! memory: opening decrypts and unpacks straight from `system/`, so nothing
//! has to have been run first and no intermediate file is left behind, and
//! saving packs and re-encrypts back over the same file. Unpacked, a record
//! is one line:
//!
//! ```text
//! msg_begin  0  1  [You have been disconnected...]  2  799BB0FF  1  1  ...  msg_end
//!            ^id                          ^message     ^colour
//! ```
//!
//! The colour bytes on disk are **blue, green, red, alpha** — not RGB. Java's
//! reader names the first byte "a", which is misleading twice over. A session
//! works in RGBA throughout and swaps only at the file boundary, so a colour
//! typed here is the colour that appears: message 0's `B09B79` is the pale
//! tan the client uses for ordinary notices.
//!
//! Editing a field in place keeps every other token on the line untouched, so
//! nothing outside the colour can drift; the file still has to survive
//! [`crate::dat_pack`]'s verify step before it reaches the client.

use crate::dat_schema::{Layout, SchemaSet};
use crate::{client_dat, dat_pack, dat_text};
use std::path::{Path, PathBuf};

/// Tab-separated position of each field within a record line.
const ID: usize = 1;
const MESSAGE: usize = 3;
const COLOUR: usize = 5;

#[derive(Clone)]
pub struct Message {
    pub id: i64,
    /// Message text with its surrounding brackets stripped.
    pub text: String,
    /// `RRGGBBAA`, uppercase hex.
    pub colour: String,
    /// Colour this message had when the file was opened, so an edit can be
    /// shown as a change and reverted.
    pub original_colour: String,
    line: usize,
}

impl Message {
    pub fn edited(&self) -> bool {
        self.colour != self.original_colour
    }

    /// The `RRGGBB` half, for rendering a swatch.
    pub fn rgb(&self) -> (u8, u8, u8) {
        let byte = |i: usize| {
            u8::from_str_radix(self.colour.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0)
        };
        (byte(0), byte(2), byte(4))
    }
}

pub struct MsgFile {
    pub name: String,
    /// The encrypted file this was read from and will be written back to.
    path: PathBuf,
    /// `Lineage2Ver` code it was enciphered with.
    version: String,
    /// The layout that unpacked it, reused verbatim to pack it again.
    layout: Layout,
    lines: Vec<String>,
    pub messages: Vec<Message>,
}

impl MsgFile {
    /// Decrypt and unpack `name` from the client's `system` directory.
    ///
    /// Everything stays in memory: no `system_decrypted` is read or written,
    /// so an editing session leaves no trace unless it is saved.
    pub fn open(set: &mut SchemaSet, system_dir: &Path, name: &str) -> Result<Self, String> {
        let u = dat_text::unpack(set, system_dir, name, &mut |_| {})?;
        Self::from_text(name, &u.text, u.version, u.layout, u.path)
    }

    /// Parse already-unpacked text into a session. `open` is the normal entry
    /// point; this is separate so the record parsing can be exercised without
    /// a client to decrypt.
    pub fn from_text(
        name: &str,
        text: &str,
        version: String,
        layout: Layout,
        path: PathBuf,
    ) -> Result<Self, String> {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let mut messages = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let fields: Vec<&str> = line.split('\t').collect();
            // Records are `msg_begin ... msg_end`; anything else is not one.
            if fields.first() != Some(&"msg_begin") || fields.len() <= COLOUR {
                continue;
            }
            let Ok(id) = fields[ID].parse::<i64>() else {
                continue;
            };
            let colour = swap_rb(&fields[COLOUR].to_ascii_uppercase());
            messages.push(Message {
                id,
                text: fields[MESSAGE]
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string(),
                original_colour: colour.clone(),
                colour,
                line: index,
            });
        }
        if messages.is_empty() {
            return Err(format!(
                "{name} has no `msg_begin` records — is it a system-message file?"
            ));
        }
        Ok(MsgFile {
            name: name.to_string(),
            path,
            version,
            layout,
            lines,
            messages,
        })
    }

    pub fn edited_count(&self) -> usize {
        self.messages.iter().filter(|m| m.edited()).count()
    }

    /// Set a message's colour. `colour` is `RRGGBB` or `RRGGBBAA`; a 6-digit
    /// value keeps the alpha the message already had.
    pub fn set_colour(&mut self, index: usize, colour: &str) -> Result<(), String> {
        let hex: String = colour.trim().trim_start_matches('#').to_ascii_uppercase();
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("colour must be hex digits".to_string());
        }
        let message = self
            .messages
            .get_mut(index)
            .ok_or_else(|| "no such message".to_string())?;
        let full = match hex.len() {
            8 => hex,
            6 => format!("{hex}{}", &message.colour[6..8]),
            _ => return Err("colour must be RRGGBB or RRGGBBAA".to_string()),
        };
        message.colour = full;
        Ok(())
    }

    pub fn revert(&mut self, index: usize) {
        if let Some(message) = self.messages.get_mut(index) {
            message.colour = message.original_colour.clone();
        }
    }

    /// Pack and re-encrypt the session over the client file.
    ///
    /// The packed bytes are re-read and must reproduce the text they came
    /// from before anything is written, so a `.dat` the client cannot parse is
    /// never produced.
    pub fn save(&mut self, set: &mut SchemaSet) -> Result<(), String> {
        let text = self.to_text();
        let bytes = dat_pack::pack(&text, &self.layout)?;

        let enums = set.enums.clone();
        let back = dat_text::read(&bytes, &self.layout, &enums, false);
        if !back.exact() || back.text.trim_end() != text.trim_end() {
            return Err(format!(
                "packed bytes did not re-read as the same text ({}) — nothing written",
                back.summary()
            ));
        }

        let encrypted = client_dat::encrypt(&bytes, &self.version)?;
        std::fs::write(&self.path, &encrypted)
            .map_err(|e| format!("{}: {e}", self.path.display()))?;

        self.lines = text.lines().map(str::to_owned).collect();
        for message in &mut self.messages {
            message.original_colour = message.colour.clone();
        }
        Ok(())
    }

    /// The session rendered back to unpacked text, edits applied.
    fn to_text(&self) -> String {
        let mut lines = self.lines.clone();
        for message in &self.messages {
            let mut fields: Vec<String> =
                lines[message.line].split('\t').map(str::to_owned).collect();
            fields[COLOUR] = swap_rb(&message.colour);
            lines[message.line] = fields.join("\t");
        }
        lines.join("\r\n")
    }
}

/// Exchange the red and blue bytes of an 8-digit colour.
///
/// The dat stores B,G,R,A and everything above works in R,G,B,A; the swap is
/// its own inverse, so one function serves both directions. Getting this wrong
/// is invisible in the editor and obvious in game — red comes out blue.
fn swap_rb(hex: &str) -> String {
    if hex.len() != 8 {
        return hex.to_string();
    }
    format!("{}{}{}{}", &hex[4..6], &hex[2..4], &hex[0..2], &hex[6..8])
}

/// Colours the client itself already uses, offered as presets.
pub const PRESETS: [(&str, &str); 10] = [
    ("Notice (default)", "B09B79FF"),
    ("White", "FFFFFFFF"),
    ("Yellow", "FFFF00FF"),
    ("Orange", "FFA500FF"),
    ("Red", "FF0000FF"),
    ("Green", "00FF00FF"),
    ("Cyan", "00FFFFFF"),
    ("Blue", "5599FFFF"),
    ("Magenta", "FF00FFFF"),
    ("Grey", "999999FF"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "msg_begin\t0\t1\t[You have been disconnected.]\t2\t799BB0FF\t1\t1\tmsg_end";

    fn session(text: &str) -> Result<MsgFile, String> {
        MsgFile::from_text(
            "SystemMsg-test.dat",
            text,
            "413".into(),
            Layout {
                version: "Helios".into(),
                safe_package: true,
                nodes: Vec::new(),
            },
            PathBuf::from("/nonexistent/SystemMsg-test.dat"),
        )
    }

    fn open(_dir: &Path) -> MsgFile {
        session(LINE).unwrap()
    }

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("l2r-msg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_id_text_and_colour_out_of_a_record() {
        let file = open(&tmp());
        let m = &file.messages[0];
        assert_eq!(m.id, 0);
        assert_eq!(m.text, "You have been disconnected.");
        // The line stores 799BB0FF as B,G,R,A; the session shows it as RGBA.
        assert_eq!(m.colour, "B09B79FF");
        assert_eq!(m.rgb(), (0xB0, 0x9B, 0x79));
        assert!(!m.edited());
    }

    #[test]
    fn six_digit_input_keeps_the_alpha_the_message_had() {
        let mut file = open(&tmp());
        file.set_colour(0, "00FF00").unwrap();
        assert_eq!(file.messages[0].colour, "00FF00FF");
        assert!(file.messages[0].edited());
        assert_eq!(file.edited_count(), 1);
    }

    #[test]
    fn eight_digit_input_sets_the_alpha_too_and_accepts_a_hash() {
        let mut file = open(&tmp());
        file.set_colour(0, "#123456ab").unwrap();
        assert_eq!(file.messages[0].colour, "123456AB");
    }

    #[test]
    fn rubbish_is_refused_rather_than_written() {
        let mut file = open(&tmp());
        assert!(file.set_colour(0, "nothex!!").is_err());
        assert!(file.set_colour(0, "FFF").is_err());
        // The message keeps what it had.
        assert_eq!(file.messages[0].colour, "B09B79FF");
    }

    #[test]
    fn revert_restores_the_colour_the_file_was_opened_with() {
        let mut file = open(&tmp());
        file.set_colour(0, "FF0000FF").unwrap();
        file.revert(0);
        assert_eq!(file.messages[0].colour, "B09B79FF");
        assert_eq!(file.edited_count(), 0);
    }

    /// The bug this guards: picking red showed blue in game.
    #[test]
    fn red_and_blue_are_exchanged_at_the_file_boundary() {
        assert_eq!(swap_rb("FF0000FF"), "0000FFFF");
        assert_eq!(swap_rb(&swap_rb("123456AB")), "123456AB", "its own inverse");
    }

    #[test]
    fn a_file_with_no_records_is_an_error_not_an_empty_session() {
        assert!(session("something\telse").is_err());
    }

    /// Edits go back into the same tab positions, leaving every other field
    /// on the line exactly as it was.
    #[test]
    fn rendering_back_to_text_touches_only_the_colour() {
        let mut file = open(&tmp());
        file.set_colour(0, "FF0000FF").unwrap();
        let out = file.to_text();
        // Written back in the file's own B,G,R,A order: red is 0000FF there.
        assert_eq!(out, LINE.replace("799BB0FF", "0000FFFF"));
    }
}
