//! The Name of Evil - 1 (125) — `quests/Q00125_TheNameOfEvil1`. A level-76
//! Primeval Isle story quest (after [`Q124`](super::q00124_meeting_the_elroki)):
//! Mushika sends the player to Karakawei, then to grind Ornithomimus Claws and
//! Deinonychus Bones, and finally to decrypt the "ancient word" from the three
//! Kaimu pillars (Ulu, Balu, Chuta) — a letter puzzle spelling the name of evil
//! — earning the Epitaph of Wisdom.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const MUSHIKA: i32 = 32114;
const KARAKAWEI: i32 = 32117;
const ULU_KAIMU: i32 = 32119;
const BALU_KAIMU: i32 = 32120;
const CHUTA_KAIMU: i32 = 32121;
/// Java `REPRESENTATION_ENTER_THE_SAILREN_NEST_QUEST_ID` — the visual each
/// Kaimu pillar casts as it presents its riddle.
const PUZZLE_FLOURISH: i32 = 5089;
// Items
const ORNITHOMIMUS_CLAW: i32 = 8779;
const DEINONYCHUS_BONE: i32 = 8780;
const EPITAPH_OF_WISDOM: i32 = 8781;
const GAZKH_FRAGMENT: i32 = 8782;
// Misc
const MIN_LEVEL: i32 = 76;
const PREREQ: &str = "Q00124_MeetingTheElroki";

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// Ornithomimus id → Claw drop chance (out of 1000), Java's `ORNITHOMIMUS` map.
fn claw_chance(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        22200 | 22202 => 661,
        22201 => 330,
        22219 | 22224 => 327,
        _ => return None,
    })
}

/// Deinonychus id → Bone drop chance (out of 1000), Java's `DEINONYCHUS` map.
fn bone_chance(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        22203 | 22205 => 651,
        22204 => 326,
        22220 | 22225 => 319,
        _ => return None,
    })
}

pub struct Q00125TheNameOfEvil1;

