//! `ai/areas/ForgeOfTheGods` — kill the forge's monsters fast enough and
//! Lavasauruses erupt out of the corpses, meaner the hotter the streak.
//! The streak counter lives on [`World::fog_kill_count`] and a 15 s
//! `FogRefresh` beat cools it back to zero.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const FOG_MOBS: [i32; 16] = [
    22634, // Scarlet Stakato Worker
    22635, // Scarlet Stakato Soldier
    22636, // Scarlet Stakato Noble
    22637, // Tepra Scorpion
    22638, // Tepra Scarab
    22639, // Assassin Beetle
    22640, // Mercenary of Destruction
    22641, // Knight of Destruction
    22642, // Lavastone Golem
    22643, // Magma Golem
    22644, // Arimanes of Destruction
    22645, // Balor of Destruction
    22646, // Ashuras of Destruction
    22647, // Lavasillisk
    22648, // Blazing Ifrit
    22649, // Magma Drake
];

/// Newborn → Ancient.
const LAVASAURUSES: [i32; 5] = [18799, 18800, 18801, 18802, 18803];

const MOBCOUNT_BONUS_MIN: i32 = 3;
const BONUS_UPPER: [i32; 5] = [5, 10, 15, 20, 35];
const BONUS_LOWER: [i32; 3] = [5, 10, 15];
const FORGE_BONUS_1: i32 = 20;
const FORGE_BONUS_2: i32 = 40;

/// A spawned Lavasaurus lives 60 s (Java's `"suicide"` timer → `doDie`).
const LAVASAURUS_LIFE_TICKS: u64 = 600;

pub struct ForgeOfTheGods;

impl QuestScript for ForgeOfTheGods {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ForgeOfTheGods"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/ForgeOfTheGods"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        &FOG_MOBS
    }
    fn spawn_npcs(&self) -> &[i32] {
        &LAVASAURUSES
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let rand = ctx.roll(100);
        ctx.world.fog_kill_count += 1;
        let count = ctx.world.fog_kill_count;
        // Java keys the floor off the *spawn line's* z; the corpse's own z
        // serves — the two forge levels sit thousands of units apart.
        let z = ctx
            .world
            .objects
            .get_component::<crate::model::components::Position>(&ctx.npc)
            .map(|p| p.z)
            .unwrap_or(0);

        let tiered = |low: i32, high: i32| -> Option<i32> {
            if rand <= FORGE_BONUS_1 {
                Some(low)
            } else if rand <= FORGE_BONUS_2 {
                Some(high)
            } else {
                None
            }
        };
        let mob = if z < -5000 {
            // Lower forge.
            if count > BONUS_LOWER[2] && rand <= FORGE_BONUS_2 {
                Some(LAVASAURUSES[4])
            } else if count > BONUS_LOWER[1] {
                tiered(LAVASAURUSES[4], LAVASAURUSES[3])
            } else if count > BONUS_LOWER[0] {
                tiered(LAVASAURUSES[3], LAVASAURUSES[2])
            } else if count >= MOBCOUNT_BONUS_MIN {
                tiered(LAVASAURUSES[2], LAVASAURUSES[1])
            } else {
                None
            }
        } else {
            // Upper forge.
            if count > BONUS_UPPER[4] && rand <= FORGE_BONUS_2 {
                Some(LAVASAURUSES[1])
            } else if count > BONUS_UPPER[3] {
                tiered(LAVASAURUSES[4], LAVASAURUSES[3])
            } else if count > BONUS_UPPER[2] {
                tiered(LAVASAURUSES[3], LAVASAURUSES[2])
            } else if count > BONUS_UPPER[1] {
                tiered(LAVASAURUSES[2], LAVASAURUSES[1])
            } else if count > BONUS_UPPER[0] {
                tiered(LAVASAURUSES[1], LAVASAURUSES[0])
            } else if count >= MOBCOUNT_BONUS_MIN && rand <= FORGE_BONUS_1 {
                Some(LAVASAURUSES[0])
            } else {
                None
            }
        };
        if let Some(npc_id) = mob {
            // `addDamageHate(killer, 0, 9999)` + ATTACK.
            ctx.spawn_attacker(npc_id, true);
        }
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        // The 60 s lifespan. Java `doDie` — the port despawns; TODO(G22):
        // Java's death animation/corpse on expiry.
        let npc = ctx.npc;
        ctx.world.scheduler.schedule(
            ctx.world.tick + LAVASAURUS_LIFE_TICKS,
            crate::scheduler::ScheduledTask::DespawnNpc { npc_oid: npc },
        );
    }
}
