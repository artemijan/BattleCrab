//! Generate `commons::system_messages::generated` from the Java reference.
//!
//! The Rust port of `tools/gen_system_messages.py` (removed in `42551abf`; the
//! original is still in git history). Three sources are merged:
//!
//! * `SystemMessageId.java` in the Java reference — the canonical name, id and
//!   English text of every retail message (`@ClientString(id, message)`);
//! * the client's own `SystemMsg*.dat`, in its unpacked text form — the colour
//!   each id is drawn in, which Java does not record;
//! * [`CUSTOM`] below — messages this server adds.
//!
//! Arity comes from the *name*, exactly as Java's `parseMessageParameters`
//! does: the highest `C<n>`/`S<n>` token wins. `C` is a name (player or npc),
//! `S` is anything else — an int, an item, a skill. Which of those a caller
//! passes is their choice, so only the count is enforced, never the type.
//!
//! Only messages that take parameters become types; the ~4100 that take none
//! are plain constants, which keeps the generated file about a quarter the
//! size it would otherwise be.
//!
//! The output is committed. This exists so it can be re-run against a newer
//! client or Java drop and the diff reviewed. Run `cargo fmt` afterwards — the
//! emitted lines are not pre-wrapped and the repo gates on formatting.

use commons::system_messages::{arity, param_tokens};
use std::collections::{HashMap, HashSet};

/// A message this server adds on top of the retail table. Ids continue the
/// retail sequence — [`generate`] refuses one that collides — and stay *close*
/// to it: the client does not accept an id far beyond the highest it loaded
/// (9000 rendered as nothing against a table ending at 7490), so keep these
/// immediately after the last retail id and move them if a client update
/// extends the table.
///
/// A literal percent sign in `message` is written `%%`, as every retail
/// message does (156 `Drain was only 50%% successful.`) — a single `%` starts
/// a format escape and the client mis-renders the line. The commons test
/// `a_literal_percent_is_doubled_in_messages_we_author` holds custom entries
/// to this.
pub struct CustomMessage {
    pub id: i32,
    pub name: &'static str,
    pub message: &'static str,
    /// `RRGGBBAA`.
    pub color: &'static str,
    /// `group` column of the client table; picks the render class together
    /// with `msg_type`. A `[none]`/`0` row draws in the default grey whatever
    /// the colour says.
    pub group: Option<i32>,
    pub msg_type: Option<&'static str>,
}

/// The server's own messages — what `tools/custom_system_messages.json` held
/// for the python script. The debuff-feedback pair, shown with the chance the
/// formula actually rolled.
pub const CUSTOM: &[CustomMessage] = &[
    CustomMessage {
        id: 7491,
        name: "C1_HAS_RESISTED_S2_CHANCE_WAS_S3",
        message: "$c1 has resisted $s2. (Chance: $s3%%)",
        color: "FF6666FF",
        group: Some(3),
        msg_type: Some("[damage]"),
    },
    CustomMessage {
        id: 7492,
        name: "S1_LANDED_ON_C2_CHANCE_WAS_S3",
        message: "$s1 landed on $c2. (Chance: $s3%%)",
        color: "99FF99FF",
        group: Some(3),
        msg_type: Some("[damage]"),
    },
];

struct Entry {
    id: i32,
    name: String,
    message: String,
    color: String,
    custom: bool,
    group: Option<i32>,
    msg_type: Option<String>,
}

pub struct Report {
    pub total: usize,
    pub typed: usize,
    pub constants: usize,
    pub custom: usize,
}

/// Which parameter positions are names (`C`) and which are values (`S`).
///
/// A name mentions its tokens in sentence order, not parameter order, so this
/// maps position -> kind rather than trusting the order they appear in. A gap
/// means the name skips an index (e.g. `C1` and `S3` with no 2); any unnamed
/// position is a value — the client still expects it to be sent.
fn slots(name: &str, n: usize) -> Vec<u8> {
    let kind: HashMap<usize, u8> = param_tokens(name).map(|(k, pos)| (pos, k)).collect();
    (1..=n)
        .map(|i| kind.get(&i).copied().unwrap_or(b'S'))
        .collect()
}

