//! Illegitimate Child of the Goddess (358) — `quests/Q00358_IllegitimateChildOfTheGoddess`.
//! Oltran (30862, level 63–67) wants 108 Snake Scales from Trives/Falibati in
//! the Cemetery; turning them in yields one random B-grade recipe. `addCondMaxLevel(67)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const OLTRAN: i32 = 30862;
const SNAKE_SCALE: i32 = 5868;
const MIN_LEVEL: i32 = 63;
const SNAKE_SCALE_COUNT: i64 = 108;
const REWARDS: [i32; 8] = [4975, 4973, 4974, 4939, 4937, 4938, 4936, 4980];

/// `MOBS`: per-mob Snake Scale drop chance (0..1 for `giveItemRandomly`).
fn drop_chance(npc_id: i32) -> Option<f64> {
    match npc_id {
        20672 => Some(0.71), // Trives
        20673 => Some(0.74), // Falibati
        _ => None,
    }
}

pub struct Q00358IllegitimateChildOfTheGoddess;

impl QuestScript for Q00358IllegitimateChildOfTheGoddess {
    fn id(&self) -> i32 {
        358
    }
    fn name(&self) -> &'static str {
        "Q00358_IllegitimateChildOfTheGoddess"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00358_IllegitimateChildOfTheGoddess"
    }
    fn start_npcs(&self) -> &[i32] {
        &[OLTRAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[OLTRAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20672, 20673]
    }
    fn quest_items(&self) -> &[i32] {
        &[SNAKE_SCALE]
    }

    /// `addCondMaxLevel(67, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 67).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30862-02.htm" | "30862-03.htm" => Some(event.to_string()),
            "30862-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, 1, 3, npc)`: a cond-1 started member.
        // Port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let Some(chance) = drop_chance(ctx.npc_id) else {
            return;
        };
        if ctx.give_item_randomly(SNAKE_SCALE, 1, SNAKE_SCALE_COUNT, chance, true) {
            ctx.set_cond(2, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30862-01.htm"
                } else {
                    "30862-05.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.quest_items_count(SNAKE_SCALE) < SNAKE_SCALE_COUNT {
                return Some("30862-06.html".to_string());
            }
            let idx = ctx.roll(REWARDS.len() as i32) as usize;
            ctx.reward_items(REWARDS[idx], 1);
            ctx.exit_quest(true, true);
            return Some("30862-07.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
