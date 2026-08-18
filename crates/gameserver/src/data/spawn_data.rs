//! Port of `data/xml/SpawnData` + `model/spawns/*` (SpawnTemplate /
//! SpawnGroup / NpcSpawnTemplate) + the `ZoneForm` shapes territory spawns
//! randomize inside, scoped to what this dist's `data/spawns/**` actually
//! uses: fixed-location spawns, `count`, `respawnTime`/`respawnRandom`
//! (sec/min/hour), `chaseRange` and `<territories>` on the spawn or group. Unused-by-data
//! features are not ported: `zone=`/`banned_territory`/`<locations>`/
//! `<minions>`/`respawnPattern` (0 occurrences each), `<parameters>`
//! (consumed by AI scripts only, G11). `dbSave` raid persistence
//! (`DBSpawnManager`) is ported — see [`crate::game_loop::boss_respawn`].

use crate::data::xml;
use crate::data::xml::{attr_i32, attr_str};
use quick_xml::events::Event;
use tracing::info;

pub const SPAWNS_DIR: &str = "data/spawns";

/// One `<spawn>` element (Java `SpawnTemplate`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpawnTemplate {
    pub name: Option<String>,
    /// Source file, relative to [`SPAWNS_DIR`] and always `/`-separated —
    /// Java's `SpawnTemplate.getFile()`, which `NpcActionShift` prints as
    /// `%spawnfile%` after stripping the datapack root and `data/spawns/`.
    pub file: String,
    /// `ai="…"` — the script that owns this template's lifecycle
    /// (`DayNightSpawns`, `NoRandomActivity`, `ClassMaster`). Java resolves it
    /// through `SpawnData`'s script map; here the consumers match on the name.
    pub ai: Option<String>,
    /// `<parameters><param name value/>` on the template (Java
    /// `SpawnTemplate.getParameters()`) — only `NoRandomActivity`'s
    /// `disableRandomWalk` / `disableRandomAnimation` are used on this dist.
    pub parameters: std::collections::HashMap<String, String>,
    pub territories: Vec<Territory>,
    pub groups: Vec<SpawnGroup>,
}

/// One `<group>` (or the implicit default group for bare `<npc>` children).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpawnGroup {
    /// `<group name="…">` — `dayTime`/`nightTime` for the day/night templates,
    /// absent for the rest.
    pub name: Option<String>,
    /// `spawnByDefault` (Java `SpawnGroup.isSpawningByDefault`, default true):
    /// a `false` group is **not** placed by the boot pass — its owning script
    /// spawns it (the day/night halves). 95 groups on this dist.
    pub spawn_by_default: bool,
    pub territories: Vec<Territory>,
    pub npcs: Vec<NpcSpawnDef>,
}

/// One `<npc>` line (Java `NpcSpawnTemplate`, minus the unused features
/// listed in the module docs).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NpcSpawnDef {
    pub npc_id: i32,
    pub count: i32,
    /// Fixed spawn point (`x`/`y`/`z` all present). `heading` defaults to 0
    /// like Java; territory spawns get a random heading instead.
    pub loc: Option<FixedLoc>,
    pub respawn_secs: i32,
    pub respawn_random_secs: i32,
    /// `chaseRange="…"` (Java `Spawn.getChaseRange()`) — a per-spawn-line
    /// override of the `AggroDistanceCheckRange` leash radius, used where the
    /// designers want a mob to follow further than the global default (Silent
    /// Valley and Tower of Insolence on this dist). `0` = unset, use the global.
    /// Java takes `max(MaxDriftRange, chaseRange)` so the override can never
    /// shrink the leash below the mob's own random-walk radius.
    pub chase_range: i32,
    /// `dbSave="true"` (Java `NpcSpawnTemplate.hasDBSave`) — this NPC's live
    /// HP/MP and pending respawn time survive a server restart, via the
    /// `npc_respawns` table and `DBSpawnManager`. 225 spawns on this dist, all
    /// raid bosses in `RaidbossSpawns.xml`.
    pub db_save: bool,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FixedLoc {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// Java `SpawnTerritory` wrapping a `ZoneForm`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Territory {
    pub form: ZoneForm,
    pub min_z: i32,
    pub max_z: i32,
}

