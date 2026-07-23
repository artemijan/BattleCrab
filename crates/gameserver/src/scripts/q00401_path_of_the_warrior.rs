//! Path Of The Warrior (401) — port of
//! `dist/game/data/scripts/quests/Q00401_PathOfTheWarrior/`.
//!
//! Awards the **Medallion of Warrior** (1145), the proof
//! `ElfHumanFighterChange1` takes to turn a Human Fighter into a Warrior.
//!
//! Auron sends you to Simplon for a guild mark, then for 10 rusted bronze
//! swords, which Simplon reforges into one whole sword; Auron sharpens that
//! into the quest weapon, and the last stage wants 20 venomous spider legs —
//! **killed with that sword, solo**. That gate is
//! [`quest_common::tag_attacker_with_weapon`], shared with quest 403.
//!
//! Two drops with different shapes, worth not conflating:
//!
//! | Stage | Roll | Gate |
//! |---|---|---|
//! | rusted swords (10) | `getRandom(10) < 4` | holding the guild mark |
//! | spider legs (20) | none — guaranteed | `isScriptValue(1)` (weapon+solo) |
//!
//! The spider leg has **no chance roll at all**: every qualifying kill pays.
//! It is the weapon gate, not a rate, that makes the stage slow.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
use crate::scripts::quest_common;

const MASTER_AURON: i32 = 30010;
const TRADER_SIMPLON: i32 = 30253;

const AURONS_LETTER: i32 = 1138;
const WARRIOR_GUILD_MARK: i32 = 1139;
const RUSTED_BRONZE_SWORD1: i32 = 1140;
const RUSTED_BRONZE_SWORD2: i32 = 1141;
const RUSTED_BRONZE_SWORD3: i32 = 1142;
const SIMPLONS_LETTER: i32 = 1143;
const VENOMOUS_SPIDERS_LEG: i32 = 1144;
const MEDALLION_OF_WARRIOR: i32 = 1145;

const TRACKER_SKELETON: i32 = 20035;
const VENOMOUS_SPIDERS: i32 = 20038;
const TRACKER_SKELETON_LIDER: i32 = 20042;
const ARACHNID_TRACKER: i32 = 20043;

const FIGHTER: i32 = 0;
const WARRIOR: i32 = 1;
const MIN_LEVEL: i32 = 19;

const QUEST_ITEMS: [i32; 7] = [
    AURONS_LETTER,
    WARRIOR_GUILD_MARK,
    RUSTED_BRONZE_SWORD1,
    RUSTED_BRONZE_SWORD2,
    RUSTED_BRONZE_SWORD3,
    SIMPLONS_LETTER,
    VENOMOUS_SPIDERS_LEG,
];

pub struct Q00401PathOfTheWarrior;

