//! `ai/areas/DwarvenVillage/Toma` — Toma's chat window. The wandering
//! lifecycle (spawn at boot, relocate every 30 minutes) lives in
//! [`crate::game_loop::area_npcs`]; this is only the first-talk hook.

use crate::game_loop::area_npcs::TOMA;
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub struct Toma;

impl QuestScript for Toma {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Toma"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/DwarvenVillage/Toma"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TOMA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TOMA]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[TOMA]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("30556.htm".into())
    }
}
