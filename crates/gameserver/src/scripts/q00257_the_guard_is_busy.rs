//! The Guard is Busy (257) — port of
//! `dist/game/data/scripts/quests/Q00257_TheGuardIsBusy/`. Gilbert (30039) in
//! Gludio pays adena for orc/werewolf trophies (5a amulet, 8a necklace, 10a
//! fang; +1000 for 10+ total). Repeatable, level 6–16.
//!
//! Each monster has a **hand-rolled** drop table (`getRandom(random) < chance`,
//! with per-drop denominators of 10 or 100 — not `giveItemRandomly`), so the
//! rate is left un-multiplied by `RateQuestDrop`, exactly as Java writes it.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const GILBERT: i32 = 30039;
const MIN_LEVEL: i32 = 6;
const GLUDIO_LORDS_MARK: i32 = 1084;
const ORC_AMULET: i32 = 752;
const ORC_NECKLACE: i32 = 1085;
const WEREWOLF_FANG: i32 = 1086;

const KILL_NPCS: [i32; 9] = [
    20006, 20093, 20096, 20098, 20130, 20131, 20132, 20342, 20343,
];

/// `(random, chance, item, count)` — `getRandom(random) < chance` gives
/// `count` of `item`; the first hit in the list wins. Transcribed from Java's
/// `MONSTERS` map.
fn drops(npc_id: i32) -> &'static [(i32, i32, i32, i64)] {
    match npc_id {
        20006 => &[(10, 2, ORC_AMULET, 2), (10, 10, ORC_AMULET, 1)], // Orc Archer
        20093 => &[(100, 85, ORC_NECKLACE, 1)],                      // Orc Fighter
        20096 => &[(100, 95, ORC_NECKLACE, 1)],                      // Orc Fighter Sub Leader
        20098 => &[(100, 100, ORC_NECKLACE, 1)],                     // Orc Fighter Leader
        20130 => &[(10, 7, ORC_AMULET, 1)],                          // Orc
        20131 => &[(10, 9, ORC_AMULET, 1)],                          // Orc Grunt
        20132 => &[(10, 7, WEREWOLF_FANG, 1)],                       // Werewolf
        20342 => &[(0, 1, WEREWOLF_FANG, 1)],                        // Werewolf Chieftain (always)
        20343 => &[(100, 85, WEREWOLF_FANG, 1)],                     // Werewolf Hunter
        _ => &[],
    }
}

pub struct Q00257TheGuardIsBusy;

impl QuestScript for Q00257TheGuardIsBusy {
    fn id(&self) -> i32 {
        257
    }
    fn name(&self) -> &'static str {
        "Q00257_TheGuardIsBusy"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00257_TheGuardIsBusy"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GILBERT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GILBERT]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[ORC_AMULET, GLUDIO_LORDS_MARK, ORC_NECKLACE, WEREWOLF_FANG]
    }

    /// `addCondMaxLevel(16, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 16).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30039-03.htm" => {
                ctx.start_quest();
                ctx.give_items(GLUDIO_LORDS_MARK, 1);
                Some(event.to_string())
            }
            "30039-05.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30039-06.html" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        for &(random, chance, item, count) in drops(ctx.npc_id) {
            if ctx.roll(random) < chance {
                ctx.give_items(item, count);
                ctx.play_sound(quest_sounds::ITEMGET);
                break;
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30039-02.htm"
                } else {
                    "30039-01.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let amulets = ctx.quest_items_count(ORC_AMULET);
            let necklace = ctx.quest_items_count(ORC_NECKLACE);
            let fang = ctx.quest_items_count(WEREWOLF_FANG);
            if amulets + necklace + fang > 0 {
                let bonus = if amulets + necklace + fang >= 10 {
                    1000
                } else {
                    0
                };
                ctx.give_adena(amulets * 5 + necklace * 8 + fang * 10 + bonus, true);
                ctx.take_items(ORC_AMULET, -1);
                ctx.take_items(ORC_NECKLACE, -1);
                ctx.take_items(WEREWOLF_FANG, -1);
                // Java's `giveNewbieReward` here is commented out in the dist.
                return Some("30039-07.html".to_string());
            }
            return Some("30039-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
