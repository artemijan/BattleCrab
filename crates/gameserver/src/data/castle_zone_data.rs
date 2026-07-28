//! Castle residence respawn points, parsed from `data/zones/castle_hall.xml`
//! (Java `CastleZone` — a `ResidenceZone`). Each `<zone type="CastleZone">`
//! carries `<stat name="castleId"/>` and an `owner_restart_point_list` of
//! `<spawn X=… Y=… Z=…>` elements: where the castle's defenders respawn during
//! a siege (Java `Castle.getResidenceZone().getSpawnLoc()`), used by the
//! restart-point handler once the control towers gate falls.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

const CASTLE_HALL_FILE: &str = "data/zones/castle_hall.xml";

/// The `<spawn>` restart points per castle id (`getSpawnLoc` picks one).
pub fn load_castle_restart_points(file_path: &str) -> HashMap<i32, Vec<(i32, i32, i32)>> {
    let Ok(content) = std::fs::read_to_string(format!("{file_path}{CASTLE_HALL_FILE}")) else {
        return HashMap::new();
    };
    let mut out: HashMap<i32, Vec<(i32, i32, i32)>> = HashMap::new();
    let mut reader = Reader::from_str(&content);

    // `castleId` and the `<spawn>` list both sit under the open `<zone>`, so we
    // accumulate the spawns and only commit them once the id is known.
    let mut castle_id = 0i32;
    let mut spawns: Vec<(i32, i32, i32)> = Vec::new();

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone" && castle_id > 0 && !spawns.is_empty() {
                    out.entry(castle_id).or_default().append(&mut spawns);
                }
                if e.name().as_ref() == b"zone" {
                    castle_id = 0;
                    spawns.clear();
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"zone" => {
                castle_id = 0;
                spawns.clear();
            }
            b"stat" if attr_str(&e, b"name").as_deref() == Some("castleId") => {
                castle_id = attr_i32(&e, b"val").unwrap_or(0);
            }
            b"spawn" => {
                let x = attr_i32(&e, b"X")
                    .or_else(|| attr_i32(&e, b"x"))
                    .unwrap_or(0);
                let y = attr_i32(&e, b"Y")
                    .or_else(|| attr_i32(&e, b"y"))
                    .unwrap_or(0);
                let z = attr_i32(&e, b"Z")
                    .or_else(|| attr_i32(&e, b"z"))
                    .unwrap_or(0);
                spawns.push((x, y, z));
            }
            _ => {}
        }
    }
    out
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|v| v.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_castle_restart_points_from_dist() {
        let pts =
            load_castle_restart_points(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // All nine castles carry an owner_restart_point_list.
        assert_eq!(pts.len(), 9, "nine castle residence zones");
        // Gludio (castle 1): first owner_restart_point is -16554,109382,-1799.
        let gludio = pts.get(&1).expect("Gludio restart points");
        assert!(
            gludio.contains(&(-16554, 109382, -1799)),
            "gludio_castle owner restart point parsed"
        );
    }
}
