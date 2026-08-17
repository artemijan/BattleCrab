//! In Search of Cloth (34) — `quests/Q00034_InSearchOfCloth`, restored to
//! **authentic Interlude** (level 60; the shipped datapack is the level-85 Grand
//! Crusade version, which even swaps the spiders for Grand Crusade mobs). A
//! [`Make Formal Wear`](super::q00037_make_formal_wear) component quest: Armor
//! Trader Radia routes the player through Varan and Ralford, who spins ten
//! Spinnerets from the Sea of Spores' Trisalim spiders into Spidersilk; with
//! 3,000 Suede and 5,000 Thread, Radia weaves the Mysterious Cloth.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const RADIA: i32 = 30088;
const RALFORD: i32 = 30165;
const VARAN: i32 = 30294;
// Monsters
const TRISALIM_SPIDER: i32 = 20560;
const TRISALIM_TARANTULA: i32 = 20561;
// Items
const SPINNERET: i32 = 7528;
const SPIDERSILK: i32 = 7161;
const SUEDE: i32 = 1866;
const THREAD: i32 = 1868;
const MYSTERIOUS_CLOTH: i32 = 7076;
// Misc
const MIN_LEVEL: i32 = 60;
const SPINNERET_COUNT: i64 = 10;
const SUEDE_COUNT: i64 = 3000;
const THREAD_COUNT: i64 = 5000;
const PARENT: &str = "Q00037_MakeFormalWear";

fn has_mats(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(SUEDE) >= SUEDE_COUNT && ctx.quest_items_count(THREAD) >= THREAD_COUNT
}

pub struct Q00034InSearchOfCloth;

impl QuestScript for Q00034InSearchOfCloth {
    fn id(&self) -> i32 {
        34
    }
    fn name(&self) -> &'static str {
        "Q00034_InSearchOfCloth"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00034_InSearchOfCloth"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RADIA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RADIA, RALFORD, VARAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[TRISALIM_SPIDER, TRISALIM_TARANTULA]
    }
    fn quest_items(&self) -> &[i32] {
        &[SPIDERSILK, SPINNERET]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            RADIA => {
                if ctx.is_created() {
                    if ctx.other_quest_cond(PARENT) < 6 {
                        ctx.no_quest_html()
                    } else if ctx.player_level() >= MIN_LEVEL {
                        "30088-01.htm".to_string()
                    } else {
                        "30088-02.html".to_string()
                    }
                } else if ctx.is_completed() {
                    ctx.already_completed_html()
                } else {
                    match cond {
                        1 => "30088-04.html".to_string(),
                        2 => "30088-05.html".to_string(),
                        3 => "30088-07.html".to_string(),
                        6 => {
                            if has_mats(ctx) {
                                "30088-08.html".to_string()
                            } else {
                                "30088-09.html".to_string()
                            }
                        }
                        _ => ctx.no_quest_html(),
                    }
                }
            }
            VARAN if ctx.is_started() => match cond {
                1 => "30294-01.html".to_string(),
                2 => "30294-03.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            RALFORD if ctx.is_started() => match cond {
                3 => "30165-01.html".to_string(),
                4 => "30165-03.html".to_string(),
                5 => "30165-04.html".to_string(),
                6 => "30165-06.html".to_string(),
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
            "30088-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30294-02.html" => {
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "30088-06.html" => {
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "30165-02.html" => {
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "30165-05.html" => {
                if ctx.quest_items_count(SPINNERET) < SPINNERET_COUNT {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(SPINNERET, SPINNERET_COUNT);
                ctx.give_items(SPIDERSILK, 1);
                ctx.set_cond(6, true);
                Some(event.to_string())
            }
            "30088-10.html" => {
                if has_mats(ctx) && ctx.quest_items_count(SPIDERSILK) > 0 {
                    ctx.take_items(SPIDERSILK, 1);
                    ctx.take_items(SUEDE, SUEDE_COUNT);
                    ctx.take_items(THREAD, THREAD_COUNT);
                    ctx.give_items(MYSTERIOUS_CLOTH, 1);
                    ctx.exit_quest(false, true);
                    Some(event.to_string())
                } else {
                    Some("30088-11.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(4) || ctx.roll(2) != 0 {
            return;
        }
        ctx.give_items(SPINNERET, 1);
        if ctx.quest_items_count(SPINNERET) >= SPINNERET_COUNT {
            ctx.set_cond(5, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
