use crate::data::npc_data::NpcTemplate;
use crate::data::spawn_data::{NpcSpawnDef, SpawnGroup, SpawnTemplate, Territory};
use crate::model::npc::{AggroList, Npc, NpcAi};
use crate::world::{World, region_of};
use commons::util::rnd;

pub(crate) mod ai;
pub(crate) mod area;
pub(crate) mod bosses;
pub(crate) mod cast;
pub(crate) mod doors;
pub(crate) mod minions;
pub(crate) mod say;
pub(crate) mod spawn_scripts;
pub(crate) mod support_magic;
pub(crate) mod teleporter;
pub mod view;
pub(crate) mod walkers;

/// First object id handed to spawned NPCs. Java draws NPC ids from the same
/// `IdManager` pool as characters/items; NPCs are transient here, so they get
/// a dedicated base far above anything the DB thread's persistent-id counter
/// (`db::FIRST_OID` + row count) can realistically reach instead of a
/// cross-thread id round-trip.
pub const FIRST_NPC_OBJECT_ID: i32 = 0x4000_0000;
/// `Rnd.get(61794)` — Java's random-heading bound in `Spawn.initializeNpc`
/// (not 65536; kept as-is for parity).
const RANDOM_HEADING_BOUND: i32 = 61794;

/// `NpcStat`'s finalizer outputs for a template — the finalizer *bases* that
/// [`crate::model::npc_finalized_stats`] then folds passive template skills and
/// buffs into (via `Stat.defaultValue`'s mul/add). Ports each Java finalizer:
/// - `p_atk`: `PAttackFinalizer` — base × STR bonus × level mod (linear).
/// - `m_atk`: `MAttackFinalizer` — base × (INT bonus × level mod)^2.2072 (a
///   power, *not* linear — a mob's m.atk is well above its raw `<attack
///   magical>` base).
/// - `p_def`/`m_def`: `P/MDefenseFinalizer` — base × level mod (m_def also
///   × MEN bonus).
/// - `p_atk_spd`/`m_atk_spd`: `P/MAttackSpeedFinalizer` — base × DEX/WIT bonus.
/// - `crit_hit`: `PCriticalRateFinalizer` — base × DEX bonus × 10.
/// - accuracy: `PAccuracyFinalizer` — `sqrt(DEX)·5 + level` + the full tier
///   ladder. evasion: `PEvasionRateFinalizer`'s NPC branch — same base but a
///   single `>69` tier of `(level-69)+2` (the NPC XML `accuracy`/`evasion`
///   attributes themselves are unimplemented in Java too).
pub fn npc_combat_stats(
    t: &NpcTemplate,
    sb: &crate::data::stat_bonus::StatBonus,
) -> crate::model::components::CombatStats {
    use crate::model::stats::BaseStat;
    let level_mod = (t.level as f64 + 89.0) / 100.0;
    let level = t.level as f64;
    // `PAccuracyFinalizer`: `sqrt(DEX)·5 + level`, plus the full high-level tier
    // ladder (same for players and NPCs).
    let mut accuracy = (t.base_dex as f64).sqrt() * 5.0 + level;
    if t.level > 69 {
        accuracy += (t.level - 69) as f64;
    }
    if t.level > 77 {
        accuracy += 1.0;
    }
    if t.level > 80 {
        accuracy += 2.0;
    }
    if t.level > 87 {
        accuracy += 2.0;
    }
    if t.level > 92 {
        accuracy += 1.0;
    }
    if t.level > 97 {
        accuracy += 1.0;
    }
    let accuracy = accuracy.round() as i32;
    // `PEvasionRateFinalizer` NPC (`else`) branch: `sqrt(DEX)·5 + level` with a
    // *single* `>69` tier of `(level-69)+2` — NOT the accuracy ladder. The
    // EvasionRate skill effects (e.g. 4414 Heavy Armor Type -10, 4789 +3) then
    // fold in via `npc_finalized_stats`.
    let mut evasion = (t.base_dex as f64).sqrt() * 5.0 + level;
    if t.level > 69 {
        evasion += (t.level - 69) as f64 + 2.0;
    }
    let evasion = evasion.round() as i32;
    // `P/MAttackSpeedFinalizer`: base × {DEX,WIT} bonus (`getX() > 0 ? bonus : 1`).
    let dex_bonus = if t.base_dex > 0 {
        sb.bonus(BaseStat::Dex, t.base_dex)
    } else {
        1.0
    };
    let wit_bonus = if t.base_wit > 0 {
        sb.bonus(BaseStat::Wit, t.base_wit)
    } else {
        1.0
    };
    crate::model::components::CombatStats {
        p_atk: t.base_p_atk * sb.bonus(BaseStat::Str, t.base_str) * level_mod,
        // `MAttackFinalizer`: `base × (INT bonus × levelMod)^2.2072` — a power,
        // not the linear `× INT × levelMod` p.atk uses. (This is why a mob's
        // m.atk is far above its raw `<attack magical>` base.)
        m_atk: t.base_m_atk * (sb.bonus(BaseStat::Int, t.base_int) * level_mod).powf(2.2072),
        p_def: t.base_p_def * level_mod,
        m_def: t.base_m_def * sb.bonus(BaseStat::Men, t.base_men) * level_mod,
        p_atk_spd: (t.base_p_atk_spd as f64 * dex_bonus).round() as i32,
        m_atk_spd: (t.base_m_atk_spd as f64 * wit_bonus).round() as i32,
        crit_hit: t.base_crit_rate * sb.bonus(BaseStat::Dex, t.base_dex) * 10.0,
        m_crit_hit: 0.0,
        evasion,
        accuracy,
        magic_evasion: 0,
        magic_accuracy: 0,
        // `ShotsBonusFinalizer` bails on a null `getActingPlayer()`, which is
        // every plain NPC — a flat 1, i.e. no increment.
        shots_bonus_add: 0.0,
        atk_range: t.base_atk_range,
        random_dmg: t.base_rnd_dam,
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
    // `Config.ALT_DEV_NO_SPAWNS` — a developer switch that empties the world of
    // NPCs entirely. Java guards `SpawnData.load` and `DBSpawnManager.load`
    // separately; here the second falls out of the first, because the `db_save`
    // lines `boss_respawn::resolve_boot` settles are collected by *this* pass
    // into `pending_boss_spawns`, which stays empty when it returns here.
    if world.cfg.general.alt_dev_no_spawns {
        tracing::info!("SpawnData: AltDevNoSpawns is set — no NPCs spawned.");
        return 0;
    }
    let mut placed = 0usize;
    let mut skipped = 0usize;
    // The data bundle can't be borrowed while `world.objects` is mutated, and the
    // spawn definitions are read-only — walk indices instead of iterators.
    for spawn_idx in 0..world.data.spawn_data.spawns.len() {
        for group_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups.len() {
            // Java `SpawnTemplate.spawnAll` = `spawn(SpawnGroup::
            // isSpawningByDefault)`: a `spawnByDefault="false"` group waits for
            // the script that owns it. 95 groups on this dist — the day/night
            // halves, placed by [`super::spawn_scripts`]. Boot
            // used to place them all, so every day/night map stood with *both*
            // populations at once.
            if !world.data.spawn_data.spawns[spawn_idx].groups[group_idx].spawn_by_default {
                continue;
            }
            for npc_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups[group_idx]
                .npcs
                .len()
            {
                let (count, ok) = {
                    let template = &world.data.spawn_data.spawns[spawn_idx];
                    let def = &template.groups[group_idx].npcs[npc_idx];
                    (def.count, spawnable(world, def))
                };
                if !ok {
                    skipped += 1;
                    continue;
                }
                // `dbSave` spawns belong to `DBSpawnManager`, not the static
                // pass — Java's `spawnNpc` hands them over and only places them
                // if the DB didn't already define them. Defer to
                // `boss_respawn::resolve_boot`, which settles them against the
                // `npc_respawns` rows once those arrive.
                if world.data.spawn_data.spawns[spawn_idx].groups[group_idx].npcs[npc_idx].db_save {
                    world
                        .pending_boss_spawns
                        .push((spawn_idx, group_idx, npc_idx));
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
    // Minions are placed underneath `spawn_one`, so fold them into the tally —
    // otherwise the reported count disagrees with the world's NPC population.
    let escorts = std::mem::take(&mut world.minions_placed);
    let placed = placed + escorts;
    tracing::info!(
        "SpawnData: spawned {placed} NPCs ({escorts} minions, {skipped} spawn lines skipped)."
    );
    placed
}

/// The parse/spawn-time skips Java spreads over `SpawnData.parseNpc`
/// (Servitor/Pet), `NpcSpawnTemplate.spawn` (missing template, Defender) and
/// `Spawn.doSpawn` (Pet/Decoy/Trap), plus the missing-instance-class failure.
fn spawnable(world: &World, def: &NpcSpawnDef) -> bool {
    let Some(t) = world.data.npc_data.get(def.npc_id) else {
        return false;
    };
    if matches!(
        t.type_name.as_str(),
        "Servitor" | "Pet" | "Defender" | "Decoy" | "Trap"
    ) {
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
pub(crate) fn spawn_one(
    world: &mut World,
    spawn_idx: usize,
    group_idx: usize,
    npc_idx: usize,
) -> Option<i32> {
    let (npc_id, loc, respawn_secs, respawn_random_secs, chase_range) = {
        let template = world.data.spawn_data.spawns.get(spawn_idx)?;
        let def = template.groups.get(group_idx)?.npcs.get(npc_idx)?;
        let loc = resolve_location(
            &mut world.rng.borrow_mut(),
            &world.geo,
            template,
            &template.groups[group_idx],
            def,
        );
        (
            def.npc_id,
            loc,
            def.respawn_secs,
            def.respawn_random_secs,
            def.chase_range,
        )
    };
    let (x, y, z, heading) = loc?;
    // `Spawn.initializeNpc`'s `ENABLE_RANDOM_MONSTER_SPAWNS` jitter.
    let (x, y) = randomize_spawn_point(world, npc_id, x, y, z, heading);
    let oid = spawn_npc_entity(
        world,
        npc_id,
        x,
        y,
        z,
        heading,
        respawn_secs,
        respawn_random_secs,
        (spawn_idx, group_idx, npc_idx),
        chase_range,
    )?;
    // `NpcSpawnTemplate.spawnNpc`: a leader brings its escort. Hooked here
    // rather than in `spawn_npc_entity` so a minion that itself declares
    // minions can't recurse — minions are placed through `spawn_npc_at`,
    // which deliberately doesn't run this.
    // `SpawnTemplate.notifySpawnNpc` — the template's own `ai=` script
    // (`NoRandomActivity` pins its NPCs down).
    self::spawn_scripts::apply_spawn_ai(world, oid, spawn_idx);
    // `WalkingManager.onSpawn` — attach a walking route if this id has one.
    crate::game_loop::npc::walkers::on_npc_spawn(world, oid, npc_id);
    // The escort lands in `world.minions_placed` inside `spawn_minion_group`
    // (the script-chosen named groups count themselves the same way).
    minions::spawn_minions(world, oid);
    announce_boss_spawn(world, oid);
    Some(oid)
}

/// Runtime spawn of a single NPC at an explicit location with no respawn and no
/// spawn-definition backing — the admin `//spawn` path (Java `AdminSpawn`'s
/// non-permanent spawn). The sentinel `spawn_ref` is never read because
/// `respawn_secs == 0` (see `handle_npc_decay`). Returns the object id, or
/// `None` if `npc_id` is unknown.
pub(crate) fn spawn_npc_at(
    world: &mut World,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> Option<i32> {
    let oid = spawn_npc_entity(world, npc_id, x, y, z, heading, 0, 0, (0, 0, 0), 0);
    if let Some(oid) = oid {
        announce_boss_spawn(world, oid);
    }
    oid
}

/// [`spawn_npc_at`] without the boss announcement — for a **minion**, which
/// Java excludes from the spawn lines (`!isMinion() && !isRaidMinion()`).
/// It needs its own entry point because `MinionOf` can only be attached once
/// the entity exists, so an announcement inside the spawn itself would fire
/// before anything could suppress it. Same reason `clear_champion_for_raid_
/// minion` runs at the call site.
pub(crate) fn spawn_minion_npc_at(
    world: &mut World,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> Option<i32> {
    spawn_npc_entity(world, npc_id, x, y, z, heading, 0, 0, (0, 0, 0), 0)
}

/// `Attackable.onRespawn`'s champion lottery — the whole of Java's guard chain
/// plus the `Rnd.get(100) < CHAMPION_FREQUENCY` roll.
///
/// Excluded, in Java's own order: the master gate, non-monsters, quest
/// monsters (title contains "Quest"), undying templates, raid bosses, raid
/// minions, a zero frequency, the level window, and instanced spawns unless
/// `ChampionEnableInInstances`.
///
/// **Raid minions are handled by the caller**, not here: Java sets
/// `_isRaidMinion` on the minion *before* it spawns, and the port has no such
/// template flag — `minions::clear_champion_for_raid_minion` unsets the roll
/// right after the spawn instead, which is observationally the same because
/// nothing has looked at the NPC in between.
///
/// Rolls on the global `rnd` stream (like the enchant glow beside it), not
/// `World::roll`, so it cannot shift the forced-roll sequences combat tests
/// depend on.
pub(crate) fn roll_champion(cfg: &crate::config::ChampionConfig, t: &NpcTemplate) -> bool {
    if !cfg.enable
        || !t.is_monster()
        || t.is_quest_monster()
        || t.undying
        || t.is_raid()
        || cfg.frequency <= 0
        || t.level < cfg.min_level
        || t.level > cfg.max_level
    {
        return false;
    }
    // `Config.CHAMPION_ENABLE_IN_INSTANCES || getInstanceId() == 0`. Every
    // spawn path that exists today is world-instance 0, so the left arm never
    // decides anything yet — kept so instanced spawns inherit the right gate.
    rnd::get(100) < cfg.frequency
}

/// Assemble one NPC entity (components, region index, `onSpawn` hook). Shared
/// by the data-driven [`spawn_one`] and the runtime [`spawn_npc_at`]. Does not
/// broadcast `NpcInfo` — the caller introduces it (boot relies on the
/// enter-world exchange; respawn/admin call [`crate::game_loop::death::introduce_npc`]).
fn spawn_npc_entity(
    world: &mut World,
    npc_id: i32,
    x: i32,
    y: i32,
    mut z: i32,
    heading: i32,
    respawn_secs: i32,
    respawn_random_secs: i32,
    spawn_ref: (usize, usize, usize),
    chase_range: i32,
) -> Option<i32> {
    let t = world.data.npc_data.get(npc_id)?;
    // `initializeNpc`: monsters snap to the geodata surface unless it's more
    // than 300 units away (broken geodata / intended overhang).
    if t.is_monster() {
        let geo_z = world.geo.get_height(x, y, z);
        if (geo_z - z).abs() < 300 {
            z = geo_z;
        }
    }
    let heading = if heading < 0 {
        rnd::get(RANDOM_HEADING_BOUND)
    } else {
        heading
    };

    // `Attackable.onRespawn`'s champion lottery. Rolled here, before the stats
    // are finalized, because it multiplies them.
    let champion = roll_champion(&world.cfg.champion, t);

    // Finalize combat/speed/vitals from template base + passive template skills
    // (Java's `Creature` ctor copies `template.getSkills()` — HP Increase, Strong
    // P.Atk, …). Spawns at full HP/MP. No player buffs yet, so pass empty.
    let (combat, speeds, max_hp, max_mp) = crate::model::npc_finalized_stats(
        &world.data,
        t,
        &crate::model::components::Buffs::default(),
        crate::model::NpcStatMods::of(&world.cfg, champion, t.is_raid()),
    );

    // `Npc.onSpawn`: an NPC standing in a castle's TAX zone wears the owner
    // clan's crest when `ShowCrestWithoutQuest` or the castle's own
    // `showNpcCrest` flag turns the display on (both off on this dist —
    // operator-only; see `siege::capture`'s note).
    let crest_clan_id = world
        .data
        .zone_data
        .tax_castle_at(x, y, z)
        .and_then(|castle_id| {
            let castle = world.castle(castle_id)?;
            if !(world.cfg.npc.show_crest_without_quest || castle.show_npc_crest) {
                return None;
            }
            world
                .clans
                .iter()
                .find(|(_, c)| c.castle_id == castle_id)
                .map(|(&id, _)| id)
        })
        .unwrap_or(0);

    let npc = Npc {
        object_id: world.next_npc_object_id,
        npc_id,
        respawn_secs,
        respawn_random_secs,
        spawn_loc: (x, y, z),
        spawn_ref,
        chase_range,
        script_value: 0,
        vars: std::collections::HashMap::new(),
        title_override: None,
        special_drop: false,
        must_reward_exp_sp: true,
        spoiler_object_id: 0,
        decay_at_tick: 0,
        crest_clan_id,
        sweep_items: None,
        seed_id: 0,
        seeder_object_id: 0,
        seeded: false,
        harvest_item: None,
        team: 0,
        display_effect: 0,
        // Java `Npc` ctor: the visual weapon glow, rolled once per instance.
        enchant_effect: if world.cfg.npc.enable_random_enchant_effect {
            rnd::get_range(4, 21)
        } else {
            t.weapon_enchant
        },
        champion,
        attackable_ai: t.is_monster() || t.is_guard(),
        is_guard: t.is_guard(),
        is_defender: t.type_name == "Defender",
        random_animation: t.random_animation,
        attackable: t.attackable,
    };
    let object_id = npc.object_id;
    let region = region_of(x, y);
    world.next_npc_object_id += 1;
    world.npc_regions.entry(region).or_default().push(object_id);
    world.objects.spawn(
        object_id,
        (
            npc,
            crate::model::components::Position { x, y, z, heading },
            crate::model::components::RegionCell(region),
            crate::model::components::Vitals {
                max_hp: max_hp as i32,
                cur_hp: max_hp,
                max_mp: max_mp as i32,
                cur_mp: max_mp,
                dead: false,
            },
            // Speeds finalized off the template (NPCs spawn walking; AI flips
            // `running` on aggro).
            speeds,
            crate::model::components::Collision {
                radius: t.collision_radius,
                height: t.collision_height,
            },
            combat,
            crate::model::components::AttackState::default(),
            NpcAi::default(),
            AggroList::default(),
            // NPCs can now carry buffs (e.g. a player casting Might on a mob);
            // their stats recompute from template + these via
            // `recompute_npc_stats_from_buffs`.
            crate::model::components::Buffs::default(),
        ),
    );
    // `onSpawn` hook (Java `Quest.notifySpawn` via `addSpawnId`) — fires for
    // the boot pass and every respawn alike.
    crate::game_loop::quests::notify_spawn(world, object_id, npc_id);
    world.npcs_by_id.entry(npc_id).or_default().push(object_id);
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
    let territory = &territories[rnd::get(territories.len() as i32) as usize];
    let (x, y) = random_point_2d(rng, territory)?;
    let z = geo.get_height(x, y, territory.mid_z());
    Some((x, y, z, -1))
}

/// `ZoneForm.getRandomPoint`: uniform in the bounding box, rejection-sampled
/// into the shape (Java caps the NPoly retry at 1000 and returns the last
/// candidate regardless; same here).
fn random_point_2d(rng: &mut rand::rngs::StdRng, territory: &Territory) -> Option<(i32, i32)> {
    use rand::Rng;
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

/// `Spawn.initializeNpc`'s `Custom/RandomSpawns.ini` offset: a monster's spawn
/// point is nudged by up to ±`MaxSpawnMobRange` on each axis, so a camp is not
/// pinned to identical coordinates every respawn.
///
/// Java's guard chain, in its own order: a heading of `-1` (already jittered
/// once — the flag it sets to avoid re-rolling), non-monsters, quest monsters,
/// NPCs a walking route targets, instanced spawns, undying templates, raids,
/// raid minions, fliers, a spawn point in water, and the `MobsSpawnNotRandom`
/// id list. The new point must also be **walkable and visible** from the old
/// one, or the offset is discarded — otherwise a mob could land inside a wall.
///
/// The port has no `setHeading(-1)` latch (headings live on the spawned entity,
/// not the definition), so the roll happens once per spawn rather than once per
/// definition; observationally the same, since Java re-rolls on every respawn
/// too — its latch only stops a *second* roll within one spawn.
///
/// **Only the datapack spawn path calls this.** A script or admin spawn goes
/// through [`spawn_npc_at`], which does not jitter: Java's `AbstractScript.
/// addSpawn` explicitly *undoes* the offset for a scripted monster ("retain
/// monster original position if ENABLE_RANDOM_MONSTER_SPAWNS is enabled"),
/// because a script that places a mob at a computed spot means that spot.
pub(crate) fn randomize_spawn_point(
    world: &mut World,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
) -> (i32, i32) {
    let cfg = &world.cfg.random_spawns;
    if !cfg.enabled || cfg.max_range <= 0 || heading == -1 || cfg.never_random.contains(&npc_id) {
        return (x, y);
    }
    let Some(t) = world.data.npc_data.get(npc_id) else {
        return (x, y);
    };
    if !t.is_monster()
        || t.is_quest_monster()
        || t.undying
        || t.is_raid()
        || world.data.routes.route_for_npc(npc_id).is_some()
    {
        return (x, y);
    }
    // Java skips a spawn point standing in water (the offset could beach it).
    if world
        .data
        .zone_data
        .zones_at(x, y, z)
        .any(|zn| zn.kind == crate::data::zone_data::ZoneKind::Water)
    {
        return (x, y);
    }
    let range = cfg.max_range;
    let (rx, ry) = (
        x + rnd::get_range(-range, range),
        y + rnd::get_range(-range, range),
    );
    // `canMoveToTarget && canSeeTarget` — keep the mob out of geometry.
    if world.geo.can_move_to_target(x, y, z, rx, ry, z)
        && world.geo.can_see_target(x, y, z, rx, ry, z)
    {
        (rx, ry)
    } else {
        (x, y)
    }
}

/// `Npc.onSpawn`'s custom boss announcement (`Custom/BossAnnouncements.ini`):
/// a server-wide chat line **and** an on-screen one when a raid or grand boss
/// appears. Minions and raid minions are excluded, and an instanced spawn only
/// counts under the matching `…InstanceAnnouncements` flag.
///
/// Both defeat flags ship `false` here, so only the spawn arm exists. Java
/// looks the name up through `NpcData` rather than using the instance's title,
/// so a champion prefix or a script's `setTitle` never leaks into the line.
fn announce_boss_spawn(world: &World, object_id: i32) {
    let cfg = &world.cfg.boss_announcements;
    if !cfg.raidboss_spawn && !cfg.grandboss_spawn {
        return;
    }
    let Some(t) = world
        .objects
        .get_component::<Npc>(&object_id)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
    else {
        return;
    };
    let grand = t.type_name == "GrandBoss";
    let (enabled, in_instance_ok) = if grand {
        (cfg.grandboss_spawn, cfg.grandboss_instance)
    } else if t.is_raid() {
        (cfg.raidboss_spawn, cfg.raidboss_instance)
    } else {
        return;
    };
    if !enabled {
        return;
    }
    // `!isInInstance() || …InstanceAnnouncements`.
    let in_instance = world
        .objects
        .get_component::<crate::model::components::InstanceId>(&object_id)
        .is_some_and(|i| i.0 != 0);
    if in_instance && !in_instance_ok {
        return;
    }
    // Java's `!isMinion() && !isRaidMinion()` is handled at the call site
    // instead: minions spawn through [`spawn_minion_npc_at`], which does not
    // announce. Checking `MinionOf` here would be dead code — the tag is
    // attached after the entity exists.
    if t.name.is_empty() {
        return; // Java: `if (name != null)`.
    }
    let text = format!("{} has spawned!", t.name);
    let say = crate::network::server_packets::creature_say(
        0,
        crate::enums::ChatType::Announcement,
        "",
        &text,
        None,
    );
    let screen = crate::network::server_packets::ex_show_screen_message(&text, 2, 5000);
    world.broadcast_to_all_online(&say);
    world.broadcast_to_all_online(&screen);
}
