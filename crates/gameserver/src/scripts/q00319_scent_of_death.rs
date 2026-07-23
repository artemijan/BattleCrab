//! Scent of Death (319) — `quests/Q00319_ScentOfDeath`. Minaless (30138, level
//! 11–18) wants 5 Zombie Skins (`roll(10) > 7`) for 500 adena. The min-level
//! check is in the start event.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const MINALESS: i32 = 30138;
const MONSTERS: [i32; 2] = [20015, 20020];
const ZOMBIES_SKIN: i32 = 1045;
const MIN_LEVEL: i32 = 11;
const REQUIRED: i64 = 5;
pub struct Q00319ScentOfDeath;
impl QuestScript for Q00319ScentOfDeath {
    fn id(&self) -> i32 {
        319
    }
    fn name(&self) -> &'static str {
        "Q00319_ScentOfDeath"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00319_ScentOfDeath"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MINALESS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MINALESS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[ZOMBIES_SKIN]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 18).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30138-04.htm" && ctx.player_level() >= MIN_LEVEL {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.quest_items_count(ZOMBIES_SKIN) < REQUIRED && ctx.roll(10) > 7 {
            ctx.give_items(ZOMBIES_SKIN, 1);
            // Java sets cond 2 on any skin below the target (a quirk).
            if ctx.quest_items_count(ZOMBIES_SKIN) < REQUIRED {
                ctx.set_cond(2, true);
            }
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30138-03.htm"
                } else {
                    "30138-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.quest_items_count(ZOMBIES_SKIN) >= REQUIRED {
                ctx.give_adena(500, false);
                ctx.take_items(ZOMBIES_SKIN, -1);
                ctx.exit_quest(true, true);
                return Some("30138-06.html".to_string());
            }
            return Some("30138-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