/// The three `ZoneForm` shapes (`ZoneNPoly`/`ZoneCuboid`/`ZoneCylinder`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum ZoneForm {
    NPoly { xs: Vec<i32>, ys: Vec<i32> },
    Cuboid { x1: i32, x2: i32, y1: i32, y2: i32 },
    Cylinder { x: i32, y: i32, rad: i32 },
}

/// Build a [`ZoneForm`] from the raw `<node>` coordinates a datapack zone
/// element carries — the one rule set shared by `spawns.xml`, `zones/*.xml` and
/// `mapregion/*.xml`, all three of which describe zones the same way.
///
/// `None` for a degenerate shape: a Cuboid or Cylinder missing its points, or a
/// polygon with fewer than three. Callers drop the territory rather than store
/// something nothing can be randomized inside.
///
/// The Cuboid arm normalizes the corners (`min`/`max`) because the datapack
/// does not guarantee which corner comes first.
pub(crate) fn build_zone_form(
    shape: &str,
    xs: Vec<i32>,
    ys: Vec<i32>,
    rad: Option<i32>,
) -> Option<ZoneForm> {
    match shape {
        "Cuboid" if xs.len() >= 2 && ys.len() >= 2 => Some(ZoneForm::Cuboid {
            x1: xs[0].min(xs[1]),
            x2: xs[0].max(xs[1]),
            y1: ys[0].min(ys[1]),
            y2: ys[0].max(ys[1]),
        }),
        "Cylinder" if !xs.is_empty() && !ys.is_empty() => Some(ZoneForm::Cylinder {
            x: xs[0],
            y: ys[0],
            rad: rad.unwrap_or(0),
        }),
        _ if xs.len() >= 3 => Some(ZoneForm::NPoly { xs, ys }),
        _ => None,
    }
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
                        && ((px as i64 - xi) * (yj - yi) < (xj - xi) * (py as i64 - yi))
                            == (yj > yi)
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

    /// `ZoneForm.getDistanceToZone(x, y)` — 2D only, and deliberately as crude
    /// as Java: for a polygon/cuboid it is the distance to the *nearest
    /// corner*, not to the nearest edge (a point just outside a long wall
    /// therefore measures far). `findNearestCastle` compares these, so the
    /// approximation has to be reproduced, not improved.
    pub fn distance_to_zone_2d(&self, px: i32, py: i32) -> f64 {
        let corner_dist = |cx: i32, cy: i32| {
            let (dx, dy) = ((cx - px) as f64, (cy - py) as f64);
            dx * dx + dy * dy
        };
        match &self.form {
            ZoneForm::NPoly { xs, ys } => xs
                .iter()
                .zip(ys)
                .map(|(&x, &y)| corner_dist(x, y))
                .fold(f64::INFINITY, f64::min)
                .sqrt(),
            ZoneForm::Cuboid { x1, x2, y1, y2 } => [
                corner_dist(*x1, *y1),
                corner_dist(*x1, *y2),
                corner_dist(*x2, *y1),
                corner_dist(*x2, *y2),
            ]
            .into_iter()
            .fold(f64::INFINITY, f64::min)
            .sqrt(),
            ZoneForm::Cylinder { x, y, rad } => {
                ((x - px) as f64).hypot((y - py) as f64) - *rad as f64
            }
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
        for path in &xml::xml_files_under(&dir) {
            parse_file(path, &relative_spawn_path(&dir, path), &mut spawns);
        }
        info!(
            "SpawnData: Loaded {} spawns.",
            spawns.iter().map(Self::template_spawn_count).sum::<usize>()
        );
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

/// The path a spawn file is known by inside the server: relative to
/// `data/spawns/`, `/`-separated on every platform. Java derives the same
/// string by trimming the datapack root off the absolute path and swapping
/// `\` for `/`. A file that somehow sits outside the spawns dir keeps its full
/// path rather than being dropped.
fn relative_spawn_path(dir: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(dir)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

fn parse_file(path: &std::path::Path, rel_path: &str, out: &mut Vec<SpawnTemplate>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    let mut cur_spawn: Option<SpawnTemplate> = None;
    // The group being filled: an explicit `<group>` while inside one, else the
    // implicit default group Java creates for bare `<npc>` children.
    let mut cur_group: Option<SpawnGroup> = None;
    let mut default_group: Option<SpawnGroup> = None;
    let mut in_group = false;
    // <territories> scope: attach finished territories to the group when it's
    // open, else to the spawn template.
    let mut cur_territory: Option<PendingTerritory> = None;

    for event in xml::events(&content) {
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
                    b"territory" => finish_territory(
                        &mut cur_territory,
                        &mut cur_spawn,
                        &mut cur_group,
                        in_group,
                    ),
                    _ => {}
                }
                continue;
            }
            _ => continue,
        };

        match e.name().as_ref() {
            b"spawn" => {
                cur_spawn = Some(SpawnTemplate {
                    name: attr_str(&e, b"name"),
                    file: rel_path.to_string(),
                    ai: attr_str(&e, b"ai"),
                    parameters: std::collections::HashMap::new(),
                    territories: Vec::new(),
                    groups: Vec::new(),
                });
            }
            b"group" => {
                in_group = true;
                cur_group = Some(SpawnGroup {
                    name: attr_str(&e, b"name"),
                    spawn_by_default: attr_str(&e, b"spawnByDefault")
                        .map(|v| v != "false")
                        .unwrap_or(true),
                    territories: Vec::new(),
                    npcs: Vec::new(),
                });
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
            // `<parameters><param name value/>` — kept only at template level,
            // which is where this dist puts them.
            b"param" => {
                if let (Some(sp), Some(name), Some(value)) = (
                    cur_spawn.as_mut(),
                    attr_str(&e, b"name"),
                    attr_str(&e, b"value"),
                ) {
                    sp.parameters.insert(name, value);
                }
            }
            b"node" => {
                if let Some(t) = cur_territory.as_mut() {
                    t.xs.push(attr_i32(&e, b"x").unwrap_or(0));
                    t.ys.push(attr_i32(&e, b"y").unwrap_or(0));
                }
            }
            b"npc" => {
                let Some(npc_id) = attr_i32(&e, b"id") else {
                    continue;
                };
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
                    respawn_secs: attr_str(&e, b"respawnTime")
                        .as_deref()
                        .and_then(parse_duration_secs)
                        .unwrap_or(0),
                    respawn_random_secs: attr_str(&e, b"respawnRandom")
                        .as_deref()
                        .and_then(parse_duration_secs)
                        .unwrap_or(0),
                    chase_range: attr_i32(&e, b"chaseRange").unwrap_or(0),
                    db_save: attr_str(&e, b"dbSave").as_deref() == Some("true"),
                };
                if in_group {
                    if let Some(g) = cur_group.as_mut() {
                        g.npcs.push(def);
                    }
                } else {
                    default_group
                        .get_or_insert_with(|| SpawnGroup {
                            name: None,
                            spawn_by_default: true,
                            territories: Vec::new(),
                            npcs: Vec::new(),
                        })
                        .npcs
                        .push(def);
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
    let Some(PendingTerritory {
        shape,
        min_z,
        max_z,
        rad,
        xs,
        ys,
    }) = cur.take()
    else {
        return;
    };
    // Degenerate shape — nothing sane to randomize inside, so drop it.
    let Some(form) = build_zone_form(&shape, xs, ys, rad) else {
        return;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;

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
        let data = dist::spawns();
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
        // `NpcActionShift`'s %spawnfile%: the path below `data/spawns/`, with
        // the subdirectory kept (two files on this dist are named Giran.xml).
        assert_eq!(giran.file, "Giran/Giran.xml");
        let first = &giran.groups[0].npcs[0];
        assert_eq!(first.npc_id, 30878);
        let loc = first.loc.expect("fixed loc");
        assert_eq!(
            (loc.x, loc.y, loc.z, loc.heading),
            (47984, 186832, -3445, 42000)
        );
        assert_eq!(first.respawn_secs, 60);

        // At least one template carries territories with a polygon.
        assert!(
            data.spawns.iter().any(|s| {
                let terr = s
                    .territories
                    .iter()
                    .chain(s.groups.iter().flat_map(|g| g.territories.iter()));
                terr.clone().count() > 0
            }),
            "no territories parsed"
        );
    }

    #[test]
    fn npoly_containment() {
        // A 100×100 square as an NPoly.
        let t = Territory {
            form: ZoneForm::NPoly {
                xs: vec![0, 100, 100, 0],
                ys: vec![0, 0, 100, 100],
            },
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
