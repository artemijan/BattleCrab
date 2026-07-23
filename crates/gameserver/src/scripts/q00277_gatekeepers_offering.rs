//! Gatekeeper's Offering (277) — port of
//! `dist/game/data/scripts/quests/Q00277_GatekeepersOffering/`. Tamil (30576)
//! wants **20 Starstones** off the Greystone Golems; the reward is **2
//! Gatekeeper Charms** (teleport tokens). Repeatable, level 15–21. The
//! minimum-level check lives in the **start event**, not the talk.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const TAMIL: i32 = 30576;
const GREYSTONE_GOLEM: i32 = 20333;
const STARSTONE: i32 = 1572;
const GATEKEEPER_CHARM: i32 = 1658;
const MIN_LEVEL: i32 = 15;
const STARSTONE_COUNT: i64 = 20;

pub struct Q00277GatekeepersOffering;

impl QuestScript for Q00277GatekeepersOffering {
    fn id(&self) -> i32 {
        277
    }
    fn name(&self) -> &'static str {
        "Q00277_GatekeepersOffering"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00277_GatekeepersOffering"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TAMIL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TAMIL]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[GREYSTONE_GOLEM]
    }
    fn quest_items(&self) -> &[i32] {
        &[STARSTONE]
    }

    /// `addCondMaxLevel(21, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30576-03.htm") {
            // The min-level gate is in the start event here, not the talk.
            if ctx.player_level() < MIN_LEVEL {
                return Some("30576-01.htm".to_string());
            }
            ctx.start_quest();
            return Some("30576-03.htm".to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_started() && ctx.quest_items_count(STARSTONE) < STARSTONE_COUNT {
            ctx.give_items(STARSTONE, 1);
            if ctx.quest_items_count(STARSTONE) >= STARSTONE_COUNT {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some("30576-02.htm".to_string());
        }
        if ctx.is_started() {
            if ctx.is_cond(1) {
                return Some("30576-04.html".to_string());
            }
            if ctx.is_cond(2) && ctx.quest_items_count(STARSTONE) >= STARSTONE_COUNT {
                ctx.give_items(GATEKEEPER_CHARM, 2);
                ctx.exit_quest(true, true);
                return Some("30576-05.html".to_string());
            }
        }
        Some(ctx.no_quest_html())
    }
}
