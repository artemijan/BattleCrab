//! Port of `instancemanager/ZoneManager` narrowed to the G12 slice: only
//! `data/zones/peace.xml`, `water.xml` and `no_restart.xml` load (the three
//! zone families whose consumers exist — the other ~37 files/32 types are
//! gated on systems like sieges/castles/olympiad and stay in G14). Shapes
//! reuse the `ZoneForm`/`Territory` geometry already ported for spawn
//! territories (Java shares `ZoneForm` between `ZoneManager` and `SpawnData`
//! the same way).
//!
//! Spatial index: Java buckets zones into a `ZoneRegion[][]` grid of
//! `SHIFT_BY = 15` cells (32768 units — one map tile), each region holding
//! every zone whose bounding box overlaps it; a point query walks its cell's
//! (few) zones. Same structure here as a HashMap keyed by cell coordinate.
//!
//! `NoRestartZone`'s `restartTime`/`restartAllowedTime` `<stat>`s are not
//! stored: their only Java consumer is `onPlayerLoginInside`'s
//! "teleport home if you were away too long" — deferred with the flag's
//! other consumers (nothing in this Mobius version reads `ZoneId.NO_RESTART`
//! after it's set), so the zone only tracks membership for now.

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

use super::spawn_data::{Territory, ZoneForm};

/// Java `ZoneManager.SHIFT_BY` (15) — zone-grid cells are one map tile, not
/// the 2048-unit visibility cells (`World::REGION_SHIFT` = 11).
const ZONE_SHIFT: i32 = 15;

/// The zone families this slice loads (3 of Java's 35 `ZoneType` classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Peace,
    Water,
    NoRestart,
    /// Java `ArenaZone` → `ZoneId.PVP`: free-for-all areas where players are
    /// auto-attackable and hostile actions don't raise a flag.
    Pvp,
    /// Java `SiegeZone` → `ZoneId.SIEGE`: a castle's battlefield. Only "active"
    /// (auto-attackable) while that castle's siege is in progress; the
    /// [`Zone::castle_id`] ties the zone to its siege.
    Siege,
    /// Java `EffectZone`: periodically casts a fixed skill list on the players
    /// standing in it — the Blazing Swamp's fire, Sea of Spores' poison, the
    /// Hot Springs buff trio. See [`Zone::effect`].
    Effect,
}

impl ZoneKind {
    /// Bit in a [`ZoneData::mask_at`] / `ZoneFlags` membership mask.
    pub fn bit(self) -> u8 {
        match self {
            ZoneKind::Peace => 1,
            ZoneKind::Water => 2,
            ZoneKind::NoRestart => 4,
            ZoneKind::Pvp => 8,
            ZoneKind::Siege => 16,
            // `ZoneId.ALTERED` — Java sets it on any player inside an
            // EffectZone. Nothing reads it yet, but the bit keeps the
            // membership mask complete so enter/exit diffs work.
            ZoneKind::Effect => 32,
        }
    }
}

pub struct Zone {
    pub name: String,
    pub kind: ZoneKind,
    pub territory: Territory,
    /// `<stat name="castleId">` for `SiegeZone`s; 0 otherwise.
    pub castle_id: i32,
    /// `EffectZone` parameters; `None` for every other kind.
    pub effect: Option<EffectZoneParams>,
}

/// Java `EffectZone`'s `<stat>` block. Defaults are the Java field
/// initialisers (`_chance = 100`, `_initialDelay = 0`, `_reuse = 30000`).
#[derive(Debug, Clone)]
pub struct EffectZoneParams {
    /// `skillIdLvl` — `id-lvl;id-lvl;` pairs, all cast together.
    pub skills: Vec<(i32, i32)>,
    /// `chance` — rolled per creature per tick, not per skill.
    pub chance: i32,
    /// `initialDelay` (ms) before the zone's first tick.
    pub initial_delay: i32,
    /// `reuse` (ms) between ticks.
    pub reuse: i32,
    /// `default_enabled` — a disabled zone ticks nothing until something
    /// enables it (siege scripts). Java's `_enabled` defaults to **true**.
    pub enabled: bool,
    /// Derived from `targetClass`: does this zone's tick reach players at all?
    ///
    /// Java tracks only creatures of `targetClass` (default `Creature`, i.e.
    /// everyone) as being "inside", and the tick *additionally* requires
    /// `isPlayer()`. So the 27 zones declaring `targetClass="Npc"` cast on
    /// **nobody**. Kept explicit so that stays true here rather than being
    /// accidentally revived.
    pub casts_on_players: bool,
    /// `removeEffectsOnExit` — strip the zone's own skills when leaving.
    pub remove_effects_on_exit: bool,
}

