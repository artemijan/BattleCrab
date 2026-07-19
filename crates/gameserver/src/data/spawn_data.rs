//! Port of `data/xml/SpawnData` + `model/spawns/*` (SpawnTemplate /
//! SpawnGroup / NpcSpawnTemplate) + the `ZoneForm` shapes territory spawns
//! randomize inside, scoped to what this dist's `data/spawns/**` actually
//! uses: fixed-location spawns, `count`, `respawnTime`/`respawnRandom`
//! (sec/min/hour), and `<territories>` on the spawn or group. Unused-by-data
//! features are not ported: `zone=`/`banned_territory`/`<locations>`/
//! `<minions>`/`respawnPattern` (0 occurrences each), `<parameters>`
//! (consumed by AI scripts only, G11). `dbSave` raid persistence
//! (`DBSpawnManager`) is ported — see [`crate::game_loop::boss_respawn`].

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const SPAWNS_DIR: &str = "data/spawns";

/// One `<spawn>` element (Java `SpawnTemplate`).
pub struct SpawnTemplate {
    pub name: Option<String>,
    pub territories: Vec<Territory>,
    pub groups: Vec<SpawnGroup>,
}

/// One `<group>` (or the implicit default group for bare `<npc>` children).
pub struct SpawnGroup {
    pub territories: Vec<Territory>,
    pub npcs: Vec<NpcSpawnDef>,
}

/// One `<npc>` line (Java `NpcSpawnTemplate`, minus the unused features
/// listed in the module docs).
pub struct NpcSpawnDef {
    pub npc_id: i32,
    pub count: i32,
    /// Fixed spawn point (`x`/`y`/`z` all present). `heading` defaults to 0
    /// like Java; territory spawns get a random heading instead.
    pub loc: Option<FixedLoc>,
    pub respawn_secs: i32,
    pub respawn_random_secs: i32,
    /// `dbSave="true"` (Java `NpcSpawnTemplate.hasDBSave`) — this NPC's live
    /// HP/MP and pending respawn time survive a server restart, via the
    /// `npc_respawns` table and `DBSpawnManager`. 225 spawns on this dist, all
    /// raid bosses in `RaidbossSpawns.xml`.
    pub db_save: bool,
}

#[derive(Clone, Copy)]
pub struct FixedLoc {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// Java `SpawnTerritory` wrapping a `ZoneForm`.
pub struct Territory {
    pub form: ZoneForm,
    pub min_z: i32,
    pub max_z: i32,
}

/// The three `ZoneForm` shapes (`ZoneNPoly`/`ZoneCuboid`/`ZoneCylinder`).
pub enum ZoneForm {
    NPoly { xs: Vec<i32>, ys: Vec<i32> },
    Cuboid { x1: i32, x2: i32, y1: i32, y2: i32 },
    Cylinder { x: i32, y: i32, rad: i32 },
}

impl Territory {
    /// Bounding box (`Polygon.getBounds()` / the cuboid / the circle box).
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        match &self.form {
            ZoneForm::NPoly { xs, ys } => (
                xs.iter().copied().min().unwrap_or(0),
                xs.iter().copied().max().unwrap_or(0),
                ys.iter().copied().min().unwrap_or(0),
                ys.iter().copied().max().unwrap_or(0),
            ),
            ZoneForm::Cuboid { x1, x2, y1, y2 } => (*x1, *x2, *y1, *y2),
            ZoneForm::Cylinder { x, y, rad } => (x - rad, x + rad, y - rad, y + rad),
        }
    }

    /// 2D containment (`ZoneForm.isInsideZone` minus the z band, which the
    /// caller checks — random points use `(min_z+max_z)/2` anyway).
    pub fn contains_2d(&self, px: i32, py: i32) -> bool {
        match &self.form {
            ZoneForm::NPoly { xs, ys } => {
                // `java.awt.Polygon.contains` — even-odd ray cast.
                let (n, mut inside) = (xs.len(), false);
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = (xs[i] as i64, ys[i] as i64);
                    let (xj, yj) = (xs[j] as i64, ys[j] as i64);
                    if ((yi > py as i64) != (yj > py as i64))
                        && ((px as i64 - xi) * (yj - yi) < (xj - xi) * (py as i64 - yi)) == (yj > yi)
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
            ZoneForm::Cuboid { x1, x2, y1, y2 } => px >= *x1 && px <= *x2 && py >= *y1 && py <= *y2,
            ZoneForm::Cylinder { x, y, rad } => {
                let (dx, dy) = ((px - x) as i64, (py - y) as i64);
                dx * dx + dy * dy <= (*rad as i64) * (*rad as i64)
            }
        }
    }

    /// The z the random-point helpers hand to `GeoEngine.getHeight`.
    pub fn mid_z(&self) -> i32 {
        (self.min_z + self.max_z) / 2
    }
}

pub struct SpawnData {
    pub spawns: Vec<SpawnTemplate>,
}

impl SpawnData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut spawns = Vec::new();
        let dir = std::path::PathBuf::from(format!("{file_path}{SPAWNS_DIR}"));
        let mut paths = Vec::new();
        collect_xml_files(&dir, &mut paths);
        paths.sort();
        for path in &paths {
            parse_file(path, &mut spawns);
        }
        info!("SpawnData: Loaded {} spawns.", spawns.iter().map(Self::template_spawn_count).sum::<usize>());
        Self { spawns }
    }

    /// Number of `<npc>` lines under a template (the count Java's load log
    /// reports — one line can still place `count > 1` NPCs).
    fn template_spawn_count(t: &SpawnTemplate) -> usize {
        t.groups.iter().map(|g| g.npcs.len()).sum()
    }

    pub fn npc_line_count(&self) -> usize {
        self.spawns.iter().map(Self::template_spawn_count).sum()
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self { spawns: Vec::new() }
    }
}

