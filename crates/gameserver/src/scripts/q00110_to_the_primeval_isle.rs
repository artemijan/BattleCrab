//! To the Primeval Isle (110) — `quests/Q00110_ToThePrimevalIsle`. Anton (31338,
//! level 75+) gives an Ancient Book to carry to Marquez (32113) on the Primeval
//! Isle, who pays 189208 adena + XP/SP. A one-time delivery, no kills.
//! `addCondMinLevel(75, "")` — the empty refusal html shows nothing.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const ANTON: i32 = 31338;
const MARQUEZ: i32 = 32113;
const ANCIENT_BOOK: i32 = 8777;
const MIN_LEVEL: i32 = 75;

pub struct Q00110ToThePrimevalIsle;

impl QuestScript for Q00110ToThePrimevalIsle {
    fn id(&self) -> i32 {
        110
    }
    fn name(&self) -> &'static str {
        "Q00110_ToThePrimevalIsle"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00110_ToThePrimevalIsle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ANTON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ANTON, MARQUEZ]
    }
    fn quest_items(&self) -> &[i32] {
        &[ANCIENT_BOOK]
    }

    /// `addCondMinLevel(75, "")` — refuse below 75 with an empty html (which
    /// `show_result` renders as nothing).
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() < MIN_LEVEL).then(String::new)
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == ANTON {
                return Some("31338-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_started() {
            if ctx.npc_id == ANTON && ctx.is_cond(1) {
                // Java points at 32113-06.html (a Marquez-prefixed page the dist
                // does not ship — kept verbatim; retail 404s to a blank window).
                return Some("32113-06.html".to_string());
            }
            if ctx.npc_id == MARQUEZ && ctx.is_cond(1) {
                return Some("32113-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "31338-03.htm" | "31338-04.htm" | "32113-02.html" | "32113-03.html" => {
                Some(event.to_string())
            }
            "31338-05.html" => {
                // Java shows no page here — it just hands over the book and starts.
                ctx.give_items(ANCIENT_BOOK, 1);
                ctx.start_quest();
                None
            }
            "32113-04.html" | "32113-05.html" => {
                if !ctx.is_cond(1) {
                    return None;
                }
                if ctx.player_level() >= MIN_LEVEL {
                    ctx.give_adena(189208, true);
                    ctx.add_exp_and_sp(887732, 213);
                    ctx.exit_quest(false, true);
                    None
                } else {
                    // `getNoQuestLevelRewardMsg` — a system message; unreachable
                    // in practice (75+ required to start).
                    Some(ctx.no_quest_html())
                }
            }
            _ => None,
        }
    }
}
