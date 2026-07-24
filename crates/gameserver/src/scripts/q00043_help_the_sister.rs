//! Help the Sister! (43) — `quests/Q00043_HelpTheSister`. The level-26 sibling
//! of [`Q00042`](super::q00042_help_the_uncle): Cooper wants a Crafted Dagger,
//! then 30 map pieces from a wide roster of Dion-area mobs to rebuild the map
//! Galladucci reads, for a Pet Ticket.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const COOPER: i32 = 30829;
const GALLADUCCI: i32 = 30097;
// Monsters
const HOBGOBLIN: i32 = 20147;
const DION_GRIZZLY: i32 = 20203;
const DIRE_WOLF: i32 = 20205;
const OL_MAHUM_RANGER: i32 = 20224;
const MONSTER_EYE_SEARCHER: i32 = 20265;
const MONSTER_EYE_GAZER: i32 = 20266;
const ENKU_ORC_HERO: i32 = 20291;
const ENKU_ORC_SHAMAN: i32 = 20292;
// Items
const CRAFTED_DAGGER: i32 = 220;
const MAP_PIECE: i32 = 7550;
const MAP: i32 = 7551;
const PET_TICKET: i32 = 7584;
// Misc
const MIN_LEVEL: i32 = 26;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

pub struct Q00043HelpTheSister;

impl QuestScript for Q00043HelpTheSister {
    fn id(&self) -> i32 {
        43
    }
    fn name(&self) -> &'static str {
        "Q00043_HelpTheSister"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00043_HelpTheSister"
    }
    fn start_npcs(&self) -> &[i32] {
        &[COOPER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[COOPER, GALLADUCCI]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            DION_GRIZZLY,
            HOBGOBLIN,
            DIRE_WOLF,
            OL_MAHUM_RANGER,
            MONSTER_EYE_SEARCHER,
            MONSTER_EYE_GAZER,
            ENKU_ORC_HERO,
            ENKU_ORC_SHAMAN,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[MAP, MAP_PIECE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "30829-01.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30829-03.html" => {
                if has(ctx, CRAFTED_DAGGER) {
                    ctx.take_items(CRAFTED_DAGGER, 1);
                    ctx.set_cond(2, true);
                    Some(event.to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            "30829-06.html" => {
                if ctx.quest_items_count(MAP_PIECE) == 30 {
                    ctx.take_items(MAP_PIECE, -1);
                    ctx.give_items(MAP, 1);
                    ctx.set_cond(4, true);
                    Some(event.to_string())
                } else {
                    Some("30829-06a.html".to_string())
                }
            }
            "30097-02.html" => {
                if has(ctx, MAP) {
                    ctx.take_items(MAP, -1);
                    ctx.set_cond(5, true);
                    Some(event.to_string())
                } else {
                    Some("30097-02a.html".to_string())
                }
            }
            "30829-09.html" => {
                ctx.give_items(PET_TICKET, 1);
                ctx.exit_quest(false, true);
                Some(event.to_string())
            }
            _ => Some(event.to_string()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(2) {
            return;
        }
        ctx.give_items(MAP_PIECE, 1);
        if ctx.quest_items_count(MAP_PIECE) == 30 {
            ctx.set_cond(3, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = match ctx.npc_id {
            COOPER => {
                if ctx.is_created() {
                    if ctx.player_level() >= MIN_LEVEL {
                        "30829-00.htm".to_string()
                    } else {
                        "30829-00a.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match ctx.cond() {
                        1 => {
                            if has(ctx, CRAFTED_DAGGER) {
                                "30829-02.html"
                            } else {
                                "30829-02a.html"
                            }
                        }
                        2 => "30829-04.html",
                        3 => "30829-05.html",
                        4 => "30829-07.html",
                        5 => "30829-08.html",
                        _ => return Some(ctx.no_quest_html()),
                    }
                    .to_string()
                }
            }
            GALLADUCCI if ctx.is_started() => match ctx.cond() {
                4 => "30097-01.html".to_string(),
                5 => "30097-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}
