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

use quick_xml::Reader;
use quick_xml::events::Event;
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
    /// Java `DamageZone`: flat HP/MP loss per tick. Devil's Isle lava plus the
    /// castle traps (which only bite during their castle's siege).
    Damage,
    /// Java `SwampZone`: multiplies a playable's move speed while inside.
    Swamp,
    /// Java `ScriptZone` — a named region a script addresses by id.
    Script,
    /// Java `FishingZone` → `ZoneId.FISHING`: where a rod may be cast. Queried
    /// by geometry (`zones_at`), never by membership mask, so it claims no bit.
    Fishing,
    /// Java `ClanHallZone`: the interior of a clan hall, tying a position to its
    /// `clanHallId` (banish scope, owner-member regen). Queried by geometry
    /// (`clan_hall_at`), never by membership mask, so it claims no bit.
    ClanHall,
    /// Java `DerbyTrackZone`: the Monster Race Track spectator area (G26.5). The
    /// race manager broadcasts board/animation packets to everyone standing in
    /// it. Queried by geometry (`zones_at`), never by membership mask — the u8
    /// mask is full — so it claims no bit.
    DerbyTrack,
    /// Java `JailZone` → `ZoneId.JAIL`: the GM prison (`gm_room.xml`). A jailed
    /// player is confined here — leaving triggers a teleport back (G31). Queried
    /// by geometry (`jail_zone_at`), never by membership mask — the u8 mask is
    /// full — so it claims no bit.
    Jail,
    /// Java `HqZone` → `ZoneId.HQ` (`castle_hq.xml`): the marked-out patches of
    /// a castle's battlefield where an attacking clan may plant its
    /// headquarters. `BuildCampSkillCondition` refuses the cast anywhere else,
    /// so without these a base camp could go up in the middle of the courtyard.
    /// Its `<stat name="castleId">` is held in [`Zone::castle_id`]; queried by
    /// geometry (`hq_castle_at`), so it claims no membership bit.
    Hq,
    /// Java `TaxZone` (`tax.xml`): the territory whose merchant trade feeds a
    /// castle's treasury. Its `<stat name="domainId">` is the castle id, held in
    /// [`Zone::castle_id`]. Java latches the zone onto every NPC standing in it
    /// (`Npc.setTaxZone`) and reads it back at sale time; the port queries it by
    /// geometry (`tax_castle_at`), so it claims no membership bit.
    Tax,
    /// Java `ResidenceTeleportZone` (`castle_teleport.xml`): the castle's
    /// owner-restart territory, whose `<spawn>` is where
    /// `Castle.oustAllPlayers` (the mass gatekeeper's `MASS_TELEPORT`) drops
    /// everyone standing in it. Queried by geometry, never by membership mask
    /// — the u8 mask is full — so it claims no bit.
    ResidenceTeleport,
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
            ZoneKind::Damage => 64,
            // `ScriptZone` has no `ZoneId` of its own in Java — it is addressed
            // by id, never by membership mask — so it claims no bit.
            ZoneKind::Script => 0,
            // `ZoneId.SWAMP` — read by the speed finalizer.
            ZoneKind::Swamp => 128,
            // Queried by geometry, not membership — no bit (like `Script`).
            ZoneKind::Fishing => 0,
            // Queried by geometry (`clan_hall_at`), no membership bit.
            ZoneKind::ClanHall => 0,
            // Queried by geometry (`zones_at`), no membership bit (u8 mask full).
            ZoneKind::DerbyTrack => 0,
            // Queried by geometry (`jail_zone_at`), no membership bit (mask full).
            ZoneKind::Jail => 0,
            // Queried by geometry (`residence_teleport_zone`), no bit (mask full).
            ZoneKind::ResidenceTeleport => 0,
            // Queried by geometry (`hq_castle_at`), no membership bit (mask full).
            ZoneKind::Hq => 0,
            // Queried by geometry (`tax_castle_at`), no bit (mask full).
            ZoneKind::Tax => 0,
        }
    }
}

