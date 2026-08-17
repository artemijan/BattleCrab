//! Wrath of Verdure (267) — port of
//! `dist/game/data/scripts/quests/Q00267_WrathOfVerdure/`. Elf-only (level
//! 4–9): Treant Bremec (31853) pays a trickle of adena for Goblin Clubs off the
//! Goblin Raiders. Repeatable — and the turn-in is separate from leaving, so
//! you can hand clubs in as you go.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const TREANT_BREMEC: i32 = 31853;
const GOBLIN_CLUB: i32 = 1335;
const GOBLIN_RAIDER: i32 = 20325;
const RACE_ELF: i32 = 1;
const MIN_LEVEL: i32 = 4;

pub struct Q00267WrathOfVerdure;

impl QuestScript for Q00267WrathOfVerdure {
    fn id(&self) -> i32 {
        267
    }
    fn name(&self) -> &'static str {
        "Q00267_WrathOfVerdure"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00267_WrathOfVerdure"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TREANT_BREMEC]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TREANT_BREMEC]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[GOBLIN_RAIDER]
    }
    fn quest_items(&self) -> &[i32] {
        &[GOBLIN_CLUB]
    }

    /// `addCondMaxLevel(9, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 9).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ELF {
                    "31853-01.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "31853-03.htm"
                } else {
                    "31853-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let clubs = ctx.quest_items_count(GOBLIN_CLUB);
            if clubs > 0 {
                // Java's odd formula: 2 + the club count (not per-club).
                ctx.give_adena(2 + clubs, true);
                ctx.take_items(GOBLIN_CLUB, -1);
                return Some("31853-06.html".to_string());
            }
            return Some("31853-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31853-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31853-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "31853-08.html" => Some(event.to_string()),
            _ => None,
        }
    }

    /// A flat 50% club drop (`getRandom(10) < 5`, hand-rolled + plain give — not
    /// rate-multiplied).
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.roll(10) < 5 {
            ctx.give_items(GOBLIN_CLUB, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
