//! Certification of Fate (10360) — port of
//! `dist/game/data/scripts/quests/Q10360_CertificationOfFate/`, the Salvation
//! second class transfer.
//!
//! The race's master sends the player to Renfad in the Wastelands. Renfad,
//! Joel and Schuazen each send the player after a regenerated captain
//! (Kanilov, Poslof, then Sakum, who drops the Shine Stone). Selon then gives
//! the race's cond (8–15), and the master offers the second classes. The
//! `classChange;<id>` event changes the class and pays out. No instance is
//! involved: every NPC and target spawns in the open Wastelands.
//!
//! From level 38, the login and the level-up show question mark 10360, whose
//! click opens the race's popup.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::class_names;
use crate::network::server_packets::quest_sounds;

pub const QUEST_NAME: &str = "Q10360_CertificationOfFate";

// NPCs
const RAYMOND: i32 = 30289;
const RAINS: i32 = 30288;
const TOBIAS: i32 = 30297;
const DRIKUS: i32 = 30505;
const MENDIO: i32 = 30504;
const GERSHWIN: i32 = 32196;
const ESRANDELL: i32 = 30158;
const ELLENIA: i32 = 30155;
const RENFAD: i32 = 33524;
const JOEL: i32 = 33516;
const SCHUAZEN: i32 = 33517;
const SELON: i32 = 33518;
const REGENERATED_KANILOV: i32 = 27459;
const REGENERATED_POSLOF: i32 = 27460;
const SAKUM: i32 = 27453;

const MASTERS: [i32; 8] = [
    RAYMOND, RAINS, TOBIAS, DRIKUS, MENDIO, GERSHWIN, ESRANDELL, ELLENIA,
];

// Items
const SHINE_STONE: i32 = 17587;
const MAJOR_HEALING_POTION: i32 = 1061;
const SOULSHOT: i32 = 1464;
const SPIRITSHOT: i32 = 3949;
const PAULINA_EQUIPMENT_SET: i32 = 46850;

// Locations
const WASTELANDS_TELEPORT: (i32, i32, i32) = (-24795, 188754, -3960);

// Misc
const MIN_LEVEL: i32 = 38;
/// `ExShowUsm.SECOND_TRANSFER_QUEST`.
const USM_SECOND_TRANSFER_QUEST: i32 = 4;
/// The `MultisellData.separateAndSend(718, …)` shop opened on the transfer.
const TRANSFER_MULTISELL: i32 = 718;

// `Race` ordinals.
const HUMAN: i32 = 0;
const ELF: i32 = 1;
const DARK_ELF: i32 = 2;
const ORC: i32 = 3;
const DWARF: i32 = 4;
const KAMAEL: i32 = 5;
const ERTHEIA: i32 = 6;

pub struct Q10360CertificationOfFate;

impl Q10360CertificationOfFate {
    /// The race and category checks the login and level-up marks share.
    fn may_see_mark(ctx: &QuestCtx) -> bool {
        ctx.player_race() != ERTHEIA && ctx.is_in_category("SECOND_CLASS_GROUP")
    }

    /// `isRightMaster`: the master of the player's race, and for Humans and
    /// Elves of the player's side (Raymond and Esrandell take mystics, Rains
    /// and Ellenia fighters).
    fn is_right_master(ctx: &QuestCtx) -> bool {
        if ctx.npc_race() != Some(ctx.player_race()) {
            return false;
        }
        match ctx.npc_id {
            RAYMOND | ESRANDELL => ctx.is_mage_class(),
            RAINS | ELLENIA => !ctx.is_mage_class(),
            TOBIAS | DRIKUS | MENDIO | GERSHWIN => true,
            _ => false,
        }
    }

    /// `player.getPlayerClass().toString().toLowerCase()`.
    fn class_file_name(ctx: &QuestCtx) -> String {
        class_names::class_name(ctx.player_class_id())
            .unwrap_or_default()
            .to_lowercase()
    }

    /// Selon's `33518-06.htm`: the race's cond and page. Java checks no cond
    /// here, and an Ertheia gets neither.
    fn selon_race_cond(ctx: &mut QuestCtx) -> Option<String> {
        let mage = ctx.is_mage_class();
        let (cond, html) = match ctx.player_race() {
            HUMAN if mage => (9, "33518-06.htm"),
            HUMAN => (8, "33518-07.htm"),
            DARK_ELF => (12, "33518-08.htm"),
            ORC => (13, "33518-09.htm"),
            DWARF => (14, "33518-10.htm"),
            KAMAEL => (15, "33518-11.htm"),
            ELF if mage => (11, "33518-12.htm"),
            ELF => (10, "33518-13.htm"),
            _ => {
                ctx.play_sound(quest_sounds::MIDDLE);
                return None;
            }
        };
        ctx.set_cond(cond, true);
        Some(html.to_string())
    }

