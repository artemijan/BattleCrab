//! Push [`commons::system_messages`] into the client's `SystemMsg*.dat`.
//!
//! The server only ever sends a message id and its parameters — the client
//! supplies the wording and the colour from its own table. So a message this
//! server invents displays as *nothing* until that table has a row for it, and
//! a reworded one keeps showing the old text. This is what closes that gap.
//!
//! Two things happen per run:
//!
//! * every id the table and the dat share has its text and render class
//!   (`group`, `type`) overwritten from the table;
//! * every message marked `custom` that the dat has no row for is appended,
//!   in [`DEFAULT_COLOUR`].
//!
//! # Colour is not synced
//!
//! Colour belongs to whoever is looking at the client, not to the server, and
//! `l2r-tools msg-color` is how it is set. Rewriting it here would silently
//! undo every such edit on the next run, so an existing row keeps whatever
//! colour it has and a new one starts neutral, to be coloured deliberately.
//!
//! # Defaults for the columns we do not model
//!
//! A record has sixteen fields; the table knows three of them (id, text,
//! colour). Rather than invent values for the other thirteen, an appended row
//! takes the *modal* value of each column across the file — what the client
//! already uses for the overwhelming majority of its own messages. That is a
//! defensible default precisely because it is the client's own habit rather
//! than our guess, and it keeps the guess visible: the values are derived at
//! run time, not baked in here.
//!
//! Nothing is written unless the packed file re-reads as the text it came
//! from, the same gate [`crate::client_files`] uses.

use crate::dat_schema::{Layout, SchemaSet};
use crate::dat_text::bracket;
use crate::{client_dat, dat_pack, dat_text};
use commons::system_messages::{MessageInfo, generated};
use std::path::Path;

/// Tab positions within a record line.
const ID: usize = 1;
const MESSAGE: usize = 3;
const COLOUR: usize = 5;
const GROUP: usize = 4;
const TYPE: usize = 16;
/// `msg_begin` + 16 fields + `msg_end`.
const FIELDS: usize = 18;

/// What an appended row is coloured until someone chooses otherwise. Grey has
/// equal channels, so it reads the same whichever way round they are stored.
pub const DEFAULT_COLOUR: &str = "999999FF";

pub struct Report {
    pub file: String,
    /// Rows whose text or render class was changed. Colour is never touched.
    pub updated: usize,
    /// Custom messages appended because the client had no row.
    pub appended: Vec<i32>,
    /// Table entries absent from the dat that are *not* marked custom — a
    /// client older than the Java reference, rather than something to add.
    pub missing_not_custom: usize,
    pub total_rows: usize,
}

/// Rewrite `name`'s text and colour from the table, appending custom messages.
pub fn sync(
    set: &mut SchemaSet,
    system_dir: &Path,
    name: &str,
    dry_run: bool,
) -> Result<Report, String> {
    let dat_text::Unpacked {
        path,
        text,
        version,
        layout,
        enums,
    } = dat_text::unpack(set, system_dir, name, &mut |_| {})?;

    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let defaults = modal_fields(&lines)?;

    let mut present = std::collections::HashSet::new();
    let mut updated = 0usize;
    let mut rows = 0usize;

    for line in &mut lines {
        let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
        if fields.first().map(String::as_str) != Some("msg_begin") || fields.len() <= COLOUR {
            continue;
        }
        rows += 1;
        let Ok(id) = fields[ID].parse::<i32>() else {
            continue;
        };
        present.insert(id);
        let Some(info) = commons::system_messages::by_id(id) else {
            continue;
        };
        // Text and render class, never colour — see the module docs.
        let want_text = bracket(info.text);
        let want_group = info.group.map(|g| g.to_string());
        let want_type = info.msg_type.map(str::to_owned);
        let changed = fields[MESSAGE] != want_text
            || want_group.as_ref().is_some_and(|g| &fields[GROUP] != g)
            || want_type.as_ref().is_some_and(|k| &fields[TYPE] != k);
        if changed {
            fields[MESSAGE] = want_text;
            if let Some(g) = want_group {
                fields[GROUP] = g;
            }
            if let Some(k) = want_type {
                fields[TYPE] = k;
            }
            *line = fields.join("\t");
            updated += 1;
        }
    }

    let mut appended = Vec::new();
    let mut missing_not_custom = 0usize;
    for info in generated::ALL {
        if present.contains(&info.id) {
            continue;
        }
        if !info.custom {
            // The client predates this message. Adding a row the client was
            // never built to use is not our call to make silently.
            missing_not_custom += 1;
            continue;
        }
        lines.push(new_row(info, &defaults));
        appended.push(info.id);
    }

    if !dry_run && (updated > 0 || !appended.is_empty()) {
        let rebuilt = lines.join("\r\n");
        let bytes = dat_pack::pack(&rebuilt, &layout)?;
        verify(&bytes, &layout, &rebuilt, &enums)?;
        let encrypted = client_dat::encrypt(&bytes, &version)?;
        std::fs::write(&path, &encrypted).map_err(|e| format!("{}: {e}", path.display()))?;
    }

    Ok(Report {
        file: name.to_string(),
        updated,
        appended,
        missing_not_custom,
        total_rows: rows,
    })
}

