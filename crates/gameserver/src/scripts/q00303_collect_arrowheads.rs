//! Collect Arrowheads (303) — port of
//! `dist/game/data/scripts/quests/Q00303_CollectArrowheads/`. Minia in the
//! Orc village wants 10 orcish arrowheads from Tunath Orc Marksmen (40%
//! drop); reward 500 adena.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const MINIA: i32 = 30029;
const ORCISH_ARROWHEAD: i32 = 963;
const TUNATH_ORC_MARKSMAN: i32 = 20361;
const MIN_LEVEL: i32 = 10;
const REQUIRED_ITEM_COUNT: i64 = 10;

pub struct Q00303CollectArrowheads;

impl QuestScript for Q00303CollectArrowheads {
    fn id(&self) -> i32 {
        303
    }
    fn name(&self) -> &'static str {
        "Q00303_CollectArrowheads"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00303_CollectArrowheads"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MINIA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MINIA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[TUNATH_ORC_MARKSMAN]
    }
    fn quest_items(&self) -> &[i32] {
        &[ORCISH_ARROWHEAD]
    }

    /// `addCondMaxLevel(14, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 14).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30029-04.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    /// `getRandomPartyMember(player, 1)` — a random cond-1 party member in
    /// range collects the arrowhead (`retarget_random_party_member`).
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.retarget_random_party_member(1, 1)
            && ctx.give_item_randomly(ORCISH_ARROWHEAD, 1, REQUIRED_ITEM_COUNT, 0.4, true)
        {
            ctx.set_cond(2, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30029-03.htm"
                } else {
                    "30029-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 if ctx.quest_items_count(ORCISH_ARROWHEAD) < REQUIRED_ITEM_COUNT => {
                    return Some("30029-05.html".to_string());
                }
                2 if ctx.quest_items_count(ORCISH_ARROWHEAD) >= REQUIRED_ITEM_COUNT => {
                    ctx.give_adena(500, true);
                    ctx.exit_quest(true, true);
                    return Some("30029-06.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
