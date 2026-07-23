//! Obtain a Wolf Pet (210) — port of
//! `dist/game/data/scripts/quests/Q00210_ObtainAWolfPet/`. A pure dialog
//! chain: Lundy (30827, Gludin) sends you round three NPCs — Bella (30256)
//! → Bynn (30335) → Sydnia (30321) — and back to Lundy, who hands over the
//! **Wolf Collar** (2375), the summon item for the starter wolf pet (G29).
//!
//! One-time (`exitQuest(false)`), minimum level 15. The named
//! `Quest Q00210_ObtainAWolfPet` bypass drives every button, so the html
//! filenames are load-bearing.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const LUNDY: i32 = 30827;
const BELLA: i32 = 30256;
const BYNN: i32 = 30335;
const SYDNIA: i32 = 30321;
const WOLF_COLLAR: i32 = 2375;
const MIN_LEVEL: i32 = 15;

pub struct Q00210ObtainAWolfPet;

impl QuestScript for Q00210ObtainAWolfPet {
    fn id(&self) -> i32 {
        210
    }
    fn name(&self) -> &'static str {
        "Q00210_ObtainAWolfPet"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00210_ObtainAWolfPet"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LUNDY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[LUNDY, BELLA, BYNN, SYDNIA]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Java bails to the raw event page when there is no quest state.
        if !ctx.has_qs() {
            return Some(event.to_string());
        }
        match event {
            // Plain pages — shown as-is (`30827-04.htm` is dead: no button
            // links it, but Java lists it, so it is kept).
            "30827-02.htm" | "30827-04.htm" | "30256-02.html" => {}
            "30827-03.htm" => ctx.start_quest(),
            "30256-03.html" => {
                if ctx.is_cond(1) {
                    ctx.set_cond(2, true);
                }
            }
            "30335-02.html" => {
                if ctx.is_cond(2) {
                    ctx.set_cond(3, true);
                }
            }
            "30321-02.html" => {
                if ctx.is_cond(3) {
                    ctx.set_cond(4, true);
                }
            }
            "30827-05.html" if ctx.is_cond(4) => {
                ctx.reward_items(WOLF_COLLAR, 1);
                ctx.exit_quest(false, true);
            }
            _ => {}
        }
        Some(event.to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            // Only Lundy opens the quest; `addCondMinLevel(15, "no_level.htm")`.
            if ctx.npc_id == LUNDY {
                return Some(
                    if ctx.player_level() < MIN_LEVEL {
                        "no_level.htm"
                    } else {
                        "30827-01.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_started() {
            return Some(
                match ctx.npc_id {
                    LUNDY if ctx.is_cond(1) || ctx.is_cond(2) => "30827-07.html",
                    LUNDY if ctx.is_cond(4) => "30827-04.html",
                    BELLA if ctx.is_cond(1) => "30256-01.html",
                    BYNN if ctx.is_cond(2) => "30335-01.html",
                    SYDNIA if ctx.is_cond(3) => "30321-01.html",
                    _ => return Some(ctx.no_quest_html()),
                }
                .to_string(),
            );
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        Some(ctx.no_quest_html())
    }
}
