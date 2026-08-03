//! The client's system-message table, as an editable model.
//!
//! `SystemMsg*-eu.dat` is what the client prints for every server message —
//! the text and the colour it appears in. Once [`crate::client_files`] has
//! turned it into text it is one record per line, so this works on those lines
//! directly rather than re-deriving the schema:
//!
//! ```text
//! msg_begin  0  1  [You have been disconnected...]  2  799BB0FF  1  1  ...  msg_end
//!            ^id                          ^message     ^colour
//! ```
//!
//! The colour is `RRGGBBAA`, one byte each, and the alpha is almost always
//! `FF`. Java's reader calls the first byte "a", but the bytes on disk are in
//! red-green-blue-alpha order — `799BB0FF` is the pale blue the client uses
//! for ordinary notices, not a 47%-opaque orange.
//!
//! Editing a field in place keeps every other token on the line untouched, so
//! nothing outside the colour can drift; the file still has to survive
//! [`crate::dat_pack`]'s verify step before it reaches the client.

use crate::client_files;
use crate::dat_schema::SchemaSet;
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
    path: PathBuf,
    lines: Vec<String>,
    pub messages: Vec<Message>,
}

impl MsgFile {
    /// Read the decrypted text form of `name` from `decrypted_dir`.
    pub fn open(decrypted_dir: &Path, name: &str) -> Result<Self, String> {
        let path = decrypted_dir.join(name);
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "cannot read {} ({e}) — run `client-dat decrypt` first",
                path.display()
            )
        })?;
        let lines: Vec<String> = raw.lines().map(str::to_owned).collect();

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
            let colour = fields[COLOUR].to_ascii_uppercase();
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
                "{} has no `msg_begin` records — is it a system-message file?",
                path.display()
            ));
        }
        Ok(MsgFile {
            name: name.to_string(),
            path,
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

    /// Write the edits back to the decrypted text, then pack and encrypt the
    /// file so the client can read it.
    ///
    /// The text is only rewritten once packing has succeeded, so a failure
    /// leaves both directories as they were rather than a half-applied edit.
    pub fn save(&mut self, set: &mut SchemaSet, cfg: &client_files::Config) -> Result<(), String> {
        let mut lines = self.lines.clone();
        for message in &self.messages {
            let mut fields: Vec<String> =
                lines[message.line].split('\t').map(str::to_owned).collect();
            fields[COLOUR] = message.colour.clone();
            lines[message.line] = fields.join("\t");
        }
        let text = lines.join("\r\n");

        client_files::write_one(set, cfg, &self.name, text.as_bytes())?;

        std::fs::write(&self.path, &text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.lines = lines;
        for message in &mut self.messages {
            message.original_colour = message.colour.clone();
        }
        Ok(())
    }
}

/// Colours the client itself already uses, offered as presets.
pub const PRESETS: [(&str, &str); 10] = [
    ("Notice (default)", "799BB0FF"),
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

    fn open(dir: &Path) -> MsgFile {
        std::fs::write(dir.join("SystemMsg-test.dat"), format!("{LINE}\r\n")).unwrap();
        MsgFile::open(dir, "SystemMsg-test.dat").unwrap()
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
        assert_eq!(m.colour, "799BB0FF");
        // Red-green-blue order, not Java's misleading "a" first.
        assert_eq!(m.rgb(), (0x79, 0x9B, 0xB0));
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
        assert_eq!(file.messages[0].colour, "799BB0FF");
    }

    #[test]
    fn revert_restores_the_colour_the_file_was_opened_with() {
        let mut file = open(&tmp());
        file.set_colour(0, "FF0000FF").unwrap();
        file.revert(0);
        assert_eq!(file.messages[0].colour, "799BB0FF");
        assert_eq!(file.edited_count(), 0);
    }

    #[test]
    fn a_file_with_no_records_is_an_error_not_an_empty_session() {
        let dir = tmp();
        std::fs::write(dir.join("NotMessages.dat"), "something\telse\r\n").unwrap();
        assert!(MsgFile::open(&dir, "NotMessages.dat").is_err());
    }
}