#[derive(Default)]
pub struct ZoneData {
    pub zones: Vec<Zone>,
    /// Zone-grid cell → indexes into `zones` whose bounds overlap the cell.
    grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl ZoneData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut zones = Vec::new();
        for (file, kind) in [
            ("peace.xml", ZoneKind::Peace),
            ("water.xml", ZoneKind::Water),
            ("no_restart.xml", ZoneKind::NoRestart),
            // `pvp.xml` is uniformly `ArenaZone`, so the filename→kind mapping
            // is correct. `underground_coliseum.xml` mixes zone types and needs
            // per-zone `type=` parsing before it can be loaded — deferred.
            ("pvp.xml", ZoneKind::Pvp),
            // `castle_siege.xml` is uniformly `SiegeZone`; each zone's `castleId`
            // stat ties it to its castle's siege.
            ("castle_siege.xml", ZoneKind::Siege),
            // Files carrying `EffectZone`s. Several mix zone types, which is
            // why the parser now reads each zone's own `type=` and skips the
            // kinds that aren't ported — the mapping below is only a fallback.
            ("effect.xml", ZoneKind::Effect),
            ("cleft.xml", ZoneKind::Effect),
            ("devil_isle.xml", ZoneKind::Effect),
            ("plains_of_the_lizardmen.xml", ZoneKind::Effect),
            ("zone.xml", ZoneKind::Effect),
            ("underground_coliseum.xml", ZoneKind::Effect),
        ] {
            parse_file(&format!("{file_path}data/zones/{file}"), kind, &mut zones);
        }
        let mut data = Self { zones, grid: Default::default() };
        data.build_grid();
        let effects = data.zones.iter().filter(|z| z.kind == ZoneKind::Effect).count();
        info!("ZoneData: Loaded {} zones ({effects} effect).", data.zones.len());
        data
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Test/synthetic hook: register a zone directly and re-index.
    pub fn insert(&mut self, zone: Zone) {
        self.zones.push(zone);
        self.build_grid();
    }

    /// Java `ZoneManager` registration: each zone lands in every grid cell
    /// its bounding box overlaps.
    fn build_grid(&mut self) {
        self.grid.clear();
        for (i, zone) in self.zones.iter().enumerate() {
            let (x1, x2, y1, y2) = zone.territory.bounds();
            for cx in (x1 >> ZONE_SHIFT)..=(x2 >> ZONE_SHIFT) {
                for cy in (y1 >> ZONE_SHIFT)..=(y2 >> ZONE_SHIFT) {
                    self.grid.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
    }

    /// Every loaded zone containing the point (`ZoneRegion.getZones` +
    /// `ZoneType.isInsideZone(x, y, z)` — 2D shape + the z band).
    pub fn zones_at(&self, x: i32, y: i32, z: i32) -> impl Iterator<Item = &Zone> {
        self.grid
            .get(&(x >> ZONE_SHIFT, y >> ZONE_SHIFT))
            .into_iter()
            .flatten()
            .map(|&i| &self.zones[i as usize])
            .filter(move |zn| z >= zn.territory.min_z && z <= zn.territory.max_z && zn.territory.contains_2d(x, y))
    }

    /// Indices (into [`Self::zones`]) of every zone containing the point.
    /// The effect-zone tick needs the index, not just the zone, so it can key
    /// each zone's own reuse timer.
    pub fn zone_indices_at(&self, x: i32, y: i32, z: i32) -> impl Iterator<Item = usize> + '_ {
        self.grid
            .get(&(x >> ZONE_SHIFT, y >> ZONE_SHIFT))
            .into_iter()
            .flatten()
            .map(|&i| i as usize)
            .filter(move |&i| {
                let zn = &self.zones[i];
                z >= zn.territory.min_z && z <= zn.territory.max_z && zn.territory.contains_2d(x, y)
            })
    }

    /// `ZoneKind` membership bits at a point — what `revalidate_zones` diffs.
    pub fn mask_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.zones_at(x, y, z).fold(0, |m, zn| m | zn.kind.bit())
    }

    /// The castle id of the `SiegeZone` covering the point, if any (Java
    /// `getZone(x, y, z, SiegeZone.class).getSiege().getCastle()`).
    pub fn siege_castle_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.zones_at(x, y, z).find(|zn| zn.kind == ZoneKind::Siege).map(|zn| zn.castle_id)
    }
}

