//! Collect Spores (313) — port of
//! `dist/game/data/scripts/quests/Q00313_CollectSpores/`. Herbiel wants 10
//! spore sacs from Spore Fungus (40% drop); reward 500 adena.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const HERBIEL: i32 = 30150;
const SPORE_SAC: i32 = 1118;
const SPORE_FUNGUS: i32 = 20509;
const MIN_LEVEL: i32 = 8;
const REQUIRED_SAC_COUNT: i64 = 10;

pub struct Q00313CollectSpores;

impl QuestScript for Q00313CollectSpores {
    fn id(&self) -> i32 {
        313
    }
    fn name(&self) -> &'static str {
        "Q00313_CollectSpores"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00313_CollectSpores"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HERBIEL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[HERBIEL]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[SPORE_FUNGUS]
    }
    fn quest_items(&self) -> &[i32] {
        &[SPORE_SAC]
    }

    /// `addCondMaxLevel(13, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 13).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30150-03.htm"
                } else {
                    "30150-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 if ctx.quest_items_count(SPORE_SAC) < REQUIRED_SAC_COUNT => {
                    return Some("30150-06.html".to_string());
                }
                2 if ctx.quest_items_count(SPORE_SAC) >= REQUIRED_SAC_COUNT => {
                    ctx.give_adena(500, true);
                    ctx.exit_quest(true, true);
                    return Some("30150-07.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30150-05.htm" if ctx.is_created() => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30150-04.htm" => Some(event.to_string()),
            _ => None,
        }
    }

    /// The `ALT_PARTY_RANGE` corpse-distance check drops out killer-only.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs()
            && ctx.is_cond(1)
            && ctx.give_item_randomly(SPORE_SAC, 1, REQUIRED_SAC_COUNT, 0.4, true)
        {
            ctx.set_cond(2, false);
        }
    }
}