    /// The `classChange;<id>` event.
    fn class_change(ctx: &mut QuestCtx, arg: &str) -> Option<String> {
        if ctx.is_subclass_active() {
            return None;
        }
        // `PlayerClass.getPlayerClass` throws on a non-number, and an unknown
        // id ends in `setBaseClass(null)`; either way the event dies.
        let new_class = arg.parse::<i32>().ok()?;
        ctx.world.data.player_templates.get(new_class)?;
        let current = ctx.player_class_id();
        // `currentPlayerClass.childOf(newClassId)`: the new class is a strict
        // ancestor of the current one. Java tests no other relation, so any
        // class that is not an ancestor goes through from cond 8 on.
        let current_child_of_new = ctx
            .world
            .data
            .skill_trees
            .class_lineage(current)
            .iter()
            .skip(1)
            .any(|&c| c == new_class);
        if current_child_of_new || ctx.cond() < 8 {
            let message = format!(
                "Player {} tried to cheat the 2nd class transfer!",
                ctx.player_name()
            );
            ctx.punish_illegal_action(&message);
            return None;
        }
        ctx.set_class_id(new_class);
        ctx.give_adena(461_880, true);
        ctx.give_items(MAJOR_HEALING_POTION, 50);
        ctx.give_items(SOULSHOT, 3000);
        ctx.give_items(SPIRITSHOT, 3000);
        ctx.give_items(PAULINA_EQUIPMENT_SET, 1);
        ctx.add_exp_and_sp(2_700_000, 648);
        ctx.show_multisell(TRANSFER_MULTISELL);
        ctx.exit_quest(false, true);
        Some(format!(
            "transfer_complete_{}.html",
            Self::class_file_name(ctx)
        ))
    }

    fn master_talk(ctx: &QuestCtx) -> Option<String> {
        let npc = ctx.npc_id;
        if ctx.is_created() {
            return Some(if Self::is_right_master(ctx) {
                format!("{npc}-02.htm")
            } else {
                format!("{npc}-01.html")
            });
        }
        if ctx.is_completed() {
            return Some(format!("{npc}-07.html"));
        }
        if ctx.is_cond(1) && Self::is_right_master(ctx) {
            return Some(format!("{npc}-05.htm"));
        }
        if (8..=15).contains(&ctx.cond()) {
            return Some(if Self::is_right_master(ctx) {
                format!("class_select_{}.html", Self::class_file_name(ctx))
            } else {
                format!("{npc}-06.html")
            });
        }
        Some(ctx.no_quest_html())
    }

    /// The Wastelands NPCs: `first` on cond `cond`, `second` on `cond + 1`.
    fn step_talk(ctx: &QuestCtx, cond: i32, first: &str, second: &str) -> Option<String> {
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        Some(if ctx.is_cond(cond) {
            first.to_string()
        } else if ctx.is_cond(cond + 1) {
            second.to_string()
        } else {
            ctx.no_quest_html()
        })
    }

    /// A step event: on `cond`, the quest-middle sound and `cond + 1`.
    fn step_event(ctx: &mut QuestCtx, event: &str, cond: i32) -> Option<String> {
        if !ctx.is_cond(cond) {
            return None;
        }
        ctx.set_cond(cond + 1, true);
        Some(event.to_string())
    }
}

