//! Q255 Tutorial — port of
//! `dist/game/data/scripts/quests/Q00255_Tutorial/Q00255_Tutorial.java`, the
//! newbie starting quest. Login (level ≤ 6, first-occupation class) queues a
//! 5 s timer that starts the quest and opens the tutorial window; the
//! memoState machine then walks: intro (1) → Newbie Helper briefing (2) →
//! Blue Gemstone from gremlins (3) → helper hands out shots (4) → supervisor
//! reward (5) → Newbie Guide's second batch (6, in `newbie_guide.rs`).
//!
//! The quest never exits — memoState 6 (or outgrowing level 6) is simply
//! where it stops reacting.

use crate::game_loop::quests::{QuestCtx, QuestScript};

pub const QUEST_NAME: &str = "Q00255_Tutorial";

/// Race ordinal for the orc-mystic special cases.
const ORC: i32 = 3;

/// Newbie Helpers per starter village (the supervisors are the other half of
/// `talk_npcs`; `on_first_talk` branches on membership here).
const NEWBIE_HELPERS: &[i32] = &[30009, 30019, 30400, 30131, 30575, 30530];
const GREMLINS: &[i32] = &[18342, 20001];

const BLUE_GEM: i32 = 6353;
const SOULSHOT_NO_GRADE: i32 = 5789;
const SPIRITSHOT_NO_GRADE: i32 = 5790;

/// Per first-occupation class id: `(voice, intro html, helper loc, complete
/// loc)` — Java's `STARTING_VOICE_HTML` / `HELPER_LOCATION` /
/// `COMPLETE_LOCATION` tables.
type ClassRow = (&'static str, &'static str, (i32, i32, i32), (i32, i32, i32));

fn class_row(class_id: i32) -> Option<ClassRow> {
    Some(match class_id {
        0 => (
            "tutorial_voice_001a",
            "tutorial_human_fighter001.html",
            (-71424, 258336, -3109),
            (-84081, 243227, -3723),
        ),
        10 => (
            "tutorial_voice_001b",
            "tutorial_human_mage001.html",
            (-91036, 248044, -3568),
            (-84081, 243227, -3723),
        ),
        18 => (
            "tutorial_voice_001c",
            "tutorial_elven_fighter001.html",
            (46112, 41200, -3504),
            (45475, 48359, -3060),
        ),
        25 => (
            "tutorial_voice_001d",
            "tutorial_elven_mage001.html",
            (46112, 41200, -3504),
            (45475, 48359, -3060),
        ),
        31 => (
            "tutorial_voice_001e",
            "tutorial_delf_fighter001.html",
            (28384, 11056, -4233),
            (12111, 16686, -4582),
        ),
        38 => (
            "tutorial_voice_001f",
            "tutorial_delf_mage001.html",
            (28384, 11056, -4233),
            (12111, 16686, -4582),
        ),
        44 => (
            "tutorial_voice_001g",
            "tutorial_orc_fighter001.html",
            (-56736, -113680, -672),
            (-45032, -113598, -192),
        ),
        49 => (
            "tutorial_voice_001h",
            "tutorial_orc_mage001.html",
            (-56736, -113680, -672),
            (-45032, -113598, -192),
        ),
        53 => (
            "tutorial_voice_001i",
            "tutorial_dwarven_fighter001.html",
            (108567, -173994, -406),
            (115632, -177996, -905),
        ),
        _ => return None,
    })
}

/// Mage-and-not-orc gets spiritshots, everyone else soulshots (an orc mystic
/// fights with soulshot-fed fists).
fn give_shots(ctx: &mut QuestCtx) {
    if ctx.is_in_category("MAGE_GROUP") && ctx.player_race() != ORC {
        ctx.give_items(SPIRITSHOT_NO_GRADE, 100);
        ctx.play_tutorial_voice("tutorial_voice_027");
    } else {
        ctx.give_items(SOULSHOT_NO_GRADE, 200);
        ctx.play_tutorial_voice("tutorial_voice_026");
    }
}

pub struct Tutorial;

impl QuestScript for Tutorial {
    fn id(&self) -> i32 {
        255
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00255_Tutorial"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        // Helpers + supervisors (Java `addTalkId` on both groups).
        &[
            30009, 30019, 30400, 30131, 30575, 30530, //
            30008, 30017, 30370, 30129, 30573, 30528,
        ]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        // Same set: the script owns both groups' chat windows.
        &[
            30009, 30019, 30400, 30131, 30575, 30530, //
            30008, 30017, 30370, 30129, 30573, 30528,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        GREMLINS
    }
    fn quest_items(&self) -> &[i32] {
        &[BLUE_GEM]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `onPlayerLogin`: a fresh (or still-early) newbie queues the intro.
    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial || ctx.player_level() > 6 {
            return;
        }
        // Java `getQuestState(player, true)` — materialize the CREATED state.
        ctx.ensure_qs();
        if ctx.memo_state() < 4 && class_row(ctx.player_class_id()).is_some() {
            ctx.start_quest_timer("start_newbie_tutorial", 5000);
        }
    }

    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        if name == "start_newbie_tutorial" {
            self.on_event(ctx, "start_newbie_tutorial");
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "start_newbie_tutorial" => {
                if ctx.memo_state() < 4 {
                    if let Some((voice, html, _, _)) = class_row(ctx.player_class_id()) {
                        ctx.start_quest();
                        ctx.set_memo_state(1);
                        ctx.play_tutorial_voice(voice);
                        ctx.tutorial_show_html_file(html);
                    }
                }
                None
            }
            "tutorial_02.html" | "tutorial_03.html" => {
                if ctx.is_memo_state(1) {
                    ctx.tutorial_show_html_file(event);
                }
                None
            }
            "question_mark_1" => {
                if ctx.is_memo_state(1) {
                    ctx.tutorial_show_question_mark(1);
                    ctx.tutorial_close_html();
                }
                None
            }
            "reward_2" => {
                if !ctx.is_memo_state(4) {
                    return None;
                }
                ctx.set_memo_state(5);
                give_shots(ctx);
                // Java derives the html's npc id from the packet npc or —
                // on the npc-less bypass path — the player's target.
                let npc_id = if ctx.npc_id != 0 {
                    ctx.npc_id
                } else {
                    ctx.player_target_npc_id()
                };
                ctx.tutorial_show_question_mark(28);
                if npc_id != 0 {
                    Some(format!("{npc_id}-3.html"))
                } else {
                    None
                }
            }
            "close_tutorial" => {
                ctx.tutorial_close_html();
                None
            }
            _ => None,
        }
    }