/// Every `@ClientString(id = …, message = "…")` + field pair in the Java file.
fn parse_java(text: &str) -> Vec<Entry> {
    let re = regex::Regex::new(
        r#"@ClientString\(id = (-?\d+), message = "((?:[^"\\]|\\.)*)"\)\s*\n\s*public static SystemMessageId (\w+);"#,
    )
    .expect("static regex");
    re.captures_iter(text)
        .map(|c| Entry {
            id: c[1].parse().expect("regex guarantees an integer"),
            name: c[3].to_string(),
            message: c[2].replace("\\\"", "\""),
            color: String::new(),
            custom: false,
            group: None,
            msg_type: None,
        })
        .collect()
}

/// The dat stores B, G, R, A; everything in Rust is RGBA.
fn bgra_to_rgba(hex8: &str) -> String {
    if hex8.len() != 8 {
        return hex8.to_string();
    }
    format!(
        "{}{}{}{}",
        &hex8[4..6],
        &hex8[2..4],
        &hex8[0..2],
        &hex8[6..8]
    )
    .to_uppercase()
}

/// id -> `RRGGBBAA`, read from the unpacked text form of the client table.
fn parse_dat_colors(text: &str) -> HashMap<i32, String> {
    let mut colors = HashMap::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() > 5
            && fields[0] == "msg_begin"
            && let Ok(id) = fields[1].parse::<i32>()
        {
            colors.insert(id, bgra_to_rgba(&fields[5].to_uppercase()));
        }
    }
    colors
}

fn rust_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn fmt_opt_int(v: Option<i32>) -> String {
    match v {
        None => "None".to_string(),
        Some(v) => format!("Some({v})"),
    }
}

fn fmt_opt_str(v: Option<&str>) -> String {
    match v {
        None => "None".to_string(),
        Some(v) => format!("Some(\"{v}\")"),
    }
}

const HEADER: &str = r#"//! Every system message the client knows, as ids and typed constructors.
//!
//! GENERATED by `l2r-tools gen-messages` — do not hand-edit. Add server
//! messages to `CUSTOM` in `crates/tools/src/msg_gen.rs` and re-run it.
//!
//! The server sends an id and its parameters; the client supplies the wording
//! and the colour from its own `SystemMsg*.dat`. The text and colour recorded
//! here therefore exist to *generate* that file (`l2r-tools client-dat
//! sync-messages`), not to be sent — changing a string here changes nothing
//! until the client table is rebuilt.
//!
//! Messages that take parameters are types, so the count cannot be got wrong:
//!
//! ```ignore
//! C1_HAS_INFLICTED_S3_DAMAGE_ON_C2::new(attacker, victim, damage)
//! ```
//!
//! Arguments are in *parameter* order — the order the packet carries them —
//! which is not the order the tokens appear in the name. Messages that take
//! none are plain constants:
//!
//! ```ignore
//! YOU_CANNOT_MOVE_WHILE_SITTING
//! ```
//!
//! `C` positions take a [`Subject`] (a player or npc name) and `S` positions a
//! [`SmValue`]. Which kind of value an `S` carries is the caller's choice —
//! the client renders whatever type byte it is sent — so only the count is
//! enforced here.

// `new` deliberately returns a ready-to-send `SystemMessage` rather than the
// marker type it is called on: the type exists only to name the message and
// fix its arity, so returning `Self` would give the caller nothing.
#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::too_many_arguments,
    clippy::new_ret_no_self,
    clippy::new_without_default
)]

use super::{MessageInfo, SmValue, Subject, SystemMessage, SystemMessageId};

"#;

