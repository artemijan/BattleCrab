//! The `Npc` world object (Java `model/actor/Npc` + `model/Spawn`), scoped to
//! G8's static-world slice: NPCs spawn at boot from `SpawnData`, stand in the
//! region grid, and can be seen/targeted. No AI, no combat, no respawn yet —
//! nothing can kill them until G9 brings `doDie` (the respawn delays are
//! carried so G9's respawn scheduler has them).

use commons::util::rnd;

use crate::data::npc_data::NpcTemplate;
use crate::data::spawn_data::{NpcSpawnDef, SpawnGroup, SpawnTemplate, Territory};
use crate::world::{World, region_of};

/// First object id handed to spawned NPCs. Java draws NPC ids from the same
/// `IdManager` pool as characters/items; NPCs are transient here, so they get
/// a dedicated base far above anything the DB thread's persistent-id counter
/// (`db::FIRST_OID` + row count) can realistically reach instead of a
/// cross-thread id round-trip.
pub const FIRST_NPC_OBJECT_ID: i32 = 0x4000_0000;

/// `Rnd.get(61794)` — Java's random-heading bound in `Spawn.initializeNpc`
/// (not 65536; kept as-is for parity).
const RANDOM_HEADING_BOUND: i32 = 61794;

/// `AttackableAI`'s intention, narrowed to the states this port drives (IDLE
/// folds into `Active` — there is no think-task registry to drop out of;
/// inactive-region NPCs are simply skipped by the AI tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpcIntention {
    #[default]
    Active,
    Attack,
    /// `AI_INTENTION_MOVE_TO` — committed to a destination walk, currently only
    /// entered by `Fear`. Load-bearing rather than cosmetic: Java's
    /// `AttackableAI.onEvtThink` switches on the intention and has **no case for
    /// `MOVE_TO`**, so a fleeing mob thinks about nothing until it arrives —
    /// which is the only reason the flee isn't instantly overridden by the next
    /// think tick re-issuing a chase. `onEvtArrived` puts it back to `Active`.
    MoveTo,
}

/// One `Attackable._aggroList` entry (Java `AggroInfo`, minus the
/// hate-suspension bookkeeping).
#[derive(Debug, Clone, Copy, Default)]
pub struct AggroInfo {
    pub hate: f64,
    pub damage: f64,
}

