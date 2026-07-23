//! Keen Claws (264) — `quests/Q00264_KeenClaws`. Paint (30136, level 3–9) wants
//! 50 Wolf Claws; per-mob `(amount, threshold)` tables give a *variable* amount,
//! and the reward is a `getRandom(17)` roll with a **HashMap-iteration quirk**:
//! only rolls 0 and 1 pay out (item 735 is unreachable), rolls 2–16 give nothing.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
const PAINT: i32 = 30136;
const WOLF_CLAW: i32 = 1367;
const MIN_LEVEL: i32 = 3;
const REQUIRED: i64 = 50;
const KILL_NPCS: [i32; 2] = [20003, 20456];
/// `(amount, chance-threshold)` — first entry with `roll(100) < threshold` wins.
fn drops(npc_id: i32) -> &'static [(i64, i32)] {
    match npc_id {
        20003 => &[(2, 25), (8, 50)],
        20456 => &[(1, 80), (2, 100)],
        _ => &[],
    }
}
pub struct Q00264KeenClaws;
impl QuestScript for Q00264KeenClaws {
    fn id(&self) -> i32 {
        264
    }
    fn name(&self) -> &'static str {
        "Q00264_KeenClaws"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00264_KeenClaws"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PAINT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PAINT]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[WOLF_CLAW]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 9).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30136-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let roll = ctx.roll(100);
        for &(amount, threshold) in drops(ctx.npc_id) {
            if roll < threshold {
                if ctx.give_item_randomly(WOLF_CLAW, amount, REQUIRED, 1.0, true) {
                    ctx.set_cond(2, false);
                }
                break;
            }
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30136-02.htm"
                } else {
                    "30136-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30136-04.html".to_string()),
                2 if ctx.quest_items_count(WOLF_CLAW) >= REQUIRED => {
                    // Java iterates REWARDS {0:735,1:734,2:35} as `chance < key`,
                    // so only chance 0 (→734, jackpot) and 1 (→35) pay; 735 is dead.
                    match ctx.roll(17) {
                        0 => {
                            ctx.reward_items(734, 1);
                            ctx.play_sound(quest_sounds::JACKPOT);
                        }
                        1 => {
                            ctx.reward_items(35, 1);
                        }
                        _ => {}
                    }
                    ctx.exit_quest(true, true);
                    return Some("30136-05.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
