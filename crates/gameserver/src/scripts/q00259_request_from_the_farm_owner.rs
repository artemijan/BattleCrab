//! Request from the Farm Owner (259) — port of
//! `dist/game/data/scripts/quests/Q00259_RequestFromTheFarmOwner/`. Edmond
//! (30497) pays 25a per spider skin (+250 for 10+), and **Marius (30405)** will
//! instead trade **10 skins** for a batch of consumables — the player's choice
//! of Greater Healing Potions, arrows, soulshots or spiritshots. Repeatable,
//! level 15–21. The skin drops unrolled (one per kill).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const EDMOND: i32 = 30497;
const MARIUS: i32 = 30405;
const MONSTERS: [i32; 3] = [20103, 20106, 20108]; // Giant / Talon / Blade Spider
const SPIDER_SKIN: i32 = 1495;
const MIN_LEVEL: i32 = 15;
const SKIN_COUNT: i64 = 10;
const SKIN_REWARD: i64 = 25;
const SKIN_BONUS: i64 = 250;

/// Marius's consumable menu: the trade-page event → `(item, count)`. Each costs
/// `SKIN_COUNT` skins.
fn consumable(event: &str) -> Option<(i32, i64)> {
    match event {
        "30405-04.html" => Some((1061, 2)),   // Greater Healing Potion
        "30405-05.html" => Some((17, 250)),   // Wooden Arrow
        "30405-05a.html" => Some((1835, 60)), // Soulshot: No Grade
        "30405-05c.html" => Some((2509, 30)), // Spiritshot: No Grade
        _ => None,
    }
}

pub struct Q00259RequestFromTheFarmOwner;

impl QuestScript for Q00259RequestFromTheFarmOwner {
    fn id(&self) -> i32 {
        259
    }
    fn name(&self) -> &'static str {
        "Q00259_RequestFromTheFarmOwner"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00259_RequestFromTheFarmOwner"
    }
    fn start_npcs(&self) -> &[i32] {
        &[EDMOND]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[EDMOND, MARIUS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[SPIDER_SKIN]
    }

    /// `addCondMaxLevel(21, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            // Plain pages.
            "30405-03.html" | "30405-05b.html" | "30405-05d.html" | "30497-07.html" => {
                Some(event.to_string())
            }
            // Consumable trades — 10 skins for the batch.
            "30405-04.html" | "30405-05.html" | "30405-05a.html" | "30405-05c.html" => {
                if ctx.quest_items_count(SPIDER_SKIN) >= SKIN_COUNT {
                    if let Some((item, count)) = consumable(event) {
                        ctx.give_items(item, count);
                        ctx.take_items(SPIDER_SKIN, SKIN_COUNT);
                    }
                    return Some(event.to_string());
                }
                None
            }
            // Marius's menu opens only with a full batch.
            "30405-06.html" => Some(
                if ctx.quest_items_count(SPIDER_SKIN) >= SKIN_COUNT {
                    "30405-06.html"
                } else {
                    "30405-07.html"
                }
                .to_string(),
            ),
            "30497-03.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30497-06.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() {
            ctx.give_items(SPIDER_SKIN, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            EDMOND => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30497-02.htm"
                        } else {
                            "30497-01.html"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    let skins = ctx.quest_items_count(SPIDER_SKIN);
                    if skins > 0 {
                        let bonus = if skins >= SKIN_COUNT { SKIN_BONUS } else { 0 };
                        ctx.give_adena(skins * SKIN_REWARD + bonus, true);
                        ctx.take_items(SPIDER_SKIN, -1);
                        return Some("30497-05.html".to_string());
                    }
                    return Some("30497-04.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            MARIUS => Some(
                if ctx.quest_items_count(SPIDER_SKIN) >= SKIN_COUNT {
                    "30405-02.html"
                } else {
                    "30405-01.html"
                }
                .to_string(),
            ),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
