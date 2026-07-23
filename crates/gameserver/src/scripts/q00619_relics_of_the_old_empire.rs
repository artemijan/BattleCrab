//! Relics of the Old Empire (619) — `quests/Q00619_RelicsOfTheOldEmpire`.
//! Ghost of Adventurer (31538, level 74+) trades 1000 Relics of the Empire for
//! one random S-grade weapon recipe. Relics drop from Imperial Tomb, IT-entrance
//! and Four Sepulchers monsters — 1 or 2 per kill (50/50) with an extra 10%
//! chance to also drop an Entrance Pass (7075, *not* a registered quest item, so
//! it survives turn-in). Turn-in (`31538-09`) is repeatable; `31538-10` exits.
use std::sync::OnceLock;

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const GHOST_OF_ADVENTURER: i32 = 31538;
const RELICS: i32 = 7254;
const ENTRANCE: i32 = 7075;
const REQUIRED: i64 = 1000;
// All S-grade weapon recipes (60%).
const RCP_REWARDS: [i32; 10] = [6881, 6883, 6885, 6887, 6891, 6893, 6895, 6897, 6899, 7580];

/// `addKillId(21396..=21434)` + `addKillId(21798, 21799, 21800)` +
/// `addKillId(18120..=18256)`.
fn kill_ids() -> &'static [i32] {
    static IDS: OnceLock<Vec<i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut v: Vec<i32> = (21396..=21434).collect();
        v.extend([21798, 21799, 21800]);
        v.extend(18120..=18256);
        v
    })
}

pub struct Q00619RelicsOfTheOldEmpire;

impl QuestScript for Q00619RelicsOfTheOldEmpire {
    fn id(&self) -> i32 {
        619
    }
    fn name(&self) -> &'static str {
        "Q00619_RelicsOfTheOldEmpire"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00619_RelicsOfTheOldEmpire"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GHOST_OF_ADVENTURER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GHOST_OF_ADVENTURER]
    }
    fn kill_npcs(&self) -> &[i32] {
        kill_ids()
    }
    fn quest_items(&self) -> &[i32] {
        &[RELICS]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31538-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31538-09.htm" => {
                if ctx.quest_items_count(RELICS) >= REQUIRED {
                    ctx.take_items(RELICS, REQUIRED);
                    let idx = ctx.roll(RCP_REWARDS.len() as i32) as usize;
                    ctx.give_items(RCP_REWARDS[idx], 1);
                    Some("31538-09.htm".to_string())
                } else {
                    Some("31538-06.htm".to_string())
                }
            }
            "31538-10.htm" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, -1, 3, npc)`: any started member near
        // the kill. Port is killer-only (G11 party deviation) → started killer.
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        // `giveItems` (plain, not rate-multiplied): 1 or 2 relics, 50/50.
        let count = if ctx.roll(2) == 0 { 2 } else { 1 };
        ctx.give_items(RELICS, count);
        ctx.play_sound(quest_sounds::ITEMGET);
        if ctx.roll(100) <= 10 {
            ctx.give_items(ENTRANCE, 1);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() < 74 { "31538-02.htm" } else { "31538-01.htm" }.to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.quest_items_count(RELICS) >= REQUIRED {
                return Some("31538-04.htm".to_string());
            } else if ctx.quest_items_count(ENTRANCE) > 0 {
                return Some("31538-06.htm".to_string());
            }
            return Some("31538-07.htm".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
