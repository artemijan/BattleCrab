//! Surgical edits to the datapack's NPC XML, for the client-to-datapack half
//! of [`crate::npc_sync`].
//!
//! # Why this is not a parser
//!
//! `dist/game/data/stats/npcs` is hand-maintained: it carries a BOM on 181 of
//! its 182 files, comments recording where a value was confirmed, and an
//! attribute order a reader would not reproduce. Round-tripping it through an
//! XML library would rewrite all 182 files to that library's taste and bury the
//! handful of real edits in a diff nobody can review. So an edit here replaces
//! one attribute inside one line and leaves every other byte of the file alone.
//!
//! That is only safe because the shape is regular, and it is checked rather
//! than assumed: on this dist every `<npc …>` opening tag both begins and ends
//! on one line, and `id` is always its first attribute.
//!
//! # Templates versus minions
//!
//! `<npc …>` appears twice in these files. The 14,866 *template* tags open an
//! element and carry `level=`; the 873 rows inside `<minions>` are
//! self-closing and carry `count=`. Nothing on this dist is both, so the two
//! are told apart structurally — putting a name on a spawn-group row would be
//! silent corruption, since nothing reads it back.

use std::path::{Path, PathBuf};

/// One XML file, held as its lines so an edit can be line-local.
///
/// Lines are split on `\n` and keep whatever else they ended with, so a file
/// that used CRLF, or a leading BOM, survives a rewrite untouched.
pub struct XmlFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub dirty: bool,
}

impl XmlFile {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Write the file back exactly as it was read, bar the edited lines.
    pub fn save(&self) -> Result<(), String> {
        std::fs::write(&self.path, self.lines.join("\n"))
            .map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

/// Every NPC XML the server itself loads, in a stable order.
///
/// Enumerated by the server's own [`gameserver::data::xml::xml_files_in`], so
/// "the files this tool may edit" and "the files the server loads" cannot drift
/// apart. Flat, not recursive, because [`gameserver::data::npc_data::NpcData`]
/// is: it reads this directory's own `.xml` files and stops. (Java also reads
/// `npcs/custom/` when `CUSTOM_NPC_DATA` is set, which this port does not
/// carry — so nothing here would match those ids anyway, and editing files the
/// server never reads would be worse than skipping them.)
///
/// An unreadable directory is therefore reported as an empty one: for a tool
/// that is about to rewrite the datapack, "nothing to edit here" and "could not
/// look" both mean stop, and the path is in the message either way.
pub fn load(dir: &Path) -> Result<Vec<XmlFile>, String> {
    let paths = gameserver::data::xml::xml_files_in(dir);
    if paths.is_empty() {
        return Err(format!("no NPC XML under {}", dir.display()));
    }
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            Ok(XmlFile {
                lines: text.split('\n').map(str::to_owned).collect(),
                path,
                dirty: false,
            })
        })
        .collect()
}

/// The id of the `<npc …>` *template* tag on this line, if it is one.
///
/// The BOM is stripped explicitly: `str::trim` leaves U+FEFF alone (it has not
/// been whitespace in Unicode for twenty years), and on this dist it only ever
/// sits on the `<?xml?>` line — but a detector that quietly skips a tag for a
/// leading byte is the kind that fails on someone else's datapack.
pub fn template_id(line: &str) -> Option<i32> {
    let trimmed = line.trim_start_matches('\u{feff}').trim();
    if !trimmed.starts_with("<npc ") || trimmed.ends_with("/>") || !line.contains(" level=\"") {
        return None;
    }
    attribute(line, "id")?.parse().ok()
}

/// The value of `key="…"` on this line, with entities resolved.
///
/// The leading space in the match is load-bearing: without it, asking for
/// `title` would find `subtitle`.
pub fn attribute(line: &str, key: &str) -> Option<String> {
    let (start, end) = value_span(line, key)?;
    Some(unescape(&line[start..end]))
}

/// Byte range of `key`'s value on this line, excluding the quotes.
fn value_span(line: &str, key: &str) -> Option<(usize, usize)> {
    let needle = format!(" {key}=\"");
    let at = line.find(&needle)?;
    let start = at + needle.len();
    let end = line[start..].find('"')? + start;
    Some((start, end))
}

/// This line with `key` set to `value`, or `None` if it already says that.
///
/// An absent attribute is inserted after the first of `after` that is present,
/// which is how a new `title=` lands in the datapack's own attribute order
/// (`id level type name title`) rather than at the end of the tag.
pub fn set_attribute(line: &str, key: &str, value: &str, after: &[&str]) -> Option<String> {
    let escaped = escape(value);
    if let Some((start, end)) = value_span(line, key) {
        if line[start..end] == escaped {
            return None;
        }
        return Some(format!("{}{escaped}{}", &line[..start], &line[end..]));
    }
    for anchor in after {
        if let Some((_, end)) = value_span(line, anchor) {
            // `end` is the closing quote; the attribute goes after it.
            return Some(format!(
                "{} {key}=\"{escaped}\"{}",
                &line[..=end],
                &line[end + 1..]
            ));
        }
    }
    None
}

