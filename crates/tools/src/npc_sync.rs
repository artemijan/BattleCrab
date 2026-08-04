//! Push the datapack's NPC names and titles into the client's `NpcName*.dat`.
//!
//! `NpcInfo` carries a name and a title only for templates flagged
//! `usingServerSideName` / `usingServerSideTitle`. For everyone else the packet
//! says nothing and the client looks the strings up itself, keyed by
//! `displayId`, in `NpcName*.dat`. So a mob this datapack renamed keeps its old
//! name on screen, and one the client has no row for is nameless, until this
//! has run.
//!
//! # The datapack wins, but silence is not a value
//!
//! `dist/` is the specification, so where the XML states a name or a title it
//! overwrites the client's. An *absent* `name=` / `title=` attribute is not a
//! statement that the string is empty, though — it is the datapack declining to
//! say, and for a non-server-side NPC the client's own row is then the only
//! thing that knows. Blanking those would delete retail data that nothing else
//! in this tree carries, so a row keeps what it has and the disagreement is
//! reported instead ([`Report::kept`]).
//!
//! # Colour is not synced
//!
//! The fifth column is the title's render colour — grey for a monster, blue for
//! the summon-and-servitor rows. Nothing in the datapack models it, so an
//! existing row keeps whatever colour it has and an appended one takes the
//! file's own modal colour rather than a value invented here.
//!
//! # Keyed by `displayId`, not by id
//!
//! `NpcInfo` writes `getDisplayId() + 1000000` and the client resolves both the
//! model and the name from that, so the row that governs what a player sees for
//! template X is X's *display* id. Two templates sharing a display id and
//! disagreeing about their strings cannot both be honoured by one row; those
//! are reported and left alone rather than resolved by whichever the map
//! happened to see last.
//!
//! Nothing is written unless the packed file re-reads as the text it came from,
//! the same gate [`crate::client_files`] uses.

use crate::dat_schema::{Layout, SchemaSet};
use crate::{client_dat, dat_pack, dat_text};
use gameserver::data::npc_data::{NpcData, NpcTemplate};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

/// Tab positions within a record line: `npc_begin id [name] [title] colour
/// npc_end`. The trailing columns, if a chronicle ever grows any, are carried
/// through untouched — only these three are ours.
const ID: usize = 1;
const NAME: usize = 2;
const TITLE: usize = 3;
/// Only the tests name the colour column, because the sync never writes it —
/// see the module docs. Its position is here so the row layout reads whole.
#[cfg(test)]
const COLOUR: usize = 4;
/// Shortest row that still has all four fields plus both labels.
const MIN_FIELDS: usize = 6;

/// Stages [`sync`] announces through its progress callback, so a caller can
/// size a bar before starting. A dry run stops after the comparison.
pub const STAGES_DRY: usize = 3;
pub const STAGES_WRITE: usize = 6;

pub struct Options {
    /// Report what would change without writing anything.
    pub dry_run: bool,
    /// Add rows for templates the client has no row for at all.
    pub append: bool,
}

/// One field of one row, as it is and as the datapack would have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: &'static str,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowChange {
    pub id: i32,
    pub fields: Vec<FieldChange>,
}

/// An NPC one side has and the other does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    pub name: String,
    pub title: String,
}

/// A field the datapack leaves unset where the client has something — the
/// client's value is kept, because an absent attribute is not an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub id: i32,
    pub field: &'static str,
    pub value: String,
}

pub struct Report {
    pub file: String,
    /// Rows in the client's table.
    pub total_rows: usize,
    /// Templates in the datapack, after display-id collisions are dropped.
    pub templates: usize,
    pub changed: Vec<RowChange>,
    /// Templates the client has no row for. Always listed, whether or not this
    /// run was allowed to add them — the caller says which by reading
    /// [`Report::written`] and its own `append` flag.
    pub missing: Vec<Entry>,
    /// Client rows no template claims: NPCs of other chronicles, mostly.
    pub orphans: Vec<Entry>,
    pub kept: Vec<Kept>,
    /// Templates with no name *and* no title that the client also lacks —
    /// nothing to say about them, so no row is invented.
    pub skipped_blank: usize,
    /// Display ids claimed by two templates that disagree; left alone.
    pub conflicts: Vec<i32>,
    /// Of the rows touched, how many belong to templates whose strings the
    /// server sends anyway, making the client row cosmetic.
    pub server_side_name: usize,
    pub server_side_title: usize,
    pub written: bool,
}

