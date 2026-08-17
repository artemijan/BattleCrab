//! Arrow of Vengeance (331) — `quests/Q00331_ArrowOfVengeance`. Belton (30125,
//! level 32–39) buys Harpy Feathers / Medusa Venom / Wyrm's Teeth. Single cond,
//! turn-in separate from leaving.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const BELTON: i32 = 30125;
const HARPY_FEATHER: i32 = 1452;
const MEDUSA_VENOM: i32 = 1453;
const WYRMS_TOOTH: i32 = 1454;
const MIN_LEVEL: i32 = 32;
const KILL_NPCS: [i32; 3] = [20145, 20158, 20176];
fn chance(npc_id: i32) -> i32 {
    match npc_id {
        20145 => 59,
        20158 => 61,
        20176 => 60,
        _ => 0,
    }
}
pub struct Q00331ArrowOfVengeance;
impl QuestScript for Q00331ArrowOfVengeance {
    fn id(&self) -> i32 {
        331
    }
    fn name(&self) -> &'static str {
        "Q00331_ArrowOfVengeance"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00331_ArrowOfVengeance"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BELTON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[BELTON]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[HARPY_FEATHER, MEDUSA_VENOM, WYRMS_TOOTH]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 39).then(|| ctx.no_quest_html())
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < MIN_LEVEL {
                    "30125-01.htm"
                } else {
                    "30125-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let feathers = ctx.quest_items_count(HARPY_FEATHER);
            let venoms = ctx.quest_items_count(MEDUSA_VENOM);
            let teeth = ctx.quest_items_count(WYRMS_TOOTH);
            if feathers + venoms + teeth > 0 {
                let bonus = if feathers + venoms + teeth >= 10 {
                    1000
                } else {
                    0
                };
                ctx.give_adena(feathers * 6 + venoms * 7 + teeth * 9 + bonus, true);
                ctx.take_items(HARPY_FEATHER, -1);
                ctx.take_items(MEDUSA_VENOM, -1);
                ctx.take_items(WYRMS_TOOTH, -1);
                return Some("30125-05.html".to_string());
            }
            return Some("30125-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30125-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30125-06.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30125-07.html" => Some(event.to_string()),
            _ => None,
        }
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        if ctx.roll(100) < chance(ctx.npc_id) {
            let item = match ctx.npc_id {
                20145 => HARPY_FEATHER,
                20158 => MEDUSA_VENOM,
                20176 => WYRMS_TOOTH,
                _ => return,
            };
            ctx.give_items(item, 1);
        }
    }
}