impl QuestScript for Q10360CertificationOfFate {
    fn id(&self) -> i32 {
        10360
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10360_CertificationOfFate"
    }
    fn start_npcs(&self) -> &[i32] {
        &MASTERS
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            RAYMOND, RAINS, TOBIAS, DRIKUS, MENDIO, GERSHWIN, ESRANDELL, ELLENIA, RENFAD, JOEL,
            SCHUAZEN, SELON,
        ]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[JOEL, SCHUAZEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[REGENERATED_KANILOV, REGENERATED_POSLOF, SAKUM]
    }
    fn quest_items(&self) -> &[i32] {
        &[SHINE_STONE]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `addCondNotRace(ERTHEIA, "")`, `addCondMinLevel(38, "")` and
    /// `addCondCompletedQuest(Q10331, "restriction.html")`, in that order.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() == ERTHEIA || ctx.player_level() < MIN_LEVEL {
            return Some(String::new());
        }
        if !ctx.other_quest_completed(super::q10331_start_of_fate::QUEST_NAME) {
            return Some("restriction.html".to_string());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30155-03.htm" | "30158-03.htm" | "32196-03.htm" | "30504-03.htm" | "30505-03.htm"
            | "30297-03.htm" | "30288-03.htm" | "30289-03.htm" | "33524-02.htm"
            | "33516-02.htm" | "33517-02.htm" | "33518-03.htm" | "33518-04.htm"
            | "33518-05.htm" => Some(event.to_string()),
            "30155-04.htm" | "30158-04.htm" | "32196-04.htm" | "30504-04.htm" | "30505-04.htm"
            | "30297-04.htm" | "30288-04.htm" | "30289-04.htm" => {
                if ctx.is_subclass_active() {
                    return None;
                }
                ctx.start_quest();
                Some(event.to_string())
            }
            "teleport" => {
                let (x, y, z) = WASTELANDS_TELEPORT;
                ctx.teleport_to(x, y, z);
                None
            }
            "33524-03.htm" => Self::step_event(ctx, event, 1),
            "33516-03.htm" => Self::step_event(ctx, event, 3),
            "33517-03.htm" => {
                let html = Self::step_event(ctx, event, 5);
                if html.is_some() {
                    ctx.show_usm(USM_SECOND_TRANSFER_QUEST);
                }
                html
            }
            "33518-06.htm" => Self::selon_race_cond(ctx),
            _ if event.starts_with("class_preview_") => Some(event.to_string()),
            _ => event
                .strip_prefix("classChange;")
                .and_then(|arg| Self::class_change(ctx, arg)),
        }
    }

    /// Joel and Schuazen: their quest page until the player reaches their
    /// step, else the NPC's own chat window.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // Java's `getQuestState(player, true)`: no state reads as cond 0.
        let html = if ctx.is_completed() {
            None
        } else if ctx.npc_id == JOEL && ctx.cond() < 3 {
            Some("33516.html".to_string())
        } else if ctx.npc_id == SCHUAZEN && ctx.cond() < 5 {
            Some("33517.html".to_string())
        } else {
            None
        };
        if html.is_none() {
            ctx.show_npc_chat_window();
        }
        html
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.player_race() == ERTHEIA || ctx.player_level() < MIN_LEVEL {
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            npc if MASTERS.contains(&npc) => Self::master_talk(ctx),
            RENFAD => Self::step_talk(ctx, 1, "33524-01.htm", "33524-04.htm"),
            JOEL => Self::step_talk(ctx, 3, "33516-01.htm", "33516-04.htm"),
            SCHUAZEN => Self::step_talk(ctx, 5, "33517-01.htm", "33517-04.htm"),
            SELON => Self::step_talk(ctx, 6, "33518-01.htm", "33518-02.htm"),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            REGENERATED_KANILOV if ctx.is_cond(2) => {
                ctx.play_sound(quest_sounds::MIDDLE);
                ctx.set_cond(0, false);
                ctx.set_cond(3, false);
            }
            REGENERATED_POSLOF if ctx.is_cond(4) => {
                ctx.set_cond(5, true);
            }
            SAKUM if ctx.is_cond(6) => {
                ctx.give_items(SHINE_STONE, 1);
                ctx.set_cond(7, true);
            }
            _ => {}
        }
    }

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK`: mark 10360 opens the race's popup,
    /// split by side for Humans and Elves.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id != self.id() {
            return;
        }
        let side = if ctx.is_mage_class() { "m" } else { "f" };
        let file = match ctx.player_race() {
            DARK_ELF => "popup-dark_elf.htm".to_string(),
            DWARF => "popup-dwarf.htm".to_string(),
            KAMAEL => "popup-kamael.htm".to_string(),
            ORC => "popup-orc.htm".to_string(),
            ELF => format!("popup-elf-{side}.htm"),
            HUMAN => format!("popup-human-{side}.htm"),
            _ => String::new(),
        };
        ctx.tutorial_show_html_file(&file);
    }

    /// `ON_PLAYER_LEVEL_CHANGED`: reaching exactly level 38 shows the mark.
    /// Java checks neither the quest state nor `DisableTutorial` here.
    fn on_level_changed(&self, ctx: &mut QuestCtx, old_level: i32, new_level: i32) {
        if old_level < new_level && new_level == MIN_LEVEL && Self::may_see_mark(ctx) {
            ctx.tutorial_show_question_mark(self.id(), 1);
        }
    }

    /// `ON_PLAYER_LOGIN`: from level 38 on, the mark until the quest starts.
    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.player_level() >= MIN_LEVEL
            && Self::may_see_mark(ctx)
            && (!ctx.has_qs() || ctx.is_created())
        {
            ctx.tutorial_show_question_mark(self.id(), 1);
        }
    }
}
