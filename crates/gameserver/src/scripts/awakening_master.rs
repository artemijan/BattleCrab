//! The eight Awakening Masters of the Reliquary of the Giants — port of
//! `dist/game/data/scripts/ai/areas/TalkingIsland/AwakeningMaster/`.
//!
//! A fourth-class player above level 84 who carries the Scroll of Afterlife
//! from Seize Your Destiny ([`super::q10338_seize_your_destiny`]) asks the
//! master of their category to awaken them; the master answers with the
//! client's awakening dialog (`ExChangeToAwakenedClass`). Accepting it
//! (`RequestChangeToAwakenedClass`) fires `ON_PLAYER_CHANGE_TO_AWAKENED_CLASS`,
//! where the class changes, the old class skills go and the race skills come.
//!
//! The Ertheia branch of `onFirstTalk` belongs to Q10472 (Winds of Fate:
//! Encroaching Shadows), which is not ported: an Ertheia gets `ertheia.html`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::sm_ids;

// NPCs
const SIGEL_MASTER: i32 = 33397;
const TYRR_MASTER: i32 = 33398;
const OTHELL_MASTER: i32 = 33399;
const YUL_MASTER: i32 = 33400;
const FEOH_MASTER: i32 = 33401;
const ISS_MASTER: i32 = 33402;
const WYNN_MASTER: i32 = 33403;
const AEORE_MASTER: i32 = 33404;
const MASTERS: [i32; 8] = [
    SIGEL_MASTER,
    TYRR_MASTER,
    OTHELL_MASTER,
    YUL_MASTER,
    FEOH_MASTER,
    ISS_MASTER,
    WYNN_MASTER,
    AEORE_MASTER,
];

// Items
const SCROLL_OF_AFTERLIFE: i32 = 17600;
const CHAOS_POMANDER: i32 = 37374;
/// `AWAKE_POWER`, in `CategoryType` order (Java's `EnumMap`).
const AWAKE_POWER: [(&str, i32); 8] = [
    ("SIXTH_SIGEL_GROUP", 32264),
    ("SIXTH_TIR_GROUP", 32265),
    ("SIXTH_OTHEL_GROUP", 32266),
    ("SIXTH_YR_GROUP", 32267),
    ("SIXTH_FEOH_GROUP", 32268),
    ("SIXTH_IS_GROUP", 32270),
    ("SIXTH_WYNN_GROUP", 32269),
    ("SIXTH_EOLH_GROUP", 32271),
];

// Misc
const FEMALE_SOUL_HOUND: i32 = 133;
const FEOH_SOUL_HOUND: i32 = 170;
/// `ExShowUsm.AWAKENING_END`.
const AWAKENING_END: i32 = 10;
const AWAKENING_END_EVENT: &str = "AWAKENING_END";
/// `Race.ERTHEIA` ordinal.
const ERTHEIA: i32 = 6;

pub struct AwakeningMaster;

/// The category a master awakens.
fn master_category(npc_id: i32) -> Option<&'static str> {
    Some(match npc_id {
        SIGEL_MASTER => "TANKER_CATEGORY",
        TYRR_MASTER => "WARRIOR_CATEGORY",
        OTHELL_MASTER => "ROGUE_CATEGORY",
        YUL_MASTER => "ARCHER_CATEGORY",
        FEOH_MASTER => "WIZARD_CATEGORY",
        ISS_MASTER => "ENCHANTER_CATEGORY",
        WYNN_MASTER => "SUMMONER_CATEGORY",
        AEORE_MASTER => "HEALER_CATEGORY",
        _ => return None,
    })
}

/// The awakened class: Feoh Soul Hound for a Female Soul Hound (Java's fix,
/// her class has no next class), else the last of `getNextClasses()`.
fn awakened_class(ctx: &QuestCtx) -> Option<i32> {
    let class_id = ctx.player_class_id();
    if class_id == FEMALE_SOUL_HOUND {
        return Some(FEOH_SOUL_HOUND);
    }
    ctx.world
        .data
        .skill_trees
        .next_classes(class_id)
        .last()
        .copied()
}

impl QuestScript for AwakeningMaster {
    fn id(&self) -> i32 {
        // A `Script`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "AwakeningMaster"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/TalkingIsland/AwakeningMaster"
    }
    fn start_npcs(&self) -> &[i32] {
        &MASTERS
    }
    fn talk_npcs(&self) -> &[i32] {
        &MASTERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &MASTERS
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event != "awakening" {
            return None;
        }
        let npc = ctx.npc_id;
        if ctx.quest_items_count(SCROLL_OF_AFTERLIFE) == 0
            || ctx.player_level() <= 84
            || !ctx.is_in_category("FOURTH_CLASS_GROUP")
        {
            return Some(format!("{npc}-no.htm"));
        }
        if let Some(category) = master_category(npc)
            && !ctx.is_in_category(category)
        {
            return Some(format!("{npc}-no_class.htm"));
        }
        if let Some(class_id) = awakened_class(ctx) {
            ctx.send_change_to_awakened_class(class_id);
        }
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() == ERTHEIA {
            return Some("ertheia.html".to_string());
        }
        Some(format!("{}.html", ctx.npc_id))
    }

    /// `onPlayerChangeToAwakenedClass`. Dual classes are not modelled, so a
    /// subclass is never the dual one.
    fn on_change_to_awakened_class(&self, ctx: &mut QuestCtx) {
        if ctx.is_subclass_active() {
            return;
        }
        if ctx.player_level() < 85 || !ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return;
        }
        if ctx.is_hero_or_unclaimed_hero() {
            ctx.send_sm(
                sm_ids::YOU_CANNOT_AWAKEN_WHEN_YOU_ARE_A_HERO_OR_ON_THE_WAIT_LIST_FOR_HERO_STATUS,
            );
            return;
        }
        if !ctx.is_inventory_under_80() {
            ctx.send_sm(sm_ids::YOU_CANNOT_AWAKEN_DUE_TO_YOUR_CURRENT_INVENTORY_WEIGHT);
            return;
        }
        if ctx.is_mounted_or_transformed() {
            ctx.send_sm(sm_ids::YOU_CANNOT_AWAKEN_WHILE_YOU_RE_TRANSFORMED_OR_RIDING);
            return;
        }
        if ctx.quest_items_count(SCROLL_OF_AFTERLIFE) == 0
            || !ctx.take_items(SCROLL_OF_AFTERLIFE, -1)
        {
            return;
        }
        // Java's `newClassId` stays -1 for a class with no next class, and
        // `setPlayerClass(-1)` throws on the missing template.
        let Some(class_id) = awakened_class(ctx) else {
            return;
        };
        ctx.set_class_id(class_id);
        ctx.send_sm(sm_ids::CONGRATULATIONS_YOU_VE_COMPLETED_A_CLASS_TRANSFER);
        ctx.broadcast_social_action(20);
        if let Some(&(_, item)) = AWAKE_POWER
            .iter()
            .find(|(category, _)| ctx.is_in_category(category))
        {
            ctx.give_items(item, 1);
        }
        ctx.give_items(CHAOS_POMANDER, 2);
        ctx.clean_skills_upon_awakening();
        ctx.add_race_skills();
        ctx.send_skill_list();
        let player = ctx.player;
        ctx.add_timer(AWAKENING_END_EVENT, 10000, 0, player);
    }

    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        if event == AWAKENING_END_EVENT {
            ctx.show_usm(AWAKENING_END);
        }
    }
}
