//! Covert Business (294) — `quests/Q00294_CovertBusiness`. Dwarf-only: Keef
//! (30534) wants 100 Bat Fangs for the Ring of Raccoon (200 adena on a repeat).
//! Level 10–16. The per-mob table gives *more* fangs for a *lower* roll.
use crate::game_loop::quests::{QuestCtx, QuestScript};
const KEEF: i32 = 30534;
const BAT_FANG: i32 = 1491;
const RING_OF_RACCOON: i32 = 1508;
const RACE_DWARF: i32 = 4;
const MIN_LEVEL: i32 = 10;
const REQUIRED: i64 = 100;
const KILL_NPCS: [i32; 2] = [20370, 20480];
/// `getRandom(10)`; the count is the 1-based index of the first entry with
/// `chance > i` — so a low roll pays more fangs.
fn amount(npc_id: i32, chance: i32) -> Option<i64> {
    let table: &[i32] = match npc_id {
        20370 => &[6, 3, 1, -1],
        20480 => &[5, 2, -1],
        _ => return None,
    };
    for (i, &t) in table.iter().enumerate() {
        if chance > t {
            return Some((i + 1) as i64);
        }
    }
    None
}
pub struct Q00294CovertBusiness;
impl QuestScript for Q00294CovertBusiness {
    fn id(&self) -> i32 {
        294
    }
    fn name(&self) -> &'static str {
        "Q00294_CovertBusiness"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00294_CovertBusiness"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KEEF]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KEEF]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[BAT_FANG]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 16).then(|| ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && ctx.is_created() && event == "30534-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) {
            let roll = ctx.roll(10);
            if let Some(count) = amount(ctx.npc_id, roll)
                && ctx.give_item_randomly(BAT_FANG, count, REQUIRED, 1.0, true)
            {
                ctx.set_cond(2, false);
            }
        }
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_DWARF {
                    "30534-00.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30534-02.htm"
                } else {
                    "30534-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.is_cond(2) {
                let html = if ctx.quest_items_count(RING_OF_RACCOON) > 0 {
                    ctx.give_adena(200, true);
                    "30534-06.html"
                } else {
                    ctx.give_items(RING_OF_RACCOON, 1);
                    "30534-05.html"
                };
                ctx.take_items(BAT_FANG, -1);
                ctx.exit_quest(true, true);
                return Some(html.to_string());
            }
            return Some("30534-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
