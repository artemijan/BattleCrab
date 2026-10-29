//! Winds of Fate: A Promise (10752) — port of
//! `dist/game/data/scripts/quests/Q10752_WindsOfFateAPromise/`, the Ertheia
//! second class transfer.
//!
//! Katalin (fighters) or Ayanthe (mages) start it once Q10751 is done.
//! Karla gives Navari's Mark and the machine fragment; Siegmund, Lombert,
//! the Mysterious Wizard, the tombstone and Von Hellman's ghost lead to the
//! Broken Bookshelf, whose page enters Fortress of the Dead
//! ([`super::fortress_of_the_dead`]). The fortress's end sets cond 9 and
//! gives Kain's fragment; Karla's `checkClass` then sends the player to
//! Ayanthe (Stratomancer) or Katalin (Ripper).
//!
//! Java returns no page for most of its cond-advancing events (they are
//! missing from the `htmltext = event` list): those advance silently, here
//! too.

use super::q10751_winds_of_fate_encounters::{
    self as q10751, ERTHEIA, TOP_CENTER, is_child_of_current, tutorial_bypass,
};
use crate::game_loop::quests::{QuestCtx, QuestScript};

pub const QUEST_NAME: &str = "Q10752_WindsOfFateAPromise";

// NPCs
const KATALIN: i32 = 33943;
const AYANTHE: i32 = 33942;
const KARLA: i32 = 33933;
const SIEGMUND: i32 = 31321;
const LOMBERT: i32 = 31317;
const MYSTERIOUS_WIZARD: i32 = 31522;
const TOMBSTONE: i32 = 31523;
const GHOST_OF_VON_HELLMAN: i32 = 31524;
const BROKEN_BOOKSHELF: i32 = 31526;

// Items
const NAVARI_MARK: i32 = 39536;
const PROPHECY_MACHINE_FRAGMENT: i32 = 39537;
pub(super) const KAIN_PROPHECY_MACHINE_FRAGMENT: i32 = 39538;
const SOULSHOT: i32 = 1467;
const SPIRITSHOT: i32 = 3952;
const BLESSED_SCROLL_OF_RESURRECTION: i32 = 33518;
const PAULINAS_EQUIPMENT_SET: i32 = 46852;

// Locations
const TELEPORT_LOC: (i32, i32, i32) = (-81948, 249635, -3371);
const GHOST_SPAWN_LOC: (i32, i32, i32, i32) = (51483, -54407, -3160, 0);

// Misc
const MIN_LEVEL: i32 = 76;
/// `PlayerClass.RIPPER` / `STRATOMANCER`.
const RIPPER: i32 = 186;
const STRATOMANCER: i32 = 187;

// NpcStringIds
const TALK_TO_THE_GHOST_OF_VON_HELLMANN: i32 = 1802589;
const TIME_TO_MOVE_ONTO_THE_NEXT_PLACE: i32 = 1802509;
pub(super) const MAGISTER_AYANTHE_HAS_SENT_A_LETTER: i32 = 1802611;
pub(super) const MASTER_KATALIN_HAS_SENT_A_LETTER: i32 = 1802610;

pub struct Q10752WindsOfFateAPromise;

impl Q10752WindsOfFateAPromise {
    /// The Stratomancer / Ripper transfer and its payout.
    fn change_class(ctx: &mut QuestCtx, cond: i32, new_class: i32) {
        if !ctx.is_cond(cond) || !is_child_of_current(ctx, new_class) {
            return;
        }
        ctx.set_class_id(new_class);
        ctx.send_skill_list();
        ctx.social_action(24);
        ctx.give_adena(1_599_120, false);
        ctx.give_items(SOULSHOT, 8000);
        ctx.give_items(SPIRITSHOT, 8000);
        ctx.give_items(BLESSED_SCROLL_OF_RESURRECTION, 3);
        ctx.give_items(PAULINAS_EQUIPMENT_SET, 1);
        ctx.add_exp_and_sp(42_000_000, 0);
        ctx.exit_quest(false, true);
    }

    /// A step event: `from` → `from + 1`, no page.
    fn step(ctx: &mut QuestCtx, from: i32) {
        if ctx.is_cond(from) {
            ctx.set_cond(from + 1, true);
        }
    }

