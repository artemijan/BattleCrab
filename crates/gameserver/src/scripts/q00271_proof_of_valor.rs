//! Proof of Valor (271) — port of
//! `dist/game/data/scripts/quests/Q00271_ProofOfValor/`. Orc-only (level 4–8):
//! Rukain (30577) wants **50 Kasha Wolf Fangs**; the reward is the Necklace of
//! Valor (plus a Healing Potion 13% of the time). Repeatable — and once you
//! hold the necklace the dialog changes.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const RUKAIN: i32 = 30577;
const KASHA_WOLF_FANG: i32 = 1473;
const KASHA_WOLF: i32 = 20475;
const HEALING_POTION: i32 = 1539;
const NECKLACE_OF_VALOR: i32 = 1507;
const RACE_ORC: i32 = 3;
const MIN_LEVEL: i32 = 4;
const MAX_LEVEL: i32 = 8;
const REQUIRED_FANGS: i64 = 50;

pub struct Q00271ProofOfValor;

impl QuestScript for Q00271ProofOfValor {
    fn id(&self) -> i32 {
        271
    }
    fn name(&self) -> &'static str {
        "Q00271_ProofOfValor"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00271_ProofOfValor"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RUKAIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RUKAIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[KASHA_WOLF]
    }
    fn quest_items(&self) -> &[i32] {
        &[KASHA_WOLF_FANG]
    }

    /// `addCondMaxLevel(8, "30577-02.htm")` — the refusal uses a specific page,
    /// not the generic no-quest message.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > MAX_LEVEL).then(|| "30577-02.htm".to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30577-01.htm"
                } else if ctx.player_level() < MIN_LEVEL {
                    "30577-02.htm"
                } else if ctx.quest_items_count(NECKLACE_OF_VALOR) > 0 {
                    "30577-07.htm"
                } else {
                    "30577-03.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30577-05.html".to_string()),
                2 if ctx.quest_items_count(KASHA_WOLF_FANG) >= REQUIRED_FANGS => {
                    ctx.reward_items(NECKLACE_OF_VALOR, 1);
                    if ctx.roll(100) <= 13 {
                        ctx.reward_items(HEALING_POTION, 1);
                    }
                    ctx.take_items(KASHA_WOLF_FANG, -1);
                    ctx.exit_quest(true, true);
                    return Some("30577-06.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30577-04.htm") {
            ctx.start_quest();
            // Already wearing the necklace → a different acknowledgement page.
            return Some(
                if ctx.quest_items_count(NECKLACE_OF_VALOR) > 0 {
                    "30577-08.html"
                } else {
                    "30577-04.htm"
                }
                .to_string(),
            );
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let count = ctx.quest_items_count(KASHA_WOLF_FANG);
        // 25% for a double drop, but never when it would overshoot 50.
        let amount = if ctx.roll(100) < 25 && count < 49 {
            2
        } else {
            1
        };
        ctx.give_items(KASHA_WOLF_FANG, amount);
        if count + amount >= REQUIRED_FANGS {
            ctx.set_cond(2, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
