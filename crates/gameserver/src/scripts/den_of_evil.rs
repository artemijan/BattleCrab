//! `ai/areas/DenOfEvil` — the Ragna Orc scripts. The zone's `DenOfEvil.java`
//! itself (Kasha's Eye grid + effect-zone curse) is `@Disabled` on this dist
//! and its eyes (18812–18814) have no spawns, so only the orcs are live
//! content.

use crate::game_loop::ground_items::{LOOT_PROTECTION_TICKS, reserve_for};
use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::quests::{QuestCtx, QuestScript};

const RAGNA_ORC_COMMANDER: i32 = 22694;
const RAGNA_ORC_SEER: i32 = 22697;
const RAGNA_ORC_HERO: i32 = 22693;

/// The three leaders pick their escort at spawn from named `<minions>`
/// groups — the generic escort path only spawns the default `"Privates"`.
pub struct RagnaOrcLeaders;

impl QuestScript for RagnaOrcLeaders {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "RagnaOrcLeaders"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/DenOfEvil"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn spawn_npcs(&self) -> &[i32] {
        &[RAGNA_ORC_COMMANDER, RAGNA_ORC_SEER, RAGNA_ORC_HERO]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        let npc = ctx.npc;
        match ctx.npc_id {
            RAGNA_ORC_COMMANDER => {
                crate::game_loop::minions::spawn_minion_group(ctx.world, npc, "Privates1");
                let second = if ctx.roll(2) == 0 {
                    "Privates2"
                } else {
                    "Privates3"
                };
                crate::game_loop::minions::spawn_minion_group(ctx.world, npc, second);
            }
            RAGNA_ORC_SEER => {
                // `"Privates" + getRandom(1, 2)`.
                let group = if ctx.roll(2) == 0 {
                    "Privates1"
                } else {
                    "Privates2"
                };
                crate::game_loop::minions::spawn_minion_group(ctx.world, npc, group);
            }
            RAGNA_ORC_HERO => {
                let group = if ctx.roll(100) < 70 {
                    "Privates1"
                } else {
                    "Privates2"
                };
                crate::game_loop::minions::spawn_minion_group(ctx.world, npc, group);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Frightened Ragna Orc — the bribing coward
// ---------------------------------------------------------------------------

const FRIGHTENED_ORC: i32 = 18807;
const ADENA_ID: i32 = 57;
/// The small payout: 10 × 10 000 adena at 1 000-in-100 000.
const ADENA_SMALL: i64 = 10_000;
const CHANCE_SMALL: i32 = 1000;
/// The jackpot: 10 × 1 000 000 at 10-in-100 000.
const ADENA_BIG: i64 = 1_000_000;
const CHANCE_BIG: i32 = 10;
/// The vanish skill (6234) he casts before disappearing.
const VANISH_SKILL: i32 = 6234;

// The barks (client `NpcStringId`s).
const SAY_BRIBE: i32 = 1800832; // Wait... save me, and I'll give you 10,000,000 adena!
const SAY_SCARED: [i32; 2] = [1800833, 1800834]; // I don't want to fight / Is this really necessary?
const SAY_THANKS: [i32; 2] = [1800835, 1800836]; // Th-thanks... / I'll give you 10,000,000 adena like I promised!
const SAY_SORRY: [i32; 2] = [1800835, 1800871]; // Th-thanks... / Sorry, but this is all I have...
const SAY_LIE: [i32; 2] = [1800837, 1800838]; // ...was a lie, see ya! / You're pretty dumb...
const SAY_DEATH: [i32; 2] = [1800839, 1800840]; // A curse upon you! / I really didn't want to fight...

pub struct FrightenedRagnaOrc;

impl QuestScript for FrightenedRagnaOrc {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FrightenedRagnaOrc"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/DenOfEvil"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[FRIGHTENED_ORC]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[FRIGHTENED_ORC]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        match ctx.npc_script_value() {
            0 => {
                ctx.set_npc_script_value(1);
                let delay = (ctx.roll(5) + 3) as u64 * 1000;
                ctx.start_quest_timer("say", delay);
            }
            1 => {
                let low_hp = ctx
                    .world
                    .objects
                    .get_component::<crate::model::components::Vitals>(&ctx.npc)
                    .is_some_and(|v| v.cur_hp < v.max_hp as f64 * 0.2);
                if low_hp {
                    ctx.start_quest_timer("reward", 10_000);
                    ctx.npc_say(SAY_BRIBE);
                    ctx.set_npc_script_value(2);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let msg = SAY_DEATH[ctx.roll(2) as usize];
        ctx.npc_say(msg);
        ctx.cancel_quest_timer("say");
        ctx.cancel_quest_timer("reward");
    }

    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        let dead = ctx
            .world
            .objects
            .get_component::<crate::model::components::Vitals>(&ctx.npc)
            .is_none_or(|v| v.dead);
        match name {
            "say" => {
                // Repeating while he is merely frightened (script value 1).
                if dead || ctx.npc_script_value() != 1 {
                    return;
                }
                let msg = SAY_SCARED[ctx.roll(2) as usize];
                ctx.npc_say(msg);
                let delay = (ctx.roll(5) + 3) as u64 * 1000;
                ctx.start_quest_timer("say", delay);
            }
            "reward" => {
                if dead || ctx.npc_script_value() != 2 {
                    return;
                }
                let roll = ctx.roll(100_000);
                if roll < CHANCE_BIG {
                    let msg = SAY_THANKS[ctx.roll(2) as usize];
                    ctx.npc_say(msg);
                    ctx.set_npc_script_value(3);
                    cast_vanish(ctx);
                    drop_adena(ctx, ADENA_BIG);
                } else if roll < CHANCE_SMALL {
                    let msg = SAY_SORRY[ctx.roll(2) as usize];
                    ctx.npc_say(msg);
                    ctx.set_npc_script_value(3);
                    cast_vanish(ctx);
                    drop_adena(ctx, ADENA_SMALL);
                } else {
                    let msg = SAY_LIE[ctx.roll(2) as usize];
                    ctx.npc_say(msg);
                }
                ctx.start_quest_timer("despawn", 1000);
            }
            "despawn" => {
                // Java plays a run-away intention in the same breath as
                // `deleteMe` — the flight is unobservable, so just vanish.
                ctx.delete_npc();
            }
            _ => {}
        }
    }
}

fn cast_vanish(ctx: &mut QuestCtx) {
    let npc = ctx.npc;
    if let Some(skill) = skill_by_id(ctx.world, VANISH_SKILL, 1)
        && crate::game_loop::npc_cast::check_use_conditions_pub(ctx.world, npc, &skill)
    {
        crate::game_loop::npc_cast::start_cast(ctx.world, npc, npc, &skill);
    }
}

/// Java drops ten separate stacks on the ground (`npc.dropItem` × 10, no
/// auto-loot branch), each protected for the rescuer.
fn drop_adena(ctx: &mut QuestCtx, per_stack: i64) {
    let Some((x, y, z)) = pos_of(ctx.world, ctx.npc) else {
        return;
    };
    let npc = ctx.npc;
    let player = ctx.player;
    for _ in 0..10 {
        let ground_oid = crate::game_loop::ground_items::spawn_ground_item(
            ctx.world,
            ADENA_ID,
            per_stack,
            0,
            x,
            y,
            z,
            npc,
            crate::game_loop::ground_items::DropSource::Npc,
        );
        reserve_for(ctx.world, ground_oid, player, LOOT_PROTECTION_TICKS);
    }
}