/// The NPC residual core: identity + spawn-line bookkeeping — nothing any
/// per-tick system sweeps. Everything system-shaped lives in the extracted
/// components: `Position`/`RegionCell` (phase 1), `Vitals`/`Speeds`/
/// `Collision` (phase 2), presence-based `Movement` (phase 3),
/// `CombatStats`/`AttackState` (phase 4), `NpcAi`/`AggroList` (phase 5).
#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Npc {
    pub object_id: i32,
    /// Template id (`world.data.npc_data.get(npc_id)`).
    pub npc_id: i32,
    /// Respawn bookkeeping for the death/respawn scheduler.
    pub respawn_secs: i32,
    pub respawn_random_secs: i32,
    /// Where this NPC spawned (Java `Npc.getSpawn()` location) — the drift
    /// anchor AI walks back to.
    pub spawn_loc: (i32, i32, i32),
    /// Indices into `GameData.spawn_data` (spawn/group/npc) so death can
    /// schedule a respawn of the same spawn line.
    pub spawn_ref: (usize, usize, usize),
    /// Java `Npc.getSpawn().getChaseRange()` — the spawn line's per-mob leash
    /// override (`<npc chaseRange="5000">`), `0` when the line does not set one.
    /// Copied onto the instance rather than read back through `spawn_ref`
    /// because runtime spawns (minions, quest/admin spawns) carry a placeholder
    /// `(0, 0, 0)` ref that would otherwise resolve to an unrelated spawn line.
    pub chase_range: i32,
    /// Java `Npc._scriptValue` — per-instance scratch slot for scripts
    /// (fresh instance on respawn resets it, like Java).
    pub script_value: i32,
    /// Java `Npc.getVariables()` — the per-instance string-keyed scratch map
    /// quest scripts use alongside `script_value` (11 Interlude quests do,
    /// under 6 distinct keys such as `lastAttacker`). Empty by default and a
    /// `HashMap` does not allocate until the first insert, so idle NPCs pay
    /// only the struct size. A fresh instance on respawn resets it, like Java.
    pub vars: std::collections::HashMap<String, i32>,
    /// Java `Npc.setTitle` — a per-*instance* title that wins over the
    /// template's (and over the `ShowNpcLevel`/`ShowNpcAggression` decoration).
    /// The seal/totem an `EffectPoint` skill plants shows its **caster's name**
    /// this way, which is how a bystander tells whose symbol it is.
    /// `None` = use the template.
    pub title_override: Option<String>,
    /// `Chest._specialDrop` — set by a successful `OpenChest` (the Unlock
    /// skill). It selects **which drop table the corpse rolls**: a chest that
    /// was merely beaten to death rolls a *different* npc id's list (Java
    /// `Chest.doItemDrop` remaps 18265-18298 to the 21xxx band), so smashing
    /// a box and unlocking it are not the same loot. Reset on respawn, like
    /// Java's `onSpawn`.
    pub special_drop: bool,
    /// `Attackable._mustRewardExpSp` — cleared by a successful `OpenChest`, so
    /// an unlocked box hands out loot but no exp/sp.
    pub must_reward_exp_sp: bool,
    /// `Attackable._spoilerObjectId` — object id of the player who landed the
    /// Spoil skill on this mob (0 = not spoiled). Set by the `Spoil` effect,
    /// checked on death to roll the sweep list. A fresh instance on respawn
    /// resets it, like Java.
    pub spoiler_object_id: i32,
    /// `Attackable._sweepItems` — the spoil loot rolled at death (item id,
    /// count), waiting for a `Sweeper` cast. `None` until death rolls it (and
    /// again after `takeSweep` hands it out); `isSweepActive()` == `Some`.
    pub sweep_items: Option<Vec<(i32, i64)>>,
    /// Manor seed state (Java `Attackable._seed`/`_seederObjId`/`_seeded`/
    /// `_harvestItem`). `setSeeded(seed, player)` (the Seed item handler) sets
    /// `seed_id`/`seeder_object_id`; a successful `Sow` effect sets
    /// `seeded = true` and stashes `harvest_item` (crop id, count); a
    /// `Harvesting` cast on the corpse takes it. The seed's crop/level/
    /// alternative are resolved from the catalogue via `seed_id`.
    /// `seeder_object_id == 0` means unsown. A fresh instance on respawn resets
    /// it, like Java.
    pub seed_id: i32,
    pub seeder_object_id: i32,
    pub seeded: bool,
    pub harvest_item: Option<(i32, i64)>,
    /// Java `Npc._currentEnchant` (`getEnchantEffect()`) — the *visual* enchant
    /// level of the weapon in the NPC's hand, i.e. how brightly the blade
    /// glows. Fixed for the instance's life: the ctor rolls
    /// `Rnd.get(4, 21)` when `EnableRandomEnchantEffect` is on (this dist) and
    /// otherwise takes the template's `weaponEnchant`. A respawn is a fresh
    /// instance, so it re-rolls — the same mob glows differently each life.
    pub enchant_effect: i32,
    /// Java `Creature._team` — the blue/red aura the client draws (0 none,
    /// 1 blue, 2 red). Set by `//setteam` and by an event that splits NPCs
    /// between sides; carried in `NpcInfo`'s `TEAM` block.
    pub team: u8,
    /// Java `Npc._displayEffect` — the per-NPC visual state the client swaps
    /// the model into (`ExChangeNpcState`, e.g. a lit/unlit brazier). Stored so
    /// a client that meets the NPC *after* the change still sees it, which is
    /// why Java carries it in `NpcInfo` rather than only in the event packet.
    pub display_effect: i32,
    /// Java `Attackable._champion` — this instance rolled the champion lottery
    /// in `onRespawn` (see [`roll_champion`]). A fresh instance on respawn
    /// re-rolls, like Java, so the same spawn point is a champion only
    /// sometimes. Drives the title prefix, the red team aura, the incoming-
    /// damage divisor, the stat multipliers and the reward multipliers.
    pub champion: bool,
    /// Template-static AI facts, memoized at spawn like the speeds (the
    /// template never changes): the Attackable-subtree gate
    /// (`is_monster || is_guard`), the guard flag itself (the PK-hunting
    /// branch), the stationed-siege-guard type (`"Defender"`), and the two
    /// idle-animation inputs. The 1 s think re-derived every one of these
    /// through template hash lookups (plus a string compare) for every NPC
    /// in an active region.
    pub attackable_ai: bool,
    pub is_guard: bool,
    pub is_defender: bool,
    pub random_animation: bool,
    pub attackable: bool,
}

