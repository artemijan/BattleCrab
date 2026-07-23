//! Dig Up the Sea of Spores! (356) — `quests/Q00356_DigUpTheSeaOfSpores`.
//! Gauen (30717, level 43–51) wants 100 each of Herbivorous Spores (from Rotting
//! Trees) and Carnivorous Spores (from Spore Zombies) in the Sea of Spores;
//! `cond` tracks 2 = one kind full, 3 = both. Turn-in (`FINISH`) rolls an adena
//! reward. `addCondMaxLevel(51)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const GAUEN: i32 = 30717;
const CARNIVORE_SPORE: i32 = 5865;
const HERBIVOROUS_SPORE: i32 = 5866;
const MIN_LEVEL: i32 = 43;
const ROTTING_TREE: i32 = 20558;
const SPORE_ZOMBIE: i32 = 20562;
const REQUIRED_EACH: i64 = 100;

fn drop_chance(npc_id: i32) -> Option<f64> {
    match npc_id {
        ROTTING_TREE => Some(0.73),
        SPORE_ZOMBIE => Some(0.94),
        _ => None,
    }
}

pub struct Q00356DigUpTheSeaOfSpores;

impl QuestScript for Q00356DigUpTheSeaOfSpores {
    fn id(&self) -> i32 {
        356
    }
    fn name(&self) -> &'static str {
        "Q00356_DigUpTheSeaOfSpores"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00356_DigUpTheSeaOfSpores"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GAUEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GAUEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[ROTTING_TREE, SPORE_ZOMBIE]
    }
    fn quest_items(&self) -> &[i32] {
        &[HERBIVOROUS_SPORE, CARNIVORE_SPORE]
    }

    /// `addCondMaxLevel(51, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 51).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30717-02.htm" | "30717-03.htm" | "30717-04.htm" | "30717-10.html"
            | "30717-18.html" => Some(event.to_string()),
            "30717-05.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30717-09.html" => {
                ctx.take_items(CARNIVORE_SPORE, -1);
                ctx.take_items(HERBIVOROUS_SPORE, -1);
                Some(event.to_string())
            }
            "30717-11.html" | "30717-14.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "FINISH" => {
                let value = ctx.roll(100);
                let (adena, html) = if value < 20 {
                    (3000, "30717-15.html")
                } else if value < 70 {
                    (1300, "30717-16.html")
                } else {
                    (1300, "30717-17.html")
                };
                ctx.give_adena(adena, true);
                ctx.exit_quest(true, true);
                Some(html.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true)` is trivially
        // true for the killer; Java gates only on the quest state existing.
        if !ctx.has_qs() {
            return;
        }
        let (drop_item, other_item) = if ctx.npc_id == ROTTING_TREE {
            (HERBIVOROUS_SPORE, CARNIVORE_SPORE)
        } else {
            (CARNIVORE_SPORE, HERBIVOROUS_SPORE)
        };
        let Some(chance) = drop_chance(ctx.npc_id) else {
            return;
        };
        if ctx.give_item_randomly(drop_item, 1, REQUIRED_EACH, chance, true) {
            if ctx.quest_items_count(other_item) >= REQUIRED_EACH {
                ctx.set_cond(3, false);
            } else {
                ctx.set_cond(2, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL { "30717-01.htm" } else { "30717-06.htm" }
                    .to_string(),
            );
        }
        if ctx.is_started() {
            let has_all_herb = ctx.quest_items_count(HERBIVOROUS_SPORE) >= REQUIRED_EACH;
            let has_all_carn = ctx.quest_items_count(CARNIVORE_SPORE) >= REQUIRED_EACH;
            return Some(
                if has_all_herb && has_all_carn {
                    "30717-13.html"
                } else if has_all_carn {
                    "30717-12.html"
                } else if has_all_herb {
                    "30717-08.html"
                } else {
                    "30717-07.html"
                }
                .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }
}
