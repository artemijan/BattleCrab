//! `ai/areas/PrimevalIsle/ElrokiTeleporters` — Orahochin and Gariachin
//! ferry players across the Elroki chasm, but refuse anyone in combat.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::AttackState;

const ORAHOCHIN: i32 = 32111;
const GARIACHIN: i32 = 32112;

/// `TELEPORT_ORAHOCIN` / `TELEPORT_GARIACHIN`.
const TELEPORT_ORAHOCHIN: (i32, i32, i32) = (4990, -1879, -3178);
const TELEPORT_GARIACHIN: (i32, i32, i32) = (7557, -5513, -3221);

pub struct ElrokiTeleporters;

impl QuestScript for ElrokiTeleporters {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ElrokiTeleporters"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/PrimevalIsle/ElrokiTeleporters"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ORAHOCHIN, GARIACHIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ORAHOCHIN, GARIACHIN]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[ORAHOCHIN, GARIACHIN]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // Java `talker.isInCombat()` — the attack stance is still running.
        let in_combat = ctx
            .world
            .objects
            .get_component::<AttackState>(&ctx.player)
            .is_some_and(|a| a.stance_until_tick > ctx.world.tick);
        if in_combat {
            return Some(format!("{}-no.html", ctx.npc_id));
        }
        let (x, y, z) = if ctx.npc_id == ORAHOCHIN {
            TELEPORT_ORAHOCHIN
        } else {
            TELEPORT_GARIACHIN
        };
        ctx.teleport_to(x, y, z);
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(format!("{}.html", ctx.npc_id))
    }
}