/// `AttackableAI`'s think state (G9), NPC-only.
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct NpcAi {
    /// Active scan vs. attack loop.
    pub intention: NpcIntention,
    /// `AttackableAI._globalAggro`: starts at -10 (calm for ~10 think ticks
    /// after spawn), climbs to 0, must be ≥ 0 for aggro-range scans to act.
    pub global_aggro: i32,
    /// `AttackableAI._attackTimeout` (absolute world tick): give up chasing
    /// when it passes without landing/receiving a hit.
    pub attack_timeout_tick: u64,
    /// `RandomAnimationTaskManager` pending time (absolute world tick) for the
    /// next idle social animation. `None` until first scheduled (lazily, on the
    /// NPC's first active-region think — like Java's `add()` on spawn).
    pub next_animation_tick: Option<u64>,
    /// `Npc._lastSocialBroadcast` (absolute world tick): the 6 s throttle floor
    /// shared by all social broadcasts.
    pub last_social_tick: u64,
    /// Monotonic cast id, the NPC counterpart of `Player.cast_seq`: a scheduled
    /// launch/finish task carrying a stale seq is a cast that was aborted (or
    /// superseded) and no-ops. See [`crate::game_loop::skills::cast::live_cast`].
    pub cast_seq: u64,
    /// `AttackableAI.chaostime`: thinks elapsed since the last raid/minion
    /// target shuffle. Only raids, grand bosses and minions ever tick it — see
    /// `thinkAttack`'s "BOSS/Raid Minion Target Reconsider" block.
    pub chaos_time: i32,
}

impl Default for NpcAi {
    fn default() -> Self {
        // Java seeds _globalAggro = -10: no aggro for ~10 think seconds.
        Self {
            intention: NpcIntention::Active,
            global_aggro: -10,
            attack_timeout_tick: 0,
            next_animation_tick: None,
            last_social_tick: 0,
            cast_seq: 0,
            chaos_time: 0,
        }
    }
}

/// `Attackable._absorbersList` (NPC-only): the players who cast the Soul
/// Crystal skill (2096) on this mob, each mapped to the mob's HP **at the
/// moment of the cast**. Quest 350 reads it on kill — the crystal only levels
/// if the caster is present *and* absorbed while the mob was at ≤ half HP
/// (Java `AbsorberInfo.getAbsorbedHp()`).
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct Absorbers(pub std::collections::HashMap<i32, f64>);

/// `Attackable._aggroList`, keyed by player object id (NPC-only).
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct AggroList(pub std::collections::HashMap<i32, AggroInfo>);

impl AggroList {
    /// `Attackable.getMostHated()`: highest-hate entry. Liveness
    /// (`Vitals.dead`) is checked by the callers — a corpse's aggro list is
    /// never consulted (AI skips the dead, rewards run before decay).
    pub fn most_hated(&self) -> Option<i32> {
        self.0
            .iter()
            .filter(|(_, info)| info.hate > 0.0)
            .max_by(|a, b| {
                a.1.hate
                    .partial_cmp(&b.1.hate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(&id, _)| id)
    }
}

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
        atk_range: t.base_atk_range,
        random_dmg: t.base_rnd_dam,
    }
}

