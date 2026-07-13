//! In Search of the Nest (109) — port of
//! `dist/game/data/scripts/quests/Q00109_InSearchOfTheNest/`. Primeval
//! Isle: Pierce → the scout's corpse (note, cond 2) → back to Pierce
//! (cond 3) → Kahman pays out. **One-time** (`exitQuest(false)`) — the
//! first ported quest exercising the completed mask end to end.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const PIERCE: i32 = 31553;
const SCOUTS_CORPSE: i32 = 32015;
const KAHMAN: i32 = 31554;
const SCOUTS_NOTE: i32 = 14858;

pub struct Q00109InSearchOfTheNest;

impl QuestScript for Q00109InSearchOfTheNest {
    fn id(&self) -> i32 {
        109
    }
    fn name(&self) -> &'static str {
        "Q00109_InSearchOfTheNest"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00109_InSearchOfTheNest"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PIERCE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PIERCE, SCOUTS_CORPSE, KAHMAN]
    }
    fn quest_items(&self) -> &[i32] {
        &[SCOUTS_NOTE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "31553-0.htm" => ctx.start_quest(),
            "32015-2.html" => {
                ctx.give_items(SCOUTS_NOTE, 1);
                ctx.set_cond(2, true);
            }
            "31553-3.html" => {
                ctx.take_items(SCOUTS_NOTE, -1);
                ctx.set_cond(3, true);
            }
            "31554-2.html" => {
                ctx.give_adena(161500, true);
                ctx.add_exp_and_sp(701500, 50000);
                ctx.exit_quest(false, true);
            }
            _ => {}
        }
        Some(event.to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            PIERCE => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() < 81 { "31553-0a.htm" } else { "31553-0b.htm" }.to_string(),
                    );
                }
                if ctx.is_started() {
                    return match ctx.cond() {
                        1 => Some("31553-1.html".to_string()),
                        2 => Some("31553-2.html".to_string()),
                        3 => Some("31553-3a.html".to_string()),
                        _ => Some(ctx.no_quest_html()),
                    };
                }
                if ctx.is_completed() {
                    return Some(ctx.already_completed_html());
                }
            }
            SCOUTS_CORPSE => {
                if ctx.is_started() {
                    if ctx.is_cond(1) {
                        return Some("32015-1.html".to_string());
                    }
                    if ctx.is_cond(2) {
                        return Some("32015-3.html".to_string());
                    }
                }
            }
            KAHMAN => {
                if ctx.is_started() && ctx.is_cond(3) {
                    return Some("31554-1.html".to_string());
                }
            }
            _ => {}
        }
        Some(ctx.no_quest_html())
    }
}
