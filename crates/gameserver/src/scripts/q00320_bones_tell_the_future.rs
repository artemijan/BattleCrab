//! Bones Tell The Future (320) — port of
//! `dist/game/data/scripts/quests/Q00320_BonesTellTheFuture/`. Dark Elf
//! village: Tetrarch Kaitar wants 10 bone fragments; skeletons drop them at
//! 18% (×`RateQuestDrop`); the reward is 500 adena (×`RateQuestRewardAdena`).

use crate::game_loop::quests::{QuestCtx, QuestScript};

const TETRACH_KAITAR: i32 = 30359;
const BONE_FRAGMENT: i32 = 809;
const MONSTERS: [i32; 2] = [20517, 20518]; // Skeleton Hunter, Skeleton Hunter Archer
const MIN_LEVEL: i32 = 10;
const REQUIRED_BONE_COUNT: i64 = 10;
const DROP_CHANCE: f64 = 0.18;
/// `Race.DARK_ELF` ordinal (`characters.race`).
const RACE_DARK_ELF: i32 = 2;

pub struct Q00320BonesTellTheFuture;

impl QuestScript for Q00320BonesTellTheFuture {
    fn id(&self) -> i32 {
        320
    }

    fn name(&self) -> &'static str {
        "Q00320_BonesTellTheFuture"
    }

    fn html_dir(&self) -> &'static str {
        "quests/Q00320_BonesTellTheFuture"
    }

    fn start_npcs(&self) -> &[i32] {
        &[TETRACH_KAITAR]
    }

    fn talk_npcs(&self) -> &[i32] {
        &[TETRACH_KAITAR]
    }

    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }

    fn quest_items(&self) -> &[i32] {
        &[BONE_FRAGMENT]
    }

    /// `addCondMaxLevel(18, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 18).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30359-04.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    /// Java rolls through `getRandomPartyMemberState(killer, 1, 3, npc)` —
    /// killer-only here (documented G11 deviation), which reduces to the
    /// cond-1 gate.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs()
            && ctx.is_cond(1)
            && ctx.give_item_randomly(BONE_FRAGMENT, 1, REQUIRED_BONE_COUNT, DROP_CHANCE, true)
        {
            ctx.set_cond(2, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_race() != RACE_DARK_ELF {
                    "30359-00.htm"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30359-03.htm"
                } else {
                    "30359-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(if ctx.quest_items_count(BONE_FRAGMENT) >= REQUIRED_BONE_COUNT {
                ctx.give_adena(500, true);
                ctx.exit_quest(true, true);
                "30359-06.html"
            } else {
                "30359-05.html"
            }
            .to_string());
        }
        Some(ctx.no_quest_html())
    }
}