/// Rewrite `name`'s NPC strings from `npcs`, appending templates it lacks.
pub fn sync(
    set: &mut SchemaSet,
    system_dir: &Path,
    name: &str,
    npcs: &NpcData,
    opts: &Options,
    progress: &mut dyn FnMut(&str),
) -> Result<Report, String> {
    progress("decrypting");
    let path = system_dir.join(name);
    let raw = std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let version = client_dat::read_version(&raw)
        .ok_or_else(|| format!("{} has no Lineage2Ver header", path.display()))?;
    let plain = client_dat::decrypt(&raw, &version)?;

    progress("reading");
    let enums = set.enums.clone();
    let (text, layout) = set
        .candidates(name)
        .into_iter()
        .find_map(|(_label, layout)| {
            let outcome = dat_text::read(&plain, &layout, &enums, false);
            outcome.exact().then_some((outcome.text, layout))
        })
        .ok_or_else(|| format!("no schema layout fits {name}"))?;

    progress("comparing");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let defaults = modal_fields(&lines)?;
    let (wanted, conflicts) = by_display_id(npcs);

    let mut report = Report {
        file: name.to_string(),
        total_rows: 0,
        templates: wanted.len(),
        changed: Vec::new(),
        missing: Vec::new(),
        orphans: Vec::new(),
        kept: Vec::new(),
        skipped_blank: 0,
        conflicts,
        server_side_name: 0,
        server_side_title: 0,
        written: false,
    };
    let mut seen: HashSet<i32> = HashSet::new();

    for line in &mut lines {
        let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
        if !is_row(&fields) {
            continue;
        }
        report.total_rows += 1;
        let Ok(id) = fields[ID].parse::<i32>() else {
            continue;
        };
        seen.insert(id);
        let Some(t) = wanted.get(&id) else {
            report.orphans.push(Entry {
                id,
                name: field_text(&fields[NAME]).unwrap_or_default(),
                title: field_text(&fields[TITLE]).unwrap_or_default(),
            });
            continue;
        };

        let mut change = RowChange {
            id,
            fields: Vec::new(),
        };
        for (index, label, want, server_side) in [
            (NAME, "name", t.name.as_str(), t.server_side_name),
            (TITLE, "title", t.title.as_str(), t.server_side_title),
        ] {
            let have = field_text(&fields[index]);
            match decide(have.as_deref(), want) {
                Verdict::Same | Verdict::Unreadable => {}
                Verdict::KeepBlank => report.kept.push(Kept {
                    id,
                    field: label,
                    value: have.unwrap_or_default(),
                }),
                Verdict::Set(to) => {
                    change.fields.push(FieldChange {
                        field: label,
                        from: have.unwrap_or_default(),
                        to: to.clone(),
                    });
                    fields[index] = bracket(&to);
                    if server_side {
                        match label {
                            "name" => report.server_side_name += 1,
                            _ => report.server_side_title += 1,
                        }
                    }
                }
            }
        }
        if !change.fields.is_empty() {
            *line = fields.join("\t");
            report.changed.push(change);
        }
    }

    let mut additions: Vec<(i32, String)> = Vec::new();
    let mut missing: Vec<i32> = wanted
        .keys()
        .copied()
        .filter(|k| !seen.contains(k))
        .collect();
    missing.sort_unstable();
    for id in missing {
        let t = wanted[&id];
        if t.name.is_empty() && t.title.is_empty() {
            // Nothing to say, so nothing to add: a blank row would only make
            // the next run report a difference it cannot resolve.
            report.skipped_blank += 1;
            continue;
        }
        report.missing.push(Entry {
            id,
            name: t.name.clone(),
            title: t.title.clone(),
        });
        if opts.append {
            additions.push((id, new_row(id, t, &defaults)));
        }
    }

    if !opts.dry_run && (!report.changed.is_empty() || !additions.is_empty()) {
        let rebuilt = merge(lines, additions);
        progress("packing");
        let bytes = dat_pack::pack(&rebuilt, &layout)?;
        verify(&bytes, &layout, &rebuilt, &enums)?;
        progress("encrypting");
        let encrypted = client_dat::encrypt(&bytes, &version)?;
        progress("writing");
        std::fs::write(&path, &encrypted).map_err(|e| format!("{}: {e}", path.display()))?;
        report.written = true;
    }

    Ok(report)
}

