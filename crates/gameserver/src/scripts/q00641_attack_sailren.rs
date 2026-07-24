//! Attack Sailren! (641) — `quests/Q00641_AttackSailren`. The repeatable
//! level-77 access quest for the Sailren raid: Shilen's Stone Statue sends the
//! player to grind 30 Gazkh Fragments from the Primeval Isle raptors, which it
//! fuses into a Gazkh (the raid key). Opens only after
//! `Q00126_TheNameOfEvil2` is complete.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPC
const SHILENS_STONE_STATUE: i32 = 32109;
// Items
const GAZKH_FRAGMENT: i32 = 8782;
const GAZKH: i32 = 8784;
// Misc
const MIN_LEVEL: i32 = 77;
const FRAGMENT_GOAL: i64 = 30;
const NAME_OF_EVIL_2: &str = "Q00126_TheNameOfEvil2";

pub struct Q00641AttackSailren;

impl QuestScript for Q00641AttackSailren {
    fn id(&self) -> i32 {
        641
    }
    fn name(&self) -> &'static str {
        "Q00641_AttackSailren"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00641_AttackSailren"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SHILENS_STONE_STATUE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[SHILENS_STONE_STATUE]
    }
    fn kill_npcs(&self) -> &[i32] {
        // Velociraptors + Pterosaur.
        &[22196, 22197, 22198, 22218, 22223, 22199]
    }
    fn quest_items(&self) -> &[i32] {
        &[GAZKH_FRAGMENT]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "32109-1.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "32109-2a.html" => {
                if ctx.quest_items_count(GAZKH_FRAGMENT) >= FRAGMENT_GOAL {
                    ctx.give_items(GAZKH, 1);
                    ctx.exit_quest(true, true);
                }
                Some(event.to_string())
            }
            _ => Some(event.to_string()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        ctx.give_items(GAZKH_FRAGMENT, 1);
        if ctx.quest_items_count(GAZKH_FRAGMENT) < FRAGMENT_GOAL {
            ctx.play_sound(quest_sounds::ITEMGET);
        } else {
            ctx.set_cond(2, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = if ctx.is_created() {
            if ctx.player_level() < MIN_LEVEL {
                "32109-0.htm".to_string()
            } else if ctx.other_quest_completed(NAME_OF_EVIL_2) {
                "32109-0a.htm".to_string()
            } else {
                // TODO(quests): gated until Q00126_TheNameOfEvil2 is ported;
                // until then this branch is always taken above 77.
                "32109-0b.htm".to_string()
            }
        } else if ctx.is_cond(1) {
            "32109-1a.html".to_string()
        } else {
            "32109-2.html".to_string()
        };
        Some(html)
    }
}
