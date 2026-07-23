//! Invaders of the Holy Land (273) — port of
//! `dist/game/data/scripts/quests/Q00273_InvadersOfTheHolyLand/`. Orc only:
//! Varkees buys soulstones off Rakeclaw Imps — black (3a) at the
//! per-monster chance, red (5a) otherwise; +1000 bonus for 10+ total.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const VARKEES: i32 = 30566;
const BLACK_SOULSTONE: i32 = 1475;
const RED_SOULSTONE: i32 = 1476;
/// monster id → black-soulstone chance (out of 100).
const MONSTERS: [(i32, i32); 3] = [(20311, 90), (20312, 87), (20313, 77)];
const MIN_LEVEL: i32 = 6;
const RACE_ORC: i32 = 3;

pub struct Q00273InvadersOfTheHolyLand;

impl QuestScript for Q00273InvadersOfTheHolyLand {
    fn id(&self) -> i32 {
        273
    }
    fn name(&self) -> &'static str {
        "Q00273_InvadersOfTheHolyLand"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00273_InvadersOfTheHolyLand"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VARKEES]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VARKEES]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 3] = [20311, 20312, 20313];
        &IDS
    }
    fn quest_items(&self) -> &[i32] {
        &[BLACK_SOULSTONE, RED_SOULSTONE]
    }

    /// `addCondMaxLevel(14, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 14).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30566-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30566-08.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30566-09.html" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == ctx.npc_id)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if ctx.roll(100) <= chance {
            ctx.give_items(BLACK_SOULSTONE, 1);
        } else {
            ctx.give_items(RED_SOULSTONE, 1);
        }
        ctx.play_sound(quest_sounds::ITEMGET);
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30566-01.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30566-03.htm"
                } else {
                    "30566-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let black = ctx.quest_items_count(BLACK_SOULSTONE);
            let red = ctx.quest_items_count(RED_SOULSTONE);
            return Some(if black + red > 0 {
                ctx.give_adena(
                    (red * 5) + (black * 3) + if red + black >= 10 { 1000 } else { 0 },
                    true,
                );
                ctx.take_items(BLACK_SOULSTONE, -1);
                ctx.take_items(RED_SOULSTONE, -1);
                if red > 0 {
                    "30566-07.html"
                } else {
                    "30566-06.html"
                }
                .to_string()
            } else {
                "30566-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }
}
