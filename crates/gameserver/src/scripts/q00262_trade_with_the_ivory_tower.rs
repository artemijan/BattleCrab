//! Trade with the Ivory Tower (262) — port of
//! `dist/game/data/scripts/quests/Q00262_TradeWithTheIvoryTower/`. Vollodos
//! (30137) buys **10 Spore Sacs** off the Gludio fungi for 300 adena.
//! Repeatable, level 8–16.
//!
//! The drop is a **third rate convention**: `getRandom(10) < base *
//! RATE_QUEST_DROP` (the rate folded into the roll *threshold*), then
//! `rewardItems` gives one (its own reward-rate applies to the amount) — so it
//! is neither the plain hand-roll nor `giveItemRandomly`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const VOLLODOS: i32 = 30137;
const SPORE_SAC: i32 = 707;
const MIN_LEVEL: i32 = 8;
const REQUIRED_ITEM_COUNT: i64 = 10;

const KILL_NPCS: [i32; 2] = [20007, 20400];
/// Base drop chance out of 10, before `RATE_QUEST_DROP`.
fn base_chance(npc_id: i32) -> i32 {
    match npc_id {
        20007 => 3, // Green Fungus
        20400 => 4, // Blood Fungus
        _ => 0,
    }
}

pub struct Q00262TradeWithTheIvoryTower;

impl QuestScript for Q00262TradeWithTheIvoryTower {
    fn id(&self) -> i32 {
        262
    }
    fn name(&self) -> &'static str {
        "Q00262_TradeWithTheIvoryTower"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00262_TradeWithTheIvoryTower"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VOLLODOS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VOLLODOS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[SPORE_SAC]
    }

    /// `addCondMaxLevel(16, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 16).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30137-03.htm") {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let threshold = base_chance(ctx.npc_id) as f64 * ctx.rate_quest_drop();
        if (ctx.roll(10) as f64) < threshold {
            ctx.reward_items(SPORE_SAC, 1);
            if ctx.quest_items_count(SPORE_SAC) >= REQUIRED_ITEM_COUNT {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL { "30137-02.htm" } else { "30137-01.htm" }.to_string());
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 if ctx.quest_items_count(SPORE_SAC) < REQUIRED_ITEM_COUNT => return Some("30137-04.html".to_string()),
                2 if ctx.quest_items_count(SPORE_SAC) >= REQUIRED_ITEM_COUNT => {
                    ctx.give_adena(300, true);
                    ctx.exit_quest(true, true);
                    return Some("30137-05.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
