//! The small combat behaviours from `ai/others` — one hook each, no dialogs:
//!
//! - `PolymorphingOnAttack` — wounded mobs shed their skin into a stronger form.
//! - `PolymorphingAngel` — killing one angel calls its twin.
//! - `TimakOrcTroopLeader` — a leader whistles up one private per swing.
//! - `FleeMonsters` — Elpies run away instead of fighting back.
//! - `FairyTrees` — immobile trees that burst into 20 guardians when felled.
//! - `NonLethalableNpcs` — the siege Headquarters cannot be lethal-blown.
//!
//! (`Scarecrow`'s two NPCs have templates but no spawns and no spawner on this
//! dist — Classic event content — so it is not ported; see
//! `PLAN_G22_AI_OTHERS.md`'s skip table.)

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::{Immobilized, Position, Vitals};

// ---------------------------------------------------------------------------
// PolymorphingOnAttack
// ---------------------------------------------------------------------------

pub struct PolymorphingOnAttack;

/// Java's `MOBSPAWNS`: `npcId → (next form, HP threshold %, chance %, bark
/// group)`. A bark group of `-1` means the change is silent.
struct Morph {
    from: i32,
    into: i32,
    hp_percent: i32,
    chance: i32,
    bark_group: i32,
}

const MORPHS: &[Morph] = &[
    // Fallen Orc Shaman → Sharp Talon Tiger (always polymorphs).
    Morph {
        from: 21258,
        into: 21259,
        hp_percent: 100,
        chance: 100,
        bark_group: -1,
    },
    // Ol Mahum Transcender, three stages.
    Morph {
        from: 21261,
        into: 21262,
        hp_percent: 100,
        chance: 20,
        bark_group: 0,
    },
    Morph {
        from: 21262,
        into: 21263,
        hp_percent: 100,
        chance: 10,
        bark_group: 1,
    },
    Morph {
        from: 21263,
        into: 21264,
        hp_percent: 100,
        chance: 5,
        bark_group: 2,
    },
    // Cave Ant larvae and the ant ladder.
    Morph {
        from: 21265,
        into: 21271,
        hp_percent: 100,
        chance: 33,
        bark_group: 0,
    },
    Morph {
        from: 21266,
        into: 21269,
        hp_percent: 100,
        chance: 100,
        bark_group: -1,
    },
    Morph {
        from: 21267,
        into: 21270,
        hp_percent: 100,
        chance: 100,
        bark_group: -1,
    },
    Morph {
        from: 21271,
        into: 21272,
        hp_percent: 66,
        chance: 10,
        bark_group: 1,
    },
    Morph {
        from: 21272,
        into: 21273,
        hp_percent: 33,
        chance: 5,
        bark_group: 2,
    },
    // The Splendor mobs (Forge of the Gods / Imperial Tomb approach).
    Morph {
        from: 21521,
        into: 21522,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21524,
        into: 21525,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21527,
        into: 21528,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21531,
        into: 21658,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21533,
        into: 21534,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21537,
        into: 21538,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
    Morph {
        from: 21539,
        into: 21540,
        hp_percent: 100,
        chance: 30,
        bark_group: -1,
    },
];

/// Java's `MOBTEXTS` — three barks per stage, one picked at random.
const MORPH_BARKS: [[i32; 3]; 3] = [
    [1_000_407, 1_000_408, 1_000_406],
    [1_000_411, 1_000_410, 1_000_409],
    [1_000_414, 1_000_413, 1_000_412],
];

const POLYMORPH_ON_ATTACK_IDS: &[i32] = &[
    21258, 21261, 21262, 21263, 21265, 21266, 21267, 21271, 21272, 21521, 21524, 21527, 21531,
    21533, 21537, 21539,
];

impl QuestScript for PolymorphingOnAttack {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PolymorphingOnAttack"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        POLYMORPH_ON_ATTACK_IDS
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        let Some(morph) = MORPHS.iter().find(|m| m.from == ctx.npc_id) else {
            return;
        };
        // `npc.isSpawned() && !npc.isDead()`.
        let Some((hp, max_hp)) = ctx
            .world
            .objects
            .get_component::<Vitals>(&ctx.npc)
            .filter(|v| !v.dead)
            .map(|v| (v.cur_hp, v.max_hp))
        else {
            return;
        };
        if hp > (max_hp as f64 * morph.hp_percent as f64) / 100.0 {
            return;
        }
        if ctx.roll(100) >= morph.chance {
            return;
        }
        if morph.bark_group >= 0 {
            let barks = MORPH_BARKS[morph.bark_group as usize];
            let pick = ctx.roll(barks.len() as i32) as usize;
            ctx.npc_say(barks[pick]);
        }
        let Some(pos) = ctx
            .world
            .objects
            .get_component::<Position>(&ctx.npc)
            .copied()
        else {
            return;
        };
        // The new form appears where the old one stood, 10 units higher —
        // Java's `addSpawn(id, x, y, z + 10, heading, …)`.
        let attacker = attacking_creature(ctx);
        ctx.delete_npc();
        let Some(spawned) = crate::model::npc::spawn_npc_at(
            ctx.world,
            morph.into,
            pos.x,
            pos.y,
            pos.z + 10,
            pos.heading,
        ) else {
            return;
        };
        crate::game_loop::death::introduce_npc(ctx.world, spawned);
        // `addDamageHate(originalAttacker, 0, 500)` + `AI_INTENTION_ATTACK`.
        ctx.seed_npc_attack(spawned, attacker);
    }
}

