//! Dark Winged Spies (275) — `quests/Q00275_DarkWingedSpies`. Orc-only, level
//! 11–15: Neruga Chief Tantus (30567) wants 70 **Darkwing Bat Fangs** for 5
//! adena each. Killing Darkwing Bats (20316) drops fangs; occasionally a
//! **Varangka's Tracker** (27043) ambushes the hunter, and felling it yields a
//! bundle of fangs.
//!
//! `onCreatureSee` (the Tracker aggroing on sight) isn't a general port hook, so
//! the Tracker is spawned already hostile to the killer — the same outcome.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const NERUGA_CHIEF_TANTUS: i32 = 30567;
const DARKWING_BAT_FANG: i32 = 1478;
const VARANGKAS_PARASITE: i32 = 1479;
const DARKWING_BAT: i32 = 20316;
const VARANGKAS_TRACKER: i32 = 27043;
const RACE_ORC: i32 = 3;
const MIN_LEVEL: i32 = 11;
const MAX_LEVEL: i32 = 15;
const FANG_PRICE: i64 = 5;
const MAX_BAT_FANG_COUNT: i64 = 70;

pub struct Q00275DarkWingedSpies;

impl QuestScript for Q00275DarkWingedSpies {
    fn id(&self) -> i32 {
        275
    }
    fn name(&self) -> &'static str {
        "Q00275_DarkWingedSpies"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00275_DarkWingedSpies"
    }
    fn start_npcs(&self) -> &[i32] {
        &[NERUGA_CHIEF_TANTUS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[NERUGA_CHIEF_TANTUS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[DARKWING_BAT, VARANGKAS_TRACKER]
    }
    fn quest_items(&self) -> &[i32] {
        &[DARKWING_BAT_FANG, VARANGKAS_PARASITE]
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `addCondMaxLevel(15, …)`.
        (ctx.player_level() > MAX_LEVEL).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30567-00.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30567-02.htm"
                } else {
                    "30567-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30567-05.html".to_string()),
                2 => {
                    let count = ctx.quest_items_count(DARKWING_BAT_FANG);
                    if count >= MAX_BAT_FANG_COUNT {
                        ctx.give_adena(count * FANG_PRICE, true);
                        ctx.exit_quest(true, true);
                        return Some("30567-05.html".to_string());
                    }
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30567-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let count = ctx.quest_items_count(DARKWING_BAT_FANG);
        match ctx.npc_id {
            DARKWING_BAT => {
                if ctx.give_item_randomly(DARKWING_BAT_FANG, 1, MAX_BAT_FANG_COUNT, 1.0, true) {
                    ctx.set_cond(2, false);
                } else if count > 10 && count < 66 && ctx.roll(100) < 10 {
                    // The Tracker ambush: spawn it hostile and hand over the
                    // parasite that makes its kill worthwhile.
                    ctx.spawn_attacker(VARANGKAS_TRACKER, true);
                    ctx.give_items(VARANGKAS_PARASITE, 1);
                }
            }
            VARANGKAS_TRACKER if count < 66 && ctx.quest_items_count(VARANGKAS_PARASITE) > 0 => {
                if ctx.give_item_randomly(DARKWING_BAT_FANG, 5, MAX_BAT_FANG_COUNT, 1.0, true) {
                    ctx.set_cond(2, false);
                }
                ctx.take_items(VARANGKAS_PARASITE, -1);
            }
            _ => {}
        }
    }
}
