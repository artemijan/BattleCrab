//! The schema language that describes a decrypted `.dat` file's layout.
//!
//! [`crate::client_dat`] turns a client file into plaintext, but that plaintext
//! is a binary record stream, not text: `SysString-eu.dat` is a `UINT` count
//! followed by that many `UINT` + string pairs, and every other `.dat` has its
//! own shape. The layouts are not derivable from the bytes, so they come from
//! the schema set vendored under `dist/client/structure/` (from
//! MobiusDevelopment/l2clientdat, GPL-3.0 like this repo).
//!
//! # The three layers
//!
//! ```text
//! structure/44_path_of_rogue.xml   chronicle: filename pattern -> schema + version
//!   <link pattern="sysstring-[\w]+\.dat" file="sysstring" version="ScionsOfDestiny"/>
//! structure/dats/sysstring.xml     the schema, one <file> block per version
//!   <file pattern="ScionsOfDestiny" isSafePackage="true">
//!     <node name="data" reader="UINT"/>
//!     <for name="string" size="#data"> ... </for>
//! definitions.xml                  macros: reader="MTX" expands to a node group
//! ```
//!
//! A chronicle is a *guess* until proven: several of them map our client's
//! files identically, and only actually parsing tells you which is right. The
//! oracle is exact consumption — a correct schema lands on the last byte (plus
//! the 13-byte `SafePackage` trailer), and a wrong one derails long before.
//! [`crate::dat_text`] reports that, and `--chronicle auto` picks the schema
//! set that parses the most files cleanly.
//!
//! # Sizes and back-references
//!
//! `size="#data"` on a `<for>` means "repeat as many times as the value read
//! into the variable `data`", so a walk has to keep every field it has read in
//! scope. `<if param="#x" val="3">` and `<mask param="#x" val="4">` read the
//! same variable pool.

use quick_xml::Reader as XmlReader;
use quick_xml::events::Event;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A leaf field type. These are the `reader="..."` values that are not macros.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    /// One byte, signed (Java reads a `char` then casts through `byte`).
    UChar,
    /// One byte, unsigned.
    UByte,
    /// Unreal compact index: 1-5 bytes, high bit of byte 0 is the sign.
    Cntr,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    Float,
    Double,
    /// Length-prefixed by a compact int. A *negative* length means the payload
    /// is UTF-16LE and twice as long; positive means single-byte chars. Either
    /// way the stored length counts the terminator.
    Ascf,
    /// `int32` byte count, then UTF-16 stored as byte-swapped pairs.
    Unicode,
    /// AARRGGBB, one byte each, printed as hex.
    Rgba,
    /// RRGGBB.
    Rgb,
    Hex,
    /// A `UInt` index into `L2GameDataName.dat`. Kept numeric here — resolving
    /// it to a name needs that file and is not required to walk the stream.
    MapInt,
    /// Only reachable as the type of a `<write>` constant, which consumes no
    /// bytes.
    Str,
}

impl Field {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "UCHAR" => Field::UChar,
            "UBYTE" => Field::UByte,
            "CNTR" => Field::Cntr,
            "SHORT" => Field::Short,
            "USHORT" => Field::UShort,
            "INT" => Field::Int,
            "UINT" => Field::UInt,
            "LONG" => Field::Long,
            "FLOAT" => Field::Float,
            "DOUBLE" => Field::Double,
            "ASCF" => Field::Ascf,
            "UNICODE" => Field::Unicode,
            "RGBA" => Field::Rgba,
            "RGB" => Field::Rgb,
            "HEX" => Field::Hex,
            "MAP_INT" => Field::MapInt,
            "STRING" => Field::Str,
            _ => return None,
        })
    }
}

/// How many times a `<for>` repeats.
#[derive(Clone, Debug)]
pub enum Count {
    Fixed(i64),
    /// `size="#name"` — read at walk time from the variable pool.
    Var(String),
}

#[derive(Clone, Debug)]
pub enum Node {
    /// A field that consumes bytes and binds its value to `name`.
    Value {
        name: String,
        field: Field,
        hidden: bool,
        enum_name: Option<String>,
    },
    /// `<write name="\r\n"/>` — literal text, consumes nothing.
    Constant { text: String },
    /// `<for>`: repeat `children` `count` times.
    Cycle {
        name: String,
        count: Count,
        hidden: bool,
        children: Vec<Node>,
    },
    /// `<wrapper>`: groups fields under one name, printed inline.
    Wrapper { name: String, children: Vec<Node> },
    /// `<if param="#x" val="v">` — walk `children` only when `x == v`.
    /// `negate` carries `<else>`, which compares the same pair the other way.
    If {
        param: String,
        val: String,
        negate: bool,
        children: Vec<Node>,
    },
    /// `<mask param="#x" val="4">` — walk `children` when `(x & val) == val`.
    Mask {
        param: String,
        val: i64,
        children: Vec<Node>,
    },
}