pub struct Zone {
    /// `<zone id="…">`. Scripts address zones by this (`getZoneById(12012)` is
    /// Queen Ant's lair), so it is kept even though the spatial lookups don't
    /// need it. `0` when the file omits it.
    pub id: i32,
    pub name: String,
    pub kind: ZoneKind,
    pub territory: Territory,
    /// `<stat name="castleId">` for `SiegeZone`s; 0 otherwise.
    pub castle_id: i32,
    /// `<stat name="clanHallId">` for `ClanHallZone`s; 0 otherwise.
    pub clan_hall_id: i32,
    /// `EffectZone` parameters; `None` for every other kind.
    pub effect: Option<EffectZoneParams>,
    /// `DamageZone` parameters; `None` for every other kind.
    pub damage: Option<DamageZoneParams>,
    /// `SwampZone` move-speed multiplier; `None` for every other kind.
    pub swamp: Option<SwampZoneParams>,
}

/// Java `DamageZone`'s `<stat>` block. Defaults are the Java field
/// initialisers — note **`damageHPPerSec` defaults to 200 and no zone in this
/// dist overrides it**, so every damage zone here hits for 200.
#[derive(Debug, Clone)]
pub struct DamageZoneParams {
    pub hp_per_tick: i32,
    pub mp_per_tick: i32,
    /// `initialDelay` (Java `_startTask`, default 10 ms).
    pub initial_delay: i32,
    /// `reuse` (Java `_reuseTask`, default 5000 ms).
    pub reuse: i32,
    pub enabled: bool,
    /// `castleId` — a castle trap, live only while that castle's siege runs
    /// (and never against that castle's own defenders).
    pub castle_id: i32,
}

/// Java `SwampZone`. `move_bonus` defaults to 0.5; this dist uses 0.2.
#[derive(Debug, Clone)]
pub struct SwampZoneParams {
    pub move_bonus: f64,
    pub enabled: bool,
    pub castle_id: i32,
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
    /// castle id → its `ResidenceTeleportZone`'s `<spawn>` points (Java
    /// `ZoneRespawn._spawnLocs`), kept beside the zones because only this one
    /// kind carries them.
    residence_teleport_spawns: std::collections::HashMap<i32, Vec<(i32, i32, i32)>>,
}

