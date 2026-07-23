//! Hunting Leto Lizardman (300) — port of
//! `dist/game/data/scripts/quests/Q00300_HuntingLetoLizardman/`. Rath (30126)
//! wants **60 Bracelets of Lizardman** off the Leto camp (level 34–39); the
//! reward is a `getRandom(1000)` pick — 5000 adena (50%), 50 Animal Skin (25%)
//! or 50 Animal Bone (25%). Repeatable. Per-mob drop chances are out of 1000.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const RATH: i32 = 30126;
const BRACELET_OF_LIZARDMAN: i32 = 7139;
const ANIMAL_BONE: i32 = 1872;
const ANIMAL_SKIN: i32 = 1867;
const ADENA: i32 = 57;
const REQUIRED_BRACELET_COUNT: i64 = 60;

/// `MOBS_SAC`: Leto Lizardman variants → bracelet drop chance out of 1000.
const KILL_NPCS: [i32; 5] = [20577, 20578, 20579, 20580, 20582];
fn drop_chance(npc_id: i32) -> i32 {
    match npc_id {
        20577 => 360, // Leto Lizardman
        20578 => 390, // Archer
        20579 => 410, // Soldier
        20580 => 790, // Warrior
        20582 => 890, // Overlord
        _ => 0,
    }
}

pub struct Q00300HuntingLetoLizardman;

impl QuestScript for Q00300HuntingLetoLizardman {
    fn id(&self) -> i32 {
        300
    }
    fn name(&self) -> &'static str {
        "Q00300_HuntingLetoLizardman"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00300_HuntingLetoLizardman"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RATH]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RATH]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[BRACELET_OF_LIZARDMAN]
    }

    /// `addCondMaxLevel(39, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 39).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30126-03.htm" if ctx.is_created() => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30126-06.html" => {
                if ctx.quest_items_count(BRACELET_OF_LIZARDMAN) >= REQUIRED_BRACELET_COUNT {
                    ctx.take_items(BRACELET_OF_LIZARDMAN, -1);
                    // Java `giveItems(ItemHolder)` adds without rate scaling.
                    let rand = ctx.roll(1000);
                    if rand < 500 {
                        ctx.give_items(ADENA, 5000);
                    } else if rand < 750 {
                        ctx.give_items(ANIMAL_SKIN, 50);
                    } else {
                        ctx.give_items(ANIMAL_BONE, 50);
                    }
                    ctx.exit_quest(true, true);
                    Some("30126-06.html".to_string())
                } else {
                    Some("30126-07.html".to_string())
                }
            }
            _ => None,
        }
    }

    /// `getRandomPartyMember(player, 1)` → the killer (G11 party deviation),
    /// which reduces to the cond-1 gate. A bracelet drops on
    /// `getRandom(1000) < chance`; the **60th** flips cond to 2.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) && ctx.roll(1000) < drop_chance(ctx.npc_id) {
            ctx.give_items(BRACELET_OF_LIZARDMAN, 1);
            if ctx.quest_items_count(BRACELET_OF_LIZARDMAN) == REQUIRED_BRACELET_COUNT {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= 34 {
                    "30126-01.htm"
                } else {
                    "30126-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30126-04.html".to_string()),
                2 if ctx.quest_items_count(BRACELET_OF_LIZARDMAN) >= REQUIRED_BRACELET_COUNT => {
                    return Some("30126-05.html".to_string())
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