/// Splice `additions` (sorted by id) into `lines`, each before the first row
/// with a higher id. The file ships sorted; keeping it that way means a diff of
/// two runs reads as the edit it was rather than as a reshuffle.
fn merge(lines: Vec<String>, additions: Vec<(i32, String)>) -> String {
    let mut pending = additions.into_iter().peekable();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + pending.len());
    for line in lines {
        if let Some(id) = row_id(&line) {
            while pending.peek().is_some_and(|&(new_id, _)| new_id < id) {
                out.push(pending.next().expect("peeked").1);
            }
        }
        out.push(line);
    }
    out.extend(pending.map(|(_, row)| row));
    out.join("\r\n")
}

/// What the sync should do with one field of one row.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Same,
    Set(String),
    /// The datapack says nothing and the client does — keep the client's.
    KeepBlank,
    /// The field is not a bracketed string; leave it entirely alone.
    Unreadable,
}

fn decide(current: Option<&str>, want: &str) -> Verdict {
    match current {
        None => Verdict::Unreadable,
        Some(have) if have == want => Verdict::Same,
        Some(have) if want.is_empty() => {
            if have.is_empty() {
                Verdict::Same
            } else {
                Verdict::KeepBlank
            }
        }
        Some(_) => Verdict::Set(want.to_string()),
    }
}

/// Index the datapack by the id the client keys on, dropping display ids two
/// templates claim with different strings — one row cannot serve both, and
/// picking a winner by iteration order would make the run non-deterministic.
fn by_display_id(npcs: &NpcData) -> (HashMap<i32, &NpcTemplate>, Vec<i32>) {
    let mut map: HashMap<i32, &NpcTemplate> = HashMap::new();
    let mut conflicts: BTreeSet<i32> = BTreeSet::new();
    for t in npcs.all() {
        if let Some(other) = map.insert(t.display_id, t)
            && (other.name != t.name || other.title != t.title)
        {
            conflicts.insert(t.display_id);
        }
    }
    for id in &conflicts {
        map.remove(id);
    }
    (map, conflicts.into_iter().collect())
}

fn is_row(fields: &[String]) -> bool {
    fields.first().map(String::as_str) == Some("npc_begin") && fields.len() >= MIN_FIELDS
}

fn row_id(line: &str) -> Option<i32> {
    let fields: Vec<&str> = line.split('\t').collect();
    (fields.first() == Some(&"npc_begin") && fields.len() >= MIN_FIELDS)
        .then(|| fields[ID].parse().ok())
        .flatten()
}

/// The text inside a `[...]` field, with the reader's escapes undone and its
/// storage marker (`w`/`n`/`z`) dropped. `None` for anything not bracketed.
///
/// Comparing the *text* rather than the rendered token is what keeps a
/// wide-stored name that happens to be ASCII from reading as a difference on
/// every run and being rewritten narrow for nothing.
fn field_text(field: &str) -> Option<String> {
    let body = match field.strip_prefix(['w', 'n', 'z']) {
        Some(rest) if rest.starts_with('[') => rest,
        _ => field,
    };
    let inner = body.strip_prefix('[')?.strip_suffix(']')?;
    Some(dat_text::unescape_string(inner))
}

/// Wrap a string the way the reader would have emitted it, so a repack
/// round-trips. Text that will not fit in single bytes packs as UTF-16, which
/// the reader marks `w[...]` — 40 rows of the shipped table are such names.
fn bracket(text: &str) -> String {
    let escaped = escape(text);
    if text.chars().any(|c| (c as u32) >= 0x100) {
        format!("w[{escaped}]")
    } else {
        format!("[{escaped}]")
    }
}