    /// `onFirstTalk` — the helpers/supervisors own their chat windows.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc_id = ctx.npc_id;
        ctx.ensure_qs();
        if NEWBIE_HELPERS.contains(&npc_id) {
            // Holding the gem forces the turn-in step even if the pickup
            // event was missed.
            if ctx.quest_items_count(BLUE_GEM) > 0 && ctx.memo_state() < 3 {
                ctx.set_memo_state(3);
            }
            let mystic = ctx.is_in_category("MAGE_GROUP");
            let orc = ctx.player_race() == ORC;
            match ctx.memo_state() {
                0 | 1 => {
                    ctx.tutorial_close_html();
                    ctx.set_memo_state(2);
                    Some(
                        match (mystic, orc) {
                            (false, _) => "tutorial_05_fighter.html",
                            (true, true) => "tutorial_05_mystic_orc.html",
                            (true, false) => "tutorial_05_mystic.html",
                        }
                        .to_string(),
                    )
                }
                2 => Some(
                    match (mystic, orc) {
                        (false, _) => "tutorial_05_fighter_back.html",
                        (true, true) => "tutorial_05_mystic_orc_back.html",
                        (true, false) => "tutorial_05_mystic_back.html",
                    }
                    .to_string(),
                ),
                3 => {
                    ctx.tutorial_close_html();
                    ctx.set_memo_state(4);
                    ctx.take_items(BLUE_GEM, -1);
                    give_shots(ctx);
                    if mystic && !orc {
                        Some(format!("{npc_id}-3.html"))
                    } else {
                        Some(format!("{npc_id}-2.html"))
                    }
                }
                4 => Some(format!("{npc_id}-4.html")),
                _ => Some(format!("{npc_id}-5.html")),
            }
        } else {
            // Supervisors.
            match ctx.memo_state() {
                0..=3 => Some(format!("{npc_id}-1.html")),
                4 => Some(format!("{npc_id}-2.html")),
                _ => Some(format!("{npc_id}-4.html")),
            }
        }
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        // The chat window is fully owned by `on_first_talk`.
        None
    }

    /// `onKill`: a gremlin has a 30% chance to toss a Blue Gemstone, capped
    /// at 10 gems lying within 1500 units.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.is_memo_state(2) || ctx.quest_items_count(BLUE_GEM) > 0 {
            return;
        }
        if ctx.world.roll(100) >= 30 {
            return;
        }
        if ctx.count_ground_items_near_npc(BLUE_GEM, 1500.0) >= 10 {
            return;
        }
        ctx.drop_item_from_npc(BLUE_GEM, 1);
    }

    /// `ON_PLAYER_ITEM_PICKUP` on the Blue Gemstone.
    fn on_item_pickup(&self, ctx: &mut QuestCtx, item_id: i32) {
        if item_id != BLUE_GEM || ctx.memo_state() >= 3 || !ctx.is_started() {
            return;
        }
        ctx.set_memo_state(3);
        ctx.play_sound("ItemSound.quest_tutorial");
        ctx.play_tutorial_voice("tutorial_voice_013");
        ctx.tutorial_show_question_mark(5);
    }

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK` — marks 1/5/28 belong to this quest.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        let Some((_, _, helper_loc, complete_loc)) = class_row(ctx.player_class_id()) else {
            return;
        };
        match mark_id {
            1 if ctx.is_memo_state(1) => {
                ctx.show_screen_message("Speak with the Newbie Helper", 2, 5000);
                ctx.add_radar(helper_loc.0, helper_loc.1, helper_loc.2);
                ctx.tutorial_show_html_file("tutorial_04.html");
            }
            5 if ctx.is_memo_state(3) => {
                ctx.add_radar(helper_loc.0, helper_loc.1, helper_loc.2);
                ctx.tutorial_show_html_file("tutorial_06.html");
            }
            28 if ctx.is_memo_state(5) => {
                ctx.add_radar(complete_loc.0, complete_loc.1, complete_loc.2);
                ctx.play_sound("ItemSound.quest_tutorial");
            }
            _ => {}
        }
    }
}
