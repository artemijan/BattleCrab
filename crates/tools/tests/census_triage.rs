//! Throwaway triage dump for the coverage census: for every unhandled name,
//! list the *reachable* skills behind it with their names and how a player
//! reaches them (L = learnable, N = npc, I = item, P = pet).

use gameserver::data::skill_data::{GapMap, SkillData};
use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::{DIST, ids_in, learnable, scan};

fn npc_ids() -> BTreeSet<i32> {
    let mut out = BTreeSet::new();
    scan("data/stats/npcs", false, &[r#"<skill id=""#], &mut out);
    out
}

fn item_ids() -> BTreeSet<i32> {
    let mut out = BTreeSet::new();
    scan(
        "data/stats/items",
        false,
        &[r#"<skill id=""#, r#"skillId=""#],
        &mut out,
    );
    out
}

fn pet_ids() -> BTreeSet<i32> {
    let mut out = BTreeSet::new();
    let pets = std::fs::read_to_string(format!("{DIST}data/PetSkillData.xml")).unwrap_or_default();
    ids_in(&pets, r#"skillId=""#, &mut out);
    out
}

/// item/npc **owner names** per skill id: `<item ... name="X">` enclosing the ref.
fn owners(dir: &str, tag: &str, needles: &[&str], out: &mut BTreeMap<i32, BTreeSet<String>>) {
    let Ok(entries) = std::fs::read_dir(format!("{DIST}{dir}")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // (byte offset, owner name) for every opening tag, in order.
        let mut marks: Vec<(usize, String)> = Vec::new();
        for (at, _) in text.match_indices(tag) {
            let rest = &text[at..];
            let end = rest.find('>').unwrap_or(0);
            let head = &rest[..end];
            let name = head
                .find(r#"name=""#)
                .map(|p| {
                    let s = &head[p + 6..];
                    s[..s.find('"').unwrap_or(0)].to_string()
                })
                .unwrap_or_default();
            marks.push((at, name));
        }
        for needle in needles {
            for (at, _) in text.match_indices(needle) {
                let rest = &text[at + needle.len()..];
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let Ok(id) = digits.parse::<i32>() else {
                    continue;
                };
                let owner = marks
                    .partition_point(|(off, _)| *off <= at)
                    .checked_sub(1)
                    .map(|i| marks[i].1.clone())
                    .unwrap_or_default();
                if !owner.is_empty() {
                    out.entry(id).or_default().insert(owner);
                }
            }
        }
    }
}

/// Machine-readable dump: `category<TAB>name<TAB>skill_id<TAB>skill_name`.
#[test]
#[ignore = "reporting aid"]
fn dump_gaps_tsv() {
    let sd = SkillData::load_from(DIST);
    let mut out = String::new();
    for (label, map) in sd.gaps().categories() {
        for (name, ids) in map.iter() {
            for id in ids {
                out.push_str(&format!(
                    "{label}\t{name}\t{id}\t{}\n",
                    sd.name(*id).unwrap_or("?")
                ));
            }
        }
    }
    let dest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gaps.tsv");
    std::fs::write(&dest, out).unwrap();
    println!("wrote {}", dest.display());
}

#[test]
#[ignore = "reporting aid"]
fn triage() {
    let sd = SkillData::load_from(DIST);
    let gaps = sd.gaps();
    let learn = learnable();
    let npcs = npc_ids();
    let items = item_ids();
    let pets = pet_ids();
    let mut reach = BTreeSet::new();
    reach.extend(&learn);
    reach.extend(&npcs);
    reach.extend(&items);
    reach.extend(&pets);

    let mut own: BTreeMap<i32, BTreeSet<String>> = BTreeMap::new();
    owners(
        "data/stats/items",
        "<item ",
        &[r#"<skill id=""#, r#"skillId=""#],
        &mut own,
    );
    owners("data/stats/npcs", "<npc ", &[r#"<skill id=""#], &mut own);

    let src = |id: i32| {
        let mut s = String::new();
        if learn.contains(&id) {
            s.push('L');
        }
        if npcs.contains(&id) {
            s.push('N');
        }
        if items.contains(&id) {
            s.push('I');
        }
        if pets.contains(&id) {
            s.push('P');
        }
        s
    };

    for (label, map) in gaps.categories() {
        let map: &GapMap = map;
        let mut rows: Vec<(&String, Vec<i32>)> = map
            .iter()
            .map(|(n, ids)| (n, ids.intersection(&reach).copied().collect::<Vec<_>>()))
            .filter(|(_, v)| !v.is_empty())
            .collect();
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        println!("\n\n########## {label} — {} reachable names", rows.len());
        for (name, ids) in rows {
            println!("\n### {name}  ({} reachable)", ids.len());
            for id in ids.iter().take(14) {
                let sk = sd.name(*id).unwrap_or("?");
                let o: Vec<&str> = own
                    .get(id)
                    .map(|s| s.iter().take(3).map(String::as_str).collect())
                    .unwrap_or_default();
                println!("    {id} [{}] {sk}   <- {}", src(*id), o.join(" | "));
            }
            if ids.len() > 14 {
                println!("    ... +{} more", ids.len() - 14);
            }
        }
    }
}
