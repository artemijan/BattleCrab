//! Bring Wolf Pelts (258) — port of
//! `dist/game/data/scripts/quests/Q00258_BringWolfPelts/`. Talking Island:
//! Lector wants 40 wolf pelts; wolves drop one per kill; the turn-in reward
//! is rolled from a small table.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const LECTOR: i32 = 30001;
const WOLF_PELT: i32 = 702;
const MONSTERS: [i32; 2] = [20120, 20442]; // Wolf, Elder Wolf
/// (reward item, roll upper bound) — Java takes the first HashMap entry
/// with `getRandom(16) < value`, so its per-item odds depend on the map's
/// arbitrary iteration order. Ascending-bound order is the deterministic
/// reading: roll < 1 → Cloth Cap, < 6 → Leather Cap, < 9 → Stockings,
/// else nothing.
const REWARDS: [(i32, i32); 3] = [(41, 1), (42, 6), (462, 9)];
const MIN_LEVEL: i32 = 3;
const WOLF_PELT_COUNT: i64 = 40;

pub struct Q00258BringWolfPelts;

impl QuestScript for Q00258BringWolfPelts {
    fn id(&self) -> i32 {
        258
    }

    fn name(&self) -> &'static str {
        "Q00258_BringWolfPelts"
    }

    fn html_dir(&self) -> &'static str {
        "quests/Q00258_BringWolfPelts"
    }

    fn start_npcs(&self) -> &[i32] {
        &[LECTOR]
    }

    fn talk_npcs(&self) -> &[i32] {
        &[LECTOR]
    }

    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }

    fn quest_items(&self) -> &[i32] {
        &[WOLF_PELT]
    }

    /// `addCondMaxLevel(9, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 9).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL { "30001-02.htm" } else { "30001-01.html" }.to_string());
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30001-04.html".to_string()),
                2 if ctx.quest_items_count(WOLF_PELT) >= WOLF_PELT_COUNT => {
                    let chance = ctx.roll(16);
                    for (item_id, bound) in REWARDS {
                        if chance < bound {
                            ctx.give_items(item_id, 1);
                            break;
                        }
                    }
                    ctx.exit_quest(true, true);
                    return Some("30001-05.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30001-03.html") {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) {
            ctx.give_items(WOLF_PELT, 1);
            if ctx.quest_items_count(WOLF_PELT) >= WOLF_PELT_COUNT {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(crate::network::server_packets::quest_sounds::ITEMGET);
            }
        }
    }
}