impl QuestScript for Q00401PathOfTheWarrior {
    fn id(&self) -> i32 {
        401
    }
    fn name(&self) -> &'static str {
        "Q00401_PathOfTheWarrior"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00401_PathOfTheWarrior"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_AURON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MASTER_AURON, TRADER_SIMPLON]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            TRACKER_SKELETON,
            VENOMOUS_SPIDERS,
            TRACKER_SKELETON_LIDER,
            ARACHNID_TRACKER,
        ]
    }
    /// Only the spiders are tagged — the skeletons pay on a plain roll.
    fn attack_npcs(&self) -> &[i32] {
        &[VENOMOUS_SPIDERS, ARACHNID_TRACKER]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(
                match ctx.player_class_id() {
                    FIGHTER if ctx.player_level() < MIN_LEVEL => "30010-02.htm",
                    FIGHTER if ctx.quest_items_count(MEDALLION_OF_WARRIOR) > 0 => "30010-04.htm",
                    FIGHTER => "30010-05.htm",
                    WARRIOR => "30010-02a.htm",
                    _ => "30010-03.htm",
                }
                .to_string(),
            ),
            "30010-06.htm" => {
                // Guarded so a double-click can't restart and re-issue.
                if ctx.quest_items_count(AURONS_LETTER) > 0 {
                    return None;
                }
                ctx.start_quest();
                ctx.give_items(AURONS_LETTER, 1);
                Some(event.to_string())
            }
            "30010-10.html" => Some(event.to_string()),
            "30010-11.html" => {
                // Auron sharpens sword 2 into the quest weapon (sword 3).
                if ctx.quest_items_count(SIMPLONS_LETTER) > 0
                    && ctx.quest_items_count(RUSTED_BRONZE_SWORD2) > 0
                {
                    ctx.take_items(RUSTED_BRONZE_SWORD2, 1);
                    ctx.give_items(RUSTED_BRONZE_SWORD3, 1);
                    ctx.take_items(SIMPLONS_LETTER, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30253-02.html" => {
                if ctx.quest_items_count(AURONS_LETTER) > 0 {
                    ctx.take_items(AURONS_LETTER, 1);
                    ctx.give_items(WARRIOR_GUILD_MARK, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        quest_common::tag_attacker_with_weapon(ctx, &[RUSTED_BRONZE_SWORD3]);
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            TRACKER_SKELETON | TRACKER_SKELETON_LIDER => {
                if ctx.quest_items_count(WARRIOR_GUILD_MARK) > 0
                    && ctx.quest_items_count(RUSTED_BRONZE_SWORD1) < 10
                    && ctx.roll(10) < 4
                {
                    ctx.give_items(RUSTED_BRONZE_SWORD1, 1);
                    if ctx.quest_items_count(RUSTED_BRONZE_SWORD1) == 10 {
                        ctx.set_cond(3, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            // No chance roll here — the weapon/solo tag *is* the gate.
            VENOMOUS_SPIDERS | ARACHNID_TRACKER
                if ctx.quest_items_count(VENOMOUS_SPIDERS_LEG) < 20
                    && quest_common::is_tag_qualified(ctx) =>
            {
                ctx.give_items(VENOMOUS_SPIDERS_LEG, 1);
                if ctx.quest_items_count(VENOMOUS_SPIDERS_LEG) == 20 {
                    ctx.set_cond(6, true);
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == MASTER_AURON {
                return Some("30010-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            MASTER_AURON => self.talk_auron(ctx),
            TRADER_SIMPLON => self.talk_simplon(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00401PathOfTheWarrior {
    fn talk_auron(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(AURONS_LETTER) > 0 {
            return Some("30010-07.html".to_string());
        }
        if ctx.quest_items_count(WARRIOR_GUILD_MARK) > 0 {
            return Some("30010-08.html".to_string());
        }
        // Java re-checks `!hasAtLeastOneQuestItem(MARK, LETTER)` on both of
        // the remaining branches; both are already excluded above.
        if ctx.quest_items_count(SIMPLONS_LETTER) > 0
            && ctx.quest_items_count(RUSTED_BRONZE_SWORD2) > 0
        {
            return Some("30010-09.html".to_string());
        }
        if ctx.quest_items_count(RUSTED_BRONZE_SWORD3) > 0 {
            if ctx.quest_items_count(VENOMOUS_SPIDERS_LEG) < 20 {
                return Some("30010-12.html".to_string());
            }
            ctx.give_items(MEDALLION_OF_WARRIOR, 1);
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30010-13.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_simplon(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(AURONS_LETTER) > 0 {
            return Some("30253-01.html".to_string());
        }
        if ctx.quest_items_count(WARRIOR_GUILD_MARK) > 0 {
            let swords = ctx.quest_items_count(RUSTED_BRONZE_SWORD1);
            if swords == 0 {
                return Some("30253-03.html".to_string());
            }
            if swords < 10 {
                return Some("30253-04.html".to_string());
            }
            ctx.take_items(WARRIOR_GUILD_MARK, 1);
            ctx.take_items(RUSTED_BRONZE_SWORD1, -1);
            ctx.give_items(RUSTED_BRONZE_SWORD2, 1);
            ctx.give_items(SIMPLONS_LETTER, 1);
            ctx.set_cond(4, true);
            return Some("30253-05.html".to_string());
        }
        if ctx.quest_items_count(SIMPLONS_LETTER) > 0 {
            return Some("30253-06.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
