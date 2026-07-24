//! An Arrogant Search (348) — `quests/Q00348_AnArrogantSearch`. A level-60+
//! Seven Signs errand for **Hanellin**: hunt a Shell of Monsters, slay the
//! summoned **Stone Watchman Ezekiel** for the Book of Saint, then gather White
//! Cloth from either the Platinum Tribe or the Angels, redeemed for a piece of
//! **Blooded Fabric**. A long linear cond ladder (2 → 11). Repeatable.
//!
//! The radar navigation pings (Java `addRadar` / `RadarControl`) are cosmetic
//! wayfinding and left as TODO(radar) — they don't gate progress.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const HANELLIN: i32 = 30864;
const CLAUDIA_ATHEBALT: i32 = 31001;
const TABLE_OF_VISION: i32 = 31646;
// Monsters
const CRIMSON_DRAKE: i32 = 20670;
const KADIOS: i32 = 20671;
const PLATINUM_TRIBE_SHAMAN: i32 = 20828;
const PLATINUM_TRIBE_PREFECT: i32 = 20829;
const GUARDIAN_ANGEL: i32 = 20830;
const SEAL_ANGEL: i32 = 20831;
const STONE_WATCHMAN_EZEKIEL: i32 = 27296;
const KILL_IDS: [i32; 7] = [
    CRIMSON_DRAKE,
    KADIOS,
    PLATINUM_TRIBE_SHAMAN,
    PLATINUM_TRIBE_PREFECT,
    GUARDIAN_ANGEL,
    SEAL_ANGEL,
    STONE_WATCHMAN_EZEKIEL,
];
// Items
const SHELL_OF_MONSTERS: i32 = 14857;
const BOOK_OF_SAINT: i32 = 4397;
const HEALING_POTION: i32 = 1061;
const WHITE_CLOTH_PLATINUM: i32 = 4294;
const WHITE_CLOTH_ANGLE: i32 = 4400;
const BLOODED_FABRIC: i32 = 4295;
const MIN_LEVEL: i32 = 60;

pub struct Q00348AnArrogantSearch;

impl QuestScript for Q00348AnArrogantSearch {
    fn id(&self) -> i32 {
        348
    }
    fn name(&self) -> &'static str {
        "Q00348_AnArrogantSearch"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00348_AnArrogantSearch"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HANELLIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[HANELLIN, CLAUDIA_ATHEBALT, TABLE_OF_VISION]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_IDS
    }
    fn quest_items(&self) -> &[i32] {
        &[
            SHELL_OF_MONSTERS,
            BOOK_OF_SAINT,
            HEALING_POTION,
            WHITE_CLOTH_PLATINUM,
            WHITE_CLOTH_ANGLE,
        ]
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `addCondMinLevel(60, "lvl.htm")`.
        (ctx.player_level() < MIN_LEVEL).then(|| ctx.get_htm("lvl.htm"))
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30864.htm" | "30864-01.htm" | "30864-02.htm" | "30864-07a.htm" => {
                Some(event.to_string())
            }
            "30864-03.htm" => {
                if ctx.player_level() >= MIN_LEVEL {
                    ctx.start_quest();
                    ctx.set_cond(2, false);
                }
                None
            }
            "30864-04.htm" => {
                if ctx.cond() == 3 {
                    ctx.set_cond(4, false);
                    ctx.take_items(SHELL_OF_MONSTERS, -1);
                }
                None
            }
            "30864-05.htm" => {
                if ctx.cond() == 4 {
                    ctx.set_cond(5, false);
                }
                None
            }
            "31001-01.htm" => {
                if ctx.cond() == 5 {
                    // TODO(radar): addRadar(player, 120112, 30912, -3616).
                }
                None
            }
            "31646-01.htm" => {
                if ctx.cond() == 5 {
                    // Summon the Watchman beside the Table of Vision, hostile.
                    ctx.spawn_attacker(STONE_WATCHMAN_EZEKIEL, true);
                    // TODO(radar): RadarControl(2, 2, …) to clear the ping.
                }
                None
            }
            "30864-06.htm" => {
                if ctx.cond() == 6 {
                    ctx.set_cond(7, false);
                }
                None
            }
            "30864-07.htm" => {
                if ctx.cond() == 7 {
                    ctx.take_items(HEALING_POTION, 1);
                    ctx.set_cond(8, false);
                }
                None
            }
            "30864-08.htm" => {
                if ctx.cond() == 7 {
                    ctx.take_items(HEALING_POTION, 1);
                    ctx.set_cond(9, false);
                }
                None
            }
            "end.htm" => {
                if ctx.cond() == 10 || ctx.cond() == 11 {
                    ctx.take_items(WHITE_CLOTH_PLATINUM, -1);
                    ctx.take_items(WHITE_CLOTH_ANGLE, -1);
                    ctx.reward_items(BLOODED_FABRIC, 1);
                    ctx.exit_quest(true, true);
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
            CRIMSON_DRAKE | KADIOS => {
                // A coin-flip Shell of Monsters advances cond 2 → 3.
                if ctx.cond() == 2 && ctx.roll(2) == 0 {
                    ctx.give_items(SHELL_OF_MONSTERS, 1);
                    ctx.set_cond(3, false);
                }
            }
            PLATINUM_TRIBE_SHAMAN | PLATINUM_TRIBE_PREFECT => {
                if ctx.cond() == 8
                    && ctx.give_item_randomly(WHITE_CLOTH_PLATINUM, 1, 100, 0.5, true)
                {
                    ctx.set_cond(10, false);
                }
            }
            GUARDIAN_ANGEL | SEAL_ANGEL => {
                if ctx.cond() == 9 && ctx.give_item_randomly(WHITE_CLOTH_ANGLE, 1, 1000, 0.5, true)
                {
                    ctx.set_cond(11, false);
                }
            }
            STONE_WATCHMAN_EZEKIEL => {
                if ctx.cond() == 5 {
                    ctx.give_items(BOOK_OF_SAINT, 1);
                    ctx.set_cond(6, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc_id = ctx.npc_id;
        if ctx.is_created() {
            return Some(if npc_id == HANELLIN {
                "30864.htm".to_string()
            } else {
                ctx.no_quest_html()
            });
        }
        if ctx.is_started() {
            match npc_id {
                HANELLIN => {
                    return Some(match ctx.cond() {
                        2 => "30864-09.htm".to_string(),
                        3 => "30864-10.htm".to_string(),
                        4 => "30864-04.htm".to_string(),
                        5 => "30864-05.htm".to_string(),
                        6 => "30864-11.htm".to_string(),
                        7 => {
                            if ctx.quest_items_count(HEALING_POTION) > 0 {
                                "30864-12.htm".to_string()
                            } else {
                                "noz.htm".to_string()
                            }
                        }
                        9 => "30864-07.htm".to_string(),
                        10 | 11 => "30864-13.htm".to_string(),
                        _ => ctx.no_quest_html(),
                    });
                }
                CLAUDIA_ATHEBALT if ctx.cond() == 5 => return Some("31001.htm".to_string()),
                TABLE_OF_VISION if ctx.cond() == 5 => return Some("31646.htm".to_string()),
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
