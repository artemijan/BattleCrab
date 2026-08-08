//! `data/residences/clanHalls/**` — the 48 clan-hall definitions, one XML per
//! hall grouped into per-region subdirectories. Java `data/xml/ClanHallData`.

use std::collections::HashMap;

use quick_xml::events::Event;

use crate::data::xml::attr_str as attr;
use crate::data::xml::{attr_i32, attr_i64};
use crate::model::clan_hall::{ClanHall, ClanHallGrade, ClanHallType};

const CLAN_HALLS_DIR: &str = "data/residences/clanHalls";

/// Load every clan hall, keyed by id. Ownership fields default to unowned; the
/// `clanhall` table is overlaid at boot.
pub fn load_clan_halls(file_path: &str) -> HashMap<i32, ClanHall> {
    let mut out = HashMap::new();
    let root = format!("{file_path}{CLAN_HALLS_DIR}");
    for path in xml_files(&root) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            parse_file(&content, &mut out);
        }
    }
    out
}

/// Every `.xml` under `root`, recursing into region subdirectories.
fn xml_files(root: &str) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "xml") {
                files.push(path);
            }
        }
    }
    files
}

fn point(e: &quick_xml::events::BytesStart) -> (i32, i32, i32) {
    (
        attr_i32(e, b"x").unwrap_or(0),
        attr_i32(e, b"y").unwrap_or(0),
        attr_i32(e, b"z").unwrap_or(0),
    )
}

fn parse_file(content: &str, out: &mut HashMap<i32, ClanHall>) {
    let mut reader = quick_xml::Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut current: Option<ClanHall> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) if e.name().as_ref() == b"clanHall" => {
                current = Some(ClanHall {
                    id: attr_i32(&e, b"id").unwrap_or(0),
                    name: attr(&e, b"name").unwrap_or_default(),
                    grade: ClanHallGrade::from_name(&attr(&e, b"grade").unwrap_or_default()),
                    hall_type: ClanHallType::from_name(&attr(&e, b"type").unwrap_or_default()),
                    min_bid: 0,
                    lease: 0,
                    deposit: 0,
                    npcs: Vec::new(),
                    doors: Vec::new(),
                    owner_restart: (0, 0, 0),
                    banish: (0, 0, 0),
                    owner_id: 0,
                    paid_until: 0,
                });
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"clanHall" => {
                if let Some(hall) = current.take() {
                    out.insert(hall.id, hall);
                }
            }
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                let Some(hall) = current.as_mut() else {
                    continue;
                };
                match e.name().as_ref() {
                    b"auction" => {
                        hall.min_bid = attr_i64(&e, b"minBid").unwrap_or(0);
                        hall.lease = attr_i64(&e, b"lease").unwrap_or(0);
                        hall.deposit = attr_i64(&e, b"deposit").unwrap_or(0);
                    }
                    b"npc" => {
                        if let Some(id) = attr_i32(&e, b"id") {
                            hall.npcs.push(id);
                        }
                    }
                    b"door" => {
                        if let Some(id) = attr_i32(&e, b"id") {
                            hall.doors.push(id);
                        }
                    }
                    b"ownerRestartPoint" => hall.owner_restart = point(&e),
                    b"banishPoint" => hall.banish = point(&e),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
