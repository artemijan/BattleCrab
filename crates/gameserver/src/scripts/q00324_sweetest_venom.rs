//! Sweetest Venom (324) — port of
//! `dist/game/data/scripts/quests/Q00324_SweetestVenom/`. Astaron wants 10
//! venom sacs; three spider kinds drop them at per-monster chances; the
//! 10th sac bumps to cond 2 (with the quest-middle sound); reward 1000
//! adena.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const ASTARON: i32 = 30351;
const VENOM_SAC: i32 = 1077;
/// monster id → drop chance (out of 100).
const MONSTERS: [(i32, i32); 3] = [(20034, 26), (20038, 29), (20043, 30)];
const MIN_LEVEL: i32 = 18;
const REQUIRED_COUNT: i64 = 10;
const ADENA_COUNT: i64 = 1000;

pub struct Q00324SweetestVenom;

impl QuestScript for Q00324SweetestVenom {
    fn id(&self) -> i32 {
        324
    }
    fn name(&self) -> &'static str {
        "Q00324_SweetestVenom"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00324_SweetestVenom"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ASTARON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ASTARON]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 3] = [20034, 20038, 20043];
        &IDS
    }
    fn quest_items(&self) -> &[i32] {
        &[VENOM_SAC]
    }

    /// `addCondMaxLevel(23, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 23).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < MIN_LEVEL {
                    "30351-02.html"
                } else {
                    "30351-03.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(if ctx.is_cond(2) {
                ctx.give_adena(ADENA_COUNT, true);
                ctx.exit_quest(true, true);
                "30351-06.html".to_string()
            } else {
                "30351-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30351-04.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let sacs = ctx.quest_items_count(VENOM_SAC);
        let chance = MONSTERS
            .iter()
            .find(|(id, _)| *id == ctx.npc_id)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if sacs < REQUIRED_COUNT && ctx.roll(100) < chance {
            ctx.give_items(VENOM_SAC, 1);
            if sacs + 1 < REQUIRED_COUNT {
                ctx.play_sound(quest_sounds::ITEMGET);
            } else {
                ctx.set_cond(2, true);
            }
        }
    }
}