impl QuestScript for Q00125TheNameOfEvil1 {
    fn id(&self) -> i32 {
        125
    }
    fn name(&self) -> &'static str {
        "Q00125_TheNameOfEvil1"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00125_TheNameOfEvil1"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MUSHIKA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MUSHIKA, KARAKAWEI, ULU_KAIMU, BALU_KAIMU, CHUTA_KAIMU]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            22200, 22201, 22202, 22219, 22224, // Ornithomimus
            22203, 22204, 22205, 22220, 22225, // Deinonychus
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            ORNITHOMIMUS_CLAW,
            DEINONYCHUS_BONE,
            EPITAPH_OF_WISDOM,
            GAZKH_FRAGMENT,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "32114-05.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "32114-08.html" => {
                if ctx.is_cond(1) {
                    ctx.give_items(GAZKH_FRAGMENT, 1);
                    ctx.set_cond(2, true);
                }
                Some(event.to_string())
            }
            "32117-09.html" => {
                if ctx.is_cond(2) {
                    ctx.set_cond(3, true);
                }
                Some(event.to_string())
            }
            "32117-15.html" => {
                if ctx.is_cond(4) {
                    ctx.set_cond(5, true);
                }
                Some(event.to_string())
            }
            // --- Ulu Kaimu puzzle (cond 5): T-E-P-U ---
            "T_One" => {
                ctx.set_var("T", "1");
                Some("32119-04.html".to_string())
            }
            "E_One" => {
                ctx.set_var("E", "1");
                Some("32119-05.html".to_string())
            }
            "P_One" => {
                ctx.set_var("P", "1");
                Some("32119-06.html".to_string())
            }
            "U_One" => {
                ctx.set_var("U", "1");
                let solved = ctx.is_cond(5)
                    && ctx.get_int("T") > 0
                    && ctx.get_int("E") > 0
                    && ctx.get_int("P") > 0
                    && ctx.get_int("U") > 0;
                let html = if solved {
                    ctx.set_var("Memo", "1");
                    "32119-08.html"
                } else {
                    "32119-07.html"
                };
                for v in ["T", "E", "P", "U"] {
                    ctx.unset(v);
                }
                Some(html.to_string())
            }
            "32119-07.html" => {
                for v in ["T", "E", "P", "U"] {
                    ctx.unset(v);
                }
                Some(event.to_string())
            }
            "32119-18.html" => {
                if ctx.is_cond(5) {
                    ctx.set_cond(6, true);
                    ctx.unset("Memo");
                }
                Some(event.to_string())
            }
            // --- Balu Kaimu puzzle (cond 6): T-O-O2-N ---
            "T_Two" => {
                ctx.set_var("T", "1");
                Some("32120-04.html".to_string())
            }
            "O_Two" => {
                ctx.set_var("O", "1");
                Some("32120-05.html".to_string())
            }
            "O2_Two" => {
                ctx.set_var("O2", "1");
                Some("32120-06.html".to_string())
            }
            "N_Two" => {
                ctx.set_var("N", "1");
                let solved = ctx.is_cond(6)
                    && ctx.get_int("T") > 0
                    && ctx.get_int("O") > 0
                    && ctx.get_int("O2") > 0
                    && ctx.get_int("N") > 0;
                let html = if solved {
                    ctx.set_var("Memo", "1");
                    "32120-08.html"
                } else {
                    "32120-07.html"
                };
                for v in ["T", "O", "O2", "N"] {
                    ctx.unset(v);
                }
                Some(html.to_string())
            }
            "32120-07.html" => {
                for v in ["T", "O", "O2", "N"] {
                    ctx.unset(v);
                }
                Some(event.to_string())
            }
            "32120-17.html" => {
                if ctx.is_cond(6) {
                    ctx.set_cond(7, true);
                    ctx.unset("Memo");
                }
                Some(event.to_string())
            }
            // --- Chuta Kaimu puzzle (cond 7): W-A-G-U ---
            "W_Three" => {
                ctx.set_var("W", "1");
                Some("32121-04.html".to_string())
            }
            "A_Three" => {
                ctx.set_var("A", "1");
                Some("32121-05.html".to_string())
            }
            "G_Three" => {
                ctx.set_var("G", "1");
                Some("32121-06.html".to_string())
            }
            "U_Three" => {
                ctx.set_var("U", "1");
                let solved = ctx.is_cond(7)
                    && ctx.get_int("W") > 0
                    && ctx.get_int("A") > 0
                    && ctx.get_int("G") > 0
                    && ctx.get_int("U") > 0;
                let html = if solved {
                    ctx.set_var("Memo", "1");
                    "32121-08.html"
                } else {
                    "32121-07.html"
                };
                for v in ["W", "A", "G", "U"] {
                    ctx.unset(v);
                }
                Some(html.to_string())
            }
            "32121-07.html" => {
                for v in ["W", "A", "G", "U"] {
                    ctx.unset(v);
                }
                Some(event.to_string())
            }
            "32121-11.html" => {
                ctx.set_var("Memo", "2");
                Some(event.to_string())
            }
            "32121-16.html" => {
                ctx.set_var("Memo", "3");
                Some(event.to_string())
            }
            "32121-18.html" => {
                if ctx.is_cond(7) && has(ctx, GAZKH_FRAGMENT) {
                    ctx.give_items(EPITAPH_OF_WISDOM, 1);
                    ctx.take_items(GAZKH_FRAGMENT, -1);
                    ctx.set_cond(8, true);
                    ctx.unset("Memo");
                }
                Some(event.to_string())
            }
            // Navigation htmls (including wrong letter picks) just load.
            _ if event.ends_with(".html") || event.ends_with(".htm") => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let rate = ctx.rate_quest_drop();
        if let Some(chance) = claw_chance(ctx.npc_id) {
            if ctx.quest_items_count(ORNITHOMIMUS_CLAW) < 2
                && (ctx.roll(1000) as f64) < chance as f64 * rate
            {
                ctx.give_items(ORNITHOMIMUS_CLAW, 1);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        } else if let Some(chance) = bone_chance(ctx.npc_id)
            && ctx.quest_items_count(DEINONYCHUS_BONE) < 2
            && (ctx.roll(1000) as f64) < chance as f64 * rate
        {
            ctx.give_items(DEINONYCHUS_BONE, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
        if ctx.quest_items_count(ORNITHOMIMUS_CLAW) == 2
            && ctx.quest_items_count(DEINONYCHUS_BONE) == 2
        {
            ctx.set_cond(4, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            MUSHIKA => mushika_talk(ctx, cond),
            KARAKAWEI if ctx.is_started() => karakawei_talk(ctx, cond),
            ULU_KAIMU if ctx.is_started() => ulu_talk(ctx, cond),
            BALU_KAIMU if ctx.is_started() => balu_talk(ctx, cond),
            CHUTA_KAIMU if ctx.is_started() => chuta_talk(ctx, cond),
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}

fn mushika_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    if ctx.is_created() {
        return if ctx.player_level() < MIN_LEVEL {
            "32114-01a.htm".to_string()
        } else if ctx.other_quest_completed(PREREQ) {
            "32114-01.htm".to_string()
        } else {
            "32114-01b.htm".to_string()
        };
    }
    if ctx.is_completed() {
        return ctx.already_completed_html();
    }
    match cond {
        1 => "32114-09.html".to_string(),
        2 => "32114-10.html".to_string(),
        3..=7 => "32114-11.html".to_string(),
        8 => {
            if has(ctx, EPITAPH_OF_WISDOM) {
                ctx.add_exp_and_sp(859_195, 86_603);
                ctx.exit_quest(false, true);
                "32114-12.html".to_string()
            } else {
                ctx.no_quest_html()
            }
        }
        _ => ctx.no_quest_html(),
    }
}

fn karakawei_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1 => "32117-01.html".to_string(),
        2 => "32117-02.html".to_string(),
        3 => "32117-10.html".to_string(),
        4 => {
            if ctx.quest_items_count(ORNITHOMIMUS_CLAW) >= 2
                && ctx.quest_items_count(DEINONYCHUS_BONE) >= 2
            {
                ctx.take_items(ORNITHOMIMUS_CLAW, -1);
                ctx.take_items(DEINONYCHUS_BONE, -1);
                "32117-11.html".to_string()
            } else {
                ctx.no_quest_html()
            }
        }
        5 => "32117-16.html".to_string(),
        6 | 7 => "32117-17.html".to_string(),
        8 => "32117-18.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

/// `npc.broadcastPacket(new MagicSkillUse(npc, player, 5089, 1, 1000, 0))` —
/// the Kaimu pillar's flourish as it presents its riddle.
///
/// All three pillars do this, at their own cond (Ulu 5, Balu 6, Chuta 7), and
/// each Java site is the same call. The port carried a single `cosmetic`
/// deferral marker on Ulu, which undercounted the gap threefold — the other two
/// were missing it with nothing to say so.
fn present_puzzle(ctx: &QuestCtx) {
    ctx.cast_visual_at(ctx.npc, ctx.player, PUZZLE_FLOURISH, 1, 1000);
}

/// Ulu Kaimu (live at cond 5).
fn ulu_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1..=4 => "32119-01.html".to_string(),
        5 => {
            if ctx.get_int("Memo") == 0 {
                present_puzzle(ctx);
                for v in ["T", "E", "P", "U"] {
                    ctx.unset(v);
                }
                "32119-02.html".to_string()
            } else {
                "32119-09.html".to_string()
            }
        }
        6 => "32119-18.html".to_string(),
        _ => "32119-19.html".to_string(),
    }
}

/// Balu Kaimu (live at cond 6). The past-pillar default reproduces Java's
/// copy-paste quirk: it shows Ulu's `32119-18`, not a `32120` page.
fn balu_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1..=5 => "32120-01.html".to_string(),
        6 => {
            if ctx.get_int("Memo") == 0 {
                present_puzzle(ctx);
                for v in ["T", "O", "O2", "N"] {
                    ctx.unset(v);
                }
                "32120-02.html".to_string()
            } else {
                "32120-09.html".to_string()
            }
        }
        7 => "32120-17.html".to_string(),
        _ => "32119-18.html".to_string(),
    }
}

fn chuta_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1..=6 => "32121-01.html".to_string(),
        7 => {
            if ctx.get_int("Memo") == 0 {
                present_puzzle(ctx);
                for v in ["W", "A", "G", "U"] {
                    ctx.unset(v);
                }
                "32121-02.html".to_string()
            } else {
                match ctx.get_int("Memo") {
                    1 => "32121-09.html".to_string(),
                    2 => "32121-19.html".to_string(),
                    3 => "32121-20.html".to_string(),
                    _ => ctx.no_quest_html(),
                }
            }
        }
        8 => "32121-21.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}