    /// The Created-state page of Katalin or Ayanthe: `own` for the
    /// matching side once Q10751 is complete from level 76, else `refuse`.
    /// Java dereferences Q10751's state unguarded, so without one the talk
    /// dies (`None`).
    fn created_talk(ctx: &QuestCtx, own_side: bool, own: &str, refuse: &str) -> Option<String> {
        if ctx.player_level() < MIN_LEVEL {
            return Some(refuse.to_string());
        }
        if !ctx.has_other_quest_state(q10751::QUEST_NAME) {
            return None;
        }
        if ctx.other_quest_completed(q10751::QUEST_NAME) && own_side {
            return Some(own.to_string());
        }
        Some(refuse.to_string())
    }
}

/// The login and level-up letter of Q10752 and Q10753: Ayanthe's for a
/// mage, Katalin's for a fighter, then the question mark.
pub(super) fn show_side_letter(ctx: &mut QuestCtx, quest_id: i32) {
    let letter = if ctx.is_mage_class() {
        MAGISTER_AYANTHE_HAS_SENT_A_LETTER
    } else {
        MASTER_KATALIN_HAS_SENT_A_LETTER
    };
    ctx.send_screen_message_npc_string(letter, TOP_CENTER, 10000);
    ctx.tutorial_show_question_mark(quest_id, 1);
}

/// The question-mark popup of Q10752 and Q10753: the side's voice line,
/// then `popup1.html` (mage) or `popup2.html` (fighter).
pub(super) fn show_side_popup(ctx: &mut QuestCtx, mage_voice: &str, fighter_voice: &str) {
    if ctx.is_mage_class() {
        ctx.play_voice(mage_voice);
        ctx.tutorial_show_html_file("popup1.html");
    } else {
        ctx.play_voice(fighter_voice);
        ctx.tutorial_show_html_file("popup2.html");
    }
}