/// Build the whole of `generated.rs` from the Java source text and the
/// unpacked client table. Pure text in, text out — the CLI owns the files.
pub fn generate(java: &str, dat: &str) -> Result<(String, Report), String> {
    let mut entries = parse_java(java);
    if entries.is_empty() {
        return Err("no @ClientString entries found — wrong Java file?".to_string());
    }
    let colors = parse_dat_colors(dat);
    for e in &mut entries {
        e.color = colors
            .get(&e.id)
            .cloned()
            .unwrap_or_else(|| "FFFFFFFF".to_string());
    }

    let known: HashSet<i32> = entries.iter().map(|e| e.id).collect();
    for c in CUSTOM {
        if known.contains(&c.id) {
            return Err(format!(
                "custom message id {} collides with a retail one",
                c.id
            ));
        }
        entries.push(Entry {
            id: c.id,
            name: c.name.to_string(),
            message: c.message.to_string(),
            color: c.color.to_uppercase(),
            custom: true,
            group: c.group,
            msg_type: c.msg_type.map(str::to_owned),
        });
    }

    // A name can appear twice in the Java file; keep the first (lowest-id,
    // ties in file order) and drop later duplicates so the generated module
    // has unique items.
    entries.sort_by_key(|e| e.id);
    let mut seen = HashSet::new();
    let unique: Vec<Entry> = entries
        .into_iter()
        .filter(|e| seen.insert(e.name.clone()))
        .collect();

    let mut out = String::from(HEADER);
    let (with_params, without): (Vec<&Entry>, Vec<&Entry>) =
        unique.iter().partition(|e| arity(&e.name) > 0);

    out.push_str(&format!(
        "/// Messages that take no parameters ({} of them).\n\
         ///\n\
         /// Each is a [`SystemMessageId`]; `.into()` turns one into a\n\
         /// [`SystemMessage`] ready to send.\n",
        without.len()
    ));
    for e in &without {
        out.push_str(&format!("/// `{}`\n", rust_string(&e.message)));
        out.push_str(&format!(
            "pub const {}: SystemMessageId = SystemMessageId::new({});\n",
            e.name, e.id
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "// --- {} messages that take parameters ---\n\n",
        with_params.len()
    ));
    for e in &with_params {
        let n = arity(&e.name);
        let kinds = slots(&e.name, n);
        let mut args = Vec::new();
        let mut pushes = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            let (lower, ty) = if *kind == b'C' {
                ('c', "Subject")
            } else {
                ('s', "SmValue")
            };
            args.push(format!("{}{}: impl Into<{}>", lower, i + 1, ty));
            pushes.push(format!("{}{}.into().into_param()", lower, i + 1));
        }
        out.push_str(&format!("/// `{}`\n", rust_string(&e.message)));
        out.push_str(&format!("pub struct {};\n", e.name));
        out.push_str(&format!("impl {} {{\n", e.name));
        out.push_str(&format!("    pub const ID: i32 = {};\n\n", e.id));
        out.push_str(&format!(
            "    pub fn new({}) -> SystemMessage {{\n",
            args.join(", ")
        ));
        out.push_str(&format!("        SystemMessage::new({}, vec![\n", e.id));
        for push in &pushes {
            out.push_str(&format!("            {push},\n"));
        }
        out.push_str("        ])\n    }\n}\n\n");
    }

    // The table the dat exporter reads.
    out.push_str(
        "/// Every message, for rebuilding the client table.\n\
         ///\n\
         /// `custom` marks the ones this server adds: `sync-messages` appends\n\
         /// those to the dat with defaults for the columns it cannot know,\n\
         /// because an id the client has no row for renders as nothing.\n\
         pub static ALL: &[MessageInfo] = &[\n",
    );
    for e in &unique {
        out.push_str(&format!(
            "    MessageInfo {{ id: {}, name: \"{}\", text: \"{}\", color: \"{}\", params: {}, custom: {}, group: {}, msg_type: {} }},\n",
            e.id,
            e.name,
            rust_string(&e.message),
            e.color,
            arity(&e.name),
            e.custom,
            fmt_opt_int(e.group),
            fmt_opt_str(e.msg_type.as_deref()),
        ));
    }
    out.push_str("];\n");

    let report = Report {
        total: unique.len(),
        typed: with_params.len(),
        constants: without.len(),
        custom: unique.iter().filter(|e| e.custom).count(),
    };
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arity_is_the_highest_token_in_the_name() {
        assert_eq!(arity("YOU_HAVE_BEEN_DISCONNECTED"), 0);
        assert_eq!(arity("C1_HAS_INFLICTED_S3_DAMAGE_ON_C2"), 3);
        // A trailing digit only counts after C or S.
        assert_eq!(arity("EMPTY_305"), 0);
    }

    /// The bug this guards: tokens appear in *sentence* order, so trusting it
    /// would swap parameter kinds for names like `S1_LANDED_ON_C2…`.
    #[test]
    fn slots_map_positions_not_mention_order() {
        assert_eq!(
            slots("S1_LANDED_ON_C2_CHANCE_WAS_S3", 3),
            vec![b'S', b'C', b'S']
        );
        // A skipped index is a value the client still expects.
        assert_eq!(slots("C1_GOT_S3", 3), vec![b'C', b'S', b'S']);
    }

    #[test]
    fn the_java_annotation_parses_including_escaped_quotes() {
        let java = "@ClientString(id = 5, message = \"Say \\\"hi\\\" now.\")\n\
                    public static SystemMessageId SAY_HI_NOW;\n";
        let entries = parse_java(java);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 5);
        assert_eq!(entries[0].name, "SAY_HI_NOW");
        assert_eq!(entries[0].message, "Say \"hi\" now.");
    }

    /// The dat stores B, G, R, A — message 0's tan `B09B79` is written
    /// `799BB0` there. See [[l2r-client-dat-roundtrip]]'s twin in `msg_color`.
    #[test]
    fn dat_colours_swap_channel_order() {
        assert_eq!(bgra_to_rgba("799BB0FF"), "B09B79FF");
        let colors =
            parse_dat_colors("msg_begin\t0\t1\t[text]\t0\t799bb0ff\t1\tmsg_end\nnoise line\n");
        assert_eq!(colors.get(&0).map(String::as_str), Some("B09B79FF"));
    }

    #[test]
    fn generate_rejects_a_custom_id_that_collides() {
        let java = format!(
            "@ClientString(id = {}, message = \"x\")\n\
             public static SystemMessageId X;\n",
            CUSTOM[0].id
        );
        assert!(generate(&java, "").is_err());
    }

    #[test]
    fn generated_output_has_the_three_sections() {
        let java = "@ClientString(id = 0, message = \"Plain.\")\n\
                    public static SystemMessageId PLAIN;\n\
                    \n\
                    @ClientString(id = 1, message = \"$c1 hit $s2.\")\n\
                    public static SystemMessageId C1_HIT_S2;\n\
                    \n\
                    @ClientString(id = 2, message = \"Plain.\")\n\
                    public static SystemMessageId PLAIN;\n";
        let dat = "msg_begin\t0\t1\t[Plain.]\t0\tFF0000FF\t1\tmsg_end";
        let (text, report) = generate(java, dat).unwrap();
        // The duplicate name is dropped, the customs are appended.
        assert_eq!(report.total, 2 + CUSTOM.len());
        assert_eq!(report.typed, 1 + CUSTOM.len());
        assert_eq!(report.constants, 1);
        assert_eq!(report.custom, CUSTOM.len());
        assert!(text.contains("pub const PLAIN: SystemMessageId = SystemMessageId::new(0);"));
        assert!(text.contains("pub struct C1_HIT_S2;"));
        assert!(text.contains("pub fn new(c1: impl Into<Subject>, s2: impl Into<SmValue>)"));
        // Colour read from the dat, B/R swapped; an id the dat lacks is white.
        assert!(text.contains("color: \"0000FFFF\""));
        assert!(text.contains("color: \"FFFFFFFF\""));
        assert_eq!(
            text.matches("pub struct PLAIN").count(),
            0,
            "constants stay constants"
        );
    }
}
