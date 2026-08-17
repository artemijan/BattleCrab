//! Meeting the Elroki (124) — `quests/Q00124_MeetingTheElroki`. The Primeval
//! Isle follow-up to [`Q00110`](super::q00110_to_the_primeval_isle): Marquez
//! (32113, level 75+) sends the player around the Elroki village — Mushika →
//! Asamah → Karakawei → Mantarasa — a pure dialog chain (`cond` 1 → 6, no
//! kills) that ends with a Mantarasa Egg and Asamah's 100013-adena reward.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const MARQUEZ: i32 = 32113;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const KARAKAWEI: i32 = 32117;
const MANTARASA: i32 = 32118;
const MANTARASA_EGG: i32 = 8778;

pub struct Q00124MeetingTheElroki;

impl QuestScript for Q00124MeetingTheElroki {
    fn id(&self) -> i32 {
        124
    }
    fn name(&self) -> &'static str {
        "Q00124_MeetingTheElroki"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00124_MeetingTheElroki"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MARQUEZ]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MARQUEZ, MUSHIKA, ASAMAH, KARAKAWEI, MANTARASA]
    }
    fn quest_items(&self) -> &[i32] {
        &[MANTARASA_EGG]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            MARQUEZ => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() < 75 {
                            "32113-01a.htm"
                        } else {
                            "32113-01.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_completed() {
                    return Some(ctx.already_completed_html());
                }
                if ctx.is_started() {
                    return Some(
                        match ctx.cond() {
                            1 => "32113-05.html",
                            2 => "32113-06.html",
                            3..=5 => "32113-07.html",
                            _ => return Some(ctx.no_quest_html()),
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            MUSHIKA => {
                if ctx.is_started() {
                    return Some(
                        match ctx.cond() {
                            1 => "32114-01.html",
                            2 => "32114-02.html",
                            _ => "32114-03.html",
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            ASAMAH => {
                if ctx.is_started() {
                    match ctx.cond() {
                        1 | 2 => return Some("32115-01.html".to_string()),
                        3 => return Some("32115-02.html".to_string()),
                        4 => return Some("32115-07.html".to_string()),
                        5 => return Some("32115-08.html".to_string()),
                        6 if ctx.quest_items_count(MANTARASA_EGG) > 0 => {
                            ctx.give_adena(100013, true);
                            ctx.add_exp_and_sp(301922, 30294);
                            ctx.exit_quest(false, true);
                            return Some("32115-09.html".to_string());
                        }
                        _ => {}
                    }
                }
                Some(ctx.no_quest_html())
            }
            KARAKAWEI => {
                if ctx.is_started() {
                    return Some(
                        match ctx.cond() {
                            1..=3 => "32117-01.html",
                            4 => "32117-02.html",
                            5 => "32117-07.html",
                            6 => "32117-06.html",
                            _ => return Some(ctx.no_quest_html()),
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            MANTARASA => {
                if ctx.is_started() {
                    return Some(
                        match ctx.cond() {
                            1..=4 => "32118-01.html",
                            5 => "32118-03.html",
                            6 => "32118-02.html",
                            _ => return Some(ctx.no_quest_html()),
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "32113-03.html" => ctx.start_quest(),
            "32113-04.html" if ctx.is_cond(1) => {
                ctx.set_cond(2, true);
            }
            "32114-04.html" if ctx.is_cond(2) => {
                ctx.set_cond(3, true);
            }
            "32115-06.html" if ctx.is_cond(3) => {
                ctx.set_cond(4, true);
            }
            "32117-05.html" if ctx.is_cond(4) => {
                ctx.set_cond(5, true);
            }
            "32118-04.html" if ctx.is_cond(5) => {
                ctx.give_items(MANTARASA_EGG, 1);
                ctx.set_cond(6, true);
            }
            _ => {}
        }
        Some(event.to_string())
    }
}