impl QuestScript for Q10752WindsOfFateAPromise {
    fn id(&self) -> i32 {
        10752
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10752_WindsOfFateAPromise"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KATALIN, AYANTHE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            KATALIN,
            AYANTHE,
            KARLA,
            SIEGMUND,
            LOMBERT,
            MYSTERIOUS_WIZARD,
            GHOST_OF_VON_HELLMAN,
            TOMBSTONE,
            BROKEN_BOOKSHELF,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            NAVARI_MARK,
            PROPHECY_MACHINE_FRAGMENT,
            KAIN_PROPHECY_MACHINE_FRAGMENT,
        ]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `addCondRace(ERTHEIA, "")` and
    /// `addCondInCategory(ERTHEIA_SECOND_CLASS_GROUP, "")`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() != ERTHEIA || !ctx.is_in_category("ERTHEIA_SECOND_CLASS_GROUP") {
            return Some(String::new());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "33933-02.html" | "33933-03.html" | "33943-02.htm" | "33943-03.htm"
            | "33943-04.htm" | "33942-02.htm" | "33942-03.htm" | "33942-04.htm"
            | "31522-02.html" | "31522-03.html" | "31522-04.html" | "31321-02.html"
            | "31317-02.html" | "31317-03.html" | "31317-04.html" | "31523-02.html"
            | "31524-02.html" | "31526-02.html" | "33933-07.html" | "33933-08.html"
            | "33943-08.html" | "33943-09.html" | "33942-08.html" | "33942-11.html" => {
                return Some(event.to_string());
            }
            "33942-05.html" | "33943-05.html" => ctx.start_quest(),
            "33933-04.html" => {
                if ctx.is_cond(1) {
                    ctx.give_items(NAVARI_MARK, 1);
                    ctx.give_items(PROPHECY_MACHINE_FRAGMENT, 1);
                    ctx.set_cond(2, true);
                }
            }
            "31321-03.html" => Self::step(ctx, 2),
            "31317-05.html" => Self::step(ctx, 3),
            "31522-05.html" => Self::step(ctx, 4),
            "31523-03.html" => {
                Self::step(ctx, 5);
                // Java spawns the ghost on every click, whatever the cond.
                ctx.send_screen_message_npc_string(
                    TALK_TO_THE_GHOST_OF_VON_HELLMANN,
                    TOP_CENTER,
                    5000,
                );
                ctx.add_spawn(GHOST_OF_VON_HELLMAN, GHOST_SPAWN_LOC, true, 20000, 0);
            }
            "31524-03.html" => {
                if ctx.is_cond(6) {
                    ctx.set_cond(7, true);
                    ctx.send_screen_message_npc_string(
                        TIME_TO_MOVE_ONTO_THE_NEXT_PLACE,
                        TOP_CENTER,
                        5000,
                    );
                }
            }
            "31526-03.html" => Self::step(ctx, 7),
            "checkClass" => {
                if ctx.is_cond(9) {
                    ctx.take_items(NAVARI_MARK, -1);
                    ctx.take_items(PROPHECY_MACHINE_FRAGMENT, -1);
                    ctx.take_items(KAIN_PROPHECY_MACHINE_FRAGMENT, -1);
                    if ctx.is_mage_class() {
                        ctx.set_cond(10, true);
                        return Some("33933-10.html".to_string());
                    }
                    ctx.set_cond(11, true);
                    return Some("33933-09.html".to_string());
                }
            }
            "33942-12.html" => Self::change_class(ctx, 10, STRATOMANCER),
            "33943-10.html" => Self::change_class(ctx, 11, RIPPER),
            _ => {}
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        let mage = ctx.is_mage_class();
        if ctx.is_created() {
            return match ctx.npc_id {
                KATALIN => Self::created_talk(ctx, !mage, "33943-01.htm", "33943-12.html"),
                AYANTHE => Self::created_talk(ctx, mage, "33942-01.htm", "33942-09.html"),
                _ => Some(ctx.no_quest_html()),
            };
        }
        let cond = ctx.cond();
        let html = match (ctx.npc_id, cond) {
            (KATALIN, 1) => "33943-06.html",
            (KATALIN, 11) => "33943-07.html",
            (AYANTHE, 1) => "33942-06.html",
            (AYANTHE, 10) => "33942-07.html",
            (KARLA, 1) => "33933-01.html",
            (KARLA, 2) => "33933-05.html",
            (KARLA, 9) => "33933-06.html",
            (KARLA, 10) => "33933-12.html",
            (KARLA, 11) => "33933-11.html",
            (SIEGMUND, 2) => "31321-01.html",
            (SIEGMUND, 3) => "31321-04.html",
            (LOMBERT, 3) => "31317-01.html",
            (LOMBERT, 4) => "31317-06.html",
            (MYSTERIOUS_WIZARD, 4) => "31522-01.html",
            (MYSTERIOUS_WIZARD, 5) => "31522-06.html",
            (TOMBSTONE, 5 | 6) => "31523-01.html",
            (TOMBSTONE, 7) => "31523-03.html",
            (GHOST_OF_VON_HELLMAN, 6) => "31524-01.html",
            (GHOST_OF_VON_HELLMAN, 7) => "31524-03.html",
            (BROKEN_BOOKSHELF, 7) => "31526-01.html",
            (BROKEN_BOOKSHELF, 8) => "31526-03.html",
            (BROKEN_BOOKSHELF, 9) => "31526-04.html",
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id == self.id() && !ctx.has_qs() {
            show_side_popup(ctx, "Npcdialog1.aris_quest_1", "Npcdialog1.katrina_quest_1");
        }
    }

    fn on_player_bypass(&self, ctx: &mut QuestCtx, command: &str) {
        tutorial_bypass(ctx, self.id(), command, TELEPORT_LOC);
    }

    fn on_level_changed(&self, ctx: &mut QuestCtx, old_level: i32, new_level: i32) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if !ctx.has_qs()
            && ctx.player_race() == ERTHEIA
            && old_level < new_level
            && new_level >= MIN_LEVEL
            && ctx.is_in_category("ERTHEIA_SECOND_CLASS_GROUP")
        {
            show_side_letter(ctx, self.id());
        }
    }

    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if !ctx.has_qs()
            && ctx.player_race() == ERTHEIA
            && ctx.player_level() >= MIN_LEVEL
            && ctx.is_in_category("ERTHEIA_SECOND_CLASS_GROUP")
        {
            show_side_letter(ctx, self.id());
        }
    }
}
