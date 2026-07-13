//! Orc Subjugation (263) — port of
//! `dist/game/data/scripts/quests/Q00263_OrcSubjugation/`. Dark Elf only:
//! Kayleen buys Balor Orc amulets (8a) and necklaces (10a, +1100 for 10+
//! total); each registered monster drops its own item at 50%.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const KAYLEEN: i32 = 30346;
const ORC_AMULET: i32 = 1116;
const ORC_NECKLACE: i32 = 1117;
/// monster id → dropped item.
const MONSTERS: [(i32, i32); 4] =
    [(20385, ORC_AMULET), (20386, ORC_NECKLACE), (20387, ORC_NECKLACE), (20388, ORC_NECKLACE)];
const MIN_LEVEL: i32 = 8;
const RACE_DARK_ELF: i32 = 2;

pub struct Q00263OrcSubjugation;

impl QuestScript for Q00263OrcSubjugation {
    fn id(&self) -> i32 {
        263
    }
    fn name(&self) -> &'static str {
        "Q00263_OrcSubjugation"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00263_OrcSubjugation"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KAYLEEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KAYLEEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 4] = [20385, 20386, 20387, 20388];
        &IDS
    }
    fn quest_items(&self) -> &[i32] {
        &[ORC_AMULET, ORC_NECKLACE]
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
            "30346-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30346-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30346-08.html" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        if ctx.roll(10) > 4 {
            let item = MONSTERS.iter().find(|(id, _)| *id == ctx.npc_id).map(|(_, i)| *i).unwrap_or(ORC_AMULET);
            ctx.give_items(item, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_DARK_ELF {
                    "30346-01.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30346-03.htm"
                } else {
                    "30346-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let amulets = ctx.quest_items_count(ORC_AMULET);
            let necklaces = ctx.quest_items_count(ORC_NECKLACE);
            return Some(if amulets + necklaces > 0 {
                ctx.give_adena(
                    (amulets * 8) + (necklaces * 10) + if amulets + necklaces >= 10 { 1100 } else { 0 },
                    true,
                );
                ctx.take_items(ORC_AMULET, -1);
                ctx.take_items(ORC_NECKLACE, -1);
                "30346-06.html".to_string()
            } else {
                "30346-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }
}