/// Java's `originalAttacker`: the summon actually swinging, else the player
/// (`isSummon ? attacker.getServitors()… : attacker`).
fn attacking_creature(ctx: &QuestCtx) -> i32 {
    if ctx.attack_is_summon() {
        ctx.owner_servitor()
            .or_else(|| ctx.pet_control_object_id())
            .unwrap_or(ctx.player)
    } else {
        ctx.player
    }
}

// ---------------------------------------------------------------------------
// PolymorphingAngel
// ---------------------------------------------------------------------------

pub struct PolymorphingAngel;

/// Java's `ANGELSPAWNS`: the angel that rises where the first one fell.
const ANGEL_SPAWNS: &[(i32, i32)] = &[
    (20830, 20859),
    (21067, 21068),
    (21062, 21063),
    (20831, 20860),
    (21070, 21071),
];

const ANGEL_IDS: &[i32] = &[20830, 21067, 21062, 20831, 21070];

impl QuestScript for PolymorphingAngel {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PolymorphingAngel"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        ANGEL_IDS
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onKill`: the twin spawns on the corpse and starts running — Java sets
    /// no hate, so it only joins in through its own aggro scan.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        let Some(&(_, into)) = ANGEL_SPAWNS.iter().find(|(from, _)| *from == ctx.npc_id) else {
            return;
        };
        ctx.spawn_near_npc(into, false);
    }
}

// ---------------------------------------------------------------------------
// TimakOrcTroopLeader
// ---------------------------------------------------------------------------

pub struct TimakOrcTroopLeader;

const TIMAK_ORC_TROOP_LEADER: i32 = 20767;
/// Java's `ON_ATTACK_MSG`.
const TIMAK_BARKS: &[i32] = &[1_000_294, 1_000_403, 1_000_405, 1_000_404];

impl QuestScript for TimakOrcTroopLeader {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "TimakOrcTroopLeader"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[TIMAK_ORC_TROOP_LEADER]
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onAttack`: on a `SummonPrivateRate` roll, and while fewer than three
    /// privates stand, call in the first `Privates` entry that isn't out yet.
    ///
    /// Java also skips while `monster.isTeleporting()`; NPC teleport state is
    /// not modelled here (nothing teleports this mob), so that guard is absent.
    fn on_attack(&self, ctx: &mut QuestCtx) {
        let rate = ctx
            .world
            .data
            .npc_data
            .get(ctx.npc_id)
            .map_or(0, |t| t.ai_param_i32("SummonPrivateRate", 0));
        // `getRandom(1, 100) <= rate`.
        if rate <= 0 || ctx.roll(100) + 1 > rate {
            return;
        }
        if crate::game_loop::minions::count_spawned_minions(ctx.world, ctx.npc) >= 3 {
            return;
        }
        let privates: Vec<i32> = ctx
            .world
            .data
            .npc_data
            .get(ctx.npc_id)
            .map(|t| {
                t.minions
                    .iter()
                    .filter(|m| m.group == "Privates")
                    .map(|m| m.npc_id)
                    .collect()
            })
            .unwrap_or_default();
        for npc_id in privates {
            let already_out =
                crate::game_loop::minions::minion_of_id_alive(ctx.world, ctx.npc, npc_id);
            if already_out {
                continue;
            }
            let pick = ctx.roll(TIMAK_BARKS.len() as i32) as usize;
            ctx.npc_say(TIMAK_BARKS[pick]);
            crate::game_loop::minions::add_minion(ctx.world, ctx.npc, npc_id);
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// FleeMonsters
// ---------------------------------------------------------------------------

pub struct FleeMonsters;

/// Java lists the Rabbit (20002) too, but it has no spawns on this dist.
const FLEE_MOBS: &[i32] = &[20002, 20432];
/// `FLEE_DISTANCE`.
const FLEE_DISTANCE: f64 = 500.0;

impl QuestScript for FleeMonsters {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FleeMonsters"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        FLEE_MOBS
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onAttack`: turn tail and run 500 units directly away from whoever hit
    /// you. Java first calls `disableCoreAI(true)` so the mob never fights
    /// back; here the `MoveTo` intention is what keeps the AI from re-issuing a
    /// chase (the same mechanism the Fear effect uses), and these two mobs are
    /// passive to begin with.
    fn on_attack(&self, ctx: &mut QuestCtx) {
        let attacker = attacking_creature(ctx);
        let (Some(npc_pos), Some(att_pos)) = (
            ctx.world
                .objects
                .get_component::<Position>(&ctx.npc)
                .copied(),
            ctx.world
                .objects
                .get_component::<Position>(&attacker)
                .copied(),
        ) else {
            return;
        };
        // `Util.calculateAngleFrom(attacker, npc)` — the direction that points
        // from the attacker to the mob, i.e. straight away.
        let radians = ((npc_pos.y - att_pos.y) as f64).atan2((npc_pos.x - att_pos.x) as f64);
        let dest_x = (npc_pos.x as f64 + FLEE_DISTANCE * radians.cos()) as i32;
        let dest_y = (npc_pos.y as f64 + FLEE_DISTANCE * radians.sin()) as i32;
        let (vx, vy, vz) = ctx
            .world
            .geo
            .get_valid_location(npc_pos.x, npc_pos.y, npc_pos.z, dest_x, dest_y, npc_pos.z);
        if let Some(ai) = ctx
            .world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&ctx.npc)
        {
            ai.intention = crate::model::npc::NpcIntention::MoveTo;
        }
        crate::game_loop::npc_ai::move_npc_to(ctx.world, ctx.npc, vx, vy, vz);
    }
}

// ---------------------------------------------------------------------------
// FairyTrees
// ---------------------------------------------------------------------------

pub struct FairyTrees;

const FAIRY_TREES: &[i32] = &[27185, 27186, 27187, 27188];
/// Soul of Tree Guardian.
const SOUL_GUARDIAN: i32 = 27189;
/// Venomous Poison, level 1.
const VENOMOUS_POISON: (i32, i32) = (4243, 1);
/// How many guardians a felled tree releases, and how long they last.
const GUARDIAN_COUNT: usize = 20;
const GUARDIAN_LIFETIME_MS: u64 = 30_000;
/// `MIN_DISTANCE` — kill it from farther away and nothing happens.
const REVENGE_RANGE: f64 = 1500.0;

impl QuestScript for FairyTrees {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FairyTrees"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        FAIRY_TREES
    }
    fn spawn_npcs(&self) -> &[i32] {
        FAIRY_TREES
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onSpawn`: a tree is rooted to its spot. Java calls
    /// `setRandomWalking(false)` **and** `setImmobilized(true)`; here the
    /// second covers both, since `is_movement_disabled` (which the idle
    /// random-walk branch goes through) already reads `Immobilized`.
    fn on_spawn(&self, ctx: &mut QuestCtx) {
        ctx.world.objects.add_components(&ctx.npc, Immobilized);
    }

    /// `onKill`: 20 guardians boil out of the stump, each set on the killer and
    /// each with an even chance of opening with Venomous Poison. They vanish
    /// after 30 s.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        let killer = attacking_creature(ctx);
        let (Some(npc_pos), Some(killer_pos)) = (
            ctx.world
                .objects
                .get_component::<Position>(&ctx.npc)
                .copied(),
            ctx.world
                .objects
                .get_component::<Position>(&killer)
                .copied(),
        ) else {
            return;
        };
        let dx = (npc_pos.x - killer_pos.x) as f64;
        let dy = (npc_pos.y - killer_pos.y) as f64;
        let dz = (npc_pos.z - killer_pos.z) as f64;
        if (dx * dx + dy * dy + dz * dz).sqrt() > REVENGE_RANGE {
            return;
        }
        for _ in 0..GUARDIAN_COUNT {
            let Some(guardian) = ctx.spawn_near_npc(SOUL_GUARDIAN, false) else {
                continue;
            };
            ctx.schedule_despawn(guardian, GUARDIAN_LIFETIME_MS);
            ctx.seed_npc_attack(guardian, killer);
            // `getRandomBoolean()` — half of them lead with the poison.
            if ctx.roll(2) == 0
                && let Some(skill) = ctx
                    .world
                    .data
                    .skill_data
                    .get(VENOMOUS_POISON.0, VENOMOUS_POISON.1)
                    .cloned()
            {
                crate::game_loop::npc_cast::start_cast(ctx.world, guardian, killer, &skill);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NonLethalableNpcs
// ---------------------------------------------------------------------------

pub struct NonLethalableNpcs;

/// Java's `NPCS` — the siege Headquarters. It has no spawn entry (a clan plants
/// it with the HQ item during a siege), so this only fires at runtime.
const HEADQUARTERS: i32 = 35062;

impl QuestScript for NonLethalableNpcs {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "NonLethalableNpcs"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others"
    }
    /// No dialog: these scripts register combat hooks only.
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn spawn_npcs(&self) -> &[i32] {
        &[HEADQUARTERS]
    }
    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        ctx.world
            .objects
            .add_components(&ctx.npc, crate::model::components::NotLethalable);
    }
}