impl ZoneData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let mut zones = Vec::new();
        let mut residence_teleport_spawns = std::collections::HashMap::new();
        for (file, kind) in [
            ("peace.xml", ZoneKind::Peace),
            ("water.xml", ZoneKind::Water),
            ("fishing.xml", ZoneKind::Fishing),
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
            // `ScriptZone` files. These carry no behaviour of their own — the
            // zones exist to be addressed by id from a script — but they must
            // be *loaded* or `by_id` finds nothing, which is how zone 12012
            // (Queen Ant's lair) was missing on the first run of this slice.
            ("custom_script.xml", ZoneKind::Script),
            ("dummy.xml", ZoneKind::Script),
            ("damage.xml", ZoneKind::Damage),
            ("castle_trap.xml", ZoneKind::Damage),
            ("swamp.xml", ZoneKind::Swamp),
            // `clan_hall.xml` is uniformly `ClanHallZone`; each zone's
            // `clanHallId` stat ties it to its residence.
            ("clan_hall.xml", ZoneKind::ClanHall),
            // `gm_room.xml` is uniformly `JailZone` — the GM prison a jailed
            // player is confined to (G31).
            ("gm_room.xml", ZoneKind::Jail),
            // `castle_teleport.xml` is uniformly `ResidenceTeleportZone` — the
            // owner-restart territory the mass gatekeeper ousts people from
            // (`territory_war_teleport.xml` is the later-chronicle sibling and
            // is deliberately not loaded).
            ("castle_teleport.xml", ZoneKind::ResidenceTeleport),
            // `tax.xml` is uniformly `TaxZone` — the trade territory whose
            // purchases pay tax into `domainId`'s castle treasury.
            ("tax.xml", ZoneKind::Tax),
            // `castle_hq.xml` is uniformly `HqZone` — the patches of each
            // battlefield where an attacker may plant its headquarters.
            // (`fortress_hq.xml` and `territory_war_hq.xml` are the fort-siege
            // and territory-war siblings, neither of which exists here.)
            ("castle_hq.xml", ZoneKind::Hq),
        ] {
            let before = zones.len();
            parse_file(
                &format!("{file_path}data/zones/{file}"),
                kind,
                &mut zones,
                &mut residence_teleport_spawns,
            );
            // The two `ScriptZone` files are pulled in *only* for their script
            // zones. They also contain unrelated kinds — `custom_script.xml`
            // ships a stray `SiegeZone` ("GainakSiege", a later-chronicle area
            // with no `castleId`) — and letting those through would set the
            // Siege membership bit on players standing there, which
            // `death.rs`'s free-death-zone check reads: dying in Gainak would
            // silently skip the exp penalty. Adding a file for one kind must
            // not change behaviour for another.
            if kind == ZoneKind::Script {
                let kept: Vec<Zone> = zones
                    .split_off(before)
                    .into_iter()
                    .filter(|z| z.kind == ZoneKind::Script)
                    .collect();
                zones.extend(kept);
            }
        }
        let mut data = Self {
            zones,
            grid: Default::default(),
            residence_teleport_spawns,
        };
        data.build_grid();
        let effects = data
            .zones
            .iter()
            .filter(|z| z.kind == ZoneKind::Effect)
            .count();
        info!(
            "ZoneData: Loaded {} zones ({effects} effect).",
            data.zones.len()
        );
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
            .filter(move |zn| {
                z >= zn.territory.min_z && z <= zn.territory.max_z && zn.territory.contains_2d(x, y)
            })
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
        self.zones_at(x, y, z)
            .find(|zn| zn.kind == ZoneKind::Siege)
            .map(|zn| zn.castle_id)
    }

    /// Java `CastleManager.findNearestCastle(obj)` — the castle whose
    /// `SiegeZone` covers the point, else the castle whose zone is closest by
    /// `Castle.getDistance` (`ZoneForm.getDistanceToZone`: 2D, corner-distance).
    /// `Npc.getCastle()` resolves through here, so this is what names the
    /// castle in the Mammon spawn announce and gates the castle-service NPCs.
    ///
    /// Zones carrying no `castleId` (the stray later-chronicle `GainakSiege`)
    /// belong to no castle and are skipped — Java iterates castles, not zones.
    pub fn nearest_castle_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        if let Some(id) = self.siege_castle_at(x, y, z) {
            return Some(id);
        }
        self.zones
            .iter()
            .filter(|zn| zn.kind == ZoneKind::Siege && zn.castle_id > 0)
            .min_by(|a, b| {
                a.territory
                    .distance_to_zone_2d(x, y)
                    .total_cmp(&b.territory.distance_to_zone_2d(x, y))
            })
            .map(|zn| zn.castle_id)
    }

    /// A castle's `ResidenceTeleportZone` (Java `Castle.getTeleZone()`) — the
    /// owner-restart territory the mass gatekeeper ousts players from.
    pub fn residence_teleport_zone(&self, castle_id: i32) -> Option<&Zone> {
        self.zones
            .iter()
            .find(|zn| zn.kind == ZoneKind::ResidenceTeleport && zn.castle_id == castle_id)
    }

    /// That zone's `<spawn>` points — where the ousted land
    /// (`ZoneRespawn.getSpawnLoc()` picks one at random).
    pub fn residence_teleport_spawns(&self, castle_id: i32) -> &[(i32, i32, i32)] {
        self.residence_teleport_spawns
            .get(&castle_id)
            .map_or(&[][..], |v| v.as_slice())
    }

    /// The clan hall whose interior contains `(x, y, z)` (Java
    /// `ClanHallZone.getResidenceId` via `ZoneManager`), if any.
    pub fn clan_hall_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.zones_at(x, y, z)
            .find(|zn| zn.kind == ZoneKind::ClanHall)
            .map(|zn| zn.clan_hall_id)
    }

    /// Whether `(x, y, z)` is inside a `JailZone` (Java `isInsideZone(ZoneId
    /// .JAIL)`) — the confinement check for jailed players (G31).
    pub fn in_jail_zone(&self, x: i32, y: i32, z: i32) -> bool {
        self.zones_at(x, y, z).any(|zn| zn.kind == ZoneKind::Jail)
    }

    /// Which TvT headquarters peace zone covers `(x, y, z)` — `1` for
    /// `colosseum_peace1` (blue), `2` for `colosseum_peace2` (red), `0` for
    /// neither. Java holds the two `ZoneType`s by name and compares identity;
    /// the port resolves the same two names by geometry.
    pub fn tvt_hq_zone_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.zones_at(x, y, z)
            .find_map(|zn| match zn.name.as_str() {
                "colosseum_peace1" => Some(1),
                "colosseum_peace2" => Some(2),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// The castle whose `TaxZone` covers `(x, y, z)` — Java `Npc.getTaxCastle()`
    /// via the zone the NPC stands in. `None` outside every tax zone, which is
    /// where most of the map is (only the nine castle territories are covered).
    pub fn tax_castle_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.zones_at(x, y, z)
            .find(|zn| zn.kind == ZoneKind::Tax)
            .map(|zn| zn.castle_id)
    }

    /// Java `isInsideZone(ZoneId.HQ)` — the castle whose headquarters area
    /// covers this point, if any (`BuildCampSkillCondition`'s last gate).
    pub fn hq_castle_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.zones_at(x, y, z)
            .find(|zn| zn.kind == ZoneKind::Hq)
            .map(|zn| zn.castle_id)
    }
}

