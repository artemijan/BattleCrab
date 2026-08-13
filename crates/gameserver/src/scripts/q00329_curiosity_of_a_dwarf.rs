//! Curiosity of a Dwarf (329) — `quests/Q00329_CuriosityOfADwarf`. Trader
//! Rolento (30437, level 33–38) buys Golem/Broken Heartstones off the golems.
//! Single cond; turn-in separate from leaving. **Inverted bonus**: fewer than
//! 700 items pays the *larger* 1000-adena bonus.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const ROLENTO: i32 = 30437;
const GOLEM_HEARTSTONE: i32 = 1346;
const BROKEN_HEARTSTONE: i32 = 1365;
const MIN_LEVEL: i32 = 33;
const KILL_NPCS: [i32; 2] = [20083, 20085];
/// `(item, chance-threshold)` on one `roll(100)`; first hit wins.
fn drops(npc_id: i32) -> &'static [(i32, i32)] {
    match npc_id {
        20083 => &[(GOLEM_HEARTSTONE, 3), (BROKEN_HEARTSTONE, 54)],
        20085 => &[(GOLEM_HEARTSTONE, 3), (BROKEN_HEARTSTONE, 58)],
        _ => &[],
    }
}
pub struct Q00329CuriosityOfADwarf;
impl QuestScript for Q00329CuriosityOfADwarf {
    fn id(&self) -> i32 {
        329
    }
    fn name(&self) -> &'static str {
        "Q00329_CuriosityOfADwarf"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00329_CuriosityOfADwarf"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ROLENTO]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ROLENTO]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[GOLEM_HEARTSTONE, BROKEN_HEARTSTONE]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 38).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30437-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30437-06.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30437-07.html" => Some(event.to_string()),
            _ => None,
        }
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let table = drops(ctx.npc_id);
        if let Some(item) = super::quest_common::roll_drop_table(ctx, table) {
            ctx.give_item_randomly(item, 1, 0, 1.0, true);
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30437-02.htm"
                } else {
                    "30437-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let broken = ctx.quest_items_count(BROKEN_HEARTSTONE);
            let golem = ctx.quest_items_count(GOLEM_HEARTSTONE);
            if broken + golem > 0 {
                let bonus = if broken + golem >= 700 { 700 } else { 1000 };
                ctx.give_adena(broken * 5 + golem * 40 + bonus, true);
                ctx.take_items(GOLEM_HEARTSTONE, -1);
                ctx.take_items(BROKEN_HEARTSTONE, -1);
                return Some("30437-05.html".to_string());
            }
            return Some("30437-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
