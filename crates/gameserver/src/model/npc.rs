//! The `Npc` world object (Java `model/actor/Npc` + `model/Spawn`), scoped to
//! G8's static-world slice: NPCs spawn at boot from `SpawnData`, stand in the
//! region grid, and can be seen/targeted. No AI, no combat, no respawn yet —
//! nothing can kill them until G9 brings `doDie` (the respawn delays are
//! carried so G9's respawn scheduler has them).

use rand::Rng;

use crate::data::npc_data::NpcTemplate;
use crate::data::spawn_data::{NpcSpawnDef, SpawnGroup, SpawnTemplate, Territory};
use crate::world::{region_of, World};

/// First object id handed to spawned NPCs. Java draws NPC ids from the same
/// `IdManager` pool as characters/items; NPCs are transient here, so they get
/// a dedicated base far above anything the DB thread's persistent-id counter
/// (`db::FIRST_OID` + row count) can realistically reach instead of a
/// cross-thread id round-trip.
pub const FIRST_NPC_OBJECT_ID: i32 = 0x4000_0000;

/// `Rnd.get(61794)` — Java's random-heading bound in `Spawn.initializeNpc`
/// (not 65536; kept as-is for parity).
const RANDOM_HEADING_BOUND: i32 = 61794;

/// `AttackableAI`'s intention, narrowed to the two states the G9 slice
/// drives (IDLE folds into `Active` — there is no think-task registry to
/// drop out of; inactive-region NPCs are simply skipped by the AI tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpcIntention {
    #[default]
    Active,
    Attack,
}

/// One `Attackable._aggroList` entry (Java `AggroInfo`, minus the
/// hate-suspension bookkeeping).
#[derive(Debug, Clone, Copy, Default)]
pub struct AggroInfo {
    pub hate: f64,
    pub damage: f64,
}

/// A spawned NPC. Stats-wise this carries only what displaying, targeting and
/// the combat formulas need — everything else reads through the template
/// (`world.data.npc_data`).
/// An ECS component (one fat component per NPC entity for now — see
/// `store::EntityStore`).
#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Npc {
    pub object_id: i32,
    /// Template id (`world.data.npc_data.get(npc_id)`).
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Region cell, kept in sync by the movement tick (AI chase movement can
    /// cross cells; `World.npc_regions` is re-indexed on change).
    pub region: (i32, i32),
    /// Unbuffed NPC max HP/MP = template base values (`NpcStat` finalizers).
    pub max_hp: i32,
    pub max_mp: i32,
    pub cur_hp: f64,
    pub cur_mp: f64,
    /// `Creature._isRunning` — NPCs spawn walking; AI flips to run on aggro.
    pub running: bool,
    /// Respawn bookkeeping for the death/respawn scheduler.
    pub respawn_secs: i32,
    pub respawn_random_secs: i32,

    // --- Combat/AI state (G9) ---
    /// Java `Creature._isDead` — corpse until the decay task removes it.
    pub dead: bool,
    /// `AttackableAI`'s intention (active scan vs. attack loop).
    pub intention: NpcIntention,
    /// `Attackable._aggroList`, keyed by player object id.
    pub aggro: std::collections::HashMap<i32, AggroInfo>,
    /// `AttackableAI._globalAggro`: starts at -10 (calm for ~10 think ticks
    /// after spawn), climbs to 0, must be ≥ 0 for aggro-range scans to act.
    pub global_aggro: i32,
    /// `AttackableAI._attackTimeout` (absolute world tick): give up chasing
    /// when it passes without landing/receiving a hit.
    pub attack_timeout_tick: u64,
    /// Busy-swinging until this tick (Java `_attackEndTime`).
    pub attack_end_tick: u64,
    /// `Some` while moving (chase/return-home), same tick interpolation as
    /// players.
    pub move_data: Option<crate::model::movement::MoveData>,
    /// Where this NPC spawned (Java `Npc.getSpawn()` location) — the drift
    /// anchor AI walks back to.
    pub spawn_loc: (i32, i32, i32),
    /// Indices into `GameData.spawn_data` (spawn/group/npc) so death can
    /// schedule a respawn of the same spawn line.
    pub spawn_ref: (usize, usize, usize),
}

impl Npc {
    pub fn template<'a>(&self, world: &'a World) -> Option<&'a NpcTemplate> {
        world.data.npc_data.get(self.npc_id)
    }

    /// A synthetic instance for unit tests (spawn-fresh AI/combat state).
    #[doc(hidden)]
    pub fn for_test(object_id: i32, npc_id: i32, x: i32, y: i32, z: i32, max_hp: i32, max_mp: i32) -> Self {
        Self {
            object_id,
            npc_id,
            x,
            y,
            z,
            heading: 0,
            region: region_of(x, y),
            max_hp,
            max_mp,
            cur_hp: max_hp as f64,
            cur_mp: max_mp as f64,
            running: false,
            respawn_secs: 0,
            respawn_random_secs: 0,
            dead: false,
            intention: NpcIntention::Active,
            aggro: std::collections::HashMap::new(),
            global_aggro: -10,
            attack_timeout_tick: 0,
            attack_end_tick: 0,
            move_data: None,
            spawn_loc: (x, y, z),
            spawn_ref: (0, 0, 0),
        }
    }

    /// `Attackable.getMostHated()`: highest-hate living entry.
    pub fn most_hated(&self) -> Option<i32> {
        if self.dead {
            return None;
        }
        self.aggro
            .iter()
            .filter(|(_, info)| info.hate > 0.0)
            .max_by(|a, b| a.1.hate.partial_cmp(&b.1.hate).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id)
    }
}

