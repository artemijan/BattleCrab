//! Trial of the Challenger (211) — `quests/Q00211_TrialOfTheChallenger`. One of
//! the six class-group "Trial" quests (this one for the `WARRIOR_GROUP`, level
//! 35+). Kash sends the challenger to slay a chain of named beasts — Shyslassys,
//! Gorr, Baraham, and finally the Queen of Succubus — collecting Watcher's Eyes
//! and reporting to Martian, Raldo and Filaur along the way, for the Mark of the
//! Challenger.
//!
//! Cond-driven state machine. One flourish: the first kill (Shyslassys) drops a
//! Broken Key and conjures a Chest of Shyslassys; feeding the chest a key is a
//! gamble — 20% jackpot into a tiered crafting-material table, otherwise a
//! handful of adena.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const FILAUR: i32 = 30535;
const KASH: i32 = 30644;
const MARTIAN: i32 = 30645;
const RALDO: i32 = 30646;
const CHEST_OF_SHYSLASSYS: i32 = 30647;
// Monsters
const SHYSLASSYS: i32 = 27110;
const GORR: i32 = 27112;
const BARAHAM: i32 = 27113;
const QUEEN_OF_SUCCUBUS: i32 = 27114;
// Items
const LETTER_OF_KASH: i32 = 2628;
const WATCHERS_EYE1: i32 = 2629;
const WATCHERS_EYE2: i32 = 2630;
const SCROLL_OF_SHYSLASSYS: i32 = 2631;
const BROKEN_KEY: i32 = 2632;
// Rewards (chest jackpot table)
const ELVEN_NECKLACE_BEADS: i32 = 1904;
const WHITE_TUNIC_PATTERN: i32 = 1936;
const IRON_BOOTS_DESIGN: i32 = 1940;
const MANTICOR_SKIN_GAITERS_PATTERN: i32 = 1943;
const GAUNTLET_OF_REPOSE_PATTERN: i32 = 1946;
const MITHRIL_SCALE_GAITERS_MATERIAL: i32 = 2918;
const BRIGAMDINE_GAUNTLET_PATTERN: i32 = 2927;
const TOME_OF_BLOOD_PAGE: i32 = 2030;
const MARK_OF_CHALLENGER: i32 = 2627;
// Misc
const MIN_LEVEL: i32 = 35;

pub struct Q00211TrialOfTheChallenger;

