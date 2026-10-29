//! Seize Your Destiny (10338) — port of
//! `dist/game/data/scripts/quests/Q10338_SeizeYourDestiny/`, the Salvation
//! awakening quest.
//!
//! Cellphine starts it from level 85 on a fourth class and remembers the
//! class it started with. Hadel sends the player into Harnak Underground
//! Ruins ([`super::harnak_underground_ruins`]), where Harnak's Wraith falls
//! (cond 2 → 3); Hermuncus then gives the Scroll of Afterlife the Awakening
//! Masters ([`super::awakening_master`]) take, and his `TELEPORT` sends the
//! player to the Reliquary of the Giants.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::quest::state;

pub const QUEST_NAME: &str = "Q10338_SeizeYourDestiny";

// NPCs
const CELLPHINE: i32 = 33477;
const HADEL: i32 = 33344;
const HERMUNCUS: i32 = 33340;

// Monsters
const HARNAKS_WRAITH: i32 = 27445;

// Items
const SCROLL_OF_AFTERLIFE: i32 = 17600;
const STEEL_DOOR_GUILD_COIN: (i32, i64) = (37045, 400);

// Locations
const RELIQUARY_OF_THE_GIANT: (i32, i32, i32) = (-114962, 226564, -2864);

// Misc
const STARTED_CLASS_VAR: &str = "STARTED_CLASS";
const MIN_LV: i32 = 85;
/// `Race.ERTHEIA` ordinal.
const ERTHEIA: i32 = 6;
/// `PlayerClass.JUDICATOR`.
const JUDICATOR: i32 = 136;
/// `Movie.SC_AWAKENING_VIEW`.
const SC_AWAKENING_VIEW: i32 = 55;
/// `ExShowScreenMessage.TOP_CENTER`.
const TOP_CENTER: i32 = 2;
/// `NpcStringId.YOU_MAY_USE_SCROLL_OF_AFTERLIFE_FROM_HERMUNCUS_TO_AWAKEN`.
const YOU_MAY_USE_SCROLL_OF_AFTERLIFE: i32 = 10338010;

pub struct Q10338SeizeYourDestiny;

/// `isInCategory(SIXTH_CLASS_GROUP) || hasQuestItems(SCROLL_OF_AFTERLIFE)`.
fn awakened_or_holds_scroll(ctx: &QuestCtx) -> bool {
    ctx.is_in_category("SIXTH_CLASS_GROUP") || ctx.quest_items_count(SCROLL_OF_AFTERLIFE) > 0
}

fn started_class_changed(ctx: &QuestCtx) -> bool {
    ctx.get_int(STARTED_CLASS_VAR) != ctx.player_class_id()
}

impl QuestScript for Q10338SeizeYourDestiny {
    fn id(&self) -> i32 {
        10338
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10338_SeizeYourDestiny"
    }
    fn start_npcs(&self) -> &[i32] {
        &[CELLPHINE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[CELLPHINE, HADEL, HERMUNCUS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[HARNAKS_WRAITH]
    }

    /// `addCondNotRace(ERTHEIA, "33477-08.html")`,
    /// `addCondNotClassId(JUDICATOR, "")`, `addCondIsNotSubClassActive("")`,
    /// `addCondMinLevel(85, "33477-07.html")` and
    /// `addCondInCategory(FOURTH_CLASS_GROUP, "33477-07.html")`, in order.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() == ERTHEIA {
            return Some("33477-08.html".to_string());
        }
        if ctx.player_class_id() == JUDICATOR || ctx.is_subclass_active() {
            return Some(String::new());
        }
        if ctx.player_level() < MIN_LV || !ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some("33477-07.html".to_string());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "TELEPORT" => {
                // Dual classes are not modelled: any subclass is refused.
                if ctx.is_subclass_active() {
                    return Some(String::new());
                }
                let (x, y, z) = RELIQUARY_OF_THE_GIANT;
                ctx.teleport_to(x, y, z);
                let player = ctx.player;
                ctx.play_movie(player, SC_AWAKENING_VIEW);
                None
            }
            "33477-03.html" => {
                if ctx.is_in_category("SIXTH_CLASS_GROUP") {
                    return None;
                }
                ctx.set_state(state::CREATED);
                ctx.start_quest();
                let class_id = ctx.player_class_id();
                ctx.set_var(STARTED_CLASS_VAR, class_id.to_string());
                Some(event.to_string())
            }
            "33344-05.html" => {
                if !ctx.is_cond(1) {
                    return None;
                }
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "33340-02.html" => {
                if !ctx.is_cond(3) {
                    return None;
                }
                ctx.send_screen_message_npc_string(
                    YOU_MAY_USE_SCROLL_OF_AFTERLIFE,
                    TOP_CENTER,
                    10000,
                );
                ctx.give_items(SCROLL_OF_AFTERLIFE, 1);
                ctx.reward_items(STEEL_DOOR_GUILD_COIN.0, STEEL_DOOR_GUILD_COIN.1);
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "33344-02.html" | "33344-03.html" | "33344-04.html" | "33477-02.htm" => {
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = match ctx.npc_id {
            CELLPHINE => {
                if ctx.is_started() {
                    "33477-06.html"
                } else if awakened_or_holds_scroll(ctx) {
                    "33477-05.html"
                } else if ctx.player_level() > 84 {
                    // Java sends the `.htm` as a plain chat window, not the
                    // quest window its name would pick.
                    ctx.send_plain_html("33477-01.htm");
                    return None;
                } else {
                    "33477-07.html"
                }
            }
            HADEL => {
                if awakened_or_holds_scroll(ctx) {
                    "33344-07.html"
                } else if ctx.player_level() < MIN_LV {
                    "33344-06.html"
                } else if started_class_changed(ctx) || ctx.is_subclass_active() {
                    "33344-09.html"
                } else {
                    match ctx.cond() {
                        1 => "33344-01.html",
                        2 => "33344-08.html",
                        3 => "33344-07.html",
                        _ => return Some(ctx.no_quest_html()),
                    }
                }
            }
            HERMUNCUS => {
                let holds_scroll = ctx.quest_items_count(SCROLL_OF_AFTERLIFE) > 0;
                if started_class_changed(ctx) && !holds_scroll {
                    "33340-04.html"
                } else if ctx.is_cond(3) {
                    "33340-01.html"
                } else if holds_scroll {
                    "33340-03.html"
                } else {
                    "33340-02.html"
                }
            }
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(2) && !started_class_changed(ctx) {
            ctx.set_cond(3, true);
        }
    }
}
