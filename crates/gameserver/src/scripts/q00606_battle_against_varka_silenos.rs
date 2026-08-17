//! Battle against Varka Silenos (606) — `quests/Q00606_BattleAgainstVarkaSilenos`.
//! Kadun Zu Ketra (31370, level 74+) — a Ketra Orc quartermaster — buys Varka
//! Silenos Manes off the enemy camp: 100 Manes trade for 20 Varka Silenos Horns
//! (the Ketra-alliance token). Repeatable, no cond progression.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const KADUN: i32 = 31370;
const HORN: i32 = 7186;
const MANE: i32 = 7233;
const MIN_LEVEL: i32 = 74;
const MANE_COUNT: i64 = 100;

/// `MOBS`: per-mob Mane drop chance out of 1000.
fn mane_chance(npc_id: i32) -> Option<i32> {
    let v = match npc_id {
        21350 => 500,
        21353 => 510,
        21354 => 522,
        21355 => 519,
        21357 => 529,
        21358 => 529,
        21360 => 539,
        21362 => 539,
        21364 => 558,
        21365 => 568,
        21366 => 568,
        21368 => 568,
        21369 => 664,
        21371 => 713,
        21373 => 738,
        _ => return None,
    };
    Some(v)
}

pub struct Q00606BattleAgainstVarkaSilenos;

impl QuestScript for Q00606BattleAgainstVarkaSilenos {
    fn id(&self) -> i32 {
        606
    }
    fn name(&self) -> &'static str {
        "Q00606_BattleAgainstVarkaSilenos"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00606_BattleAgainstVarkaSilenos"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KADUN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KADUN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            21350, 21353, 21354, 21355, 21357, 21358, 21360, 21362, 21364, 21365, 21366, 21368,
            21369, 21371, 21373,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[MANE]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "31370-01.htm"
                } else {
                    "31370-02.htm"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                if ctx.quest_items_count(MANE) > 0 {
                    "31370-04.html"
                } else {
                    "31370-05.html"
                }
                .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31370-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31370-06.html" => Some(event.to_string()),
            "31370-07.html" => {
                if ctx.quest_items_count(MANE) < MANE_COUNT {
                    Some("31370-08.html".to_string())
                } else {
                    ctx.take_items(MANE, MANE_COUNT);
                    ctx.give_items(HORN, 20);
                    Some(event.to_string())
                }
            }
            "31370-09.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMember(killer, 1)` — a cond-1 member. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        if let Some(chance) = mane_chance(ctx.npc_id)
            && ctx.roll(1000) < chance
        {
            ctx.give_items(MANE, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
