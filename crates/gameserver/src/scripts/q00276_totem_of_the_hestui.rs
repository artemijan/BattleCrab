//! Totem of the Hestui (276) — `quests/Q00276_TotemOfTheHestui`. Tanapi (30571)
//! sends an Orc (level 15–21) to hunt Kasha Bears. Their parasites accumulate,
//! and the more you carry the likelier a kill conjures a **Kasha Bear Totem**
//! (a weighted ladder keyed on parasite count); slaying the totem yields the
//! Kasha Crystal that finishes the quest. `addCondMaxLevel(21)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const TANAPI: i32 = 30571;
const KASHA_PARASITE: i32 = 1480;
const KASHA_CRYSTAL: i32 = 1481;
const KASHA_BEAR: i32 = 20479;
const KASHA_BEAR_TOTEM: i32 = 27044;
// rewardItems ×1 each (Leather Shirt 29, adena-value token 1500).
const REWARDS: [i32; 2] = [29, 1500];
const MIN_LEVEL: i32 = 15;
const RACE_ORC: i32 = 3;
// SPAWN_CHANCES: (parasite_threshold, max roll /100) — checked high→low, so a
// bigger hoard both unlocks and steepens the totem's spawn odds.
const SPAWN_CHANCES: [(i64, i32); 5] = [(79, 100), (69, 20), (59, 15), (49, 10), (39, 2)];

pub struct Q00276TotemOfTheHestui;

impl QuestScript for Q00276TotemOfTheHestui {
    fn id(&self) -> i32 {
        276
    }
    fn name(&self) -> &'static str {
        "Q00276_TotemOfTheHestui"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00276_TotemOfTheHestui"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TANAPI]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[TANAPI]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[KASHA_BEAR, KASHA_BEAR_TOTEM]
    }
    fn quest_items(&self) -> &[i32] {
        &[KASHA_PARASITE, KASHA_CRYSTAL]
    }

    /// `addCondMaxLevel(21, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30571-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `Util.checkIfInRange(ALT_PARTY_RANGE, killer, npc, true)` is trivially
        // true for the killer; port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        match ctx.npc_id {
            KASHA_BEAR => {
                let parasites = ctx.quest_items_count(KASHA_PARASITE);
                let roll = ctx.roll(100);
                let mut spawned = false;
                for (threshold, max) in SPAWN_CHANCES {
                    if parasites >= threshold && roll <= max {
                        ctx.spawn_near_npc(KASHA_BEAR_TOTEM, false);
                        ctx.take_items(KASHA_PARASITE, -1);
                        spawned = true;
                        break;
                    }
                }
                if !spawned {
                    ctx.give_item_randomly(KASHA_PARASITE, 1, 0, 1.0, true);
                }
            }
            KASHA_BEAR_TOTEM => {
                if ctx.give_item_randomly(KASHA_CRYSTAL, 1, 1, 1.0, true) {
                    ctx.set_cond(2, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30571-00.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30571-02.htm"
                } else {
                    "30571-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30571-04.html".to_string()),
                2 => {
                    if ctx.quest_items_count(KASHA_CRYSTAL) > 0 {
                        // Java calls `Q00261_CollectorsDream.giveNewbieReward`
                        // here — one of only two live callers on this dist
                        // (it is commented out in every other newbie quest).
                        crate::scripts::q00261_collectors_dream::give_newbie_reward(ctx);
                        for reward in REWARDS {
                            ctx.reward_items(reward, 1);
                        }
                        ctx.exit_quest(true, true);
                        return Some("30571-05.html".to_string());
                    }
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
