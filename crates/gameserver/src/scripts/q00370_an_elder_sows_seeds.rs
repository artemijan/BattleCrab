//! An Elder Sows Seeds (370) — `quests/Q00370_AnElderSowsSeeds`. Casian (30612)
//! in Dion sends level-28–42 scholars to hunt the Ant Nest for **Spellbook
//! Pages**. Pages are traded (via the NPC's multisell, outside this script) for
//! the four elemental **Chapters**; a full matched set of Fire/Water/Wind/Earth
//! chapters is exchanged here for 3,600 adena apiece.
//!
//! The page→chapter step lives in a multisell referenced by the htmls, so this
//! script only drops pages and cashes in complete chapter sets — faithful to the
//! Java, which likewise never grants chapters itself.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const CASIAN: i32 = 30612;
const SPELLBOOK_PAGE: i32 = 5916;
const CHAPTER_OF_FIRE: i32 = 5917;
const CHAPTER_OF_WATER: i32 = 5918;
const CHAPTER_OF_WIND: i32 = 5919;
const CHAPTER_OF_EARTH: i32 = 5920;
const CHAPTERS: [i32; 4] = [
    CHAPTER_OF_FIRE,
    CHAPTER_OF_WATER,
    CHAPTER_OF_WIND,
    CHAPTER_OF_EARTH,
];
const MIN_LEVEL: i32 = 28;
const MAX_LEVEL: i32 = 42;
/// Ants that drop a page on a flat percent roll: `(npc_id, percent)`.
const MOBS_PERCENT: [(i32, i32); 3] = [(20082, 9), (20086, 9), (20090, 22)];
/// Ants that drop a page via a rate-scaled chance: `(npc_id, chance)`.
const MOBS_CHANCE: [(i32, f64); 2] = [(20084, 0.101), (20089, 0.100)];

pub struct Q00370AnElderSowsSeeds;

impl Q00370AnElderSowsSeeds {
    /// Java `exchangeChapters`: cash in as many *complete* Fire/Water/Wind/Earth
    /// sets as the player holds. `take_all` empties the chapters (quest exit)
    /// rather than just the paid-out sets. Returns whether anything paid out.
    fn exchange_chapters(&self, ctx: &mut QuestCtx, take_all: bool) -> bool {
        let min_count = CHAPTERS
            .iter()
            .map(|&c| ctx.quest_items_count(c))
            .min()
            .unwrap_or(0);
        if min_count > 0 {
            ctx.give_adena(min_count * 3600, true);
        }
        let take = if take_all { -1 } else { min_count };
        for &c in &CHAPTERS {
            ctx.take_items(c, take);
        }
        min_count > 0
    }
}

impl QuestScript for Q00370AnElderSowsSeeds {
    fn id(&self) -> i32 {
        370
    }
    fn name(&self) -> &'static str {
        "Q00370_AnElderSowsSeeds"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00370_AnElderSowsSeeds"
    }
    fn start_npcs(&self) -> &[i32] {
        &[CASIAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[CASIAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20082, 20086, 20090, 20084, 20089]
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `addCondMaxLevel(42, …)`.
        (ctx.player_level() > MAX_LEVEL).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30612-02.htm" | "30612-03.htm" | "30612-06.html" | "30612-07.html"
            | "30612-09.html" => Some(event.to_string()),
            "30612-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "REWARD" => {
                if !ctx.is_started() {
                    return None;
                }
                Some(if self.exchange_chapters(ctx, false) {
                    "30612-08.html".to_string()
                } else {
                    "30612-11.html".to_string()
                })
            }
            "30612-10.html" => {
                if !ctx.is_started() {
                    return None;
                }
                self.exchange_chapters(ctx, true);
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        if let Some(&(_, percent)) = MOBS_PERCENT.iter().find(|(id, _)| *id == npc_id) {
            if ctx.roll(100) < percent {
                ctx.give_item_randomly(SPELLBOOK_PAGE, 1, 0, 1.0, true);
            }
        } else if let Some(&(_, chance)) = MOBS_CHANCE.iter().find(|(id, _)| *id == npc_id) {
            ctx.give_item_randomly(SPELLBOOK_PAGE, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL {
                "30612-01.htm".to_string()
            } else {
                "30612-05.html".to_string()
            });
        }
        if ctx.is_started() {
            return Some("30612-06.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
