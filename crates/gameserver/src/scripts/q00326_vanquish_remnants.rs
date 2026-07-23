//! Vanquish Remnants (326) — `quests/Q00326_VanquishRemnants`. Leopold (30435,
//! level 21–30) buys Ol Mahum cross-badges; reaching 100 total earns the Black
//! Lion Mark (kept across turn-ins). Single cond, turn-in separate from exit.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const LEOPOLD: i32 = 30435;
const RED_CROSS_BADGE: i32 = 1359;
const BLUE_CROSS_BADGE: i32 = 1360;
const BLACK_CROSS_BADGE: i32 = 1361;
const BLACK_LION_MARK: i32 = 1369;
const KILL_NPCS: [i32; 9] = [20053, 20058, 20061, 20063, 20066, 20436, 20437, 20438, 20439];
/// `(chance, badge item)`.
fn drop(npc_id: i32) -> Option<(i32, i32)> {
    match npc_id {
        20053 | 20058 => Some((61, RED_CROSS_BADGE)),
        20437 => Some((59, RED_CROSS_BADGE)),
        20061 => Some((57, BLUE_CROSS_BADGE)),
        20063 => Some((63, BLUE_CROSS_BADGE)),
        20436 => Some((55, BLUE_CROSS_BADGE)),
        20439 => Some((62, BLUE_CROSS_BADGE)),
        20066 => Some((59, BLACK_CROSS_BADGE)),
        20438 => Some((60, BLACK_CROSS_BADGE)),
        _ => None,
    }
}
pub struct Q00326VanquishRemnants;
impl QuestScript for Q00326VanquishRemnants {
    fn id(&self) -> i32 { 326 }
    fn name(&self) -> &'static str { "Q00326_VanquishRemnants" }
    fn html_dir(&self) -> &'static str { "quests/Q00326_VanquishRemnants" }
    fn start_npcs(&self) -> &[i32] { &[LEOPOLD] }
    fn talk_npcs(&self) -> &[i32] { &[LEOPOLD] }
    fn kill_npcs(&self) -> &[i32] { &KILL_NPCS }
    fn quest_items(&self) -> &[i32] { &[RED_CROSS_BADGE, BLUE_CROSS_BADGE, BLACK_CROSS_BADGE] }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 30).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() { return None; }
        match event {
            "30435-03.htm" => { ctx.start_quest(); Some(event.to_string()) }
            "30435-07.html" => { ctx.exit_quest(true, true); Some(event.to_string()) }
            "30435-08.html" => Some(event.to_string()),
            _ => None,
        }
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_started() {
            if let Some((chance, badge)) = drop(ctx.npc_id) {
                if ctx.roll(100) < chance { ctx.give_items(badge, 1); }
            }
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= 21 { "30435-02.htm" } else { "30435-01.htm" }.to_string());
        }
        if ctx.is_started() {
            let red = ctx.quest_items_count(RED_CROSS_BADGE);
            let blue = ctx.quest_items_count(BLUE_CROSS_BADGE);
            let black = ctx.quest_items_count(BLACK_CROSS_BADGE);
            let sum = red + blue + black;
            if sum > 0 {
                if sum >= 100 && ctx.quest_items_count(BLACK_LION_MARK) == 0 {
                    ctx.give_items(BLACK_LION_MARK, 1);
                }
                let bonus = if sum >= 10 { 1000 } else { 0 };
                ctx.give_adena(red * 10 + blue * 10 + black * 12 + bonus, true);
                ctx.take_items(RED_CROSS_BADGE, -1);
                ctx.take_items(BLUE_CROSS_BADGE, -1);
                ctx.take_items(BLACK_CROSS_BADGE, -1);
                // Java's `06` (mark just earned) is unreachable — the mark is
                // given above, so `hasQuestItems` is true → `09`.
                return Some(if sum >= 100 { "30435-09.html" } else { "30435-05.html" }.to_string());
            }
            return Some("30435-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
