//! Make Formal Wear (37) — `quests/Q00037_MakeFormalWear`, restored to its
//! **authentic Interlude** form (level 60, not the level-85 Grand Crusade
//! version shipped in this datapack). Trader Alexis in Aden sends the player on
//! a courier run — Maid Leikar's Signet Ring, Chef Jeremy's Ice Wine for Broker
//! Mist, then a Box of Cookies back to Leikar — before Leikar assembles the four
//! components (Mysterious Cloth, Jewel Box, Sewing Kit and a Dress Shoes Box,
//! each from a sub-quest [`Q33`](super::q00033_make_a_pair_of_dress_shoes)..
//! [`Q36`](super::q00036_make_a_sewing_kit)) into the wedding Formal Wear.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const ALEXIS: i32 = 30842;
const LEIKAR: i32 = 31520;
const JEREMY: i32 = 31521;
const MIST: i32 = 31627;
// Items
const FORMAL_WEAR: i32 = 6408;
const MYSTERIOUS_CLOTH: i32 = 7076;
const JEWEL_BOX: i32 = 7077;
const SEWING_KIT: i32 = 7078;
const DRESS_SHOES_BOX: i32 = 7113;
const BOX_OF_COOKIES: i32 = 7159;
const ICE_WINE: i32 = 7160;
const SIGNET_RING: i32 = 7164;
// Misc
const MIN_LEVEL: i32 = 60;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// Whether the three tailoring components (Cloth + Jewel Box + Sewing Kit) are
/// all in hand.
fn has_components(ctx: &QuestCtx) -> bool {
    has(ctx, MYSTERIOUS_CLOTH) && has(ctx, JEWEL_BOX) && has(ctx, SEWING_KIT)
}

pub struct Q00037MakeFormalWear;

impl QuestScript for Q00037MakeFormalWear {
    fn id(&self) -> i32 {
        37
    }
    fn name(&self) -> &'static str {
        "Q00037_MakeFormalWear"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00037_MakeFormalWear"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ALEXIS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ALEXIS, JEREMY, LEIKAR, MIST]
    }
    fn quest_items(&self) -> &[i32] {
        &[SIGNET_RING, ICE_WINE, BOX_OF_COOKIES]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30842-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31520-02.html" => {
                ctx.give_items(SIGNET_RING, 1);
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "31521-02.html" => {
                // Authentic Interlude: Jeremy takes the Signet Ring in exchange
                // for the Ice Wine.
                ctx.take_items(SIGNET_RING, 1);
                ctx.give_items(ICE_WINE, 1);
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "31627-02.html" => {
                if !has(ctx, ICE_WINE) {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(ICE_WINE, 1);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "31521-05.html" => {
                ctx.give_items(BOX_OF_COOKIES, 1);
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            "31520-05.html" => {
                if !has(ctx, BOX_OF_COOKIES) {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(BOX_OF_COOKIES, 1);
                ctx.set_cond(6, true);
                Some(event.to_string())
            }
            "31520-08.html" => {
                if !has_components(ctx) {
                    return Some("31520-09.html".to_string());
                }
                ctx.take_items(SEWING_KIT, 1);
                ctx.take_items(JEWEL_BOX, 1);
                ctx.take_items(MYSTERIOUS_CLOTH, 1);
                ctx.set_cond(7, true);
                Some(event.to_string())
            }
            "31520-12.html" => {
                if !has(ctx, DRESS_SHOES_BOX) {
                    return Some("31520-13.html".to_string());
                }
                ctx.take_items(DRESS_SHOES_BOX, 1);
                ctx.give_items(FORMAL_WEAR, 1);
                ctx.exit_quest(false, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            ALEXIS => {
                if ctx.is_created() {
                    if ctx.player_level() >= MIN_LEVEL {
                        "30842-01.htm".to_string()
                    } else {
                        "30842-02.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else if cond == 1 {
                    "30842-04.html".to_string()
                } else {
                    ctx.no_quest_html()
                }
            }
            LEIKAR if ctx.is_started() => match cond {
                1 => "31520-01.html".to_string(),
                2 => "31520-03.html".to_string(),
                5 => "31520-04.html".to_string(),
                6 => {
                    if has_components(ctx) {
                        "31520-06.html".to_string()
                    } else {
                        "31520-07.html".to_string()
                    }
                }
                7 => {
                    if has(ctx, DRESS_SHOES_BOX) {
                        "31520-10.html".to_string()
                    } else {
                        "31520-11.html".to_string()
                    }
                }
                _ => ctx.no_quest_html(),
            },
            JEREMY if ctx.is_started() => match cond {
                2 => "31521-01.html".to_string(),
                3 => "31521-03.html".to_string(),
                4 => "31521-04.html".to_string(),
                5 => "31521-06.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            MIST if ctx.is_started() => match cond {
                3 => "31627-01.html".to_string(),
                4 => "31627-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}