/// Match [`crate::dat_text`]'s string escaping.
fn escape(s: &str) -> String {
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

/// A record for an NPC the client has never seen. Every column we do not model
/// — the colour, and anything a later chronicle added — takes the file's own
/// modal value, so the row is shaped like the client's own habit rather than
/// like a guess made here.
fn new_row(id: i32, t: &NpcTemplate, defaults: &[String]) -> String {
    let mut fields = defaults.to_vec();
    fields[ID] = id.to_string();
    fields[NAME] = bracket(&t.name);
    fields[TITLE] = bracket(&t.title);
    fields.join("\t")
}

/// The most common value of each column, over rows of the most common arity.
///
/// The arity is derived rather than assumed: `NpcName` has four columns in
/// every chronicle this tree ships, but a table that grew a fifth should carry
/// it through, not be truncated to what this file expected.
fn modal_fields(lines: &[String]) -> Result<Vec<String>, String> {
    let mut arities: HashMap<usize, usize> = HashMap::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.first() == Some(&"npc_begin") && fields.len() >= MIN_FIELDS {
            *arities.entry(fields.len()).or_default() += 1;
        }
    }
    let width = arities
        .into_iter()
        .max_by_key(|&(_, n)| n)
        .map(|(w, _)| w)
        .ok_or_else(|| "no npc_begin rows to derive defaults from".to_string())?;

    let mut counts: Vec<HashMap<&str, usize>> = vec![HashMap::new(); width];
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.first() != Some(&"npc_begin") || fields.len() != width {
            continue;
        }
        for (i, value) in fields.iter().enumerate() {
            *counts[i].entry(value).or_default() += 1;
        }
    }
    Ok(counts
        .into_iter()
        .map(|c| {
            c.into_iter()
                .max_by_key(|&(_, n)| n)
                .map(|(v, _)| v.to_string())
                .unwrap_or_default()
        })
        .collect())
}

