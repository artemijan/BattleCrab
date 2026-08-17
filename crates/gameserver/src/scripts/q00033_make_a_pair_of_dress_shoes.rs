//! Make a Pair of Dress Shoes (33) — `quests/Q00033_MakeAPairOfDressShoes`,
//! restored to **authentic Interlude** (level 60 and Interlude leather/thread;
//! the shipped datapack is the level-85 Grand Crusade version keyed on GoD
//! crafting items). The final [`Make Formal Wear`](super::q00037_make_formal_wear)
//! component: Trader Woodley sends the player to Maid Leikar, then Ian sells the
//! Worn Leather and Thread (for 300,000 Adena), which Woodley crafts into the
//! Dress Shoes Box for a further 200,000 Adena.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const IAN: i32 = 30164;
const WOODLEY: i32 = 30838;
const LEIKAR: i32 = 31520;
// Items (authentic Interlude ids)
const LEATHER: i32 = 1882;
const THREAD: i32 = 1868;
const ADENA: i32 = 57;
const DRESS_SHOES_BOX: i32 = 7113;
// Misc
const MIN_LEVEL: i32 = 60;
const LEATHER_COUNT: i64 = 360;
const THREAD_COUNT: i64 = 90;
const IAN_FEE: i64 = 300_000;
const WOODLEY_FEE: i64 = 200_000;
const PARENT: &str = "Q00037_MakeFormalWear";

fn count(ctx: &QuestCtx, item: i32) -> i64 {
    ctx.quest_items_count(item)
}

/// Whether the crafted leather/thread and Woodley's fee are all in hand.
fn ready_for_woodley(ctx: &QuestCtx) -> bool {
    count(ctx, LEATHER) >= LEATHER_COUNT
        && count(ctx, THREAD) >= THREAD_COUNT
        && count(ctx, ADENA) >= WOODLEY_FEE
}

pub struct Q00033MakeAPairOfDressShoes;

impl QuestScript for Q00033MakeAPairOfDressShoes {
    fn id(&self) -> i32 {
        33
    }
    fn name(&self) -> &'static str {
        "Q00033_MakeAPairOfDressShoes"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00033_MakeAPairOfDressShoes"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WOODLEY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[WOODLEY, IAN, LEIKAR]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            WOODLEY => {
                if ctx.is_created() {
                    if ctx.other_quest_cond(PARENT) < 7 {
                        ctx.no_quest_html()
                    } else if ctx.player_level() >= MIN_LEVEL {
                        "30838-01.htm".to_string()
                    } else {
                        "30838-02.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match cond {
                        1 => "30838-04.html".to_string(),
                        2 => "30838-05.html".to_string(),
                        3 => {
                            if ready_for_woodley(ctx) {
                                "30838-07.html".to_string()
                            } else {
                                "30838-08.html".to_string()
                            }
                        }
                        5 => "30838-07.html".to_string(),
                        _ => ctx.no_quest_html(),
                    }
                }
            }
            LEIKAR if ctx.is_started() => match cond {
                1 => "31520-01.html".to_string(),
                2 => "31520-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            IAN if ctx.is_started() => match cond {
                3 => "30164-01.html".to_string(),
                5 => "30164-04.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30838-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31520-02.html" => {
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "30838-06.html" => {
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            // Ian sells the leather and thread for his fee.
            "30164-02.html" => {
                if count(ctx, ADENA) < IAN_FEE {
                    return Some("30164-03.html".to_string());
                }
                ctx.give_items(LEATHER, LEATHER_COUNT);
                ctx.give_items(THREAD, THREAD_COUNT);
                ctx.take_items(ADENA, IAN_FEE);
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            // Woodley makes the shoes for his fee.
            "30838-13.html" => {
                if count(ctx, ADENA) < WOODLEY_FEE {
                    return Some("30838-10.html".to_string());
                }
                ctx.take_items(LEATHER, LEATHER_COUNT);
                ctx.take_items(THREAD, THREAD_COUNT);
                ctx.take_items(ADENA, WOODLEY_FEE);
                ctx.give_items(DRESS_SHOES_BOX, 1);
                ctx.exit_quest(false, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }
}