/// Make a string safe inside a double-quoted XML attribute. `'` needs no
/// escape there and 1228 names contain one, so leaving it alone keeps the diff
/// to the lines that actually changed.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Inverse of [`escape`], plus `&apos;` for XML this tool did not write.
pub fn unescape(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        // Last, or `&amp;lt;` would decode to `<` instead of `&lt;`.
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str =
        r#"  <npc id="21654" level="81" type="Monster" name="Dreco" title="Earth Guardian">"#;

    #[test]
    fn a_template_tag_is_recognised_by_its_shape() {
        assert_eq!(template_id(TEMPLATE), Some(21654));
    }

    /// The bug this guards: a `<minions>` row is also `<npc id=…`, and naming
    /// one would write a value nothing ever reads back.
    #[test]
    fn a_minion_row_is_not_a_template() {
        let minion = r#"    <npc id="3402" count="3" respawnTime="360" weightPoint="1" />"#;
        assert_eq!(template_id(minion), None);
        let no_level = r#"  <npc id="1" type="Monster">"#;
        assert_eq!(template_id(no_level), None);
        assert_eq!(template_id("  <parameters>"), None);
    }

    #[test]
    fn attributes_read_back_with_entities_resolved() {
        assert_eq!(attribute(TEMPLATE, "name").as_deref(), Some("Dreco"));
        assert_eq!(
            attribute(TEMPLATE, "title").as_deref(),
            Some("Earth Guardian")
        );
        assert_eq!(attribute(TEMPLATE, "level").as_deref(), Some("81"));
        assert_eq!(attribute(TEMPLATE, "displayId"), None);
        let amp = r#"  <npc id="1594" level="55" type="Folk" name="Singer &amp; Dancer">"#;
        assert_eq!(attribute(amp, "name").as_deref(), Some("Singer & Dancer"));
    }

    /// The bug this guards: matching `title=` without the leading space finds
    /// `subtitle=` and edits the wrong attribute.
    #[test]
    fn a_longer_attribute_name_is_not_a_match() {
        let line = r#"  <npc id="1" level="1" type="Monster" subtitle="no">"#;
        assert_eq!(attribute(line, "title"), None);
    }

    #[test]
    fn setting_an_existing_attribute_replaces_only_its_value() {
        let out = set_attribute(TEMPLATE, "name", "Dreko", &["type"]).unwrap();
        assert_eq!(
            out,
            r#"  <npc id="21654" level="81" type="Monster" name="Dreko" title="Earth Guardian">"#
        );
    }

    #[test]
    fn setting_what_is_already_there_is_no_edit() {
        assert_eq!(set_attribute(TEMPLATE, "name", "Dreco", &["type"]), None);
    }

    /// A new attribute lands in the datapack's own order, not at the end.
    #[test]
    fn a_missing_attribute_is_inserted_after_its_anchor() {
        let line = r#"  <npc id="20142" level="60" type="Monster" name="Blood Queen">"#;
        let out = set_attribute(line, "title", "Queen", &["name", "type"]).unwrap();
        assert_eq!(
            out,
            r#"  <npc id="20142" level="60" type="Monster" name="Blood Queen" title="Queen">"#
        );
    }

    #[test]
    fn the_first_anchor_present_wins() {
        let line = r#"  <npc id="1" level="1" type="Monster">"#;
        let out = set_attribute(line, "title", "T", &["name", "type"]).unwrap();
        assert_eq!(out, r#"  <npc id="1" level="1" type="Monster" title="T">"#);
    }

    #[test]
    fn no_anchor_means_no_edit_rather_than_a_mangled_tag() {
        assert_eq!(set_attribute("<npc>", "title", "T", &["name"]), None);
    }

    /// The bug this guards: an unescaped `&` makes the whole file unparseable,
    /// so one bad name would take a hundred NPCs down with it.
    #[test]
    fn values_are_escaped_on_the_way_in() {
        let out = set_attribute(TEMPLATE, "name", "Singer & Dancer", &["type"]).unwrap();
        assert!(out.contains(r#"name="Singer &amp; Dancer""#));
        assert_eq!(attribute(&out, "name").as_deref(), Some("Singer & Dancer"));
    }

    #[test]
    fn escaping_and_reading_back_are_inverses() {
        for text in [
            "Dreco",
            "",
            "a & b",
            "a \" b",
            "a < b > c",
            "Deadman's Lamp",
        ] {
            let line = set_attribute(TEMPLATE, "name", text, &["type"])
                .unwrap_or_else(|| TEMPLATE.to_string());
            assert_eq!(attribute(&line, "name").as_deref(), Some(text));
        }
    }

    /// The bug this guards: unescaping `&amp;` first turns a literal `&lt;`
    /// into a `<` the file never contained.
    #[test]
    fn unescaping_does_not_cascade() {
        assert_eq!(unescape("&amp;lt;"), "&lt;");
        assert_eq!(unescape("plain"), "plain");
    }

    /// A line's trailing `\r`, and the BOM on the first line of 181 files,
    /// must survive an edit that happens in the middle of the line.
    #[test]
    fn an_edit_preserves_everything_around_it() {
        let crlf = format!("{TEMPLATE}\r");
        let out = set_attribute(&crlf, "name", "Dreko", &["type"]).unwrap();
        assert!(out.ends_with(">\r"), "line ending kept: {out:?}");
        let bom = format!("\u{feff}{TEMPLATE}");
        let out = set_attribute(&bom, "name", "Dreko", &["type"]).unwrap();
        assert!(out.starts_with('\u{feff}'), "BOM kept");
        assert_eq!(template_id(&bom), Some(21654));
    }
}