fn verify(
    bytes: &[u8],
    layout: &Layout,
    expect: &str,
    enums: &HashMap<String, HashMap<i64, String>>,
) -> Result<(), String> {
    let back = dat_text::read(bytes, layout, enums, false);
    if !back.exact() {
        return Err(format!(
            "packed table did not re-read cleanly ({}) — nothing written",
            back.summary()
        ));
    }
    if let Some(diff) = dat_pack::diff_text(&back.text, expect) {
        return Err(format!(
            "packed table did not re-read as itself: {diff} — nothing written"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i32, name: &str, title: &str, colour: &str) -> String {
        format!("npc_begin\t{id}\t[{name}]\t[{title}]\t{colour}\tnpc_end")
    }

    #[test]
    fn defaults_come_from_what_the_file_mostly_does() {
        let lines = vec![
            row(1, "a", "", "9CE8A9FF"),
            row(2, "b", "", "9CE8A9FF"),
            row(3, "c", "", "3F8BFEFF"),
        ];
        let d = modal_fields(&lines).unwrap();
        assert_eq!(d[COLOUR], "9CE8A9FF", "modal colour, not the outlier");
        assert_eq!(d[0], "npc_begin");
        assert_eq!(d[MIN_FIELDS - 1], "npc_end");
    }

    #[test]
    fn a_file_with_no_rows_is_an_error() {
        assert!(modal_fields(&["nonsense".to_string()]).is_err());
    }

    /// The bug this guards: truncating a wider chronicle's row to six columns
    /// would silently drop whatever the extra column held.
    #[test]
    fn a_wider_table_keeps_its_extra_columns() {
        let wide = format!("{}\textra", row(1, "a", "", "9CE8A9FF"));
        let d = modal_fields(&[wide.clone(), wide]).unwrap();
        assert_eq!(d.len(), 7);
        assert_eq!(d[6], "extra");
    }

    #[test]
    fn a_field_reads_back_through_its_marker_and_escapes() {
        assert_eq!(field_text("[Dreco]").as_deref(), Some("Dreco"));
        assert_eq!(field_text("[]").as_deref(), Some(""));
        assert_eq!(
            field_text("w[Deadman\u{2019}s Lamp]").as_deref(),
            Some("Deadman\u{2019}s Lamp")
        );
        assert_eq!(field_text("[a\\]b]").as_deref(), Some("a]b"));
        assert_eq!(field_text("9CE8A9FF"), None, "a colour is not a string");
    }

    /// The bug this guards: comparing the rendered token instead of the text
    /// makes every wide-stored ASCII name look changed, and rewrites it narrow
    /// on a run that should have been a no-op.
    #[test]
    fn a_wide_marker_alone_is_not_a_difference() {
        assert_eq!(
            decide(field_text("w[Dreco]").as_deref(), "Dreco"),
            Verdict::Same
        );
    }

    /// The bug this guards: an absent `name=` attribute is the datapack
    /// declining to say, not a claim that the name is empty. Writing it would
    /// delete a retail string nothing else in this tree carries.
    #[test]
    fn silence_in_the_datapack_never_blanks_the_client() {
        assert_eq!(decide(Some("Gremlin"), ""), Verdict::KeepBlank);
        assert_eq!(decide(Some(""), ""), Verdict::Same);
        assert_eq!(
            decide(Some("Gremlin"), "Goblin"),
            Verdict::Set("Goblin".into())
        );
    }

    #[test]
    fn an_unreadable_field_is_left_alone() {
        assert_eq!(decide(None, "Goblin"), Verdict::Unreadable);
    }

    #[test]
    fn wide_text_is_bracketed_the_way_it_will_read_back() {
        assert_eq!(bracket("plain"), "[plain]");
        assert_eq!(bracket("Deadman\u{2019}s"), "w[Deadman\u{2019}s]");
        assert_eq!(bracket("a]b"), "[a\\]b]");
    }

    /// A round trip through the two halves must be the identity, or a name
    /// that survives one run is mangled by the next.
    #[test]
    fn bracketing_and_reading_back_are_inverses() {
        for text in ["Dreco", "", "a]b", "Deadman\u{2019}s Lamp", "a\\b"] {
            assert_eq!(field_text(&bracket(text)).as_deref(), Some(text));
        }
    }

    #[test]
    fn an_appended_row_keeps_the_shape_of_a_real_one() {
        let defaults = modal_fields(&[row(1, "a", "", "9CE8A9FF")]).unwrap();
        let mut t = gameserver::data::npc_data::default_template(9000);
        t.name = "Dreco".into();
        t.title = "Earth Guardian".into();
        let built = new_row(9000, &t, &defaults);
        let fields: Vec<&str> = built.split('\t').collect();
        assert_eq!(fields.len(), MIN_FIELDS);
        assert_eq!(fields[ID], "9000");
        assert_eq!(fields[NAME], "[Dreco]");
        assert_eq!(fields[TITLE], "[Earth Guardian]");
        // Colour is nobody's business here — the file's own habit stands.
        assert_eq!(fields[COLOUR], "9CE8A9FF");
        assert_eq!(fields[MIN_FIELDS - 1], "npc_end");
    }

    /// The bug this guards: appending at the end leaves the table unsorted, so
    /// the next diff of the decrypted text reads as a reshuffle.
    #[test]
    fn additions_land_in_id_order() {
        let lines = vec![row(10, "a", "", "9CE8A9FF"), row(30, "c", "", "9CE8A9FF")];
        let merged = merge(
            lines,
            vec![
                (20, row(20, "b", "", "9CE8A9FF")),
                (40, row(40, "d", "", "9CE8A9FF")),
            ],
        );
        let ids: Vec<i32> = merged.lines().filter_map(row_id).collect();
        assert_eq!(ids, vec![10, 20, 30, 40]);
    }

    /// A line the reader emitted that is not a record must survive untouched,
    /// and must not be treated as an insertion point.
    #[test]
    fn non_record_lines_pass_through() {
        let lines = vec!["header".to_string(), row(10, "a", "", "9CE8A9FF")];
        let merged = merge(lines, vec![(5, row(5, "z", "", "9CE8A9FF"))]);
        let first: Vec<&str> = merged.lines().collect();
        assert_eq!(first[0], "header");
        assert_eq!(row_id(first[1]), Some(5));
    }
}