/// Instance-class (`type` attribute) names that exist under Java's
/// `model/actor/instance` as `Npc` subclasses — `Spawn`'s reflective
/// `Class.forName` fails for anything else (e.g. `FestivalMonster`,
/// `TerritoryWard` templates exist but have no class), and those spawn lines
/// error out on the Java server too.
const SPAWNABLE_TYPES: &[&str] = &[
    "Artefact",
    "BroadcastingTower",
    "Chest",
    "CommissionManager",
    "ControllableMob",
    "Doorman",
    "Doppelganger",
    "EffectPoint",
    "EventMonster",
    "FeedableBeast",
    "Fisherman",
    "FlyTerrainObject",
    "Folk",
    "FortCommander",
    "FortDoorman",
    "FortLogistics",
    "FortManager",
    "FriendlyMob",
    "FriendlyNpc",
    "GrandBoss",
    "Guard",
    "Merchant",
    "Monster",
    "PetManager",
    "RaceManager",
    "RaidBoss",
    "SchemeBuffer",
    "SiegeFlag",
    "TamedBeast",
    "Teleporter",
    "TerrainObject",
    "VillageMaster",
    "VillageMasterDElf",
    "VillageMasterDwarf",
    "VillageMasterFighter",
    "VillageMasterMystic",
    "VillageMasterOrc",
    "VillageMasterPriest",
    "Warehouse",
];

/// Java `SpawnData.init()` → `SpawnTemplate.spawnAll` at boot: place every
/// spawn line from `data/spawns/**`. Returns the number of NPCs placed.
pub fn spawn_all(world: &mut World) -> usize {
    let mut placed = 0usize;
    let mut skipped = 0usize;
    // The data bundle can't be borrowed while `world.npcs` is mutated, and the
    // spawn definitions are read-only — walk indices instead of iterators.
    for spawn_idx in 0..world.data.spawn_data.spawns.len() {
        for group_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups.len() {
            for npc_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups[group_idx].npcs.len() {
                let (count, ok) = {
                    let template = &world.data.spawn_data.spawns[spawn_idx];
                    let def = &template.groups[group_idx].npcs[npc_idx];
                    (def.count, spawnable(world, def))
                };
                if !ok {
                    skipped += 1;
                    continue;
                }
                for _ in 0..count {
                    if spawn_one(world, spawn_idx, group_idx, npc_idx).is_some() {
                        placed += 1;
                    }
                }
            }
        }
    }
    tracing::info!("SpawnData: spawned {placed} NPCs ({skipped} spawn lines skipped).");
    placed
}

/// The parse/spawn-time skips Java spreads over `SpawnData.parseNpc`
/// (Servitor/Pet), `NpcSpawnTemplate.spawn` (missing template, Defender) and
/// `Spawn.doSpawn` (Pet/Decoy/Trap), plus the missing-instance-class failure.
fn spawnable(world: &World, def: &NpcSpawnDef) -> bool {
    let Some(t) = world.data.npc_data.get(def.npc_id) else { return false };
    if matches!(t.type_name.as_str(), "Servitor" | "Pet" | "Defender" | "Decoy" | "Trap") {
        return false;
    }
    SPAWNABLE_TYPES.contains(&t.type_name.as_str())
}

