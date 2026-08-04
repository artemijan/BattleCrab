//! Collector's Dream (261) — port of
//! `dist/game/data/scripts/quests/Q00261_CollectorsDream/`. Alshupes (30222)
//! in the Dwarven Village wants 8 spider legs from the three Gludio spiders;
//! reward 700 adena. Repeatable, level 15–21.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const ALSHUPES: i32 = 30222;
// Hook / Crimson / Pincer Spider.
const MONSTERS: [i32; 3] = [20308, 20460, 20466];
const SPIDER_LEG: i32 = 1087;
const MIN_LEVEL: i32 = 15;
const MAX_LEG_COUNT: i64 = 8;

/// `GUIDE_MISSION` — the bit-packed newbie-guide progress variable. Java packs
/// several missions into one integer; this quest owns the ten-millions digit.
const GUIDE_MISSION: &str = "GUIDE_MISSION";
/// This quest's digit within `GUIDE_MISSION` (`/ 10000000 % 10`).
const GUIDE_MISSION_STEP: i32 = 10_000_000;
/// The value Java seeds the variable with on first award.
const GUIDE_MISSION_INITIAL: i32 = 100_000;
/// `NpcStringId.LAST_DUTY_COMPLETE_N_GO_FIND_THE_NEWBIE_HELPER` (4155).
const LAST_DUTY_COMPLETE: i32 = 4155;
/// `ExShowScreenMessage` position 2 = top centre, for 5000 ms — Java's
/// `MESSAGE` constant.
const MESSAGE_POSITION: i32 = 2;
const MESSAGE_TIME: i32 = 5_000;

/// Java `Q00261_CollectorsDream.giveNewbieReward(Player)` — a **static** helper
/// that other newbie quests call, which is why it lives here and is `pub`.
///
/// Award the newbie-guide mission credit once: seed `GUIDE_MISSION` if unset,
/// otherwise add this quest's digit if it is not already 1. Either way the
/// player sees the "last duty complete" banner; if the digit was already set,
/// nothing happens at all.
///
/// The variable half is bookkeeping with no reader in this port yet — the
/// newbie-guide mission-list UI that consumes it is not ported. It is written
/// anyway because it *persists*: a player who finishes the errand today should
/// not have to redo it when that UI lands. The banner half is real,
/// player-visible behaviour and was the actual gap.
///
/// This must be a **player** variable, not a `QuestState` var: both callers are
/// repeatable quests that `exit_quest`, which drops the quest state. Stored
/// there the credit would vanish on the same turn-in that earned it, and the
/// once-only guard would never fire.
///
/// Note the dist: `giveNewbieReward` is commented out in nearly every other
/// newbie quest (Q257, Q260, Q265, Q273, …), so their ports correctly omit it.
/// Only this quest and Q276 call it live — do not "restore" it elsewhere.
pub fn give_newbie_reward(ctx: &mut QuestCtx) {
    // Java tests `getString(key, null) == null`, so an *absent* variable and a
    // stored 0 take different branches. Read the raw value to keep that.
    let next = match ctx.player_var(GUIDE_MISSION) {
        None => GUIDE_MISSION_INITIAL,
        Some(raw) => {
            let v: i32 = raw.parse().unwrap_or(0);
            // Java: `((vars % 100000000) / 10000000) != 1` — this quest's digit.
            if (v % 100_000_000) / GUIDE_MISSION_STEP == 1 {
                return; // already credited; Java sends no message either
            }
            v + GUIDE_MISSION_STEP
        }
    };
    ctx.set_player_var_int(GUIDE_MISSION, next);
    ctx.send_screen_message_npc_string(LAST_DUTY_COMPLETE, MESSAGE_POSITION, MESSAGE_TIME);
}

pub struct Q00261CollectorsDream;

impl QuestScript for Q00261CollectorsDream {
    fn id(&self) -> i32 {
        261
    }
    fn name(&self) -> &'static str {
        "Q00261_CollectorsDream"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00261_CollectorsDream"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ALSHUPES]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ALSHUPES]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[SPIDER_LEG]
    }

    /// `addCondMaxLevel(21, getNoQuestMsg(null))` — a newbie quest, refused
    /// above level 21.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 21).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event == "30222-03.htm" {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }

    /// Java gates on `checkIfInRange(ALT_PARTY_RANGE, npc, killer)`; the port
    /// credits the killer only (the G11 party deviation), which reduces to the
    /// cond-1 gate.
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs()
            && ctx.is_cond(1)
            && ctx.give_item_randomly(SPIDER_LEG, 1, MAX_LEG_COUNT, 1.0, true)
        {
            ctx.set_cond(2, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30222-02.htm"
                } else {
                    "30222-01.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30222-04.html".to_string()),
                2 if ctx.quest_items_count(SPIDER_LEG) >= MAX_LEG_COUNT => {
                    give_newbie_reward(ctx);
                    ctx.give_adena(700, true);
                    ctx.exit_quest(true, true);
                    return Some("30222-05.html".to_string());
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
}
