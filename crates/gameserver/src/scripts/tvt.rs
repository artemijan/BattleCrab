//! The Team vs Team event manager's player-facing hooks (NPC 70010) — the
//! registration talk window and the register/cancel/buff bypass buttons. The
//! lifecycle and runtime live in [`tvt`]; this is the
//! thin `QuestScript` that routes the manager's `onFirstTalk`/`onEvent` (Java
//! `custom/events/TeamVsTeam/TvT`). G28.

use crate::game_loop::events::tvt;
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct Tvt;

impl QuestScript for Tvt {
    fn id(&self) -> i32 {
        // Not a real quest — an event script (Java `Event`); no quest id.
        -1
    }

    fn name(&self) -> &'static str {
        tvt::NAME
    }

    fn html_dir(&self) -> &'static str {
        "custom/events/TeamVsTeam"
    }

    fn start_npcs(&self) -> &[i32] {
        &[tvt::MANAGER]
    }

    fn talk_npcs(&self) -> &[i32] {
        &[tvt::MANAGER]
    }

    fn first_talk_npcs(&self) -> &[i32] {
        &[tvt::MANAGER]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        // The whole window is `on_first_talk`; a bare talk shows nothing.
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        tvt::on_manager_first_talk(ctx.world, ctx.player, ctx.npc)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        tvt::on_manager_event(ctx.world, ctx.client_id, ctx.player, event)
    }
}
