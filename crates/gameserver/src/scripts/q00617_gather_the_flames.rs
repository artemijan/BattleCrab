//! Gather the Flames (617) — `quests/Q00617_GatherTheFlames`. Hilda (31271) and
//! Vulcan (31539) send level-74+ players into the Fields of Massacre / Forge of
//! the Gods for Torches (1–2 per kill, per-mob odds). 1000 Torches buy one
//! *random* S-grade weapon recipe from Vulcan; 1200 Torches buy a *chosen* one
//! from Rooney (32049). Repeatable.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const HILDA: i32 = 31271;
const VULCAN: i32 = 31539;
const ROONEY: i32 = 32049;
const TORCH: i32 = 7264;
const REWARD: [i32; 10] = [6881, 6883, 6885, 6887, 6891, 6893, 6895, 6897, 6899, 7580];
const KILL_NPCS: [i32; 16] = [
    22634, 22635, 22636, 22637, 22638, 22639, 22640, 22641, 22642, 22643, 22644, 22645, 22646,
    22647, 22648, 22649,
];

/// `MOBS`: per-mob threshold /1000 for dropping 2 torches instead of 1.
fn torch2_threshold(npc_id: i32) -> Option<i32> {
    let v = match npc_id {
        22634 => 639,
        22635 => 611,
        22636 => 649,
        22637 => 639,
        22638 => 639,
        22639 => 645,
        22640 => 559,
        22641 => 588,
        22642 => 537,
        22643 => 618,
        22644 => 633,
        22645 => 550,
        22646 => 593,
        22647 => 688,
        22648 => 632,
        22649 => 685,
        _ => return None,
    };
    Some(v)
}

pub struct Q00617GatherTheFlames;

impl QuestScript for Q00617GatherTheFlames {
    fn id(&self) -> i32 {
        617
    }
    fn name(&self) -> &'static str {
        "Q00617_GatherTheFlames"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00617_GatherTheFlames"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HILDA, VULCAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ROONEY, HILDA, VULCAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[TORCH]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        match event {
            "31539-03.htm" | "31271-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "32049-02.html" | "31539-04.html" | "31539-06.html" => Some(event.to_string()),
            "31539-07.html" => {
                if ctx.quest_items_count(TORCH) < 1000 || !ctx.is_started() {
                    return Some(ctx.no_quest_html());
                }
                let idx = ctx.roll(REWARD.len() as i32) as usize;
                ctx.give_items(REWARD[idx], 1);
                ctx.take_items(TORCH, 1000);
                Some(event.to_string())
            }
            "31539-08.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            // Rooney's chosen-recipe buttons: 1200 torches for one named recipe.
            "6883" | "6885" | "7580" | "6891" | "6893" | "6895" | "6897" | "6899" => {
                if ctx.quest_items_count(TORCH) < 1200 || !ctx.is_started() {
                    return Some(ctx.no_quest_html());
                }
                ctx.give_items(event.parse().unwrap(), 1);
                ctx.take_items(TORCH, 1200);
                Some("32049-04.html".to_string())
            }
            "6887" | "6881" => {
                if ctx.quest_items_count(TORCH) < 1200 || !ctx.is_started() {
                    return Some(ctx.no_quest_html());
                }
                ctx.give_items(event.parse().unwrap(), 1);
                ctx.take_items(TORCH, 1200);
                Some("32049-03.html".to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMember(player, 1)`: a cond-1 member. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let Some(threshold) = torch2_threshold(ctx.npc_id) else {
            return;
        };
        // Plain `giveItems` (not rate-multiplied).
        let count = if ctx.roll(1000) < threshold { 2 } else { 1 };
        ctx.give_items(TORCH, count);
        ctx.play_sound(quest_sounds::ITEMGET);
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            ROONEY => {
                if ctx.is_started() {
                    return Some(
                        if ctx.quest_items_count(TORCH) >= 1200 {
                            "32049-02.html"
                        } else {
                            "32049-01.html"
                        }
                        .to_string(),
                    );
                }
                Some(ctx.no_quest_html())
            }
            VULCAN => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= 74 { "31539-01.htm" } else { "31539-02.htm" }
                            .to_string(),
                    );
                }
                Some(
                    if ctx.quest_items_count(TORCH) >= 1000 { "31539-04.html" } else { "31539-05.html" }
                        .to_string(),
                )
            }
            HILDA => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= 74 { "31271-01.htm" } else { "31271-02.htm" }
                            .to_string(),
                    );
                }
                Some("31271-04.html".to_string())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }
}
