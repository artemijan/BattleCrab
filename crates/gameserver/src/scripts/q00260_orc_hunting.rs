//! Orc Hunting (260) — port of
//! `dist/game/data/scripts/quests/Q00260_OrcHunting/`. Elf only: Rayen
//! buys Kaboo Orc amulets (4a) and necklaces (10a, +1000 for 10+ total);
//! each monster drops its own item at 50%.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const RAYEN: i32 = 30221;
const ORC_AMULET: i32 = 1114;
const ORC_NECKLACE: i32 = 1115;
const MONSTERS: [(i32, i32); 6] = [
    (20468, ORC_AMULET),
    (20469, ORC_AMULET),
    (20470, ORC_AMULET),
    (20471, ORC_NECKLACE),
    (20472, ORC_NECKLACE),
    (20473, ORC_NECKLACE),
];
const MIN_LEVEL: i32 = 6;
const RACE_ELF: i32 = 1;

pub struct Q00260OrcHunting;

impl QuestScript for Q00260OrcHunting {
    fn id(&self) -> i32 {
        260
    }
    fn name(&self) -> &'static str {
        "Q00260_OrcHunting"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00260_OrcHunting"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RAYEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RAYEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        const IDS: [i32; 6] = [20468, 20469, 20470, 20471, 20472, 20473];
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
            "30221-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30221-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30221-08.html" => Some(event.to_string()),
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
                if ctx.player_race() != RACE_ELF {
                    "30221-01.html"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30221-03.htm"
                } else {
                    "30221-02.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let amulets = ctx.quest_items_count(ORC_AMULET);
            let necklaces = ctx.quest_items_count(ORC_NECKLACE);
            return Some(if amulets + necklaces > 0 {
                ctx.give_adena(
                    (amulets * 4) + (necklaces * 10) + if amulets + necklaces >= 10 { 1000 } else { 0 },
                    true,
                );
                ctx.take_items(ORC_AMULET, -1);
                ctx.take_items(ORC_NECKLACE, -1);
                "30221-06.html".to_string()
            } else {
                "30221-05.html".to_string()
            });
        }
        Some(ctx.no_quest_html())
    }
}