fn collect_xml_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_xml_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("xml") {
            out.push(path);
        }
    }
}

/// `TimeUtil.parseDuration` narrowed to the units the spawn files use
/// (`sec`/`min`/`hour`, singular or plural; `day` kept for completeness).
pub fn parse_duration_secs(s: &str) -> Option<i32> {
    let split = s.find(|c: char| !c.is_ascii_digit())?;
    let value: i32 = s[..split].parse().ok()?;
    let mult = match s[split..].to_ascii_lowercase().as_str() {
        "sec" | "secs" => 1,
        "min" | "mins" => 60,
        "hour" | "hours" => 3600,
        "day" | "days" => 86400,
        _ => return None,
    };
    Some(value * mult)
}

fn parse_file(path: &std::path::Path, out: &mut Vec<SpawnTemplate>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    let mut cur_spawn: Option<SpawnTemplate> = None;
    // The group being filled: an explicit `<group>` while inside one, else the
    // implicit default group Java creates for bare `<npc>` children.
    let mut cur_group: Option<SpawnGroup> = None;
    let mut default_group: Option<SpawnGroup> = None;
    let mut in_group = false;
    // <territories> scope: attach finished territories to the group when it's
    // open, else to the spawn template.
    let mut cur_territory: Option<PendingTerritory> = None;

    while let Ok(event) = reader.read_event() {
        let (e, self_closing) = match event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                match e.name().as_ref() {
                    b"spawn" => {
                        if let Some(mut s) = cur_spawn.take() {
                            if let Some(g) = default_group.take() {
                                s.groups.push(g);
                            }
                            out.push(s);
                        }
                    }
                    b"group" => {
                        in_group = false;
                        if let (Some(s), Some(g)) = (cur_spawn.as_mut(), cur_group.take()) {
                            s.groups.push(g);
                        }
                    }
                    b"territory" => finish_territory(&mut cur_territory, &mut cur_spawn, &mut cur_group, in_group),
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };

        match e.name().as_ref() {
            b"spawn" => {
                cur_spawn = Some(SpawnTemplate {
                    name: attr_str(&e, b"name"),
                    territories: Vec::new(),
                    groups: Vec::new(),
                });
            }
            b"group" => {
                in_group = true;
                cur_group = Some(SpawnGroup { territories: Vec::new(), npcs: Vec::new() });
            }
            b"territory" => {
                cur_territory = Some(PendingTerritory {
                    shape: attr_str(&e, b"shape").unwrap_or_else(|| "NPoly".to_string()),
                    min_z: attr_i32(&e, b"minZ").unwrap_or(0),
                    max_z: attr_i32(&e, b"maxZ").unwrap_or(0),
                    rad: attr_i32(&e, b"rad"),
                    xs: Vec::new(),
                    ys: Vec::new(),
                });
                if self_closing {
                    finish_territory(&mut cur_territory, &mut cur_spawn, &mut cur_group, in_group);
                }
            }
            b"node" => {
                if let Some(t) = cur_territory.as_mut() {
                    t.xs.push(attr_i32(&e, b"x").unwrap_or(0));
                    t.ys.push(attr_i32(&e, b"y").unwrap_or(0));
                }
            }
            b"npc" => {
                let Some(npc_id) = attr_i32(&e, b"id") else { continue };
                let (x, y, z) = (attr_i32(&e, b"x"), attr_i32(&e, b"y"), attr_i32(&e, b"z"));
                let loc = match (x, y, z) {
                    (Some(x), Some(y), Some(z)) => Some(FixedLoc {
                        x,
                        y,
                        z,
                        heading: attr_i32(&e, b"heading").unwrap_or(0),
                    }),
                    _ => None,
                };
                let def = NpcSpawnDef {
                    npc_id,
                    count: attr_i32(&e, b"count").unwrap_or(1),
                    loc,
                    respawn_secs: attr_str(&e, b"respawnTime").as_deref().and_then(parse_duration_secs).unwrap_or(0),
                    respawn_random_secs: attr_str(&e, b"respawnRandom").as_deref().and_then(parse_duration_secs).unwrap_or(0),
                    db_save: attr_str(&e, b"dbSave").as_deref() == Some("true"),
                };
                if in_group {
                    if let Some(g) = cur_group.as_mut() {
                        g.npcs.push(def);
                    }
                } else {
                    default_group.get_or_insert_with(|| SpawnGroup { territories: Vec::new(), npcs: Vec::new() }).npcs.push(def);
                }
            }
            _ => {}
        }
    }
}