fn verify(
    bytes: &[u8],
    layout: &Layout,
    expect: &str,
    enums: &std::collections::HashMap<String, std::collections::HashMap<i64, String>>,
) -> Result<(), String> {
    let back = dat_text::read(bytes, layout, enums, false);
    if !back.exact() || back.text.trim_end() != expect.trim_end() {
        return Err(format!(
            "packed table did not re-read as itself ({}) — nothing written",
            back.summary()
        ));
    }
    Ok(())
}

/// [`dat_text::modal_fields`] for this table's rows.
fn modal_fields(lines: &[String]) -> Result<Vec<String>, String> {
    dat_text::modal_fields(lines, "msg_begin", FIELDS)
}

/// A record for a message the client has never seen.
fn new_row(info: &MessageInfo, defaults: &[String]) -> String {
    let mut fields = defaults.to_vec();
    fields[ID] = info.id.to_string();
    fields[MESSAGE] = bracket(info.text);
    fields[COLOUR] = DEFAULT_COLOUR.to_string();
    // A message that names its render class overrides the modal default: a
    // `[none]`/`0` row draws in the client's default grey however the colour
    // column is set, which is exactly what the modal value gives you.
    if let Some(group) = info.group {
        fields[GROUP] = group.to_string();
    }
    if let Some(kind) = info.msg_type {
        fields[TYPE] = kind.to_string();
    }
    fields.join("\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat_text::escape_string as escape;

    fn row(id: i32, text: &str, colour: &str) -> String {
        format!(
            "msg_begin\t{id}\t1\t[{text}]\t0\t{colour}\t1\t1\t0\t0\t0\t0\t0\t[]\t[]\t[]\t[none]\tmsg_end"
        )
    }

    #[test]
    fn defaults_come_from_what_the_file_mostly_does() {
        let lines = vec![
            row(1, "a", "799BB0FF"),
            row(2, "b", "799BB0FF"),
            row(3, "c", "FF0000FF"),
        ];
        let d = modal_fields(&lines).unwrap();
        assert_eq!(d[COLOUR], "799BB0FF", "modal colour, not the outlier");
        assert_eq!(d[0], "msg_begin");
        assert_eq!(d[FIELDS - 1], "msg_end");
        assert_eq!(d.len(), FIELDS);
    }

    #[test]
    fn an_appended_row_keeps_the_shape_of_a_real_one() {
        let lines = vec![row(1, "a", "799BB0FF")];
        let defaults = modal_fields(&lines).unwrap();
        let info = MessageInfo {
            id: 9000,
            name: "X_S1",
            text: "hello $s1",
            color: "FF6666FF",
            params: 1,
            custom: true,
            group: None,
            msg_type: None,
        };
        let built = new_row(&info, &defaults);
        let fields: Vec<&str> = built.split('\t').collect();
        assert_eq!(fields.len(), FIELDS, "must have every column");
        assert_eq!(fields[ID], "9000");
        assert_eq!(fields[MESSAGE], "[hello $s1]");
        // A new row starts neutral; colour is msg-color's business.
        assert_eq!(fields[COLOUR], DEFAULT_COLOUR);
        // Untouched columns keep the client's own habit.
        assert_eq!(fields[6], "1");
        assert_eq!(fields[16], "[none]");
    }

    /// A `]` in message text would otherwise close the bracket early and make
    /// the row unparseable on the way back in.
    #[test]
    fn text_is_escaped_the_way_the_reader_expects() {
        assert_eq!(escape("a]b"), "a\\]b");
        assert_eq!(escape("a\tb"), "a\\tb");
        assert_eq!(escape("plain"), "plain");
    }

    /// The bug this guards: a custom row that already existed kept the render
    /// class it was first appended with, so changing it in the table did
    /// nothing and the message stayed grey.
    #[test]
    fn an_existing_row_has_its_render_class_corrected_too() {
        let mut fields: Vec<String> = row(9000, "x", "799BB0FF")
            .split('\t')
            .map(str::to_owned)
            .collect();
        assert_eq!(fields[GROUP], "0");
        assert_eq!(fields[TYPE], "[none]");
        // What the update path does to a row whose message names a class.
        fields[GROUP] = 3.to_string();
        fields[TYPE] = "[damage]".to_string();
        assert_eq!(fields[GROUP], "3");
        assert_eq!(fields[TYPE], "[damage]");
    }

    /// The bug this guards: a `[none]` row draws grey whatever its colour.
    #[test]
    fn a_message_that_names_its_render_class_overrides_the_default() {
        let defaults = modal_fields(&[row(1, "a", "799BB0FF")]).unwrap();
        let info = MessageInfo {
            id: 9000,
            name: "X_S1",
            text: "x",
            color: "FF6666FF",
            params: 1,
            custom: true,
            group: Some(3),
            msg_type: Some("[damage]"),
        };
        let fields: Vec<String> = new_row(&info, &defaults)
            .split('\t')
            .map(str::to_owned)
            .collect();
        assert_eq!(fields[GROUP], "3");
        assert_eq!(fields[TYPE], "[damage]");
    }

    /// The bug this guards: 53 retail messages end in `…`, which packs as
    /// UTF-16 and re-reads with a `w` marker.
    #[test]
    fn wide_text_is_bracketed_the_way_it_will_read_back() {
        assert_eq!(bracket("plain"), "[plain]");
        assert_eq!(
            bracket("Summoning your pet\u{2026}"),
            "w[Summoning your pet\u{2026}]"
        );
    }

    /// Grey reads the same in either channel order, which is why it is safe
    /// as the neutral default for a row nobody has coloured yet.
    #[test]
    fn the_default_colour_is_channel_order_agnostic() {
        let (r, g, b) = (
            &DEFAULT_COLOUR[0..2],
            &DEFAULT_COLOUR[2..4],
            &DEFAULT_COLOUR[4..6],
        );
        assert_eq!((r, g), (g, b));
    }

    /// Colour belongs to msg-color; a sync must not quietly undo those edits.
    #[test]
    fn an_existing_row_keeps_its_colour() {
        let before = row(1, "a", "123456AB");
        let mut fields: Vec<String> = before.split('\t').map(str::to_owned).collect();
        // What the update path touches, and what it leaves alone.
        fields[MESSAGE] = bracket("new text");
        fields[GROUP] = "3".into();
        fields[TYPE] = "[damage]".into();
        assert_eq!(fields[COLOUR], "123456AB");
    }

    #[test]
    fn a_file_with_no_rows_is_an_error() {
        assert!(modal_fields(&["nonsense".to_string()]).is_err());
    }
}