impl QuestScript for Q00211TrialOfTheChallenger {
    fn id(&self) -> i32 {
        211
    }
    fn name(&self) -> &'static str {
        "Q00211_TrialOfTheChallenger"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00211_TrialOfTheChallenger"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KASH]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[FILAUR, KASH, MARTIAN, RALDO, CHEST_OF_SHYSLASSYS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[SHYSLASSYS, GORR, BARAHAM, QUEEN_OF_SUCCUBUS]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            LETTER_OF_KASH,
            WATCHERS_EYE1,
            WATCHERS_EYE2,
            SCROLL_OF_SHYSLASSYS,
            BROKEN_KEY,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        match ctx.npc_id {
            KASH => {
                if ctx.is_created() {
                    if !ctx.is_in_category("WARRIOR_GROUP") {
                        Some("30644-02.html".to_string())
                    } else if ctx.player_level() < MIN_LEVEL {
                        Some("30644-01.html".to_string())
                    } else {
                        Some("30644-03.htm".to_string())
                    }
                } else if ctx.is_completed() {
                    Some(ctx.already_completed_html())
                } else {
                    match cond {
                        1 => Some("30644-07.html".to_string()),
                        2 if ctx.quest_items_count(SCROLL_OF_SHYSLASSYS) > 0 => {
                            ctx.take_items(SCROLL_OF_SHYSLASSYS, -1);
                            ctx.give_items(LETTER_OF_KASH, 1);
                            ctx.set_cond(3, true);
                            Some("30644-08.html".to_string())
                        }
                        3 if ctx.quest_items_count(LETTER_OF_KASH) > 0 => {
                            Some("30644-09.html".to_string())
                        }
                        8..=10 => Some("30644-10.html".to_string()),
                        _ => Some(ctx.no_quest_html()),
                    }
                }
            }
            MARTIAN => match cond {
                3 if ctx.quest_items_count(LETTER_OF_KASH) > 0 => Some("30645-01.html".to_string()),
                4 => Some("30645-03.html".to_string()),
                5 if ctx.quest_items_count(WATCHERS_EYE1) > 0 => {
                    ctx.take_items(WATCHERS_EYE1, -1);
                    ctx.set_cond(6, true);
                    Some("30645-04.html".to_string())
                }
                6 => Some("30645-05.html".to_string()),
                7 => Some("30645-06.html".to_string()),
                8 | 9 => Some("30645-09.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            CHEST_OF_SHYSLASSYS => {
                if ctx.is_started() {
                    Some("30647-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            RALDO => match cond {
                7 if ctx.quest_items_count(WATCHERS_EYE2) > 0 => Some("30646-01.html".to_string()),
                8 => Some("30646-06.html".to_string()),
                10 => {
                    ctx.add_exp_and_sp(1067606, 69242);
                    ctx.give_adena(194556, true);
                    ctx.give_items(MARK_OF_CHALLENGER, 1);
                    ctx.social_action(3);
                    ctx.exit_quest(false, true);
                    Some("30646-07.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            FILAUR => match cond {
                8 => {
                    ctx.set_cond(9, true);
                    Some("30535-01.html".to_string())
                }
                9 => {
                    // Java sends RadarControl(0, 2, ...) pointing at the Queen of
                    // Succubus lair — the quest pin, not the red flag.
                    ctx.add_quest_radar(151589, -174823, -1776);
                    Some("30535-02.html".to_string())
                }
                10 => Some("30535-03.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30644-04.htm" => Some(event.to_string()),
            "30645-07.html" | "30645-08.html" | "30646-02.html" | "30646-03.html" => {
                if ctx.is_started() {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30644-06.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30647-02.html" => {
                if ctx.cond() == 2 && ctx.quest_items_count(BROKEN_KEY) > 0 {
                    ctx.take_items(BROKEN_KEY, -1);
                    if ctx.roll(10) < 2 {
                        ctx.play_sound(quest_sounds::JACKPOT);
                        let random = ctx.roll(100);
                        if random > 90 {
                            ctx.reward_items(MITHRIL_SCALE_GAITERS_MATERIAL, 1);
                            ctx.reward_items(BRIGAMDINE_GAUNTLET_PATTERN, 1);
                            ctx.reward_items(MANTICOR_SKIN_GAITERS_PATTERN, 1);
                            ctx.reward_items(GAUNTLET_OF_REPOSE_PATTERN, 1);
                            ctx.reward_items(IRON_BOOTS_DESIGN, 1);
                        } else if random > 70 {
                            ctx.reward_items(TOME_OF_BLOOD_PAGE, 1);
                            ctx.reward_items(ELVEN_NECKLACE_BEADS, 1);
                        } else if random > 40 {
                            ctx.reward_items(WHITE_TUNIC_PATTERN, 1);
                        } else {
                            ctx.reward_items(IRON_BOOTS_DESIGN, 1);
                        }
                        Some("30647-03.html".to_string())
                    } else {
                        let adena = (ctx.roll(1000) + 1) as i64;
                        ctx.give_adena(adena, true);
                        Some(event.to_string())
                    }
                } else {
                    Some("30647-04.html".to_string())
                }
            }
            "30645-02.html" => {
                if ctx.cond() == 3 && ctx.quest_items_count(LETTER_OF_KASH) > 0 {
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30646-04.html" | "30646-05.html" => {
                if ctx.cond() == 7 && ctx.quest_items_count(WATCHERS_EYE2) > 0 {
                    ctx.take_items(WATCHERS_EYE2, -1);
                    ctx.set_cond(8, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        match ctx.npc_id {
            SHYSLASSYS if ctx.cond() == 1 => {
                // **Not a missing cap.** Java's guard is
                // `SpawnTable.getSpawns(npc.getId()).size() < 10` — the
                // number of spawn *points* the killed mob itself has, not
                // how many chests exist. Shyslassys has **one** spawn point
                // on this dist, so the condition is always true and the
                // unconditional spawn is exact.
                ctx.spawn_near_npc(CHEST_OF_SHYSLASSYS, false);
                ctx.give_items(SCROLL_OF_SHYSLASSYS, 1);
                ctx.give_items(BROKEN_KEY, 1);
                ctx.set_cond(2, true);
            }
            GORR if ctx.cond() == 4 => {
                ctx.give_items(WATCHERS_EYE1, 1);
                ctx.set_cond(5, true);
            }
            BARAHAM if ctx.cond() == 6 => {
                // Same vacuous `getSpawns(BARAHAM).size() < 10` guard —
                // Baraham has one spawn point here.
                ctx.spawn_near_npc(RALDO, false);
                ctx.give_items(WATCHERS_EYE2, 1);
                ctx.set_cond(7, true);
            }
            QUEEN_OF_SUCCUBUS if ctx.cond() == 9 => {
                // Same vacuous guard — the Queen has one spawn point here.
                ctx.spawn_near_npc(RALDO, false);
                ctx.set_cond(10, true);
            }
            _ => {}
        }
    }
}