/// Map a `type="…"` attribute to a [`ZoneKind`]. `None` for kinds not ported
/// yet, so mixed files can be read without pulling in unported behaviour.
fn kind_from_type(ty: &str) -> Option<ZoneKind> {
    Some(match ty {
        "PeaceZone" => ZoneKind::Peace,
        "WaterZone" => ZoneKind::Water,
        "FishingZone" => ZoneKind::Fishing,
        "NoRestartZone" => ZoneKind::NoRestart,
        "ArenaZone" => ZoneKind::Pvp,
        "SiegeZone" => ZoneKind::Siege,
        "EffectZone" => ZoneKind::Effect,
        "DamageZone" => ZoneKind::Damage,
        "SwampZone" => ZoneKind::Swamp,
        // `ScriptZone` carries no behaviour of its own — it exists to be looked
        // up **by id** from a script (`ZoneManager.getZoneById`). The grand-boss
        // scripts use it for `isInsideZone` and `movePlayersTo`; 133 ship here.
        "ScriptZone" => ZoneKind::Script,
        "ClanHallZone" => ZoneKind::ClanHall,
        "DerbyTrackZone" => ZoneKind::DerbyTrack,
        "JailZone" => ZoneKind::Jail,
        "ResidenceTeleportZone" => ZoneKind::ResidenceTeleport,
        "TaxZone" => ZoneKind::Tax,
        "HqZone" => ZoneKind::Hq,
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
fn parse_file(
    path: &str,
    kind: ZoneKind,
    out: &mut Vec<Zone>,
    spawns_out: &mut std::collections::HashMap<i32, Vec<(i32, i32, i32)>>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);

    struct Pending {
        id: i32,
        name: String,
        shape: String,
        min_z: i32,
        max_z: i32,
        rad: Option<i32>,
        xs: Vec<i32>,
        ys: Vec<i32>,
        castle_id: i32,
        clan_hall_id: i32,
        kind: ZoneKind,
        skills: Vec<(i32, i32)>,
        chance: i32,
        initial_delay: i32,
        reuse: i32,
        enabled: bool,
        casts_on_players: bool,
        remove_effects_on_exit: bool,
        dmg_hp: i32,
        dmg_mp: i32,
        move_bonus: f64,
        spawns: Vec<(i32, i32, i32)>,
    }
    let mut cur: Option<Pending> = None;

    while let Ok(event) = reader.read_event() {
        let e = match event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::End(e) => {
                if e.name().as_ref() == b"zone"
                    && let Some(p) = cur.take()
                    && let Some(form) = build_form(&p.shape, p.xs, p.ys, p.rad)
                {
                    let effect = (p.kind == ZoneKind::Effect).then_some(EffectZoneParams {
                        skills: p.skills,
                        chance: p.chance,
                        initial_delay: p.initial_delay,
                        reuse: p.reuse,
                        enabled: p.enabled,
                        casts_on_players: p.casts_on_players,
                        remove_effects_on_exit: p.remove_effects_on_exit,
                    });
                    let damage = (p.kind == ZoneKind::Damage).then_some(DamageZoneParams {
                        hp_per_tick: p.dmg_hp,
                        mp_per_tick: p.dmg_mp,
                        initial_delay: p.initial_delay,
                        // Java's DamageZone default reuse is 5000, not
                        // the EffectZone's 30000.
                        reuse: if p.reuse == 30_000 { 5_000 } else { p.reuse },
                        enabled: p.enabled,
                        castle_id: p.castle_id,
                    });
                    let swamp = (p.kind == ZoneKind::Swamp).then_some(SwampZoneParams {
                        move_bonus: p.move_bonus,
                        enabled: p.enabled,
                        castle_id: p.castle_id,
                    });
                    if p.kind == ZoneKind::ResidenceTeleport && p.castle_id > 0 {
                        spawns_out.insert(p.castle_id, p.spawns.clone());
                    }
                    out.push(Zone {
                        id: p.id,
                        name: p.name,
                        kind: p.kind,
                        territory: Territory {
                            form,
                            min_z: p.min_z,
                            max_z: p.max_z,
                        },
                        castle_id: p.castle_id,
                        clan_hall_id: p.clan_hall_id,
                        effect,
                        damage,
                        swamp,
                    });
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"zone" => {
                cur = Some(Pending {
                    id: attr_i32(&e, b"id").unwrap_or(0),
                    name: attr_str(&e, b"name").unwrap_or_default(),
                    shape: attr_str(&e, b"shape").unwrap_or_else(|| "NPoly".to_string()),
                    min_z: attr_i32(&e, b"minZ").unwrap_or(i32::MIN),
                    max_z: attr_i32(&e, b"maxZ").unwrap_or(i32::MAX),
                    rad: attr_i32(&e, b"rad"),
                    xs: Vec::new(),
                    ys: Vec::new(),
                    castle_id: 0,
                    clan_hall_id: 0,
                    // A zone's own `type=` wins; the filename mapping is the
                    // fallback for the single-type files.
                    kind: attr_str(&e, b"type")
                        .as_deref()
                        .and_then(kind_from_type)
                        .unwrap_or(kind),
                    skills: Vec::new(),
                    chance: 100,
                    initial_delay: 0,
                    reuse: 30_000,
                    enabled: true,
                    casts_on_players: true,
                    remove_effects_on_exit: false,
                    dmg_hp: 200,
                    dmg_mp: 0,
                    move_bonus: 0.5,
                    spawns: Vec::new(),
                });
                // A zone whose `type=` names a kind we don't port yet is
                // skipped outright rather than mis-filed under the fallback.
                if let Some(ty) = attr_str(&e, b"type")
                    && kind_from_type(&ty).is_none()
                {
                    cur = None;
                }
            }
            // `<spawn X Y Z/>` — a `ResidenceTeleportZone`'s oust destination
            // (Java `ZoneRespawn._spawnLocs`).
            b"spawn" => {
                if let Some(p) = cur.as_mut() {
                    p.spawns.push((
                        attr_i32(&e, b"X")
                            .or_else(|| attr_i32(&e, b"x"))
                            .unwrap_or(0),
                        attr_i32(&e, b"Y")
                            .or_else(|| attr_i32(&e, b"y"))
                            .unwrap_or(0),
                        attr_i32(&e, b"Z")
                            .or_else(|| attr_i32(&e, b"z"))
                            .unwrap_or(0),
                    ));
                }
            }
            // Zone files capitalize the node attributes (`X`/`Y`), unlike
            // the spawn territories' lowercase — accept either.
            b"node" => {
                if let Some(p) = cur.as_mut() {
                    p.xs.push(
                        attr_i32(&e, b"X")
                            .or_else(|| attr_i32(&e, b"x"))
                            .unwrap_or(0),
                    );
                    p.ys.push(
                        attr_i32(&e, b"Y")
                            .or_else(|| attr_i32(&e, b"y"))
                            .unwrap_or(0),
                    );
                }
            }
            // `SiegeZone`s carry `<stat name="castleId" val="N"/>`; other zone
            // stats are still skipped (see module docs).
            b"stat" => {
                let (Some(p), Some(name)) = (cur.as_mut(), attr_str(&e, b"name")) else {
                    continue;
                };
                let val = attr_str(&e, b"val").unwrap_or_default();
                match name.as_str() {
                    "castleId" => p.castle_id = val.parse().unwrap_or(0),
                    // `ResidenceTeleportZone` names its castle `residenceId`.
                    "residenceId" if p.kind == ZoneKind::ResidenceTeleport => {
                        p.castle_id = val.parse().unwrap_or(0);
                    }
                    // `TaxZone` names its castle `domainId` (Java
                    // `TaxZone.setParameter` → `CastleManager.getCastleById`).
                    "domainId" if p.kind == ZoneKind::Tax => {
                        p.castle_id = val.parse().unwrap_or(0);
                    }
                    "clanHallId" => p.clan_hall_id = val.parse().unwrap_or(0),
                    "skillIdLvl" => p.skills = parse_skill_id_lvl(&val),
                    "chance" => p.chance = val.parse().unwrap_or(p.chance),
                    "initialDelay" => p.initial_delay = val.parse().unwrap_or(p.initial_delay),
                    "reuse" => p.reuse = val.parse().unwrap_or(p.reuse),
                    "default_enabled" => p.enabled = val == "true",
                    "targetClass" => p.casts_on_players = val != "Npc",
                    "removeEffectsOnExit" => p.remove_effects_on_exit = val == "true",
                    "damageHPPerSec" => p.dmg_hp = val.parse().unwrap_or(p.dmg_hp),
                    "damageMPPerSec" => p.dmg_mp = val.parse().unwrap_or(p.dmg_mp),
                    "move_bonus" => p.move_bonus = val.parse().unwrap_or(p.move_bonus),
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
        "Cylinder" if !xs.is_empty() && !ys.is_empty() => Some(ZoneForm::Cylinder {
            x: xs[0],
            y: ys[0],
            rad: rad.unwrap_or(0),
        }),
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
        // 134 peace + 423 water + 47 no_restart + 12 pvp + 9 siege + 218 effect
        // + 35 damage + 20 swamp.
        //
        // Was 605 before per-zone `type=` parsing landed. Effect zones account
        // for 218 of the increase; the other 20 are Peace/NoRestart/Pvp zones
        // that live in the *mixed* files (devil_isle, zone, underground_
        // coliseum) which the filename→kind loader couldn't read at all — so
        // they were silently missing from the world.
        // 898 → 1030: the two `ScriptZone` files (custom_script 109, dummy 24)
        // now load. They carry no behaviour — a script addresses them by id —
        // but they must be present for `by_id` to find anything. The count is
        // 1031 rather than 1032 because those files are filtered to script
        // zones: `custom_script.xml`'s stray `SiegeZone` (GainakSiege) is
        // deliberately left out (see the loader).
        // 1031 → 1044: `fishing.xml`'s 13 `FishingZone`s (G32).
        // 1044 → 1092: `clan_hall.xml`'s 48 `ClanHallZone`s (G24).
        // 1092 → 1100: `zone.xml`'s 8 `DerbyTrackZone`s (G26.5).
        // 1100 → 1103: `gm_room.xml`'s 3 `JailZone`s (G31).
        // 1103 → 1112: `castle_teleport.xml`'s 9 `ResidenceTeleportZone`s —
        // one owner-restart territory per castle, where the mass gatekeeper's
        // `MASS_TELEPORT` ousts from (G22 `ai/others`).
        // 1112 → 1234: `tax.xml`'s 122 `TaxZone`s — the trade territories whose
        // purchases pay into `domainId`'s castle treasury.
        // 1234 → 1253: `castle_hq.xml`'s 19 `HqZone`s — the patches of each
        // battlefield where an attacker may plant its headquarters.
        assert_eq!(data.zones.len(), 1253);
        let count = |k: ZoneKind| data.zones.iter().filter(|z| z.kind == k).count();
        assert_eq!(count(ZoneKind::Tax), 122, "tax.xml");
        // Every tax zone names a castle (1..=9) through `domainId`.
        assert!(
            data.zones
                .iter()
                .filter(|z| z.kind == ZoneKind::Tax)
                .all(|z| (1..=9).contains(&z.castle_id)),
            "every TaxZone carries a castle id"
        );
        // Gludio's town center is inside gludio's tax territory (domainId 1).
        assert_eq!(data.tax_castle_at(-14000, 123000, -3100), Some(1));
        // The middle of the ocean is in none.
        assert_eq!(data.tax_castle_at(-280000, 180000, -4000), None);
        assert_eq!(count(ZoneKind::DerbyTrack), 8, "zone.xml derby track");
        assert_eq!(count(ZoneKind::Jail), 3, "gm_room.xml jail zones");
        assert_eq!(
            count(ZoneKind::ResidenceTeleport),
            9,
            "castle_teleport.xml — one per castle"
        );
        // The jail-in location (Java `JailZone.JAIL_IN_LOC`) is inside a jail
        // zone; the jail-out location is not.
        assert!(data.in_jail_zone(-114356, -249645, -2984));
        assert!(!data.in_jail_zone(17836, 170178, -3507));
        assert_eq!(count(ZoneKind::ClanHall), 48, "clan_hall.xml");
        assert_eq!(count(ZoneKind::Script), 133, "the two ScriptZone files");
        assert_eq!(count(ZoneKind::Peace), 134);
        assert_eq!(count(ZoneKind::Water), 423);
        assert_eq!(count(ZoneKind::Fishing), 13, "fishing.xml");
        assert_eq!(count(ZoneKind::NoRestart), 47);
        assert_eq!(count(ZoneKind::Pvp), 12);
        assert_eq!(count(ZoneKind::Siege), 9);
        assert_eq!(count(ZoneKind::Effect), 218);
        assert_eq!(count(ZoneKind::Damage), 35);
        assert_eq!(count(ZoneKind::Swamp), 20);

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
            id: 0,
            name: "test_peace".into(),
            kind: ZoneKind::Peace,
            territory: Territory {
                form: ZoneForm::Cuboid {
                    x1: 0,
                    x2: 1000,
                    y1: 0,
                    y2: 1000,
                },
                min_z: -100,
                max_z: 100,
            },
            castle_id: 0,
            clan_hall_id: 0,
            effect: None,
            damage: None,
            swamp: None,
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
        let effects: Vec<&Zone> = data
            .zones
            .iter()
            .filter(|z| z.kind == ZoneKind::Effect)
            .collect();
        let with_skills = effects
            .iter()
            .filter(|z| !z.effect.as_ref().unwrap().skills.is_empty())
            .count();
        let npc_only = effects
            .iter()
            .filter(|z| !z.effect.as_ref().unwrap().casts_on_players)
            .count();
        println!(
            "EFFECT ZONES={} with_skills={with_skills} npc_targeted={npc_only}",
            effects.len()
        );
        assert_eq!(effects.len(), 218, "expected the dist's 218 EffectZones");
        assert_eq!(
            npc_only, 27,
            "expected 27 targetClass=Npc zones (which cast on nobody)"
        );

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
                    ZoneKind::Script
                        | ZoneKind::Peace
                        | ZoneKind::Water
                        | ZoneKind::NoRestart
                        | ZoneKind::Hq
                        | ZoneKind::Pvp
                        | ZoneKind::Siege
                        | ZoneKind::Effect
                        | ZoneKind::Damage
                        | ZoneKind::Swamp
                        | ZoneKind::Fishing
                        | ZoneKind::ClanHall
                        | ZoneKind::DerbyTrack
                        | ZoneKind::Jail
                        | ZoneKind::ResidenceTeleport
                        | ZoneKind::Tax
                ),
                "zone {} has an unported kind",
                z.name
            );
            if z.kind == ZoneKind::Effect {
                assert!(
                    z.effect.is_some(),
                    "effect zone {} must carry params",
                    z.name
                );
            }
        }
    }
}

impl ZoneData {
    /// Java `ZoneManager.getZoneById` — a script addressing a named region.
    ///
    /// The grand-boss scripts all open with one of these (`getZoneById(12012)`
    /// is Queen Ant's lair), so it is the entry point the whole `ai/bosses`
    /// family needs. Linear over the zone list: there are ~3k zones and this
    /// runs at script init, not per tick.
    pub fn by_id(&self, id: i32) -> Option<&Zone> {
        self.zones.iter().find(|z| z.id == id)
    }
}

impl Zone {
    /// Java `ZoneType.isInsideZone(x, y, z)`.
    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        z >= self.territory.min_z && z <= self.territory.max_z && self.territory.contains_2d(x, y)
    }
}

#[cfg(test)]
mod hq_zone_tests {
    use super::*;

    /// `castle_hq.xml` loads: the marked headquarters patches an attacker must
    /// stand in to plant a base camp. The file was never in the load list, so
    /// the gate that reads it had nothing to find.
    #[test]
    fn dist_castle_hq_zones_load_with_their_castle_ids() {
        const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
        let data = ZoneData::load_from(DIST);
        let hq: Vec<&Zone> = data
            .zones
            .iter()
            .filter(|z| z.kind == ZoneKind::Hq)
            .collect();
        assert_eq!(hq.len(), 19, "every HQ patch in castle_hq.xml");
        assert!(
            hq.iter().all(|z| z.castle_id > 0),
            "each names the castle it belongs to"
        );
        // Gludio (1) owns the first two patches; one of its nodes is inside.
        assert_eq!(data.hq_castle_at(-18000, 115000, 0), Some(1));
        // A point far outside every patch is in none.
        assert_eq!(data.hq_castle_at(0, 0, 0), None);
    }
}
