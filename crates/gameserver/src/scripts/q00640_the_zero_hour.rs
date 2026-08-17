//! The Zero Hour (640) — `quests/Q00640_TheZeroHour`. Kahman (31554) in the
//! Hot Springs sends level-66+ hunters who finished [`Q00109`] against the
//! Spiked Stakato swarm for **Fangs of Stakato**, exchanged in fixed lots for
//! crafting materials (Enria / Asofe / Thons / Varnish / Cokes / Braid /
//! Durable Metal Plate / Mithril Alloy / Oriharukon). Repeatable.
//!
//! [`Q00109`]: super::q00109_in_search_of_the_nest

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const KAHMAN: i32 = 31554;
const FANG_OF_STAKATO: i32 = 8085;
const MIN_LEVEL: i32 = 66;
const MONSTERS: [i32; 15] = [
    22617, 22618, 22619, 22620, 22621, 22622, 22623, 22625, 22626, 22627, 22628, 22629, 22630,
    22631, 22633,
];
/// Each exchange button: `(fangs_required, reward_item, reward_count)`.
const EXCHANGES: [(&str, i64, i32, i64); 9] = [
    ("1", 12, 4042, 1),   // Enria
    ("2", 6, 4043, 1),    // Asofe
    ("3", 6, 4044, 1),    // Thons
    ("4", 81, 1887, 10),  // Varnish of Purity
    ("5", 33, 1888, 5),   // Synthetic Cokes
    ("6", 30, 1889, 10),  // Compound Braid
    ("7", 150, 5550, 10), // Durable Metal Plate
    ("8", 131, 1890, 10), // Mithril Alloy
    ("9", 123, 1893, 5),  // Oriharukon
];

pub struct Q00640TheZeroHour;

impl QuestScript for Q00640TheZeroHour {
    fn id(&self) -> i32 {
        640
    }
    fn name(&self) -> &'static str {
        "Q00640_TheZeroHour"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00640_TheZeroHour"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KAHMAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KAHMAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[FANG_OF_STAKATO]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.player_level() < MIN_LEVEL {
                return Some("31554-00.htm".to_string());
            }
            // Requires In Search of the Nest (109) completed.
            return Some(if ctx.other_quest_completed("Q00109_InSearchOfTheNest") {
                "31554-01.htm".to_string()
            } else {
                "31554-10.htm".to_string()
            });
        }
        if ctx.is_started() && ctx.cond() == 1 {
            return Some(if ctx.quest_items_count(FANG_OF_STAKATO) >= 1 {
                "31554-04.htm".to_string()
            } else {
                "31554-03.htm".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31554-02.htm" => {
                ctx.start_quest();
                None
            }
            "31554-05.htm" => Some(event.to_string()),
            "31554-08.htm" => {
                ctx.exit_quest(true, true);
                None
            }
            _ => {
                // The nine fang-for-material exchange buttons.
                let ex = EXCHANGES.iter().find(|(id, _, _, _)| *id == event)?;
                let (_, needed, reward_id, reward_count) = *ex;
                if ctx.quest_items_count(FANG_OF_STAKATO) >= needed {
                    ctx.take_items(FANG_OF_STAKATO, needed);
                    ctx.reward_items(reward_id, reward_count);
                    Some("31554-09.htm".to_string())
                } else {
                    Some("31554-07.htm".to_string())
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Party sharing collapses to the killer (documented onKill deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        // `giveItems(member, FANG, (long) RATE_QUEST_DROP)` — one fang at the
        // base quest-drop rate.
        ctx.give_items(FANG_OF_STAKATO, 1);
        ctx.play_sound(quest_sounds::ITEMGET);
    }
}
