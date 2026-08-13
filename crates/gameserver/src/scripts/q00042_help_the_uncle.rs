//! Help the Uncle! (42) — `quests/Q00042_HelpTheUncle`. A level-25 pet-ticket
//! quest: Waters wants a Trident, then 30 map pieces farmed from Monster Eyes to
//! reassemble the Map of Sophya; Sophya reads it and Waters rewards a Pet Ticket.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const WATERS: i32 = 30828;
const SOPHYA: i32 = 30735;
// Monsters
const MONSTER_EYE_DESTROYER: i32 = 20068;
const MONSTER_EYE_GAZER: i32 = 20266;
// Items
const TRIDENT: i32 = 291;
const MAP_PIECE: i32 = 7548;
const MAP: i32 = 7549;
const PET_TICKET: i32 = 7583;
// Misc
const MIN_LEVEL: i32 = 25;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

pub struct Q00042HelpTheUncle;

impl QuestScript for Q00042HelpTheUncle {
    fn id(&self) -> i32 {
        42
    }
    fn name(&self) -> &'static str {
        "Q00042_HelpTheUncle"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00042_HelpTheUncle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WATERS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[WATERS, SOPHYA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[MONSTER_EYE_DESTROYER, MONSTER_EYE_GAZER]
    }
    fn quest_items(&self) -> &[i32] {
        &[MAP, MAP_PIECE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "30828-01.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30828-03.html" => {
                if has(ctx, TRIDENT) {
                    ctx.take_items(TRIDENT, 1);
                    ctx.set_cond(2, true);
                    Some(event.to_string())
                } else {
                    Some("30828-03a.html".to_string())
                }
            }
            "30828-06.html" => {
                if ctx.quest_items_count(MAP_PIECE) == 30 {
                    ctx.take_items(MAP_PIECE, -1);
                    ctx.give_items(MAP, 1);
                    ctx.set_cond(4, true);
                    Some(event.to_string())
                } else {
                    Some("30828-06a.html".to_string())
                }
            }
            "30735-02.html" => {
                if has(ctx, MAP) {
                    ctx.take_items(MAP, -1);
                    ctx.set_cond(5, true);
                    Some(event.to_string())
                } else {
                    Some("30735-02a.html".to_string())
                }
            }
            "30828-09.html" => {
                if ctx.is_cond(5) {
                    ctx.give_items(PET_TICKET, 1);
                    ctx.exit_quest(false, true);
                }
                Some(event.to_string())
            }
            _ => Some(event.to_string()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        ctx.collect_toward_on_cond(2, MAP_PIECE, 30, 3);
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = match ctx.npc_id {
            WATERS => {
                if ctx.is_created() {
                    if ctx.player_level() >= MIN_LEVEL {
                        "30828-00.htm".to_string()
                    } else {
                        "30828-00a.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match ctx.cond() {
                        1 => {
                            if has(ctx, TRIDENT) {
                                "30828-02.html"
                            } else {
                                "30828-02a.html"
                            }
                        }
                        2 => "30828-04.html",
                        3 => "30828-05.html",
                        4 => "30828-07.html",
                        5 => "30828-08.html",
                        _ => return Some(ctx.no_quest_html()),
                    }
                    .to_string()
                }
            }
            SOPHYA if ctx.is_started() => match ctx.cond() {
                4 => "30735-01.html".to_string(),
                5 => "30735-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}
