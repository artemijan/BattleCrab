//! Test of the Healer (226) — `quests/Q00226_TestOfTheHealer`. The Bishop /
//! Prophet / Elder 2nd-class proof (`WHITE_MAGIC_GROUP`, level 39+). Priest
//! Bandellos sends the healer chasing Perrin's report (ambushed by Tatoma),
//! recovering a golden statue for Father Gupu, then infiltrating the Lero
//! Lizardman conspiracy — four secret letters wrung from their leaders — to
//! expose Saint Kristina and earn the Mark of the Healer.
//!
//! `memoState`-driven (states 1..10). Whether the healer hands over or keeps the
//! Golden Statue splits the ending reward (Bandellos pays more to the one who
//! returns it). Several spawns keep Java's `getSummonedNpcCount`/
//! `isSimulatingTalking` guards only as `TODO(G22)` — cosmetic spam caps.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const MASTER_SORIUS: i32 = 30327;
const ALLANA: i32 = 30424;
const PERRIN: i32 = 30428;
const PRIEST_BANDELLOS: i32 = 30473;
const FATHER_GUPU: i32 = 30658;
const ORPHAN_GIRL: i32 = 30659;
const WINDY_SHAORING: i32 = 30660;
const MYSTERIOUS_DARK_ELF: i32 = 30661;
const PIPER_LONGBOW: i32 = 30662;
const SLEIN_SHINING_BLADE: i32 = 30663;
const CAIN_FLYING_KNIFE: i32 = 30664;
const SAINT_KRISTINA: i32 = 30665;
const DAURIN_HAMMERCRUSH: i32 = 30674;
// Items
const ADENA: i32 = 57;
const REPORT_OF_PERRIN: i32 = 2810;
const CRISTINAS_LETTER: i32 = 2811;
const PICTURE_OF_WINDY: i32 = 2812;
const GOLDEN_STATUE: i32 = 2813;
const WINDYS_PEBBLES: i32 = 2814;
const ORDER_OF_SORIUS: i32 = 2815;
const SECRET_LETTER1: i32 = 2816;
const SECRET_LETTER2: i32 = 2817;
const SECRET_LETTER3: i32 = 2818;
const SECRET_LETTER4: i32 = 2819;
// Reward
const MARK_OF_HEALER: i32 = 2820;
// Quest monsters
const LERO_LIZARDMAN_AGENT: i32 = 27122;
const LERO_LIZARDMAN_LEADER: i32 = 27123;
const LERO_LIZARDMAN_ASSASSIN: i32 = 27124;
const LERO_LIZARDMAN_SNIPER: i32 = 27125;
const LERO_LIZARDMAN_WIZARD: i32 = 27126;
const LERO_LIZARDMAN_LORD: i32 = 27127;
const TATOMA: i32 = 27134;
// Misc
const MIN_LEVEL: i32 = 39;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

fn secret_letters(ctx: &QuestCtx) -> i64 {
    ctx.quest_items_count(SECRET_LETTER1)
        + ctx.quest_items_count(SECRET_LETTER2)
        + ctx.quest_items_count(SECRET_LETTER3)
        + ctx.quest_items_count(SECRET_LETTER4)
}

pub struct Q00226TestOfTheHealer;

