//! Bonds of Slavery (265) — port of
//! `dist/game/data/scripts/quests/Q00265_BondsOfSlavery/`. Dark Elf only:
//! Kristin buys imp shackles (5 adena each, +500 for 10+); Imps drop them
//! at a per-monster chance out of 10.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const KRISTIN: i32 = 30357;
const IMP_SHACKLES: i32 = 1368;
const MONSTERS: [(i32, i32); 2] = [(20004, 5), (20005, 6)]; // Imp, Imp Elder → chance /10
const MIN_LEVEL: i32 = 6;
const RACE_DARK_ELF: i32 = 2;

pub struct Q00265BondsOfSlavery;

impl QuestScript for Q00265BondsOfSlavery {
    fn id(&self) -> i32 {
        265
    }
    fn name(&self) -> &'static str {
        "Q00265_BondsOfSlavery"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00265_BondsOfSlavery"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KRISTIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KRISTIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 2] = [20004, 20005];
        &IDS
    }
    fn quest_items(&self) -> &[i32] {
        &[IMP_SHACKLES]
    }

    /// `addCondMaxLevel(11, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 11).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30357-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30357-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30357-08.html" => Some(event.to_string()),
            _ => None,
        }
    }

    /// Per-monster `getRandom(10) < chance` — note Java gates only on the
    /// quest state existing (no cond check), kept as-is.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == ctx.npc_id)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if ctx.roll(10) < chance {
            ctx.give_items(IMP_SHACKLES, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_DARK_ELF {
                    "30357-01.html"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30357-03.htm"
                } else {
                    "30357-02.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let shackles = ctx.quest_items_count(IMP_SHACKLES);
            return Some(if shackles > 0 {
                ctx.give_adena((shackles * 5) + if shackles >= 10 { 500 } else { 0 }, true);
                ctx.take_items(IMP_SHACKLES, -1);
                "30357-06.html".to_string()
            } else {
                "30357-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }
}