/// Map a `type="…"` attribute to a [`ZoneKind`]. `None` for kinds not ported
/// yet, so mixed files can be read without pulling in unported behaviour.
fn kind_from_type(ty: &str) -> Option<ZoneKind> {
    Some(match ty {
        "PeaceZone" => ZoneKind::Peace,
        "WaterZone" => ZoneKind::Water,
        "NoRestartZone" => ZoneKind::NoRestart,
        "ArenaZone" => ZoneKind::Pvp,
        "SiegeZone" => ZoneKind::Siege,
        "EffectZone" => ZoneKind::Effect,
        _ => return None,
    })
}

/// `skillIdLvl="4150-7;4148-1;"` → `[(4150, 7), (4148, 1)]`.
fn parse_skill_id_lvl(raw: &str) -> Vec<(i32, i32)> {
    raw.split(';')
        .filter_map(|pair| {
            let (id, lvl) = pair.trim().split_once('-')?;
            Some((id.trim().parse().ok()?, lvl.trim().parse().ok()?))
        })
        .collect()
}

/// `default_kind` is the filename-derived fallback for files that predate
/// per-zone `type=` parsing; a zone's own `type=` attribute always wins.
fn parse_file(path: &str, kind: ZoneKind, out: &mut Vec<Zone>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let mut reader = Reader::from_str(&content);

    struct Pending {
        name: String,
        shape: String,
        min_z: i32,
        max_z: i32,
        rad: Option<i32>,
        xs: Vec<i32>,
        ys: Vec<i32>,
        castle_id: i32,
        kind: ZoneKind,
        skills: Vec<(i32, i32)>,
        chance: i32,
        initial_delay: i32,
        reuse: i32,
        enabled: bool,
        casts_on_players: bool,
        remove_effects_on_exit: bool,
    }
    let mut cur: Option<Pending> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone" {
                    if let Some(p) = cur.take() {
                        if let Some(form) = build_form(&p.shape, p.xs, p.ys, p.rad) {
                            let effect = (p.kind == ZoneKind::Effect).then(|| EffectZoneParams {
                                skills: p.skills,
                                chance: p.chance,
                                initial_delay: p.initial_delay,
                                reuse: p.reuse,
                                enabled: p.enabled,
                                casts_on_players: p.casts_on_players,
                                remove_effects_on_exit: p.remove_effects_on_exit,
                            });
                            out.push(Zone {
                                name: p.name,
                                kind: p.kind,
                                territory: Territory { form, min_z: p.min_z, max_z: p.max_z },
                                castle_id: p.castle_id,
                                effect,
                            });
                        }
                    }
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"zone" => {
                cur = Some(Pending {
                    name: attr_str(&e, b"name").unwrap_or_default(),
                    shape: attr_str(&e, b"shape").unwrap_or_else(|| "NPoly".to_string()),
                    min_z: attr_i32(&e, b"minZ").unwrap_or(i32::MIN),
                    max_z: attr_i32(&e, b"maxZ").unwrap_or(i32::MAX),
                    rad: attr_i32(&e, b"rad"),
                    xs: Vec::new(),
                    ys: Vec::new(),
                    castle_id: 0,
                    // A zone's own `type=` wins; the filename mapping is the
                    // fallback for the single-type files.
                    kind: attr_str(&e, b"type").as_deref().and_then(kind_from_type).unwrap_or(kind),
                    skills: Vec::new(),
                    chance: 100,
                    initial_delay: 0,
                    reuse: 30_000,
                    enabled: true,
                    casts_on_players: true,
                    remove_effects_on_exit: false,
                });
                // A zone whose `type=` names a kind we don't port yet is
                // skipped outright rather than mis-filed under the fallback.
                if let Some(ty) = attr_str(&e, b"type") {
                    if kind_from_type(&ty).is_none() {
                        cur = None;
                    }
                }
            }
            // Zone files capitalize the node attributes (`X`/`Y`), unlike
            // the spawn territories' lowercase — accept either.
            b"node" => {
                if let Some(p) = cur.as_mut() {
                    p.xs.push(attr_i32(&e, b"X").or_else(|| attr_i32(&e, b"x")).unwrap_or(0));
                    p.ys.push(attr_i32(&e, b"Y").or_else(|| attr_i32(&e, b"y")).unwrap_or(0));
                }
            }
            // `SiegeZone`s carry `<stat name="castleId" val="N"/>`; other zone
            // stats are still skipped (see module docs).
            b"stat" => {
                let (Some(p), Some(name)) = (cur.as_mut(), attr_str(&e, b"name")) else { continue };
                let val = attr_str(&e, b"val").unwrap_or_default();
                match name.as_str() {
                    "castleId" => p.castle_id = val.parse().unwrap_or(0),
                    "skillIdLvl" => p.skills = parse_skill_id_lvl(&val),
                    "chance" => p.chance = val.parse().unwrap_or(p.chance),
                    "initialDelay" => p.initial_delay = val.parse().unwrap_or(p.initial_delay),
                    "reuse" => p.reuse = val.parse().unwrap_or(p.reuse),
                    "default_enabled" => p.enabled = val == "true",
                    "targetClass" => p.casts_on_players = val != "Npc",
                    "removeEffectsOnExit" => p.remove_effects_on_exit = val == "true",
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Same shape-construction rules as `spawn_data::finish_territory`.
fn build_form(shape: &str, xs: Vec<i32>, ys: Vec<i32>, rad: Option<i32>) -> Option<ZoneForm> {
    match shape {
        "Cuboid" if xs.len() >= 2 && ys.len() >= 2 => Some(ZoneForm::Cuboid {
            x1: xs[0].min(xs[1]),
            x2: xs[0].max(xs[1]),
            y1: ys[0].min(ys[1]),
            y2: ys[0].max(ys[1]),
        }),
        "Cylinder" if !xs.is_empty() && !ys.is_empty() => {
            Some(ZoneForm::Cylinder { x: xs[0], y: ys[0], rad: rad.unwrap_or(0) })
        }
        _ if xs.len() >= 3 => Some(ZoneForm::NPoly { xs, ys }),
        _ => None,
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
    fn loads_real_dist_files() {
        let data = ZoneData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        // 134 peace + 423 water + 47 no_restart + 12 pvp + 9 siege + 218 effect.
        //
        // Was 605 before per-zone `type=` parsing landed. Effect zones account
        // for 218 of the increase; the other 20 are Peace/NoRestart/Pvp zones
        // that live in the *mixed* files (devil_isle, zone, underground_
        // coliseum) which the filename→kind loader couldn't read at all — so
        // they were silently missing from the world.
        assert_eq!(data.zones.len(), 843);
        let count = |k: ZoneKind| data.zones.iter().filter(|z| z.kind == k).count();
        assert_eq!(count(ZoneKind::Peace), 134);
        assert_eq!(count(ZoneKind::Water), 423);
        assert_eq!(count(ZoneKind::NoRestart), 47);
        assert_eq!(count(ZoneKind::Pvp), 12);
        assert_eq!(count(ZoneKind::Siege), 9);
        assert_eq!(count(ZoneKind::Effect), 218);

        // Talking Island town center sits in talking_island_town_peace_zone1
        // (NPoly, z band [-3966, -3466]).
        let mask = data.mask_at(-84300, 243000, -3700);
        assert_eq!(mask & ZoneKind::Peace.bit(), ZoneKind::Peace.bit());
        // Same x/y far outside the z band: not in the zone.
        assert_eq!(data.mask_at(-84300, 243000, 0), 0);

        // A point inside water cuboid 11_23_water1 ([-294912..-262144] ×
        // [163839..196607], z [-4779, -3779]).
        let mask = data.mask_at(-280000, 180000, -4000);
        assert_eq!(mask & ZoneKind::Water.bit(), ZoneKind::Water.bit());

        // Zaken's deck is a no-restart NPoly.
        let mask = data.mask_at(54000, 217000, -3000);
        assert_eq!(mask & ZoneKind::NoRestart.bit(), ZoneKind::NoRestart.bit());

        // gludin_pvp ArenaZone (NPoly, z band [-3752, -352]).
        let mask = data.mask_at(-88000, 142000, -1000);
        assert_eq!(mask & ZoneKind::Pvp.bit(), ZoneKind::Pvp.bit());
    }

    #[test]
    fn grid_and_mask_on_synthetic_zone() {
        let mut data = ZoneData::empty();
        data.insert(Zone {
            name: "test_peace".into(),
            kind: ZoneKind::Peace,
            territory: Territory {
                form: ZoneForm::Cuboid { x1: 0, x2: 1000, y1: 0, y2: 1000 },
                min_z: -100,
                max_z: 100,
            },
            castle_id: 0,
            effect: None,
        });
        assert_eq!(data.mask_at(500, 500, 0), ZoneKind::Peace.bit());
        assert_eq!(data.mask_at(1500, 500, 0), 0);
        assert_eq!(data.mask_at(500, 500, 200), 0);
    }
}

#[cfg(test)]
mod effect_zone_tests {
    use super::*;

    const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

    #[test]
    fn parses_effect_zones_from_dist() {
        let data = ZoneData::load_from(DIST);
        let effects: Vec<&Zone> = data.zones.iter().filter(|z| z.kind == ZoneKind::Effect).collect();
        let with_skills = effects.iter().filter(|z| !z.effect.as_ref().unwrap().skills.is_empty()).count();
        let npc_only = effects.iter().filter(|z| !z.effect.as_ref().unwrap().casts_on_players).count();
        println!("EFFECT ZONES={} with_skills={with_skills} npc_targeted={npc_only}", effects.len());
        assert_eq!(effects.len(), 218, "expected the dist's 218 EffectZones");
        assert_eq!(npc_only, 27, "expected 27 targetClass=Npc zones (which cast on nobody)");

        // Blazing Swamp: fire damage-over-time, 50% chance.
        let fire = effects
            .iter()
            .find(|z| z.name == "fireswamp_1")
            .expect("fireswamp_1 must load");
        let p = fire.effect.as_ref().unwrap();
        assert_eq!(p.skills, vec![(4150, 1)], "s_area_fire1");
        assert_eq!(p.chance, 50);
    }

    /// The mixed-type files must not smuggle unported kinds in under the
    /// filename fallback.
    #[test]
    fn mixed_files_only_yield_ported_kinds() {
        let data = ZoneData::load_from(DIST);
        for z in &data.zones {
            assert!(
                matches!(
                    z.kind,
                    ZoneKind::Peace | ZoneKind::Water | ZoneKind::NoRestart | ZoneKind::Pvp | ZoneKind::Siege | ZoneKind::Effect
                ),
                "zone {} has an unported kind",
                z.name
            );
            if z.kind == ZoneKind::Effect {
                assert!(z.effect.is_some(), "effect zone {} must carry params", z.name);
            }
        }
    }
}
