//! A Powerful Primeval Creature (642) — `quests/Q00642_APowerfulPrimevalCreature`.
//! A repeatable level-75 Primeval Isle hunt: Dinn pays 5,000 Adena per Dinosaur
//! Tissue dropped by the isle's dinosaurs, and the rare Ancient Egg yields a
//! Dinosaur Egg. Sibling of the already-ported Elroki quests
//! [`Q643`](super::q00643_rise_and_fall_of_the_elroki_tribe) /
//! [`Q688`](super::q00688_defeat_the_elrokian_raiders).

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPC
const DINN: i32 = 32105;
// Items
const DINOSAUR_TISSUE: i32 = 8774;
const DINOSAUR_EGG: i32 = 8775;
// Mob
const ANCIENT_EGG: i32 = 18344;
// Misc
const MIN_LEVEL: i32 = 75;

/// The dinosaurs that drop Dinosaur Tissue, and each one's drop chance (Java's
/// `MOBS_TISSUE`). Velociraptors/Pterosaurs 0.309, Tyrannosaurs 0.988.
fn tissue_chance(npc_id: i32) -> Option<f64> {
    Some(match npc_id {
        22196 | 22197 | 22198 | 22199 | 22218 | 22223 => 0.309,
        22215..=22217 => 0.988,
        _ => return None,
    })
}

fn has_any_loot(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(DINOSAUR_TISSUE) > 0 || ctx.quest_items_count(DINOSAUR_EGG) > 0
}

pub struct Q00642APowerfulPrimevalCreature;

impl QuestScript for Q00642APowerfulPrimevalCreature {
    fn id(&self) -> i32 {
        642
    }
    fn name(&self) -> &'static str {
        "Q00642_APowerfulPrimevalCreature"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00642_APowerfulPrimevalCreature"
    }
    fn start_npcs(&self) -> &[i32] {
        &[DINN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[DINN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            ANCIENT_EGG,
            22196,
            22197,
            22198,
            22199,
            22215,
            22216,
            22217,
            22218,
            22223,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[DINOSAUR_TISSUE, DINOSAUR_EGG]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = if ctx.is_created() {
            if ctx.player_level() < MIN_LEVEL {
                "32105-01.htm".to_string()
            } else {
                "32105-02.htm".to_string()
            }
        } else if has_any_loot(ctx) {
            "32105-08.html".to_string()
        } else {
            "32105-07.html".to_string()
        };
        Some(html)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "32105-05.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            // "Not interested" — drop the quest, still repeatable.
            "32105-06.htm" => {
                ctx.exit_quest(true, false);
                Some(event.to_string())
            }
            "32105-09.html" => {
                let tissue = ctx.quest_items_count(DINOSAUR_TISSUE);
                if tissue > 0 {
                    ctx.give_adena(5000 * tissue, true);
                    ctx.take_items(DINOSAUR_TISSUE, -1);
                    Some(event.to_string())
                } else {
                    Some("32105-14.html".to_string())
                }
            }
            "exit" => {
                let tissue = ctx.quest_items_count(DINOSAUR_TISSUE);
                let html = if tissue > 0 {
                    ctx.give_adena(5000 * tissue, true);
                    "32105-12.html"
                } else {
                    "32105-13.html"
                };
                ctx.exit_quest(true, true);
                Some(html.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if let Some(chance) = tissue_chance(ctx.npc_id) {
            ctx.give_item_randomly(DINOSAUR_TISSUE, 1, 0, chance, true);
        } else {
            // The Ancient Egg always yields a Dinosaur Egg.
            ctx.give_item_randomly(DINOSAUR_EGG, 1, 0, 1.0, true);
        }
    }
}
