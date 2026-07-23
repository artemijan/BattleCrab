//! Collector's Dream (261) — port of
//! `dist/game/data/scripts/quests/Q00261_CollectorsDream/`. Alshupes (30222)
//! in the Dwarven Village wants 8 spider legs from the three Gludio spiders;
//! reward 700 adena. Repeatable, level 15–21.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const ALSHUPES: i32 = 30222;
// Hook / Crimson / Pincer Spider.
const MONSTERS: [i32; 3] = [20308, 20460, 20466];
const SPIDER_LEG: i32 = 1087;
const MIN_LEVEL: i32 = 15;
const MAX_LEG_COUNT: i64 = 8;

pub struct Q00261CollectorsDream;

impl QuestScript for Q00261CollectorsDream {
    fn id(&self) -> i32 {
        261
    }
    fn name(&self) -> &'static str {
        "Q00261_CollectorsDream"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00261_CollectorsDream"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ALSHUPES]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ALSHUPES]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[SPIDER_LEG]
    }

    /// `addCondMaxLevel(21, getNoQuestMsg(null))` — a newbie quest, refused
    /// above level 21.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30222-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    /// Java gates on `checkIfInRange(ALT_PARTY_RANGE, npc, killer)`; the port
    /// credits the killer only (the G11 party deviation), which reduces to the
    /// cond-1 gate.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) && ctx.give_item_randomly(SPIDER_LEG, 1, MAX_LEG_COUNT, 1.0, true) {
            ctx.set_cond(2, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL { "30222-02.htm" } else { "30222-01.htm" }.to_string());
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30222-04.html".to_string()),
                2 if ctx.quest_items_count(SPIDER_LEG) >= MAX_LEG_COUNT => {
                    // TODO(newbie-guide): Java also calls `giveNewbieReward`
                    // (sets the `GUIDE_MISSION` player variable + an
                    // ExShowScreenMessage "last duty complete"). Deferred with
                    // the newbie-guide mission system — nothing reads
                    // GUIDE_MISSION in the port yet and ExShowScreenMessage is
                    // unported, so the tracking would be inert.
                    ctx.give_adena(700, true);
                    ctx.exit_quest(true, true);
                    return Some("30222-05.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
