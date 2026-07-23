//! Collector of Jewels (369) — `quests/Q00369_CollectorOfJewels`. Nell (30376,
//! level 25–37) collects Flare Shards (fire elementals) and Freezing Shards
//! (water elementals) in two stages: 100 shards → 3000 adena, then 400 shards →
//! 12000 adena. Progress is tracked by `memoState` (1 = first hunt, 2 = first
//! turned in, 3 = second hunt); `cond` mirrors it for the UI arrow.
//! `addCondMaxLevel(37)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const NELL: i32 = 30376;
const FLARE_SHARD: i32 = 5882;
const FREEZING_SHARD: i32 = 5883;
const MIN_LEVEL: i32 = 25;
const KILL_NPCS: [i32; 6] = [20609, 20612, 20749, 20616, 20619, 20747];

/// `MOBS_DROP_CHANCES`: npc → (item, chance /100, count).
fn drop_for(npc_id: i32) -> Option<(i32, i32, i64)> {
    match npc_id {
        20609 => Some((FLARE_SHARD, 75, 1)),     // salamander_lakin
        20612 => Some((FLARE_SHARD, 91, 1)),     // salamander_rowin
        20749 => Some((FLARE_SHARD, 100, 2)),    // death_fire
        20616 => Some((FREEZING_SHARD, 81, 1)),  // undine_lakin
        20619 => Some((FREEZING_SHARD, 87, 1)),  // undine_rowin
        20747 => Some((FREEZING_SHARD, 100, 2)), // roxide
        _ => None,
    }
}

/// `getQuestItemsCount(player, FLARE_SHARD, FREEZING_SHARD)` — the combined
/// count both stages gate on.
fn shard_total(ctx: &QuestCtx) -> i64 {
    ctx.quest_items_count(FLARE_SHARD) + ctx.quest_items_count(FREEZING_SHARD)
}

pub struct Q00369CollectorOfJewels;

impl QuestScript for Q00369CollectorOfJewels {
    fn id(&self) -> i32 {
        369
    }
    fn name(&self) -> &'static str {
        "Q00369_CollectorOfJewels"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00369_CollectorOfJewels"
    }
    fn start_npcs(&self) -> &[i32] {
        &[NELL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[NELL]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[FLARE_SHARD, FREEZING_SHARD]
    }

    /// `addCondMaxLevel(37, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 37).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30376-02.htm" => {
                ctx.start_quest();
                ctx.set_memo_state(1);
                Some(event.to_string())
            }
            "30376-05.html" => Some(event.to_string()),
            "30376-06.html" => {
                if ctx.memo_state() == 2 {
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30376-07.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `checkPartyMember`: only memoState 1 or 3 members collect. Port is
        // killer-only (G11 party deviation).
        if !ctx.has_qs() {
            return;
        }
        let memo = ctx.memo_state();
        if memo != 1 && memo != 3 {
            return;
        }
        let Some((item, chance, count)) = drop_for(ctx.npc_id) else {
            return;
        };
        if ctx.roll(100) < chance {
            let (item_count, cond) = if memo == 1 { (50i64, 2) } else { (200i64, 4) };
            // `giveItemRandomly(..., chance = 1, ...)` — the outer roll already
            // gated the drop; here it just fills up to the per-item cap and
            // signals when the cap is reached. Cond advances once both shard
            // types are full (combined == item_count * 2).
            if ctx.give_item_randomly(item, count, item_count, 1.0, true)
                && shard_total(ctx) >= item_count * 2
            {
                ctx.set_cond(cond, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30376-01.htm"
                } else {
                    "30376-03.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                match ctx.memo_state() {
                    1 => {
                        if shard_total(ctx) >= 100 {
                            ctx.give_adena(3000, true);
                            ctx.take_items(FLARE_SHARD, -1);
                            ctx.take_items(FREEZING_SHARD, -1);
                            ctx.set_memo_state(2);
                            "30376-04.html"
                        } else {
                            "30376-08.html"
                        }
                    }
                    2 => "30376-09.html",
                    3 => {
                        if shard_total(ctx) >= 400 {
                            ctx.give_adena(12000, true);
                            ctx.take_items(FLARE_SHARD, -1);
                            ctx.take_items(FREEZING_SHARD, -1);
                            ctx.exit_quest(true, true);
                            "30376-10.html"
                        } else {
                            "30376-11.html"
                        }
                    }
                    _ => return Some(ctx.no_quest_html()),
                }
                .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }
}