/// A `<territory>` mid-parse, before its `<node>` children are complete.
struct PendingTerritory {
    shape: String,
    min_z: i32,
    max_z: i32,
    rad: Option<i32>,
    xs: Vec<i32>,
    ys: Vec<i32>,
}

fn finish_territory(
    cur: &mut Option<PendingTerritory>,
    spawn: &mut Option<SpawnTemplate>,
    group: &mut Option<SpawnGroup>,
    in_group: bool,
) {
    let Some(PendingTerritory { shape, min_z, max_z, rad, xs, ys }) = cur.take() else { return };
    let form = match shape.as_str() {
        "Cuboid" if xs.len() >= 2 && ys.len() >= 2 => ZoneForm::Cuboid {
            x1: xs[0].min(xs[1]),
            x2: xs[0].max(xs[1]),
            y1: ys[0].min(ys[1]),
            y2: ys[0].max(ys[1]),
        },
        "Cylinder" if !xs.is_empty() && !ys.is_empty() => ZoneForm::Cylinder {
            x: xs[0],
            y: ys[0],
            rad: rad.unwrap_or(0),
        },
        _ if xs.len() >= 3 => ZoneForm::NPoly { xs, ys },
        _ => return, // degenerate polygon — nothing sane to randomize inside
    };
    let territory = Territory { form, min_z, max_z };
    if in_group {
        if let Some(g) = group.as_mut() {
            g.territories.push(territory);
        }
    } else if let Some(s) = spawn.as_mut() {
        s.territories.push(territory);
    }
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
    fn parses_durations() {
        assert_eq!(parse_duration_secs("60sec"), Some(60));
        assert_eq!(parse_duration_secs("9min"), Some(540));
        assert_eq!(parse_duration_secs("1hour"), Some(3600));
        assert_eq!(parse_duration_secs("2hours"), Some(7200));
        assert_eq!(parse_duration_secs("bogus"), None);
    }

    #[test]
    fn loads_real_dist_files() {
        let data = SpawnData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // Java startup: "SpawnData: Loaded 27155 spawns" (npc lines).
        let lines = data.npc_line_count();
        assert!(lines > 25_000, "expected >25k spawn lines, got {lines}");

        // Giran.xml is a plain fixed-location file: first line is
        // <npc id="30878" x="47984" y="186832" z="-3445" heading="42000" respawnTime="60sec"/>.
        let giran = data
            .spawns
            .iter()
            .find(|s| s.name.as_deref() == Some("Giran"))
            .expect("Giran spawn template");
        let first = &giran.groups[0].npcs[0];
        assert_eq!(first.npc_id, 30878);
        let loc = first.loc.expect("fixed loc");
        assert_eq!((loc.x, loc.y, loc.z, loc.heading), (47984, 186832, -3445, 42000));
        assert_eq!(first.respawn_secs, 60);

        // At least one template carries territories with a polygon.
        assert!(
            data.spawns.iter().any(|s| {
                let terr = s.territories.iter().chain(s.groups.iter().flat_map(|g| g.territories.iter()));
                terr.clone().count() > 0
            }),
            "no territories parsed"
        );
    }

    #[test]
    fn npoly_containment() {
        // A 100×100 square as an NPoly.
        let t = Territory {
            form: ZoneForm::NPoly { xs: vec![0, 100, 100, 0], ys: vec![0, 0, 100, 100] },
            min_z: -100,
            max_z: 100,
        };
        assert!(t.contains_2d(50, 50));
        assert!(!t.contains_2d(150, 50));
        assert!(!t.contains_2d(-1, 50));
        assert_eq!(t.bounds(), (0, 100, 0, 100));
        assert_eq!(t.mid_z(), 0);
    }
}
