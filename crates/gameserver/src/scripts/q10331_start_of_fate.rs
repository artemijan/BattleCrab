//! Start of Fate (10331) — port of
//! `dist/game/data/scripts/quests/Q10331_StartOfFate/`, the Salvation first
//! class transfer.
//!
//! Sebion sends the player into the Labyrinth of Belis ([`super::labyrinth_of_belis`]),
//! whose boss kill moves cond 1 to 2. Back at Sebion the player gets Saril's
//! Necklace and a race-specific cond (3–8), and the race's master offers the
//! first classes. The `classChange;<id>` event changes the class and pays out.
//!
//! From level 18, the login and the level-up show question mark 10331, whose
//! click opens the race's popup.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

pub const QUEST_NAME: &str = "Q10331_StartOfFate";

// NPCs
const FRANCO: i32 = 32153;
const RIVIAN: i32 = 32147;
const DEVON: i32 = 32160;
const TOOK: i32 = 32150;
const MOKA: i32 = 32157;
const VALFAR: i32 = 32146;
const SEBION: i32 = 32978;

// Items
const SARIL_NECKLACE: i32 = 17580;
const SOE: i32 = 736;
const SOULSHOT: i32 = 1463;
const BLESSED_SPIRITSHOT: i32 = 3948;
const PAULINAS_SET_D_GRADE: i32 = 46849;
const PROOF_OF_COURAGE: i32 = 17821;

const MIN_LEVEL: i32 = 18;

// `Race` ordinals.
const HUMAN: i32 = 0;
const ELF: i32 = 1;
const DARK_ELF: i32 = 2;
const ORC: i32 = 3;
const DWARF: i32 = 4;
const KAMAEL: i32 = 5;
const ERTHEIA: i32 = 6;

// `PlayerClass` ids of the classes the masters talk to.
const FIGHTER: i32 = 0;
const MAGE: i32 = 10;
const ELVEN_FIGHTER: i32 = 18;
const ELVEN_MAGE: i32 = 25;
const DARK_FIGHTER: i32 = 31;
const DARK_MAGE: i32 = 38;
const ORC_FIGHTER: i32 = 44;
const ORC_MAGE: i32 = 49;
const MALE_SOLDIER: i32 = 123;
const FEMALE_SOLDIER: i32 = 124;

/// The `classChange;<id>` switch: each first class and its master's page.
const CLASS_CHANGE_HTML: [(i32, &str); 20] = [
    (1, "32153-15.htm"),   // WARRIOR
    (4, "32153-16.htm"),   // KNIGHT
    (7, "32153-17.htm"),   // ROGUE
    (11, "32153-18.htm"),  // WIZARD
    (15, "32153-19.htm"),  // CLERIC
    (19, "32147-14.htm"),  // ELVEN_KNIGHT
    (22, "32147-15.htm"),  // ELVEN_SCOUT
    (26, "32147-16.htm"),  // ELVEN_WIZARD
    (29, "32147-17.htm"),  // ORACLE
    (32, "32160-14.htm"),  // PALUS_KNIGHT
    (35, "32160-15.htm"),  // ASSASSIN
    (39, "32160-16.htm"),  // DARK_WIZARD
    (42, "32160-17.htm"),  // SHILLIEN_ORACLE
    (45, "32150-13.htm"),  // ORC_RAIDER
    (47, "32150-14.htm"),  // ORC_MONK
    (50, "32150-15.htm"),  // ORC_SHAMAN
    (54, "32157-11.htm"),  // SCAVENGER
    (56, "32157-12.htm"),  // ARTISAN
    (125, "32146-12.htm"), // TROOPER
    (126, "32146-13.htm"), // WARDER
];

pub struct Q10331StartOfFate;

impl Q10331StartOfFate {
    /// `addCondInCategory(FIRST_CLASS_GROUP, "")` plus the race and level
    /// checks the login and level-up marks share.
    fn may_see_mark(ctx: &QuestCtx) -> bool {
        ctx.player_race() != ERTHEIA && ctx.is_in_category("FIRST_CLASS_GROUP")
    }

    /// The `classChange;<id>` event.
    fn class_change(ctx: &mut QuestCtx, arg: &str) -> Option<String> {
        if ctx.quest_items_count(SARIL_NECKLACE) < 1 {
            return None;
        }
        // `PlayerClass.getPlayerClass` throws on a non-number and answers
        // null for an unknown id; either way the event dies before the check.
        let new_class = arg.parse::<i32>().ok()?;
        ctx.world.data.player_templates.get(new_class)?;
        let current = ctx.player_class_id();
        // `newClassId.childOf(current)`: `current` is a strict ancestor. Java
        // also tests `(cond < 3) && (cond > 8)`, which is never true.
        let child_of = ctx
            .world
            .data
            .skill_trees
            .class_lineage(new_class)
            .iter()
            .skip(1)
            .any(|&c| c == current);
        if !child_of {
            let message = format!(
                "Player {} tried to cheat the 1st class transfer!",
                ctx.player_name()
            );
            ctx.punish_illegal_action(&message);
            return None;
        }
        let html = CLASS_CHANGE_HTML
            .iter()
            .find(|&&(id, _)| id == new_class)
            .map(|&(_, html)| html.to_string());
        ctx.set_class_id(new_class);
        ctx.give_items(SOE, 10);
        ctx.give_items(SOULSHOT, 1500);
        ctx.give_items(BLESSED_SPIRITSHOT, 1500);
        ctx.take_items(SARIL_NECKLACE, -1);
        ctx.give_items(PAULINAS_SET_D_GRADE, 1);
        ctx.give_items(PROOF_OF_COURAGE, 40);
        ctx.give_adena(147_600, true);
        ctx.add_exp_and_sp(296_000, 15);
        ctx.exit_quest(false, true);
        html
    }

