//! `ai/areas/BeastFarm/Tunatun` — Beast Herder Tunatun hands a Beast
//! Handler's Whip to level-82 tamers (once), plus his info pages.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const TUNATUN: i32 = 31537;
const BEAST_HANDLERS_WHIP: i32 = 15473;
const MIN_LEVEL: i32 = 82;

pub struct Tunatun;

impl QuestScript for Tunatun {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "Tunatun"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/BeastFarm/Tunatun"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TUNATUN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TUNATUN]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[TUNATUN]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("31537.html".into())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "31537-04.html" | "31537-05.html" | "31537-06.html" => Some(event.into()),
            "whip" => {
                if ctx.item_object_id(BEAST_HANDLERS_WHIP).is_some() {
                    Some("31537-01.html".into())
                } else if ctx.player_level() >= MIN_LEVEL {
                    ctx.give_items(BEAST_HANDLERS_WHIP, 1);
                    Some("31537-03.html".into())
                } else {
                    Some("31537-02.html".into())
                }
            }
            _ => Some(ctx.no_quest_html()),
        }
    }
}
