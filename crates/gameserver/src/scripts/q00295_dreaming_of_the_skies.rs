//! Dreaming of the Skies (295) — port of
//! `dist/game/data/scripts/quests/Q00295_DreamingOfTheSkies/`. Arin (30536)
//! wants **50 Floating Stones** off the Magical Weavers; first time you get the
//! Ring of Firefly, and on a repeat (ring already held) 200 adena instead.
//! Repeatable, level 11–15.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const ARIN: i32 = 30536;
const MAGICAL_WEAVER: i32 = 20153;
const FLOATING_STONE: i32 = 1492;
const RING_OF_FIREFLY: i32 = 1509;
const MIN_LEVEL: i32 = 11;
const REQUIRED_STONES: i64 = 50;

pub struct Q00295DreamingOfTheSkies;

impl QuestScript for Q00295DreamingOfTheSkies {
    fn id(&self) -> i32 {
        295
    }
    fn name(&self) -> &'static str {
        "Q00295_DreamingOfTheSkies"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00295_DreamingOfTheSkies"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ARIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ARIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[MAGICAL_WEAVER]
    }
    fn quest_items(&self) -> &[i32] {
        &[FLOATING_STONE]
    }

    /// `addCondMaxLevel(15, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 15).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30536-02.htm"
                } else {
                    "30536-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.is_cond(2) {
                // The ring is a one-time reward; a repeat run pays adena.
                let html = if ctx.quest_items_count(RING_OF_FIREFLY) > 0 {
                    ctx.give_adena(200, true);
                    "30536-06.html"
                } else {
                    ctx.give_items(RING_OF_FIREFLY, 1);
                    "30536-05.html"
                };
                ctx.take_items(FLOATING_STONE, -1);
                ctx.exit_quest(true, true);
                return Some(html.to_string());
            }
            return Some("30536-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && ctx.is_created() && event == "30536-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    /// `giveItemRandomly(FLOATING_STONE, roll(100) > 25 ? 1 : 2, 50, 1, true)`:
    /// mostly one stone, 25% two, capped at 50; reaching 50 flips cond.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) {
            let amount = if ctx.roll(100) > 25 { 1 } else { 2 };
            if ctx.give_item_randomly(FLOATING_STONE, amount, REQUIRED_STONES, 1.0, true) {
                ctx.set_cond(2, false);
            }
        }
    }
}
