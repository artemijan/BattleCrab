//! `ai/areas/LairOfAntharas/Pytan` — killing a Pytan has a 5% chance to
//! spring a Knoriks on the killer.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const PYTAN: i32 = 20761;
const KNORIKS: i32 = 20405;

/// Java `addSpawn(…, 300000)` — 5 minutes.
const KNORIKS_LIFE_MS: u64 = 300_000;

pub struct Pytan;

impl QuestScript for Pytan {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Pytan"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/LairOfAntharas"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[PYTAN]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.roll(100) < 5 {
            // TODO(G22): Java targets the killing Playable (pet included).
            if let Some(knoriks) = ctx.spawn_attacker(KNORIKS, false) {
                ctx.schedule_despawn(knoriks, KNORIKS_LIFE_MS);
            }
            ctx.delete_npc();
        }
    }
}
