//! An Obvious Lie (32) — `quests/Q00032_AnObviousLie`. A level-45 errand:
//! Maximilian sends the player to Gentler, whose "map" (an obvious lie) routes
//! them via Miki the Cat and back; along the way Gentler asks for Medicinal
//! Herbs (farmed from Alligators), Spirit Ore, and finally Thread + Suede — for
//! which he crafts a pair of animal Ears (Cat, Raccoon, or Rabbit).

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const MAXIMILIAN: i32 = 30120;
const GENTLER: i32 = 30094;
const MIKI_THE_CAT: i32 = 31706;
// Monster
const ALLIGATOR: i32 = 20135;
// Items
const MAP_OF_GENTLER: i32 = 7165;
const MEDICINAL_HERB: i32 = 7166;
const MEDICINAL_HERB_COUNT: i64 = 20;
const SPIRIT_ORE: i32 = 3031;
const SPIRIT_ORE_COUNT: i64 = 500;
const THREAD: i32 = 1868;
const THREAD_COUNT: i64 = 1000;
const SUEDE: i32 = 1866;
const SUEDE_COUNT: i64 = 500;
// Rewards
const CAT_EARS: i32 = 6843;
const RACCOON_EARS: i32 = 7680;
const RABBIT_EARS: i32 = 7683;
// Misc
const MIN_LEVEL: i32 = 45;

fn count(ctx: &QuestCtx, item: i32) -> i64 {
    ctx.quest_items_count(item)
}

fn has_herbs(ctx: &QuestCtx) -> bool {
    count(ctx, MEDICINAL_HERB) >= MEDICINAL_HERB_COUNT
}

fn has_spirit_ore(ctx: &QuestCtx) -> bool {
    count(ctx, SPIRIT_ORE) >= SPIRIT_ORE_COUNT
}

fn has_thread_and_suede(ctx: &QuestCtx) -> bool {
    count(ctx, THREAD) >= THREAD_COUNT && count(ctx, SUEDE) >= SUEDE_COUNT
}

pub struct Q00032AnObviousLie;

impl QuestScript for Q00032AnObviousLie {
    fn id(&self) -> i32 {
        32
    }
    fn name(&self) -> &'static str {
        "Q00032_AnObviousLie"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00032_AnObviousLie"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MAXIMILIAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MAXIMILIAN, GENTLER, MIKI_THE_CAT]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[ALLIGATOR]
    }
    fn quest_items(&self) -> &[i32] {
        &[MAP_OF_GENTLER, MEDICINAL_HERB]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30120-02.html" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    return Some(event.to_string());
                }
                None
            }
            "30094-02.html" => {
                if ctx.is_cond(1) {
                    ctx.give_items(MAP_OF_GENTLER, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            "31706-02.html" => {
                if ctx.is_cond(2) && count(ctx, MAP_OF_GENTLER) > 0 {
                    ctx.take_items(MAP_OF_GENTLER, -1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            "30094-06.html" => {
                if ctx.is_cond(4) && has_herbs(ctx) {
                    ctx.take_items(MEDICINAL_HERB, MEDICINAL_HERB_COUNT);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30094-09.html" => {
                if ctx.is_cond(5) && has_spirit_ore(ctx) {
                    ctx.take_items(SPIRIT_ORE, SPIRIT_ORE_COUNT);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            "30094-12.html" => {
                if ctx.is_cond(7) {
                    ctx.set_cond(8, true);
                    return Some(event.to_string());
                }
                None
            }
            "30094-15.html" => Some(event.to_string()),
            "31706-05.html" => {
                if ctx.is_cond(6) {
                    ctx.set_cond(7, true);
                    return Some(event.to_string());
                }
                None
            }
            "cat" | "raccoon" | "rabbit" => {
                if ctx.is_cond(8) && has_thread_and_suede(ctx) {
                    ctx.take_items(THREAD, THREAD_COUNT);
                    ctx.take_items(SUEDE, SUEDE_COUNT);
                    let ears = match event {
                        "cat" => CAT_EARS,
                        "raccoon" => RACCOON_EARS,
                        _ => RABBIT_EARS,
                    };
                    ctx.give_items(ears, 1);
                    ctx.exit_quest(false, true);
                    Some("30094-16.html".to_string())
                } else {
                    Some("30094-17.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, 3, 3, npc)`: a cond-3 member (solo →
        // the killer). Farm Medicinal Herbs to 20, then advance to cond 4.
        if !ctx.has_qs() || !ctx.is_cond(3) {
            return;
        }
        if ctx.give_item_randomly(MEDICINAL_HERB, 1, MEDICINAL_HERB_COUNT, 1.0, true) {
            ctx.set_cond(4, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            MAXIMILIAN => {
                if ctx.is_created() {
                    if ctx.player_level() >= MIN_LEVEL {
                        "30120-01.htm".to_string()
                    } else {
                        "30120-03.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else if cond == 1 {
                    "30120-04.html".to_string()
                } else {
                    ctx.no_quest_html()
                }
            }
            GENTLER if ctx.is_started() => match cond {
                1 => "30094-01.html".to_string(),
                2 => "30094-03.html".to_string(),
                4 => {
                    if has_herbs(ctx) {
                        "30094-04.html".to_string()
                    } else {
                        "30094-05.html".to_string()
                    }
                }
                5 => {
                    if has_spirit_ore(ctx) {
                        "30094-07.html".to_string()
                    } else {
                        "30094-08.html".to_string()
                    }
                }
                6 => "30094-10.html".to_string(),
                7 => "30094-11.html".to_string(),
                8 => {
                    if has_thread_and_suede(ctx) {
                        "30094-13.html".to_string()
                    } else {
                        "30094-14.html".to_string()
                    }
                }
                _ => ctx.no_quest_html(),
            },
            MIKI_THE_CAT if ctx.is_started() => match cond {
                2 => {
                    if count(ctx, MAP_OF_GENTLER) > 0 {
                        "31706-01.html".to_string()
                    } else {
                        ctx.no_quest_html()
                    }
                }
                3 | 4 | 5 => "31706-03.html".to_string(),
                6 => "31706-04.html".to_string(),
                7 => "31706-06.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}
