//! Tarantula's Spider Silk (296) — port of
//! `dist/game/data/scripts/quests/Q00296_TarantulasSpiderSilk/`. Trader Mion
//! (30519) buys Tarantula Spider Silk (5a); the twist is **Defender Nathan
//! (30548)**, who spins each rare **Tarantula Spinnerette** into `15 + rnd(9)`
//! Silk — the spinnerette is the jackpot. Repeatable, level 15–21.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const TRADER_MION: i32 = 30519;
const DEFENDER_NATHAN: i32 = 30548;
const TARANTULA_SPIDER_SILK: i32 = 1493;
const TARANTULA_SPINNERETTE: i32 = 1494;
const MONSTERS: [i32; 3] = [20394, 20403, 20508];
const MIN_LEVEL: i32 = 15;

pub struct Q00296TarantulasSpiderSilk;

impl QuestScript for Q00296TarantulasSpiderSilk {
    fn id(&self) -> i32 {
        296
    }
    fn name(&self) -> &'static str {
        "Q00296_TarantulasSpiderSilk"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00296_TarantulasSpiderSilk"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TRADER_MION]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TRADER_MION, DEFENDER_NATHAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[TARANTULA_SPIDER_SILK, TARANTULA_SPINNERETTE]
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
            "30519-03.htm" if ctx.is_created() => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30519-06.html" if ctx.is_started() => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30519-07.html" if ctx.is_started() => Some(event.to_string()),
            // Nathan spins each spinnerette into 15..23 silk.
            "30548-03.html" if ctx.is_started() => {
                let spinnerettes = ctx.quest_items_count(TARANTULA_SPINNERETTE);
                if spinnerettes > 0 {
                    let per = (15 + ctx.roll(9)) as i64;
                    ctx.give_items(TARANTULA_SPIDER_SILK, per * spinnerettes);
                    ctx.take_items(TARANTULA_SPINNERETTE, -1);
                    Some("30548-03.html".to_string())
                } else {
                    Some("30548-02.html".to_string())
                }
            }
            _ => None,
        }
    }

    /// One `getRandom(100)` gate: `> 95` a rare spinnerette, `> 45` a silk.
    /// Both hand out via `giveItemRandomly` (rate-multiplied), so the port
    /// mirrors it with `give_item_randomly` rather than a plain give.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let chance = ctx.roll(100);
        if chance > 95 {
            ctx.give_item_randomly(TARANTULA_SPINNERETTE, 1, 0, 1.0, true);
        } else if chance > 45 {
            ctx.give_item_randomly(TARANTULA_SPIDER_SILK, 1, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() && ctx.npc_id == TRADER_MION {
            return Some(if ctx.player_level() >= MIN_LEVEL { "30519-02.htm" } else { "30519-01.htm" }.to_string());
        }
        if ctx.is_started() {
            if ctx.npc_id == TRADER_MION {
                let silk = ctx.quest_items_count(TARANTULA_SPIDER_SILK);
                if silk >= 1 {
                    let bonus = if silk >= 10 { 1000 } else { 0 };
                    ctx.give_adena(silk * 5 + bonus, true);
                    ctx.take_items(TARANTULA_SPIDER_SILK, -1);
                    return Some("30519-05.html".to_string());
                }
                return Some("30519-04.html".to_string());
            }
            if ctx.npc_id == DEFENDER_NATHAN {
                return Some("30548-01.html".to_string());
            }
        }
        Some(ctx.no_quest_html())
    }
}
