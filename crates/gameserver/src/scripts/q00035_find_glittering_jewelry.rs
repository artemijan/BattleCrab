//! Find Glittering Jewelry (35) — `quests/Q00035_FindGlitteringJewelry`,
//! restored to **authentic Interlude** (level 60; the shipped datapack is the
//! level-85 Grand Crusade version). A [`Make Formal Wear`](super::q00037_make_formal_wear)
//! component quest: Jeweler Ellie sends the player (via Wharf Manager Felton) to
//! collect ten Rough Jewels from Alligators, then 5 Oriharukon, 500 Silver
//! Nuggets and 150 Thons, to craft the Jewel Box.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const ELLIE: i32 = 30091;
const FELTON: i32 = 30879;
// Monster
const ALLIGATOR: i32 = 20135;
// Items
const ROUGH_JEWEL: i32 = 7162;
const ORIHARUKON: i32 = 1893;
const SILVER_NUGGET: i32 = 1873;
const THONS: i32 = 4044;
const JEWEL_BOX: i32 = 7077;
// Misc
const MIN_LEVEL: i32 = 60;
const JEWEL_COUNT: i64 = 10;
const ORIHARUKON_COUNT: i64 = 5;
const SILVER_NUGGET_COUNT: i64 = 500;
const THONS_COUNT: i64 = 150;
const PARENT: &str = "Q00037_MakeFormalWear";

fn has_mats(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(ORIHARUKON) >= ORIHARUKON_COUNT
        && ctx.quest_items_count(SILVER_NUGGET) >= SILVER_NUGGET_COUNT
        && ctx.quest_items_count(THONS) >= THONS_COUNT
}

pub struct Q00035FindGlitteringJewelry;

impl QuestScript for Q00035FindGlitteringJewelry {
    fn id(&self) -> i32 {
        35
    }
    fn name(&self) -> &'static str {
        "Q00035_FindGlitteringJewelry"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00035_FindGlitteringJewelry"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ELLIE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ELLIE, FELTON]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[ALLIGATOR]
    }
    fn quest_items(&self) -> &[i32] {
        &[ROUGH_JEWEL]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            ELLIE => {
                if ctx.is_created() {
                    if ctx.other_quest_cond(PARENT) < 6 {
                        ctx.no_quest_html()
                    } else if ctx.player_level() >= MIN_LEVEL {
                        "30091-01.htm".to_string()
                    } else {
                        "30091-02.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match cond {
                        1 => "30091-04.html".to_string(),
                        3 => {
                            if ctx.quest_items_count(ROUGH_JEWEL) >= JEWEL_COUNT {
                                "30091-06.html".to_string()
                            } else {
                                "30091-05.html".to_string()
                            }
                        }
                        4 => {
                            if has_mats(ctx) {
                                "30091-09.html".to_string()
                            } else {
                                "30091-10.html".to_string()
                            }
                        }
                        _ => ctx.no_quest_html(),
                    }
                }
            }
            FELTON if ctx.is_started() => match cond {
                1 => "30879-01.html".to_string(),
                2 => "30879-03.html".to_string(),
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
            "30091-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30879-02.html" => {
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "30091-07.html" => {
                if ctx.quest_items_count(ROUGH_JEWEL) < JEWEL_COUNT {
                    return Some("30091-08.html".to_string());
                }
                ctx.take_items(ROUGH_JEWEL, -1);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "30091-11.html" => {
                if has_mats(ctx) {
                    ctx.take_items(ORIHARUKON, ORIHARUKON_COUNT);
                    ctx.take_items(SILVER_NUGGET, SILVER_NUGGET_COUNT);
                    ctx.take_items(THONS, THONS_COUNT);
                    ctx.give_items(JEWEL_BOX, 1);
                    ctx.exit_quest(false, true);
                    Some(event.to_string())
                } else {
                    Some("30091-12.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(2) || ctx.roll(2) != 0 {
            return;
        }
        ctx.give_items(ROUGH_JEWEL, 1);
        if ctx.quest_items_count(ROUGH_JEWEL) >= JEWEL_COUNT {
            ctx.set_cond(3, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
