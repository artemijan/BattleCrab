//! The `Npc` world object (Java `model/actor/Npc` + `model/Spawn`), scoped to
//! G8's static-world slice: NPCs spawn at boot from `SpawnData`, stand in the
//! region grid, and can be seen/targeted. No AI, no combat, no respawn yet —
//! nothing can kill them until G9 brings `doDie` (the respawn delays are
//! carried so G9's respawn scheduler has them).

use crate::data::npc_data::NpcTemplate;
use crate::world::{World, region_of};

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
    /// Java `Npc._clanId` (`setClanId` in `onSpawn`): the castle-owner clan
    /// whose crest a TAX-zone NPC wears. Gated at spawn on
    /// `ShowCrestWithoutQuest || castle.show_npc_crest` — both off on this
    /// dist, so 0 unless an operator turns the display on. Read by
    /// `NpcInfo`'s CLAN component (non-monster, peace zone).
    pub crest_clan_id: i32,
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
    /// in `onRespawn` (see [`npc::roll_champion`]). A fresh instance on respawn
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
    /// Java `DecayTaskManager`'s scheduled fire time for this corpse, kept on
    /// the instance because `getRemainingTime` is a *readable* quantity in
    /// Java — `Attackable.isOldCorpse` asks how much of the corpse's life is
    /// left before letting a sweep through. The port's decay is a
    /// `ScheduledTask`, which is fire-and-forget, so the deadline is recorded
    /// here at death. `0` = no corpse pending (alive, or already decayed).
    pub decay_at_tick: u64,
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
            decay_at_tick: 0,
            crest_clan_id: 0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;
    use crate::game_loop::npc::{FIRST_NPC_OBJECT_ID, spawn_all};

    /// Boot-shaped smoke test over the real datapack: every spawnable line
    /// places an NPC, fixed spawns land at retail coordinates, and the
    /// region index stays consistent with the NPC registry.
    #[test]
    fn spawns_real_dist_content() {
        let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
        let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
        let data = dist::game_data_owned();
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
        let data = dist::game_data_owned();
        let t = data
            .npc_data
            .get(22109)
            .expect("22109 Male Spiked Stakato in datapack");
        let (combat, _speeds, max_hp, max_mp) = crate::model::npc_stats::npc_finalized_stats(
            &data,
            t,
            &crate::model::components::Buffs::default(),
            crate::model::npc_stats::NpcStatMods::default(),
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
