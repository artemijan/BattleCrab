//! Plunder Their Supplies (360) — `quests/Q00360_PlunderTheirSupplies`. Coleman
//! (30873, level 52–59) buys Taik Orc Supply Items; 500 of them fetch 14000
//! adena. Turn-in is repeatable; leaving (`30873-10`) is a one-time exit.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
const COLEMAN: i32 = 30873;
const SUPPLY_ITEMS: i32 = 5872;
const MIN_LEVEL: i32 = 52;
const REQUIRED: i64 = 500;
const KILL_NPCS: [i32; 2] = [20666, 20669];
fn chance(npc_id: i32) -> i32 {
    match npc_id { 20666 => 50, 20669 => 75, _ => 0 }
}
pub struct Q00360PlunderTheirSupplies;
impl QuestScript for Q00360PlunderTheirSupplies {
    fn id(&self) -> i32 { 360 }
    fn name(&self) -> &'static str { "Q00360_PlunderTheirSupplies" }
    fn html_dir(&self) -> &'static str { "quests/Q00360_PlunderTheirSupplies" }
    fn start_npcs(&self) -> &[i32] { &[COLEMAN] }
    fn talk_npcs(&self) -> &[i32] { &[COLEMAN] }
    fn kill_npcs(&self) -> &[i32] { &KILL_NPCS }
    fn quest_items(&self) -> &[i32] { &[SUPPLY_ITEMS] }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 59).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() { return None; }
        match event {
            "30873-04.htm" => { ctx.start_quest(); Some(event.to_string()) }
            "30873-10.html" => { ctx.exit_quest(false, true); Some(event.to_string()) }
            "30873-03.htm" | "30873-09.html" => Some(event.to_string()),
            _ => None,
        }
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.roll(100) < chance(ctx.npc_id) {
            ctx.give_items(SUPPLY_ITEMS, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL { "30873-02.htm" } else { "30873-01.html" }.to_string());
        }
        if ctx.is_started() {
            if ctx.quest_items_count(SUPPLY_ITEMS) >= REQUIRED {
                ctx.give_adena(14000, true);
                ctx.take_items(SUPPLY_ITEMS, -1);
                return Some("30873-06.html".to_string());
            }
            return Some("30873-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