/// Borrowed view of an NPC's component set for packet builders (the NPC
/// counterpart of `PlayerView`).
pub struct NpcView<'a> {
    pub npc: &'a Npc,
    pub pos: &'a crate::model::components::Position,
    pub vitals: &'a crate::model::components::Vitals,
    pub speeds: &'a crate::model::components::Speeds,
}

impl<'a> NpcView<'a> {
    pub fn of(objects: &'a crate::store::EntityStore, object_id: i32) -> Option<Self> {
        Some(Self {
            npc: objects.get_component::<Npc>(&object_id)?,
            pos: objects.get_component::<crate::model::components::Position>(&object_id)?,
            vitals: objects.get_component::<crate::model::components::Vitals>(&object_id)?,
            speeds: objects.get_component::<crate::model::components::Speeds>(&object_id)?,
        })
    }
}

/// The extracted-component tuple `for_test` builds (spawn via
/// `npcs.insert_with(id, npc, extra)`).
pub type NpcExtra = (
    crate::model::components::Position,
    crate::model::components::RegionCell,
    crate::model::components::Vitals,
    crate::model::components::Speeds,
    crate::model::components::Collision,
    crate::model::components::AttackState,
    NpcAi,
    AggroList,
    crate::model::components::Buffs,
);

impl Npc {
    pub fn template<'a>(&self, world: &'a World) -> Option<&'a NpcTemplate> {
        world.data.npc_data.get(self.npc_id)
    }

    // Template-static AI facts. In production these read the spawn-time
    // memoized copy — `GameData` is immutable after boot, so it is
    // definitionally equal to the template — sparing the every-NPC-every-
    // second think its template hash lookups. Under `cfg(test)` they
    // re-derive from the template on every read instead: fixtures hand-roll
    // `Npc` instances and tweak synthetic templates after spawn, and the
    // template must stay their source of truth.

    /// Runs the `AttackableAI` subtree: monster or town guard.
    pub fn attackable_ai(&self, world: &World) -> bool {
        if cfg!(test) {
            self.template(world)
                .is_some_and(|t| t.is_monster() || t.is_guard())
        } else {
            self.attackable_ai
        }
    }

    /// Town `Guard` — the PK-hunting branch.
    pub fn is_guard(&self, world: &World) -> bool {
        if cfg!(test) {
            self.template(world).is_some_and(|t| t.is_guard())
        } else {
            self.is_guard
        }
    }

    /// Stationed siege guard (`"Defender"` type).
    pub fn is_defender(&self, world: &World) -> bool {
        if cfg!(test) {
            self.template(world)
                .is_some_and(|t| t.type_name == "Defender")
        } else {
            self.is_defender
        }
    }

    /// Template `randomAnimation` flag (idle social animations).
    pub fn random_animation(&self, world: &World) -> bool {
        if cfg!(test) {
            self.template(world).is_some_and(|t| t.random_animation)
        } else {
            self.random_animation
        }
    }

    /// Template `attackable` flag (picks the monster vs. NPC animation bounds).
    pub fn attackable(&self, world: &World) -> bool {
        if cfg!(test) {
            self.template(world).is_some_and(|t| t.attackable)
        } else {
            self.attackable
        }
    }

    /// A synthetic instance for unit tests (spawn-fresh AI/combat state),
    /// with its extracted components: spawn via
    /// `world.objects.spawn(id, (npc, extra))`.
    #[doc(hidden)]
    pub fn for_test(
        object_id: i32,
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        max_hp: i32,
        max_mp: i32,
    ) -> (Self, NpcExtra) {
        use crate::model::components::{
            AttackState, Collision, Position, RegionCell, Speeds, Vitals,
        };
        let npc = Self {
            object_id,
            npc_id,
            respawn_secs: 0,
            respawn_random_secs: 0,
            spawn_loc: (x, y, z),
            spawn_ref: (0, 0, 0),
            chase_range: 0,
            script_value: 0,
            vars: std::collections::HashMap::new(),
            title_override: None,
            special_drop: false,
            must_reward_exp_sp: true,
            spoiler_object_id: 0,
            sweep_items: None,
            seed_id: 0,
            seeder_object_id: 0,
            seeded: false,
            harvest_item: None,
            enchant_effect: 0,
            team: 0,
            display_effect: 0,
            champion: false,
            // The `default_template` these synthetic ids resolve to is "Folk";
            // `tests::add_test_npc` re-derives these from the template it
            // registers (the spawn-site mirror).
            attackable_ai: false,
            is_guard: false,
            is_defender: false,
            random_animation: false,
            attackable: false,
        };
        let extra = (
            Position {
                x,
                y,
                z,
                heading: 0,
            },
            RegionCell(region_of(x, y)),
            Vitals::hp_full(max_hp, max_mp),
            // Default-template speeds (run 120/walk 60), like spawn_one.
            Speeds {
                run_spd: 120.0,
                walk_spd: 60.0,
                swim_run_spd: 0.0,
                swim_walk_spd: 0.0,
                move_multiplier: 1.0,
                base_run_spd: 120.0,
                base_walk_spd: 60.0,
                base_swim_run_spd: 0.0,
                base_swim_walk_spd: 0.0,
                running: false,
                swimming: false,
                swamp_multiplier: 1.0,
            },
            Collision {
                radius: 8.0,
                height: 15.0,
            },
            AttackState::default(),
            NpcAi::default(),
            AggroList::default(),
            crate::model::components::Buffs::default(),
        );
        (npc, extra)
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
    // The data bundle can't be borrowed while `world.objects` is mutated, and the
    // spawn definitions are read-only — walk indices instead of iterators.
    for spawn_idx in 0..world.data.spawn_data.spawns.len() {
        for group_idx in 0..world.data.spawn_data.spawns[spawn_idx].groups.len() {
            // Java `SpawnTemplate.spawnAll` = `spawn(SpawnGroup::
            // isSpawningByDefault)`: a `spawnByDefault="false"` group waits for
            // the script that owns it. 95 groups on this dist — the day/night
            // halves, placed by [`crate::game_loop::spawn_scripts`]. Boot
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
            &mut world.rng,
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
    let Some((x, y, z, heading)) = loc else {
        return None;
    };
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
    crate::game_loop::spawn_scripts::apply_spawn_ai(world, oid, spawn_idx);
    // `WalkingManager.onSpawn` — attach a walking route if this id has one.
    crate::game_loop::walkers::on_npc_spawn(world, oid, npc_id);
    // The escort lands in `world.minions_placed` inside `spawn_minion_group`
    // (the script-chosen named groups count themselves the same way).
    crate::game_loop::minions::spawn_minions(world, oid);
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
pub(crate) fn roll_champion(
    cfg: &crate::config::ChampionConfig,
    t: &crate::data::npc_data::NpcTemplate,
) -> bool {
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
        crate::model::ChampionStatMods::of(&world.cfg.champion, champion),
    );

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
        x + commons::util::rnd::get_range(-range, range),
        y + commons::util::rnd::get_range(-range, range),
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
    for cs in world.clients.values() {
        if matches!(cs, crate::session::ClientSession::InGame(_)) {
            cs.send(say.clone());
            cs.send(screen.clone());
        }
    }
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
        assert_eq!(placed, world.objects.count::<Npc>());
        assert!(placed > 20_000, "expected >20k placed NPCs, got {placed}");

        // Giran.xml: <npc id="30878" x="47984" y="186832" z="-3445" heading="42000"/>.
        let mut candidates: Vec<i32> = Vec::new();
        world.objects.for_each_mut::<&Npc>(|n| {
            if n.npc_id == 30878 {
                candidates.push(n.object_id);
            }
        });
        let giran_guide_id = candidates
            .into_iter()
            .find(|oid| {
                world
                    .objects
                    .get_component::<crate::model::components::Position>(oid)
                    .is_some_and(|p| p.x == 47984)
            })
            .expect("Giran npc 30878 at retail coords");
        let pos = world
            .objects
            .get_component::<crate::model::components::Position>(&giran_guide_id)
            .unwrap();
        assert_eq!((pos.y, pos.z, pos.heading), (186832, -3445, 42000));
        let region = world
            .objects
            .get_component::<crate::model::components::RegionCell>(&giran_guide_id)
            .unwrap();
        assert_eq!(region.0, region_of(47984, 186832));

        // Region index covers every NPC exactly once.
        let indexed: usize = world.npc_regions.values().map(Vec::len).sum();
        assert_eq!(indexed, placed);
        for (region, ids) in &world.npc_regions {
            for id in ids {
                let cell = world
                    .objects
                    .get_component::<crate::model::components::RegionCell>(id)
                    .unwrap();
                assert_eq!(cell.0, *region);
            }
        }

        // Monsters got distinct object ids starting at the NPC base.
        let mut ids: Vec<i32> = Vec::new();
        world
            .objects
            .for_each_mut::<&Npc>(|n| ids.push(n.object_id));
        assert!(ids.iter().all(|&id| id >= FIRST_NPC_OBJECT_ID));
    }

    /// Regression for the NPC stat-parity fix: a retail mob's finalized stats
    /// are its `<vitals>`/`<attack>` base run through the CON/MEN bonus *and*
    /// its passive template skills (HP Increase, Strong P.Atk/Def, …), plus the
    /// `npc_combat_stats` finalizer shapes (m.atk power, atk-speed DEX/WIT,
    /// accuracy tier ladder). Values cross-checked against the Java Mobius
    /// finalizers for NPC 22109 (level 74 Male Spiked Stakato); without the fix
    /// each was the raw base (HP 2632, m.atk 697, p.def 512, atk-spd 253, …).
    /// Loads real datapack data, so it's a touch slower than the synthetic
    /// tests.
    #[test]
    fn stakato_22109_finalized_stats_match_java() {
        let data = crate::data::GameData::load_from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/"
        ));
        let t = data
            .npc_data
            .get(22109)
            .expect("22109 Male Spiked Stakato in datapack");
        let (combat, _speeds, max_hp, max_mp) = crate::model::npc_finalized_stats(
            &data,
            t,
            &crate::model::components::Buffs::default(),
            crate::model::ChampionStatMods::default(),
        );
        // HP: 4× (skill 4408) × (2632 base × 1.58 CON bonus).
        assert_eq!(max_hp as i32, 16635, "max HP");
        // MP: 1× (skill 4409) × (1475 base × 1.22 MEN bonus).
        assert_eq!(max_mp as i32, 1799, "max MP");
        // p.atk: 773.8 base × 1.2 STR × 1.63 levelMod × 1.33 (skill 4410). The
        // live client can read higher when the pack applies Clan Might etc.
        assert_eq!(combat.p_atk as i32, 2013, "p.atk (unbuffed base)");
        // m.atk: 528.4 base × (0.81 INT × 1.63 levelMod)^2.2072.
        assert_eq!(combat.m_atk as i32, 975, "m.atk (power finalizer)");
        // p.def: 314.7 base × 1.63 × 1.33 (4412) × 1.15 (4414 Heavy Armor Type).
        assert_eq!(combat.p_def as i32, 784, "p.def");
        // m.def: 230.3 base × MEN × 1.63 × 1.09 (4789 NPC High Level).
        assert_eq!(combat.m_def as i32, 499, "m.def");
        // atk-spd: 253 base × 1.1 DEX bonus.
        assert_eq!(combat.p_atk_spd, 278, "p.atk speed (DEX bonus)");
        // accuracy: sqrt(30)·5 + 74 + (74-69) high-level tier bonus.
        assert_eq!(combat.accuracy, 106, "accuracy (level tier ladder)");
        // evasion: [sqrt(30)·5 + 74 + (74-69)+2 NPC tier] − 10 (4414 Heavy Armor
        // Type) + 3 (4789) = 101. Distinct tier ladder from accuracy.
        assert_eq!(
            combat.evasion, 101,
            "evasion (NPC tier + EvasionRate skills)"
        );
    }
}
