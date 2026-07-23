//! Crystals of Fire and Ice (306) — `quests/Q00306_CrystalOfFireAndIce`.
//! Katerina (30004, level 17–23) buys Flame and Ice Shards off the salamanders
//! and undines of the Neutral Zone (15 adena each, +5000 for 10+ turned in at
//! once). `addCondMaxLevel(23)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const KATERINA: i32 = 30004;
const FLAME_SHARD: i32 = 1020;
const ICE_SHARD: i32 = 1021;
const MIN_LEVEL: i32 = 17;
const UNDINE_NOBLE: i32 = 20115;

/// `MONSTER_DROPS`: npc → (item, holder count). The drop chance is
/// `1000.0 / count` — which for every count here (900–950) is **> 1.0**, so the
/// shard drops on every kill (a datapack oddity kept verbatim).
fn drop_for(npc_id: i32) -> Option<(i32, f64)> {
    let (item, count) = match npc_id {
        20109 => (FLAME_SHARD, 925.0), // Salamander
        20110 => (ICE_SHARD, 900.0),   // Undine
        20112 => (FLAME_SHARD, 900.0), // Salamander Elder
        20113 => (ICE_SHARD, 925.0),   // Undine Elder
        20114 => (FLAME_SHARD, 925.0), // Salamander Noble
        UNDINE_NOBLE => (ICE_SHARD, 950.0),
        _ => return None,
    };
    Some((item, 1000.0 / count))
}

pub struct Q00306CrystalOfFireAndIce;

impl QuestScript for Q00306CrystalOfFireAndIce {
    fn id(&self) -> i32 {
        306
    }
    fn name(&self) -> &'static str {
        "Q00306_CrystalOfFireAndIce"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00306_CrystalOfFireAndIce"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KATERINA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KATERINA]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20109, 20110, 20112, 20113, 20114, UNDINE_NOBLE]
    }
    fn quest_items(&self) -> &[i32] {
        &[FLAME_SHARD, ICE_SHARD]
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
            "30004-04.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30004-08.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30004-09.html" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Undine Noble credits only the killer; the others use
        // `getRandomPartyMemberState`. Both reduce to a started killer (G11
        // party deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if let Some((item, chance)) = drop_for(ctx.npc_id) {
            ctx.give_item_randomly(item, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "30004-03.htm"
                } else {
                    "30004-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            let flame = ctx.quest_items_count(FLAME_SHARD);
            let ice = ctx.quest_items_count(ICE_SHARD);
            if flame > 0 || ice > 0 {
                ctx.give_adena(
                    flame * 15 + ice * 15 + if flame + ice >= 10 { 5000 } else { 0 },
                    true,
                );
                ctx.take_items(FLAME_SHARD, -1);
                ctx.take_items(ICE_SHARD, -1);
                return Some("30004-07.html".to_string());
            }
            return Some("30004-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
