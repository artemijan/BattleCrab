//! Pleas of Pixies (266) — port of
//! `dist/game/data/scripts/quests/Q00266_PleasOfPixies/`. Elf-only (level
//! 3–8): Pixy Murika (31852) wants **100 Predator's Fangs** off the Keltir/wolf
//! packs, then rolls a weighted reward table. Repeatable.
//!
//! Two quirks kept: per-mob drop tables give a **variable amount** on a
//! `getRandom(10)` threshold, and the reward roll is **inverted** — the 2% roll
//! hands out the *cheapest* prize (Glass Shard) with the jackpot chime, while
//! the Emerald + 5000a is the 55% common case.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const PIXY_MURIKA: i32 = 31852;
const PREDATORS_FANG: i32 = 1334;
const ADENA: i32 = 57;
const RACE_ELF: i32 = 1;
const MIN_LEVEL: i32 = 3;
const REQUIRED_FANGS: i64 = 100;

const KILL_NPCS: [i32; 4] = [20537, 20525, 20534, 20530];

/// `(threshold, count)`: a single `getRandom(10)` picks the first entry with
/// `roll < threshold`, awarding `count` fangs (via `giveItemRandomly`).
fn drops(npc_id: i32) -> &'static [(i32, i64)] {
    match npc_id {
        20537 => &[(10, 2)],         // Elder Red Keltir
        20525 => &[(5, 2), (10, 3)], // Gray Wolf
        20534 => &[(6, 1)],          // Red Keltir
        20530 => &[(8, 1)],          // Young Red Keltir
        _ => &[],
    }
}

/// `(item, count)` prizes indexed by the reward roll's bucket.
fn reward(bucket: i32) -> &'static [(i32, i64)] {
    match bucket {
        0 => &[(1336, 1), (ADENA, 100)], // Glass Shard + 100a (jackpot chime, ironically)
        1 => &[(1339, 1), (ADENA, 300)], // Onyx + 300a
        2 => &[(1338, 1), (ADENA, 500)], // Blue Onyx + 500a
        _ => &[(1337, 1), (ADENA, 5000)], // Emerald + 5000a
    }
}

pub struct Q00266PleasOfPixies;

impl QuestScript for Q00266PleasOfPixies {
    fn id(&self) -> i32 {
        266
    }
    fn name(&self) -> &'static str {
        "Q00266_PleasOfPixies"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00266_PleasOfPixies"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PIXY_MURIKA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PIXY_MURIKA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[PREDATORS_FANG]
    }

    /// `addCondMaxLevel(8, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 8).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_ELF {
                    "31852-01.htm"
                } else if ctx.player_level() < MIN_LEVEL {
                    "31852-02.htm"
                } else {
                    "31852-03.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("31852-05.html".to_string()),
                2 if ctx.quest_items_count(PREDATORS_FANG) >= REQUIRED_FANGS => {
                    let chance = ctx.roll(100);
                    let bucket = if chance < 2 {
                        ctx.play_sound(quest_sounds::JACKPOT);
                        0
                    } else if chance < 20 {
                        1
                    } else if chance < 45 {
                        2
                    } else {
                        3
                    };
                    for &(item, count) in reward(bucket) {
                        ctx.reward_items(item, count);
                    }
                    ctx.exit_quest(true, true);
                    return Some("31852-06.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "31852-04.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let chance = ctx.roll(10);
        for &(threshold, count) in drops(ctx.npc_id) {
            if chance < threshold {
                // `giveItemRandomly(..., count, 100, 1, true)` — reaching 100 flips cond.
                if ctx.give_item_randomly(PREDATORS_FANG, count, REQUIRED_FANGS, 1.0, true) {
                    ctx.set_cond(2, false);
                }
                break;
            }
        }
    }
}
