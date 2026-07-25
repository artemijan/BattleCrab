//! The Sleeping Stone Statue (29025) — the player side of Baium
//! (`ai/bosses/Baium`). Its default html (`data/html/default/29025.htm`) carries
//! a `Quest Baium wakeUp` button; clicking it wakes the boss
//! ([`crate::game_loop::baium::wake_up`]). The dist html points at `Quest Baium
//! <event>`, so this script's **name is load-bearing**.
//!
//! The Angelic Vortex (31862) entry flow (Blooded Fabric → teleport in) and the
//! teleport cube are a later slice — TODO(G23).

use crate::game_loop::baium::{self, BAIUM_STONE};
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct Baium;

impl QuestScript for Baium {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Baium"
    }
    fn html_dir(&self) -> &'static str {
        "ai/bosses/Baium"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BAIUM_STONE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[BAIUM_STONE]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        // The statue's "Wake Baium" html is the default one, served by the core;
        // nothing to add here.
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event == "wakeUp" {
            baium::wake_up(ctx.world, ctx.npc, ctx.player);
        }
        None
    }
}