/// `Spawn.doSpawn` → `initializeNpc` for one placement. Returns the placed
/// NPC's object id (respawns need it to broadcast `NpcInfo`).
///
/// Deviation from Java: `Spawn.respawnNpc` reuses the dead `Npc` object (and
/// its object id); here a respawn runs this same function and gets a fresh
/// transient id — clients saw the corpse `DeleteObject`d at decay, so the new
/// id is indistinguishable from the old one on the wire.
pub(crate) fn spawn_one(world: &mut World, spawn_idx: usize, group_idx: usize, npc_idx: usize) -> Option<i32> {
    let (npc_id, loc, respawn_secs, respawn_random_secs) = {
        let template = world.data.spawn_data.spawns.get(spawn_idx)?;
        let def = template.groups.get(group_idx)?.npcs.get(npc_idx)?;
        let loc = resolve_location(&mut world.rng, &world.geo, template, &template.groups[group_idx], def);
        (def.npc_id, loc, def.respawn_secs, def.respawn_random_secs)
    };
    let Some((x, y, mut z, heading)) = loc else { return None };

    let t = world.data.npc_data.get(npc_id).expect("checked in spawnable");
    // `initializeNpc`: monsters snap to the geodata surface unless it's more
    // than 300 units away (broken geodata / intended overhang).
    if t.is_monster() {
        let geo_z = world.geo.get_height(x, y, z);
        if (geo_z - z).abs() < 300 {
            z = geo_z;
        }
    }
    let heading = if heading < 0 { world.rng.gen_range(0..RANDOM_HEADING_BOUND) } else { heading };

    let npc = Npc {
        object_id: world.next_npc_object_id,
        npc_id,
        x,
        y,
        z,
        heading,
        region: region_of(x, y),
        max_hp: t.base_hp_max as i32,
        max_mp: t.base_mp_max as i32,
        cur_hp: t.base_hp_max,
        cur_mp: t.base_mp_max,
        running: false,
        respawn_secs,
        respawn_random_secs,
        dead: false,
        intention: NpcIntention::Active,
        aggro: std::collections::HashMap::new(),
        // Java seeds _globalAggro = -10: no aggro for ~10 think seconds.
        global_aggro: -10,
        attack_timeout_tick: 0,
        attack_end_tick: 0,
        move_data: None,
        spawn_loc: (x, y, z),
        spawn_ref: (spawn_idx, group_idx, npc_idx),
    };
    let object_id = npc.object_id;
    world.next_npc_object_id += 1;
    world.npc_regions.entry(npc.region).or_default().push(npc.object_id);
    world.npcs.insert(npc.object_id, npc);
    Some(object_id)
}

/// `NpcSpawnTemplate.getSpawnLocation`, minus the unused zone/banned-territory
/// paths: fixed location if declared, else a random point in a group
/// territory, else in a spawn-template territory. Returns `(x, y, z, heading)`
/// with `heading == -1` meaning "randomize".
fn resolve_location(
    rng: &mut rand::rngs::StdRng,
    geo: &crate::geo::GeoEngine,
    template: &SpawnTemplate,
    group: &SpawnGroup,
    def: &NpcSpawnDef,
) -> Option<(i32, i32, i32, i32)> {
    if let Some(loc) = def.loc {
        return Some((loc.x, loc.y, loc.z, loc.heading));
    }
    let territories = if !group.territories.is_empty() {
        &group.territories
    } else if !template.territories.is_empty() {
        &template.territories
    } else {
        return None;
    };
    let territory = &territories[rng.gen_range(0..territories.len())];
    let (x, y) = random_point_2d(rng, territory)?;
    let z = geo.get_height(x, y, territory.mid_z());
    Some((x, y, z, -1))
}

/// `ZoneForm.getRandomPoint`: uniform in the bounding box, rejection-sampled
/// into the shape (Java caps the NPoly retry at 1000 and returns the last
/// candidate regardless; same here).
fn random_point_2d(rng: &mut rand::rngs::StdRng, territory: &Territory) -> Option<(i32, i32)> {
    let (min_x, max_x, min_y, max_y) = territory.bounds();
    if min_x > max_x || min_y > max_y {
        return None;
    }
    let mut x = rng.gen_range(min_x..=max_x);
    let mut y = rng.gen_range(min_y..=max_y);
    let mut tries = 0;
    while !territory.contains_2d(x, y) && tries < 1000 {
        x = rng.gen_range(min_x..=max_x);
        y = rng.gen_range(min_y..=max_y);
        tries += 1;
    }
    Some((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    /// Boot-shaped smoke test over the real datapack: every spawnable line
    /// places an NPC, fixed spawns land at retail coordinates, and the
    /// region index stays consistent with the NPC registry.
    #[test]
    fn spawns_real_dist_content() {
        let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
        let data = GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
        let mut world = World::new(link_tx, 7, 3, 0, data, db_tx);

        let placed = spawn_all(&mut world);
        assert_eq!(placed, world.npcs.len());
        assert!(placed > 20_000, "expected >20k placed NPCs, got {placed}");

        // Giran.xml: <npc id="30878" x="47984" y="186832" z="-3445" heading="42000"/>.
        let giran_guide = world
            .npcs
            .values()
            .find(|n| n.npc_id == 30878 && n.x == 47984)
            .expect("Giran npc 30878 at retail coords");
        assert_eq!((giran_guide.y, giran_guide.z, giran_guide.heading), (186832, -3445, 42000));
        assert_eq!(giran_guide.region, region_of(47984, 186832));

        // Region index covers every NPC exactly once.
        let indexed: usize = world.npc_regions.values().map(Vec::len).sum();
        assert_eq!(indexed, placed);
        for (region, ids) in &world.npc_regions {
            for id in ids {
                assert_eq!(world.npcs[id].region, *region);
            }
        }

        // Monsters got distinct object ids starting at the NPC base.
        assert!(world.npcs.keys().all(|&id| id >= FIRST_NPC_OBJECT_ID));
    }
}
