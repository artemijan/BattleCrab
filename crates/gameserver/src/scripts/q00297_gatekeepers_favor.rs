//! Gatekeeper's Favor (297) — `quests/Q00297_GatekeepersFavor`. Wirphy (30540)
//! wants 20 Starstones off the Whinstone Golems for 2 Gatekeeper Tokens.
//! Repeatable, level 15–21; the min-level check is in the start event.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
const WIRPHY: i32 = 30540;
const WHINSTONE_GOLEM: i32 = 20521;
const STARSTONE: i32 = 1573;
const GATEKEEPER_TOKEN: i32 = 736;
const MIN_LEVEL: i32 = 15;
const STARSTONE_COUNT: i64 = 20;
pub struct Q00297GatekeepersFavor;
impl QuestScript for Q00297GatekeepersFavor {
    fn id(&self) -> i32 { 297 }
    fn name(&self) -> &'static str { "Q00297_GatekeepersFavor" }
    fn html_dir(&self) -> &'static str { "quests/Q00297_GatekeepersFavor" }
    fn start_npcs(&self) -> &[i32] { &[WIRPHY] }
    fn talk_npcs(&self) -> &[i32] { &[WIRPHY] }
    fn kill_npcs(&self) -> &[i32] { &[WHINSTONE_GOLEM] }
    fn quest_items(&self) -> &[i32] { &[STARSTONE] }
    /// `addCondMaxLevel(21, "30540-01.htm")` — a specific refusal page.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| "30540-01.htm".to_string())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30540-03.htm") {
            if ctx.player_level() < MIN_LEVEL { return Some("30540-01.htm".to_string()); }
            ctx.start_quest();
            return Some("30540-03.htm".to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_started() && ctx.quest_items_count(STARSTONE) < STARSTONE_COUNT {
            ctx.give_items(STARSTONE, 1);
            if ctx.quest_items_count(STARSTONE) >= STARSTONE_COUNT { ctx.set_cond(2, true); }
            else { ctx.play_sound(quest_sounds::ITEMGET); }
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() { return Some("30540-02.htm".to_string()); }
        if ctx.is_started() {
            if ctx.is_cond(1) { return Some("30540-04.html".to_string()); }
            if ctx.is_cond(2) && ctx.quest_items_count(STARSTONE) >= STARSTONE_COUNT {
                ctx.give_items(GATEKEEPER_TOKEN, 2);
                ctx.exit_quest(true, true);
                return Some("30540-05.html".to_string());
            }
        }
        Some(ctx.no_quest_html())
    }
}