    /// A master's STARTED page: the class menu for the race's cond, else
    /// the "not my race" page.
    fn master_page(
        ctx: &QuestCtx,
        race: i32,
        menus: &[(i32, &str)],
        other_race: &str,
    ) -> Option<String> {
        if ctx.player_race() != race {
            return Some(other_race.to_string());
        }
        if (3..=8).contains(&ctx.cond()) {
            let class = ctx.player_class_id();
            return menus
                .iter()
                .find(|&&(c, _)| c == class)
                .map(|&(_, html)| html.to_string())
                .or_else(|| Some(ctx.no_quest_html()));
        }
        Some(ctx.no_quest_html())
    }

    fn talk_sebion(ctx: &mut QuestCtx) -> Option<String> {
        match ctx.cond() {
            1 => Some("32978-03.htm".to_string()),
            2 => {
                ctx.give_items(SARIL_NECKLACE, 1);
                let (cond, html) = match ctx.player_race() {
                    HUMAN => (3, "32978-04.html"),
                    ELF => (4, "32978-06.html"),
                    DARK_ELF => (5, "32978-07.html"),
                    ORC => (6, "32978-08.html"),
                    DWARF => (7, "32978-09.html"),
                    KAMAEL => (8, "32978-10.html"),
                    _ => return Some(ctx.no_quest_html()),
                };
                ctx.set_cond(cond, true);
                Some(html.to_string())
            }
            3 => Some("32978-05.html".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl QuestScript for Q10331StartOfFate {
    fn id(&self) -> i32 {
        10331
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10331_StartOfFate"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SEBION]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[FRANCO, RIVIAN, DEVON, TOOK, MOKA, VALFAR, SEBION]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `addCondInCategory(FIRST_CLASS_GROUP, "")`: an empty html, which
        // also keeps the quest out of the chooser.
        (!ctx.is_in_category("FIRST_CLASS_GROUP")).then(String::new)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "32978-02.htm" => Some(event.to_string()),
            "32146-07.html" | "32146-08.html" | "32153-07.html" | "32153-08.html"
            | "32157-07.html" | "32147-07.html" | "32147-08.html" | "32160-07.html"
            | "32160-08.html" | "32150-07.html" | "32150-08.html" => {
                (3..=8).contains(&ctx.cond()).then(|| event.to_string())
            }
            "32978-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            _ => event
                .strip_prefix("classChange;")
                .and_then(|arg| Self::class_change(ctx, arg)),
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == SEBION && ctx.player_level() >= MIN_LEVEL {
                return Some("32978-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            return Some(format!("{npc}-05.htm"));
        }
        match npc {
            FRANCO => Self::master_page(
                ctx,
                HUMAN,
                &[(FIGHTER, "32153-07.html"), (MAGE, "32153-08.html")],
                "32153-04.html",
            ),
            RIVIAN => Self::master_page(
                ctx,
                ELF,
                &[
                    (ELVEN_FIGHTER, "32147-07.html"),
                    (ELVEN_MAGE, "32147-08.html"),
                ],
                "32147-04.html",
            ),
            DEVON => Self::master_page(
                ctx,
                DARK_ELF,
                &[
                    (DARK_FIGHTER, "32160-07.html"),
                    (DARK_MAGE, "32160-08.html"),
                ],
                "32160-04.html",
            ),
            TOOK => Self::master_page(
                ctx,
                ORC,
                &[(ORC_FIGHTER, "32150-07.html"), (ORC_MAGE, "32150-08.html")],
                "32150-04.html",
            ),
            MOKA => {
                if ctx.player_race() != DWARF {
                    Some("32157-04.html".to_string())
                } else if (3..=8).contains(&ctx.cond()) {
                    Some("32157-07.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            VALFAR => Self::master_page(
                ctx,
                KAMAEL,
                &[
                    (MALE_SOLDIER, "32146-07.html"),
                    (FEMALE_SOLDIER, "32146-08.html"),
                ],
                "32146-04.html",
            ),
            SEBION => Self::talk_sebion(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK`: mark 10331 opens the race's popup.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id != self.id() {
            return;
        }
        let race = match ctx.player_race() {
            HUMAN => "human",
            ELF => "elf",
            DARK_ELF => "dark_elf",
            ORC => "orc",
            DWARF => "dwarf",
            KAMAEL => "kamael",
            _ => "ertheia",
        };
        ctx.tutorial_show_html_file(&format!("popup-{race}.htm"));
    }

    /// `ON_PLAYER_LEVEL_CHANGED`: reaching exactly level 18 shows the mark.
    fn on_level_changed(&self, ctx: &mut QuestCtx, old_level: i32, new_level: i32) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if !ctx.has_qs()
            && old_level < new_level
            && new_level == MIN_LEVEL
            && Self::may_see_mark(ctx)
        {
            ctx.tutorial_show_question_mark(self.id(), 1);
            ctx.play_sound(quest_sounds::TUTORIAL);
        }
    }

    /// `ON_PLAYER_LOGIN`: from level 18 on, the mark until the quest starts.
    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if !ctx.has_qs() && ctx.player_level() >= MIN_LEVEL && Self::may_see_mark(ctx) {
            ctx.tutorial_show_question_mark(self.id(), 1);
            ctx.play_sound(quest_sounds::TUTORIAL);
        }
    }
}
