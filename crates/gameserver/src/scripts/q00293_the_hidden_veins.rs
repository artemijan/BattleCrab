//! The Hidden Veins (293) — port of
//! `dist/game/data/scripts/quests/Q00293_TheHiddenVeins/`. Dwarf-only
//! (level 6–15): Filaur (30535) buys Chrysolite Ore (5a) off the Gorgon
//! Flowers, but **Chichirin (30539) crafts 4 Torn Map Fragments into a Hidden
//! Ore Map** worth 150a — the fragment→map trade is the point. Repeatable.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const FILAUR: i32 = 30535;
const CHICHIRIN: i32 = 30539;
const CHRYSOLITE_ORE: i32 = 1488;
const TORN_MAP_FRAGMENT: i32 = 1489;
const HIDDEN_ORE_MAP: i32 = 1490;
const MONSTERS: [i32; 3] = [20446, 20447, 20448];
const MIN_LEVEL: i32 = 6;
const RACE_DWARF: i32 = 4;
const REQUIRED_TORN_MAP_FRAGMENT: i64 = 4;

pub struct Q00293TheHiddenVeins;

impl QuestScript for Q00293TheHiddenVeins {
    fn id(&self) -> i32 {
        293
    }
    fn name(&self) -> &'static str {
        "Q00293_TheHiddenVeins"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00293_TheHiddenVeins"
    }
    fn start_npcs(&self) -> &[i32] {
        &[FILAUR]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[FILAUR, CHICHIRIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[CHRYSOLITE_ORE, TORN_MAP_FRAGMENT, HIDDEN_ORE_MAP]
    }

    /// `addCondMaxLevel(15, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 15).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30535-04.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30535-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30535-08.html" => Some(event.to_string()),
            // Chichirin combines 4 map fragments into one Hidden Ore Map.
            "30539-03.html" => {
                if ctx.quest_items_count(TORN_MAP_FRAGMENT) >= REQUIRED_TORN_MAP_FRAGMENT {
                    ctx.give_items(HIDDEN_ORE_MAP, 1);
                    ctx.take_items(TORN_MAP_FRAGMENT, REQUIRED_TORN_MAP_FRAGMENT);
                    Some("30539-03.html".to_string())
                } else {
                    Some("30539-02.html".to_string())
                }
            }
            _ => None,
        }
    }

    /// One `getRandom(100)` roll decides both drops: `> 50` an ore, `< 5` a
    /// (rare) map fragment, otherwise nothing.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let chance = ctx.roll(100);
        if chance > 50 {
            ctx.give_items(CHRYSOLITE_ORE, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        } else if chance < 5 {
            ctx.give_items(TORN_MAP_FRAGMENT, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            FILAUR => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_race() != RACE_DWARF {
                            "30535-01.htm"
                        } else if ctx.player_level() >= MIN_LEVEL {
                            "30535-03.htm"
                        } else {
                            "30535-02.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    let ores = ctx.quest_items_count(CHRYSOLITE_ORE);
                    let maps = ctx.quest_items_count(HIDDEN_ORE_MAP);
                    if ores + maps > 0 {
                        let bonus = if ores + maps >= 10 { 1000 } else { 0 };
                        ctx.give_adena(ores * 5 + maps * 150 + bonus, true);
                        ctx.take_items(CHRYSOLITE_ORE, -1);
                        ctx.take_items(HIDDEN_ORE_MAP, -1);
                        // Java's `giveNewbieReward` here is commented out in the dist.
                        let html = if ores > 0 {
                            if maps > 0 { "30535-10.html" } else { "30535-06.html" }
                        } else {
                            "30535-09.html"
                        };
                        return Some(html.to_string());
                    }
                    return Some("30535-05.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            CHICHIRIN => Some("30539-01.html".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