impl QuestScript for Q00226TestOfTheHealer {
    fn id(&self) -> i32 {
        226
    }
    fn name(&self) -> &'static str {
        "Q00226_TestOfTheHealer"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00226_TestOfTheHealer"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PRIEST_BANDELLOS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            PRIEST_BANDELLOS,
            MASTER_SORIUS,
            ALLANA,
            PERRIN,
            FATHER_GUPU,
            ORPHAN_GIRL,
            WINDY_SHAORING,
            MYSTERIOUS_DARK_ELF,
            PIPER_LONGBOW,
            SLEIN_SHINING_BLADE,
            CAIN_FLYING_KNIFE,
            SAINT_KRISTINA,
            DAURIN_HAMMERCRUSH,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            LERO_LIZARDMAN_AGENT,
            LERO_LIZARDMAN_LEADER,
            LERO_LIZARDMAN_ASSASSIN,
            LERO_LIZARDMAN_SNIPER,
            LERO_LIZARDMAN_WIZARD,
            LERO_LIZARDMAN_LORD,
            TATOMA,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            REPORT_OF_PERRIN,
            CRISTINAS_LETTER,
            PICTURE_OF_WINDY,
            GOLDEN_STATUE,
            WINDYS_PEBBLES,
            ORDER_OF_SORIUS,
            SECRET_LETTER1,
            SECRET_LETTER2,
            SECRET_LETTER3,
            SECRET_LETTER4,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        let memo = ctx.memo_state();
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(REPORT_OF_PERRIN, 1);
                }
                None
            }
            "30473-08.html" => {
                if memo == 10 && has(ctx, GOLDEN_STATUE) {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30473-09.html" => {
                if memo == 10 && has(ctx, GOLDEN_STATUE) {
                    ctx.give_adena(233490, true);
                    ctx.give_items(MARK_OF_HEALER, 1);
                    ctx.add_exp_and_sp(738283, 50662);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some(event.to_string());
                }
                None
            }
            "30428-02.html" => {
                if memo == 1 && has(ctx, REPORT_OF_PERRIN) {
                    ctx.set_cond(2, true);
                    // TODO(G22): Java gates on getSummonedNpcCount() < 1.
                    ctx.spawn_attacker(TATOMA, true);
                }
                Some(event.to_string())
            }
            "30658-02.html" => {
                if memo == 4
                    && !has(ctx, PICTURE_OF_WINDY)
                    && !has(ctx, WINDYS_PEBBLES)
                    && !has(ctx, GOLDEN_STATUE)
                {
                    if ctx.quest_items_count(ADENA) >= 1000 {
                        ctx.take_items(ADENA, 1000);
                        ctx.give_items(PICTURE_OF_WINDY, 1);
                        ctx.set_cond(7, true);
                        return Some(event.to_string());
                    }
                    return Some("30658-05.html".to_string());
                }
                None
            }
            "30658-03.html" => {
                if memo == 4
                    && !has(ctx, PICTURE_OF_WINDY)
                    && !has(ctx, WINDYS_PEBBLES)
                    && !has(ctx, GOLDEN_STATUE)
                {
                    ctx.set_memo_state(5);
                    return Some(event.to_string());
                }
                None
            }
            "30658-07.html" => Some(event.to_string()),
            "30660-02.html" => {
                if has(ctx, PICTURE_OF_WINDY) {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30660-03.html" => {
                if has(ctx, PICTURE_OF_WINDY) {
                    ctx.take_items(PICTURE_OF_WINDY, 1);
                    ctx.give_items(WINDYS_PEBBLES, 1);
                    ctx.set_cond(8, true);
                    ctx.delete_npc();
                    return Some(event.to_string());
                }
                None
            }
            "30665-02.html" => {
                if secret_letters(ctx) == 4 {
                    ctx.give_items(CRISTINAS_LETTER, 1);
                    ctx.take_items(SECRET_LETTER1, 1);
                    ctx.take_items(SECRET_LETTER2, 1);
                    ctx.take_items(SECRET_LETTER3, 1);
                    ctx.take_items(SECRET_LETTER4, 1);
                    ctx.set_memo_state(9);
                    ctx.set_cond(22, true);
                    return Some(event.to_string());
                }
                None
            }
            "30674-02.html" => {
                if memo == 6 {
                    ctx.set_cond(11, false);
                    ctx.take_items(ORDER_OF_SORIUS, 1);
                    ctx.spawn_near_npc(LERO_LIZARDMAN_AGENT, true);
                    ctx.spawn_near_npc(LERO_LIZARDMAN_AGENT, true);
                    ctx.spawn_near_npc(LERO_LIZARDMAN_LEADER, true);
                    ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let memo = ctx.memo_state();
        match ctx.npc_id {
            LERO_LIZARDMAN_LEADER => {
                if memo == 6 && !has(ctx, SECRET_LETTER1) {
                    ctx.give_items(SECRET_LETTER1, 1);
                    ctx.set_cond(12, true);
                }
            }
            LERO_LIZARDMAN_ASSASSIN => {
                if memo == 8 && has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER2) {
                    ctx.give_items(SECRET_LETTER2, 1);
                    ctx.set_cond(15, true);
                }
            }
            LERO_LIZARDMAN_SNIPER => {
                if memo == 8 && has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER3) {
                    ctx.give_items(SECRET_LETTER3, 1);
                    ctx.set_cond(17, true);
                }
            }
            LERO_LIZARDMAN_LORD => {
                if memo == 8 && has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER4) {
                    ctx.give_items(SECRET_LETTER4, 1);
                    ctx.set_cond(19, true);
                }
            }
            TATOMA => {
                if memo == 1 {
                    ctx.set_memo_state(2);
                    ctx.set_cond(3, true);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        if ctx.is_created() {
            if ctx.npc_id == PRIEST_BANDELLOS {
                if ctx.is_in_category("WHITE_MAGIC_GROUP") {
                    return Some(if ctx.player_level() >= MIN_LEVEL {
                        "30473-03.htm".to_string()
                    } else {
                        "30473-01.html".to_string()
                    });
                }
                return Some("30473-02.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == PRIEST_BANDELLOS {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            PRIEST_BANDELLOS => {
                if (1..10).contains(&memo) {
                    Some("30473-05.html".to_string())
                } else if memo == 10 {
                    if has(ctx, GOLDEN_STATUE) {
                        Some("30473-07.html".to_string())
                    } else {
                        ctx.give_adena(266980, true);
                        ctx.give_items(MARK_OF_HEALER, 1);
                        ctx.add_exp_and_sp(1476566, 101324);
                        ctx.exit_quest(false, true);
                        ctx.social_action(3);
                        Some("30473-06.html".to_string())
                    }
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            MASTER_SORIUS => match memo {
                5 => {
                    ctx.give_items(ORDER_OF_SORIUS, 1);
                    ctx.set_memo_state(6);
                    ctx.set_cond(10, true);
                    Some("30327-01.html".to_string())
                }
                6..=8 => Some("30327-02.html".to_string()),
                9 if has(ctx, CRISTINAS_LETTER) => {
                    ctx.take_items(CRISTINAS_LETTER, 1);
                    ctx.set_memo_state(10);
                    ctx.set_cond(23, true);
                    Some("30327-03.html".to_string())
                }
                m if m >= 10 => Some("30327-04.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            ALLANA => match memo {
                3 => {
                    ctx.set_memo_state(4);
                    ctx.set_cond(5, true);
                    Some("30424-01.html".to_string())
                }
                4 => Some("30424-02.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            PERRIN => match memo {
                1 if has(ctx, REPORT_OF_PERRIN) => Some("30428-01.html".to_string()),
                2 => {
                    ctx.take_items(REPORT_OF_PERRIN, 1);
                    ctx.set_memo_state(3);
                    ctx.set_cond(4, true);
                    Some("30428-03.html".to_string())
                }
                3 => Some("30428-04.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            FATHER_GUPU => match memo {
                4 => {
                    if !has(ctx, PICTURE_OF_WINDY)
                        && !has(ctx, WINDYS_PEBBLES)
                        && !has(ctx, GOLDEN_STATUE)
                    {
                        ctx.set_cond(6, true);
                        Some("30658-01.html".to_string())
                    } else if has(ctx, PICTURE_OF_WINDY) {
                        Some("30658-04.html".to_string())
                    } else if has(ctx, WINDYS_PEBBLES) {
                        ctx.give_items(GOLDEN_STATUE, 1);
                        ctx.take_items(WINDYS_PEBBLES, 1);
                        ctx.set_memo_state(5);
                        Some("30658-06.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                }
                5 => {
                    ctx.set_cond(9, true);
                    Some("30658-07.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            ORPHAN_GIRL => Some(format!("30659-0{}.html", ctx.roll(5) + 1)),
            WINDY_SHAORING => {
                if has(ctx, PICTURE_OF_WINDY) {
                    Some("30660-01.html".to_string())
                } else if has(ctx, WINDYS_PEBBLES) {
                    Some("30660-04.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            MYSTERIOUS_DARK_ELF => {
                if memo == 8 {
                    if has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER2) {
                        // TODO(G22): getSummonedNpcCount()<36 / isSimulatingTalking guard.
                        ctx.spawn_near_npc(LERO_LIZARDMAN_ASSASSIN, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_ASSASSIN, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_ASSASSIN, true);
                        ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                        ctx.delete_npc();
                        ctx.set_cond(14, false);
                        Some("30661-01.html".to_string())
                    } else if has(ctx, SECRET_LETTER1)
                        && has(ctx, SECRET_LETTER2)
                        && !has(ctx, SECRET_LETTER3)
                    {
                        ctx.spawn_near_npc(LERO_LIZARDMAN_SNIPER, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_SNIPER, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_SNIPER, true);
                        ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                        ctx.delete_npc();
                        ctx.set_cond(16, false);
                        Some("30661-02.html".to_string())
                    } else if has(ctx, SECRET_LETTER1)
                        && has(ctx, SECRET_LETTER2)
                        && has(ctx, SECRET_LETTER3)
                        && !has(ctx, SECRET_LETTER4)
                    {
                        ctx.spawn_near_npc(LERO_LIZARDMAN_WIZARD, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_WIZARD, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_LORD, true);
                        ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                        ctx.delete_npc();
                        ctx.set_cond(18, false);
                        Some("30661-03.html".to_string())
                    } else if secret_letters(ctx) == 4 {
                        ctx.set_cond(20, true);
                        Some("30661-04.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            PIPER_LONGBOW => Some(direction_talk(ctx, memo, "30662")),
            SLEIN_SHINING_BLADE => Some(direction_talk(ctx, memo, "30663")),
            CAIN_FLYING_KNIFE => Some(cain_talk(ctx, memo)),
            SAINT_KRISTINA => {
                if secret_letters(ctx) == 4 {
                    Some("30665-01.html".to_string())
                } else if memo < 9 {
                    Some("30665-03.html".to_string())
                } else {
                    Some("30665-04.html".to_string())
                }
            }
            DAURIN_HAMMERCRUSH => match memo {
                6 => {
                    if has(ctx, ORDER_OF_SORIUS) {
                        Some("30674-01.html".to_string())
                    } else if !has(ctx, SECRET_LETTER1) && !has(ctx, ORDER_OF_SORIUS) {
                        // TODO(G22): getSummonedNpcCount()<4 / isSimulatingTalking guard.
                        ctx.spawn_near_npc(LERO_LIZARDMAN_AGENT, true);
                        ctx.spawn_near_npc(LERO_LIZARDMAN_LEADER, true);
                        Some("30674-02a.html".to_string())
                    } else if has(ctx, SECRET_LETTER1) {
                        ctx.set_memo_state(8);
                        ctx.set_cond(13, true);
                        Some("30674-03.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                }
                m if m >= 8 => Some("30674-04.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// Piper and Slein share the same three-line "which way did they go" script.
fn direction_talk(ctx: &mut QuestCtx, memo: i32, npc: &str) -> String {
    if memo == 8 {
        if has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER2) {
            format!("{npc}-01.html")
        } else if has(ctx, SECRET_LETTER2) && !has(ctx, SECRET_LETTER3) && !has(ctx, SECRET_LETTER4)
        {
            format!("{npc}-02.html")
        } else if has(ctx, SECRET_LETTER2) && has(ctx, SECRET_LETTER3) && has(ctx, SECRET_LETTER4) {
            ctx.set_cond(21, true);
            format!("{npc}-03.html")
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn cain_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    if memo == 8 {
        if has(ctx, SECRET_LETTER1) && !has(ctx, SECRET_LETTER4) {
            "30664-01.html".to_string()
        } else if has(ctx, SECRET_LETTER2) && !has(ctx, SECRET_LETTER3) && !has(ctx, SECRET_LETTER4)
        {
            "30664-02.html".to_string()
        } else if has(ctx, SECRET_LETTER2) && has(ctx, SECRET_LETTER3) && has(ctx, SECRET_LETTER4) {
            ctx.set_cond(21, true);
            "30664-03.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}
