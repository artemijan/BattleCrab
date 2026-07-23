//! Catch the Wind (317) — port of
//! `dist/game/data/scripts/quests/Q00317_CatchTheWind/`. Rizraell buys wind
//! shards from Lireins (50% drop, no cap); turn-in pays per shard (+2988
//! for 10+) and can keep the quest running (`-09`) or end it (`-08`).

use crate::game_loop::quests::{QuestCtx, QuestScript};

const RIZRAELL: i32 = 30361;
const WIND_SHARD: i32 = 1078;
const MONSTERS: [i32; 2] = [20036, 20044]; // Lirein, Lirein Elder
const MIN_LEVEL: i32 = 18;
const DROP_CHANCE: f64 = 0.5;

pub struct Q00317CatchTheWind;

impl Q00317CatchTheWind {
    fn pay_out(ctx: &mut QuestCtx) {
        let shards = ctx.quest_items_count(WIND_SHARD);
        if shards > 0 {
            ctx.give_adena((shards * 10) + if shards >= 10 { 2988 } else { 0 }, true);
            ctx.take_items(WIND_SHARD, -1);
        }
    }
}

impl QuestScript for Q00317CatchTheWind {
    fn id(&self) -> i32 {
        317
    }
    fn name(&self) -> &'static str {
        "Q00317_CatchTheWind"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00317_CatchTheWind"
    }
    fn start_npcs(&self) -> &[i32] {
        &[RIZRAELL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[RIZRAELL]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[WIND_SHARD]
    }

    /// `addCondMaxLevel(23, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 23).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30361-04.htm" if ctx.is_created() => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30361-08.html" => {
                Self::pay_out(ctx);
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30361-09.html" => {
                Self::pay_out(ctx);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    /// `getRandomPartyMemberState(killer, -1, 3, npc)` — killer-only:
    /// any started state qualifies. Uncapped drop (limit 0).
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_started() {
            ctx.give_item_randomly(WIND_SHARD, 1, 0, DROP_CHANCE, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30361-03.htm"
                } else {
                    "30361-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                if ctx.quest_items_count(WIND_SHARD) > 0 {
                    "30361-07.html"
                } else {
                    "30361-05.html"
                }
                .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }
}
