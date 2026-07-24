//! Help the Son! (44) — `quests/Q00044_HelpTheSon`. The level-24 sibling of
//! [`Q00042`](super::q00042_help_the_uncle): Lundy wants a Work Hammer, then 30
//! gemstone fragments from Maille Lizardmen to fuse a Gemstone that Drikus
//! appraises, for a Pet Ticket.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const LUNDY: i32 = 30827;
const DRIKUS: i32 = 30505;
// Monsters
const MAILLE_GUARD: i32 = 20921;
const MAILLE_SCOUT: i32 = 20920;
const MAILLE_LIZARDMAN: i32 = 20919;
// Items
const WORK_HAMMER: i32 = 168;
const GEMSTONE_FRAGMENT: i32 = 7552;
const GEMSTONE: i32 = 7553;
const PET_TICKET: i32 = 7585;
// Misc
const MIN_LEVEL: i32 = 24;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

pub struct Q00044HelpTheSon;

impl QuestScript for Q00044HelpTheSon {
    fn id(&self) -> i32 {
        44
    }
    fn name(&self) -> &'static str {
        "Q00044_HelpTheSon"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00044_HelpTheSon"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LUNDY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[LUNDY, DRIKUS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[MAILLE_GUARD, MAILLE_LIZARDMAN, MAILLE_SCOUT]
    }
    fn quest_items(&self) -> &[i32] {
        &[GEMSTONE, GEMSTONE_FRAGMENT]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "30827-01.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30827-03.html" => {
                if has(ctx, WORK_HAMMER) {
                    ctx.take_items(WORK_HAMMER, 1);
                    ctx.set_cond(2, true);
                    Some(event.to_string())
                } else {
                    Some("30827-03a.html".to_string())
                }
            }
            "30827-06.html" => {
                if ctx.quest_items_count(GEMSTONE_FRAGMENT) == 30 {
                    ctx.take_items(GEMSTONE_FRAGMENT, -1);
                    ctx.give_items(GEMSTONE, 1);
                    ctx.set_cond(4, true);
                    Some(event.to_string())
                } else {
                    Some("30827-06a.html".to_string())
                }
            }
            "30505-02.html" => {
                if has(ctx, GEMSTONE) {
                    ctx.take_items(GEMSTONE, -1);
                    ctx.set_cond(5, true);
                    Some(event.to_string())
                } else {
                    Some("30505-02a.html".to_string())
                }
            }
            "30827-09.html" => {
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
        ctx.give_items(GEMSTONE_FRAGMENT, 1);
        if ctx.quest_items_count(GEMSTONE_FRAGMENT) == 30 {
            ctx.set_cond(3, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = match ctx.npc_id {
            LUNDY => {
                if ctx.is_created() {
                    if ctx.player_level() >= MIN_LEVEL {
                        "30827-00.htm".to_string()
                    } else {
                        "30827-00a.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match ctx.cond() {
                        1 => {
                            if has(ctx, WORK_HAMMER) {
                                "30827-02.html"
                            } else {
                                "30827-02a.html"
                            }
                        }
                        2 => "30827-04.html",
                        3 => "30827-05.html",
                        4 => "30827-07.html",
                        5 => "30827-08.html",
                        _ => return Some(ctx.no_quest_html()),
                    }
                    .to_string()
                }
            }
            DRIKUS if ctx.is_started() => match ctx.cond() {
                4 => "30505-01.html".to_string(),
                5 => "30505-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}
