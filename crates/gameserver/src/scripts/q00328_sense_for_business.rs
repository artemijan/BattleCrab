//! Sense for Business (328) — `quests/Q00328_SenseForBusiness`. Sarien (30436,
//! level 21+) buys Monster Eye Carcasses/Lenses and Basilisk Gizzards. Single
//! cond; the turn-in is separate from leaving. No max-level gate.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const SARIEN: i32 = 30436;
const CARCASS: i32 = 1347;
const LENS: i32 = 1366;
const GIZZARD: i32 = 1348;
const MIN_LEVEL: i32 = 21;
const KILL_NPCS: [i32; 6] = [20055, 20059, 20067, 20068, 20070, 20072];
/// Monster Eye: `roll < [0]` → carcass, else `roll < [1]` → lens.
fn eye_thresholds(npc_id: i32) -> Option<(i32, i32)> {
    match npc_id {
        20055 | 20059 => Some((61, 62)),
        20067 => Some((72, 74)),
        20068 => Some((78, 79)),
        _ => None,
    }
}
/// Basilisk: `roll < chance` → gizzard.
fn basilisk_chance(npc_id: i32) -> Option<i32> {
    match npc_id {
        20070 => Some(60),
        20072 => Some(63),
        _ => None,
    }
}
pub struct Q00328SenseForBusiness;
impl QuestScript for Q00328SenseForBusiness {
    fn id(&self) -> i32 {
        328
    }
    fn name(&self) -> &'static str {
        "Q00328_SenseForBusiness"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00328_SenseForBusiness"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SARIEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[SARIEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[CARCASS, LENS, GIZZARD]
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < MIN_LEVEL {
                    "30436-01.htm"
                } else {
                    "30436-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let carcass = ctx.quest_items_count(CARCASS);
            let lens = ctx.quest_items_count(LENS);
            let gizzards = ctx.quest_items_count(GIZZARD);
            if carcass + lens + gizzards > 0 {
                let bonus = if carcass + lens + gizzards >= 10 {
                    100
                } else {
                    0
                };
                ctx.give_adena(carcass * 2 + lens * 10 + gizzards * 2 + bonus, true);
                ctx.take_items(CARCASS, -1);
                ctx.take_items(LENS, -1);
                ctx.take_items(GIZZARD, -1);
                return Some("30436-05.html".to_string());
            }
            return Some("30436-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30436-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30436-06.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let chance = ctx.roll(100);
        if let Some((c, l)) = eye_thresholds(ctx.npc_id) {
            if chance < c {
                ctx.give_items(CARCASS, 1);
            } else if chance < l {
                ctx.give_items(LENS, 1);
            }
        } else if let Some(g) = basilisk_chance(ctx.npc_id)
            && chance < g
        {
            ctx.give_items(GIZZARD, 1);
        }
    }
}