/// One `<file pattern="VERSION">` block: the layout for a single dat version.
#[derive(Clone, Debug)]
pub struct Layout {
    pub version: String,
    /// Whether the plaintext ends with the 13-byte `SafePackage` trailer.
    pub safe_package: bool,
    pub nodes: Vec<Node>,
}

/// A `<link>` from a chronicle file.
#[derive(Clone, Debug)]
pub struct Link {
    /// Regex matched case-insensitively against the dat's filename.
    pub pattern: String,
    /// Base name of the schema under `structure/dats/`.
    pub schema: String,
    /// Which `<file pattern>` inside that schema to use.
    pub version: String,
}

/// Everything loaded from a `structure/` directory.
pub struct SchemaSet {
    root: PathBuf,
    definitions: HashMap<String, Vec<Node>>,
    /// Chronicle name (`44_path_of_rogue`) -> its links, in file order.
    pub chronicles: HashMap<String, Vec<Link>>,
    /// Lazily parsed `structure/dats/<name>.xml`.
    schemas: HashMap<String, Vec<Layout>>,
    /// `enums/base.xml`: enum name -> index -> label.
    pub enums: HashMap<String, HashMap<i64, String>>,
}

impl SchemaSet {
    /// Load `definitions.xml`, `enums/` and every chronicle under `root`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let mut set = SchemaSet {
            root: root.to_path_buf(),
            definitions: HashMap::new(),
            chronicles: HashMap::new(),
            schemas: HashMap::new(),
            enums: HashMap::new(),
        };
        set.definitions = load_definitions(&root.join("definitions.xml"))?;
        set.enums = load_enums(&root.join("enums/base.xml")).unwrap_or_default();

        let dir = root.join("structure");
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "xml"))
            .collect();
        entries.sort();
        for path in entries {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            set.chronicles.insert(name, load_links(&path)?);
        }
        Ok(set)
    }

    /// Chronicle names, ordered as their numeric prefixes suggest.
    pub fn chronicle_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.chronicles.keys().map(String::as_str).collect();
        names.sort();
        names
    }

    /// The link in `chronicle` whose pattern matches `file_name`.
    pub fn link_for(&self, chronicle: &str, file_name: &str) -> Option<&Link> {
        self.chronicles
            .get(chronicle)?
            .iter()
            .find(|l| pattern_matches(&l.pattern, file_name))
    }

    /// Resolve a dat file to the layout that should describe it.
    pub fn layout_for(&mut self, chronicle: &str, file_name: &str) -> Result<Layout, String> {
        let link = self
            .link_for(chronicle, file_name)
            .ok_or_else(|| format!("no <link> in {chronicle} matches {file_name}"))?
            .clone();
        let layouts = self.schema(&link.schema)?;
        layouts
            .iter()
            .find(|l| l.version.eq_ignore_ascii_case(&link.version))
            .or_else(|| layouts.first())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "schema {} has no <file pattern=\"{}\">",
                    link.schema, link.version
                )
            })
    }

    /// Every layout that could plausibly describe `file_name`: each chronicle
    /// that links it, and within the linked schema every `<file>` version — the
    /// link names one, but a client can ship a file from a neighbouring
    /// revision. Labels are `chronicle/version`, and the caller picks the one
    /// that parses exactly.
    pub fn candidates(&mut self, file_name: &str) -> Vec<(String, Layout)> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for chronicle in self
            .chronicle_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        {
            let Some(link) = self.link_for(&chronicle, file_name).cloned() else {
                continue;
            };
            let Ok(layouts) = self.schema(&link.schema) else {
                continue;
            };
            // Prefer the version the chronicle actually names.
            let mut versions: Vec<Layout> = layouts.clone();
            versions.sort_by_key(|l| !l.version.eq_ignore_ascii_case(&link.version));
            for layout in versions {
                if seen.insert((link.schema.clone(), layout.version.clone())) {
                    out.push((format!("{chronicle}/{}", layout.version), layout));
                }
            }
        }
        out
    }

    fn schema(&mut self, name: &str) -> Result<&Vec<Layout>, String> {
        if !self.schemas.contains_key(name) {
            let path = self.root.join("structure/dats").join(format!("{name}.xml"));
            let layouts = load_schema(&path, &self.definitions)?;
            self.schemas.insert(name.to_string(), layouts);
        }
        Ok(&self.schemas[name])
    }
}

/// Match a chronicle `<link pattern>` against a filename.
///
/// These are genuine Java regexes — `AbnormalDefaultEffect(_Classic(Aden)?)?.dat`
/// nests optional groups, and plenty leave the `.` before `dat` unescaped as a
/// wildcard. They are matched whole and case-insensitively, the way
/// `Pattern.matches` does it.
fn pattern_matches(pattern: &str, name: &str) -> bool {
    compiled(pattern).is_some_and(|re| re.is_match(name))
}

