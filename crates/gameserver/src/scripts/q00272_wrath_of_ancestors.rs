//! Wrath of Ancestors (272) — `quests/Q00272_WrathOfAncestors`. Orc-only:
//! Livina (30572) wants 50 Grave Robber's Heads for 100 adena. Level 5–16.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
const LIVINA: i32 = 30572;
const GRAVE_ROBBERS_HEAD: i32 = 1474;
const MONSTERS: [i32; 2] = [20319, 20320];
const RACE_ORC: i32 = 3;
const MIN_LEVEL: i32 = 5;
const REQUIRED: i64 = 50;
pub struct Q00272WrathOfAncestors;
impl QuestScript for Q00272WrathOfAncestors {
    fn id(&self) -> i32 {
        272
    }
    fn name(&self) -> &'static str {
        "Q00272_WrathOfAncestors"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00272_WrathOfAncestors"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LIVINA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[LIVINA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[GRAVE_ROBBERS_HEAD]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 16).then(|| ctx.no_quest_html())
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30572-01.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30572-03.htm"
                } else {
                    "30572-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30572-05.html".to_string()),
                2 => {
                    ctx.give_adena(100, true);
                    ctx.exit_quest(true, true);
                    return Some("30572-06.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30572-04.htm") {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) {
            ctx.give_items(GRAVE_ROBBERS_HEAD, 1);
            if ctx.quest_items_count(GRAVE_ROBBERS_HEAD) >= REQUIRED {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }
}
