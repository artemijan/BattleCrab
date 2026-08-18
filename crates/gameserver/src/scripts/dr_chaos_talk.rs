//! Dr. Chaos's chat window — the first-talk half of `ai/bosses/DrChaos`.
//! Talking to Dr. Chaos (32033) drains his paranoia timer by 1–5 and, at ≤0,
//! tips him into the transformation. The state machine lives in
//! [`dr_chaos`]; this is only the first-talk hook.

use crate::game_loop::dr_chaos;
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct DrChaosTalk;

impl QuestScript for DrChaosTalk {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "DrChaos"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/DrChaos"
    }
    fn start_npcs(&self) -> &[i32] {
        &[dr_chaos::DOCTOR_CHAOS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[dr_chaos::DOCTOR_CHAOS]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[dr_chaos::DOCTOR_CHAOS]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        // Dr. Chaos has no menu — the paranoia dialogue is all first-talk.
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc_oid = ctx.npc;
        dr_chaos::on_first_talk(ctx.world, npc_oid)
    }
}
