//! Castle residence respawn points, parsed from `data/zones/castle_hall.xml`
//! (Java `CastleZone` — a `ResidenceZone`). Each `<zone type="CastleZone">`
//! carries `<stat name="castleId"/>` and an `owner_restart_point_list` of
//! `<spawn X=… Y=… Z=…>` elements: where the castle's defenders respawn during
//! a siege (Java `Castle.getResidenceZone().getSpawnLoc()`), used by the
//! restart-point handler once the control towers gate falls.

use std::collections::HashMap;

use crate::data::xml;
use crate::data::xml::{attr_i32, attr_str};
use quick_xml::events::Event;

const CASTLE_HALL_FILE: &str = "data/zones/castle_hall.xml";

/// One castle residence zone's four respawn lists (Java `ZoneRespawn`).
///
/// `parseLoc` sorts a `<spawn>` by its `type` attribute and an **absent** type
/// means the plain owner-restart list — the four are separate destinations, not
/// one pool. Reading them as one pool is how the defender restart handler used
/// to send a defender to the *enemy* town: `other`/`chaotic` outnumber the real
/// owner points roughly 30:4 in this file, so a random pick almost never landed
/// inside the castle.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CastleRespawnPoints {
    /// `owner_restart_point_list` — `getSpawnLoc()`, where defenders respawn.
    pub spawn: Vec<(i32, i32, i32)>,
    /// `other_restart_village_list` — `getOtherSpawnLoc()`, the "far town" a
    /// siege participant is pushed to.
    pub other: Vec<(i32, i32, i32)>,
    /// `chao_restart_point_list` — `getChaoticSpawnLoc()`, used instead of
    /// `spawn` whenever the player's reputation is negative.
    pub chaotic: Vec<(i32, i32, i32)>,
    /// `getBanishSpawnLoc()` — where non-owners are ejected to.
    pub banish: Vec<(i32, i32, i32)>,
}

impl CastleRespawnPoints {
    /// `getSpawnLoc()` / `getChaoticSpawnLoc()` — Java picks at random; the
    /// caller supplies the roll so the choice stays testable. Falls back to the
    /// lawful list when a castle declares no chaotic points.
    pub fn pick(&self, chaotic: bool, pick: usize) -> Option<(i32, i32, i32)> {
        let list = if chaotic && !self.chaotic.is_empty() {
            &self.chaotic
        } else {
            &self.spawn
        };
        (!list.is_empty()).then(|| list[pick % list.len()])
    }
}

/// The `<spawn>` restart points per castle id, split by `type` exactly as
/// Java's `ZoneRespawn.parseLoc` splits them.
pub fn load_castle_restart_points(file_path: &str) -> HashMap<i32, CastleRespawnPoints> {
    let Ok(content) = std::fs::read_to_string(format!("{file_path}{CASTLE_HALL_FILE}")) else {
        return HashMap::new();
    };
    let mut out: HashMap<i32, CastleRespawnPoints> = HashMap::new();

    // `castleId` and the `<spawn>` list both sit under the open `<zone>`, so we
    // accumulate the spawns and only commit them once the id is known.
    let mut castle_id = 0i32;
    let mut pts = CastleRespawnPoints::default();

    for event in xml::events(&content) {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone" {
                    if castle_id > 0 {
                        let slot = out.entry(castle_id).or_default();
                        slot.spawn.append(&mut pts.spawn);
                        slot.other.append(&mut pts.other);
                        slot.chaotic.append(&mut pts.chaotic);
                        slot.banish.append(&mut pts.banish);
                    }
                    castle_id = 0;
                    pts = CastleRespawnPoints::default();
                }
                continue;
            }
            _ => continue,
        };
        match e.name().as_ref() {
            b"zone" => {
                castle_id = 0;
                pts = CastleRespawnPoints::default();
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
                // Java warns and drops an unknown type rather than pooling it.
                match attr_str(&e, b"type").as_deref() {
                    None | Some("") => pts.spawn.push((x, y, z)),
                    Some("other") => pts.other.push((x, y, z)),
                    Some("chaotic") => pts.chaotic.push((x, y, z)),
                    Some("banish") => pts.banish.push((x, y, z)),
                    Some(_) => {}
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_castle_restart_points_from_dist() {
        let pts = load_castle_restart_points(crate::data::DIST_GAME);
        // All nine castles carry an owner_restart_point_list.
        assert_eq!(pts.len(), 9, "nine castle residence zones");
        // Gludio (castle 1): first owner_restart_point is -16554,109382,-1799.
        let gludio = pts.get(&1).expect("Gludio restart points");
        assert!(
            gludio.spawn.contains(&(-16554, 109382, -1799)),
            "gludio_castle owner restart point parsed"
        );
    }

    /// The typed lists must stay apart. Gludio's owner list is four points
    /// **inside** the castle; `other` is Gludin village and `chaotic` the
    /// outlaw camp north of it. Pooling them (which this loader did until the
    /// G34 live-reachability pass) meant a defender's "restart at castle"
    /// picked a village point 8 times out of 9.
    #[test]
    fn spawn_types_are_not_pooled() {
        let pts = load_castle_restart_points(crate::data::DIST_GAME);
        let gludio = pts.get(&1).expect("Gludio restart points");
        assert_eq!(gludio.spawn.len(), 4, "owner_restart_point_list");
        assert!(!gludio.other.is_empty(), "other_restart_village_list");
        assert!(!gludio.chaotic.is_empty(), "chao_restart_point_list");
        for typed in gludio.other.iter().chain(&gludio.chaotic) {
            assert!(
                !gludio.spawn.contains(typed),
                "typed point {typed:?} leaked into the owner list"
            );
        }
        // `pick` honours the reputation split, and falls back when a castle
        // declares no chaotic points.
        assert!(gludio.spawn.contains(&gludio.pick(false, 0).unwrap()));
        assert!(gludio.chaotic.contains(&gludio.pick(true, 0).unwrap()));
        let lawful_only = CastleRespawnPoints {
            spawn: vec![(1, 2, 3)],
            ..Default::default()
        };
        assert_eq!(lawful_only.pick(true, 0), Some((1, 2, 3)));
        assert_eq!(CastleRespawnPoints::default().pick(false, 0), None);
    }
}
