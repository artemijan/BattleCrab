//! Make a Sewing Kit (36) — `quests/Q00036_MakeASewingKit`, restored to
//! **authentic Interlude** (level 60; the shipped datapack is the level-85 Grand
//! Crusade version). A [`Make Formal Wear`](super::q00037_make_formal_wear)
//! component quest: Head Blacksmith Ferris needs five scraps of Reinforced Steel
//! peeled from Iron Golems by the Ivory Tower, then 10 Oriharukon and 10
//! Artisan's Frames, to forge the Sewing Kit.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPC
const FERRIS: i32 = 30847;
// Monster
const IRON_GOLEM: i32 = 20566;
// Items
const REINFORCED_STEEL: i32 = 7163;
const ORIHARUKON: i32 = 1893;
const ARTISANS_FRAME: i32 = 1891;
const SEWING_KIT: i32 = 7078;
// Misc
const MIN_LEVEL: i32 = 60;
const STEEL_COUNT: i64 = 5;
const MAT_COUNT: i64 = 10;
const PARENT: &str = "Q00037_MakeFormalWear";

fn has_mats(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(ORIHARUKON) >= MAT_COUNT
        && ctx.quest_items_count(ARTISANS_FRAME) >= MAT_COUNT
}

pub struct Q00036MakeASewingKit;

impl QuestScript for Q00036MakeASewingKit {
    fn id(&self) -> i32 {
        36
    }
    fn name(&self) -> &'static str {
        "Q00036_MakeASewingKit"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00036_MakeASewingKit"
    }
    fn start_npcs(&self) -> &[i32] {
        &[FERRIS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[FERRIS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[IRON_GOLEM]
    }
    fn quest_items(&self) -> &[i32] {
        &[REINFORCED_STEEL]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = if ctx.is_created() {
            // Authentic Interlude: only offered while Make Formal Wear is at the
            // "gather the components" stage (cond 6).
            if ctx.other_quest_cond(PARENT) < 6 {
                ctx.no_quest_html()
            } else if ctx.player_level() >= MIN_LEVEL {
                "30847-01.htm".to_string()
            } else {
                "30847-02.html".to_string()
            }
        } else if ctx.is_completed() {
            ctx.already_completed_html()
        } else {
            match ctx.cond() {
                1 => "30847-04.html".to_string(),
                2 => "30847-05.html".to_string(),
                3 => {
                    if has_mats(ctx) {
                        "30847-07.html".to_string()
                    } else {
                        "30847-08.html".to_string()
                    }
                }
                _ => ctx.no_quest_html(),
            }
        };
        Some(html)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30847-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30847-06.html" => {
                if ctx.quest_items_count(REINFORCED_STEEL) < STEEL_COUNT {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(REINFORCED_STEEL, -1);
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "30847-09.html" => {
                if has_mats(ctx) {
                    ctx.take_items(ORIHARUKON, MAT_COUNT);
                    ctx.take_items(ARTISANS_FRAME, MAT_COUNT);
                    ctx.give_items(SEWING_KIT, 1);
                    ctx.exit_quest(false, true);
                    Some(event.to_string())
                } else {
                    Some("30847-10.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Peel a scrap of Reinforced Steel (50%) until five are held.
        if !ctx.has_qs() || !ctx.is_cond(1) || ctx.roll(2) != 0 {
            return;
        }
        ctx.give_items(REINFORCED_STEEL, 1);
        if ctx.quest_items_count(REINFORCED_STEEL) >= STEEL_COUNT {
            ctx.set_cond(2, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
