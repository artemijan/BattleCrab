//! The raw-XML scan both census tests measure the datapack with.
//!
//! Reachability is deliberately read off the XML rather than through the ported
//! loaders: a text scan answers "does the dist reference this skill id
//! anywhere", which is the honest denominator, where going through
//! `SkillTreeData`/`NpcData` would measure the port's own coverage of *those*
//! files instead.

use std::collections::BTreeSet;

pub const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

/// Every `needle`-prefixed integer in the file (`skillId="123"` →  123).
pub fn ids_in(text: &str, needle: &str, out: &mut BTreeSet<i32>) {
    let mut rest = text;
    while let Some(at) = rest.find(needle) {
        rest = &rest[at + needle.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(id) = digits.parse() {
            out.insert(id);
        }
    }
}

/// [`ids_in`] over every `.xml` under `DIST/dir`, optionally recursing.
pub fn scan(dir: &str, recursive: bool, needles: &[&str], out: &mut BTreeSet<i32>) {
    let Ok(entries) = std::fs::read_dir(format!("{DIST}{dir}")) else {
        panic!("coverage census: missing datapack dir {dir}");
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                let sub = path.strip_prefix(DIST).unwrap().to_string_lossy();
                scan(&sub, true, needles, out);
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for needle in needles {
            ids_in(&text, needle, out);
        }
    }
}

/// Skill ids a player can learn — `data/skillTrees/**`. The denominator that
/// matters: rank by *learnable* usage, never by raw instance count (`StatUp` is
/// 465 skills and 9 learnable ones).
pub fn learnable() -> BTreeSet<i32> {
    let mut out = BTreeSet::new();
    scan("data/skillTrees", true, &[r#"skillId=""#], &mut out);
    out
}