/// Compile once per distinct pattern — a chronicle has ~200 of them and every
/// file is tested against all of them until one hits.
fn compiled(pattern: &str) -> Option<std::sync::Arc<Regex>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<std::sync::Arc<Regex>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(pattern.to_string())
        .or_insert_with(|| {
            Regex::new(&format!("(?i)^(?:{pattern})$"))
                .ok()
                .map(std::sync::Arc::new)
        })
        .clone()
}

// --- XML loading ------------------------------------------------------------

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == key.as_bytes()).then(|| String::from_utf8_lossy(&a.value).into_owned())
    })
}

fn open_xml(path: &Path) -> Result<XmlReader<std::io::BufReader<std::fs::File>>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut reader = XmlReader::from_reader(std::io::BufReader::new(file));
    reader.config_mut().trim_text(true);
    Ok(reader)
}

fn load_links(path: &Path) -> Result<Vec<Link>, String> {
    let mut reader = open_xml(path)?;
    let mut buf = Vec::new();
    let mut links = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"link" => {
                if let (Some(pattern), Some(schema)) = (attr(&e, "pattern"), attr(&e, "file")) {
                    links.push(Link {
                        pattern,
                        schema,
                        version: attr(&e, "version").unwrap_or_default(),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("{}: {e}", path.display())),
            _ => {}
        }
        buf.clear();
    }
    Ok(links)
}

fn load_enums(path: &Path) -> Result<HashMap<String, HashMap<i64, String>>, String> {
    let mut reader = open_xml(path)?;
    let mut buf = Vec::new();
    let mut out: HashMap<String, HashMap<i64, String>> = HashMap::new();
    let mut current = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"enum" => {
                current = attr(&e, "name").unwrap_or_default();
            }
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"node" => {
                if let (Some(i), Some(n)) = (attr(&e, "index"), attr(&e, "name"))
                    && let Ok(i) = i.parse::<i64>()
                {
                    out.entry(current.clone()).or_default().insert(i, n);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("{}: {e}", path.display())),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn load_definitions(path: &Path) -> Result<HashMap<String, Vec<Node>>, String> {
    let mut reader = open_xml(path)?;
    let mut buf = Vec::new();
    let mut out = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"definition" => {
                let name = attr(&e, "name").unwrap_or_default();
                // Definitions may not reference other definitions, so an empty
                // macro table is the right context here.
                let nodes = read_children(&mut reader, b"definition", &HashMap::new(), path)?;
                out.insert(name, nodes);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("{}: {e}", path.display())),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn load_schema(path: &Path, defs: &HashMap<String, Vec<Node>>) -> Result<Vec<Layout>, String> {
    let mut reader = open_xml(path)?;
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"file" => {
                let version = attr(&e, "pattern").unwrap_or_default();
                let safe_package = attr(&e, "isSafePackage").as_deref() == Some("true");
                let nodes = read_children(&mut reader, b"file", defs, path)?;
                out.push(Layout {
                    version,
                    safe_package,
                    nodes,
                });
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("{}: {e}", path.display())),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// Read nodes until `</close_tag>`, expanding macro readers as it goes.
fn read_children<R: std::io::BufRead>(
    reader: &mut XmlReader<R>,
    close_tag: &[u8],
    defs: &HashMap<String, Vec<Node>>,
    path: &Path,
) -> Result<Vec<Node>, String> {
    let mut buf = Vec::new();
    let mut nodes = Vec::new();
    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        match event {
            Event::Start(e) => {
                let tag = e.name().as_ref().to_vec();
                let name = attr(&e, "name").unwrap_or_default();
                let children = read_children(reader, &tag, defs, path)?;
                nodes.push(build_container(&tag, name, &e, children, path)?);
            }
            Event::Empty(e) => {
                let tag = e.name().as_ref().to_vec();
                match tag.as_slice() {
                    b"node" => nodes.extend(build_leaf(&e, defs, path)?),
                    b"write" => nodes.push(Node::Constant {
                        text: unescape_constant(&attr(&e, "name").unwrap_or_default()),
                    }),
                    // A self-closing container has no children; keep it so the
                    // shape still matches the schema.
                    _ => nodes.push(build_container(
                        &tag,
                        attr(&e, "name").unwrap_or_default(),
                        &e,
                        Vec::new(),
                        path,
                    )?),
                }
            }
            Event::End(e) if e.name().as_ref() == close_tag => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(nodes)
}

fn build_container(
    tag: &[u8],
    name: String,
    e: &quick_xml::events::BytesStart,
    children: Vec<Node>,
    path: &Path,
) -> Result<Node, String> {
    Ok(match tag {
        b"for" => {
            let size = attr(e, "size").unwrap_or_default();
            let count = match size.strip_prefix('#') {
                Some(var) => Count::Var(var.to_string()),
                None => Count::Fixed(size.parse::<i64>().unwrap_or(-1)),
            };
            Node::Cycle {
                name,
                count,
                hidden: attr(e, "hidden").as_deref() == Some("true"),
                children,
            }
        }
        b"wrapper" => Node::Wrapper { name, children },
        b"if" | b"else" => Node::If {
            param: attr(e, "param")
                .and_then(|p| p.strip_prefix('#').map(str::to_owned))
                .ok_or_else(|| format!("{}: <if> needs param=\"#var\"", path.display()))?,
            val: attr(e, "val").unwrap_or_default(),
            negate: tag == b"else",
            children,
        },
        b"mask" => Node::Mask {
            param: attr(e, "param")
                .and_then(|p| p.strip_prefix('#').map(str::to_owned))
                .ok_or_else(|| format!("{}: <mask> needs param=\"#var\"", path.display()))?,
            val: attr(e, "val")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0),
            children,
        },
        other => {
            return Err(format!(
                "{}: unexpected element <{}>",
                path.display(),
                String::from_utf8_lossy(other)
            ));
        }
    })
}

/// Build a `<node>`. A `reader` naming a macro expands to that macro's nodes,
/// each renamed to this node's name — matching `DescriptorParser`.
fn build_leaf(
    e: &quick_xml::events::BytesStart,
    defs: &HashMap<String, Vec<Node>>,
    path: &Path,
) -> Result<Vec<Node>, String> {
    let name = attr(e, "name").unwrap_or_default();
    let reader = attr(e, "reader").unwrap_or_default();
    let hidden = attr(e, "hide").as_deref() == Some("true");

    if let Some(macro_nodes) = defs.get(&reader) {
        return Ok(macro_nodes
            .iter()
            .cloned()
            .map(|n| rename(n, &name, hidden))
            .collect());
    }
    let field = Field::parse(&reader).ok_or_else(|| {
        format!(
            "{}: unknown reader \"{reader}\" on node \"{name}\"",
            path.display()
        )
    })?;
    Ok(vec![Node::Value {
        name,
        field,
        hidden,
        enum_name: attr(e, "enumName"),
    }])
}

fn rename(node: Node, name: &str, hide: bool) -> Node {
    match node {
        Node::Value {
            field,
            hidden,
            enum_name,
            ..
        } => Node::Value {
            name: name.to_string(),
            field,
            hidden: hidden || hide,
            enum_name,
        },
        Node::Cycle {
            count,
            hidden,
            children,
            ..
        } => Node::Cycle {
            name: name.to_string(),
            count,
            hidden: hidden || hide,
            children,
        },
        Node::Wrapper { children, .. } => Node::Wrapper {
            name: name.to_string(),
            children,
        },
        other => other,
    }
}

fn unescape_constant(s: &str) -> String {
    s.replace("\\r\\n", "\r\n").replace("\\t", "\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_patterns_match_the_way_the_chronicle_files_expect() {
        assert!(pattern_matches(r"sysstring-[\w]+\.dat", "SysString-eu.dat"));
        assert!(pattern_matches(r"itemname-[\w]+\.dat", "ItemName-eu.dat"));
        assert!(pattern_matches("armorgrp.dat", "Armorgrp.dat"));
        // Nested optional groups are common: `Armorgrp(_Classic(Aden)?)?.dat`.
        let grp = r"Armorgrp(_Classic(Aden)?)?.dat";
        for name in [
            "Armorgrp.dat",
            "Armorgrp_Classic.dat",
            "Armorgrp_ClassicAden.dat",
        ] {
            assert!(pattern_matches(grp, name), "{name}");
        }
        // These really are regexes, and most patterns leave the extension dot
        // unescaped — so it is a wildcard, exactly as Java's Pattern treats it.
        assert!(pattern_matches("armorgrp.dat", "armorgrpXdat"));
        // Escaped, it is literal.
        assert!(!pattern_matches(
            r"AbnormalEdgeEffectData\.dat",
            "AbnormalEdgeEffectDataXdat"
        ));
        // A `[\w]+` hole needs at least one character.
        assert!(!pattern_matches(r"sysstring-[\w]+\.dat", "sysstring-.dat"));
        // A different stem must not match, and the match is anchored whole.
        assert!(!pattern_matches(r"itemname-[\w]+\.dat", "NpcName-eu.dat"));
        assert!(!pattern_matches("armorgrp.dat", "xxarmorgrp.dat"));
    }

    #[test]
    fn constants_unescape_the_two_sequences_the_reader_handles() {
        assert_eq!(unescape_constant("\\r\\n"), "\r\n");
        assert_eq!(unescape_constant("a\\tb"), "a\tb");
    }
}
