//! Hunt of the Golden Ram Mercenary Force (628) — `quests/Q00628_HuntGoldenRam`.
//! Kahman (31554, level 66+) ranks the player up the Golden Ram mercenaries:
//! 100 Splinter Stakato Chitin → Recruit badge (cond 2), then 100 more + 100
//! Needle Stakato Chitin → Soldier badge (cond 3). The badges (kept, not
//! registered) unlock the Golden Ram buff shops. Repeatable.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const KAHMAN: i32 = 31554;
const GOLDEN_RAM_BADGE_RECRUIT: i32 = 7246;
const GOLDEN_RAM_BADGE_SOLDIER: i32 = 7247;
const SPLINTER_STAKATO_CHITIN: i32 = 7248;
const NEEDLE_STAKATO_CHITIN: i32 = 7249;
const REQUIRED_ITEM_COUNT: i64 = 100;
const MIN_LEVEL: i32 = 66;

/// `MOBS_DROP_CHANCES`: npc → (item, chance 0..1, count). Here `count` is a
/// **cond selector** (`item.getCount() <= qs.getCond()`), not a quantity:
/// Splinter (1) drops from cond 1, Needle (2) only from cond 2.
fn drop_for(npc_id: i32) -> Option<(i32, f64, i32)> {
    let v = match npc_id {
        21508 => (SPLINTER_STAKATO_CHITIN, 0.500, 1),
        21509 => (SPLINTER_STAKATO_CHITIN, 0.430, 1),
        21510 => (SPLINTER_STAKATO_CHITIN, 0.521, 1),
        21511 => (SPLINTER_STAKATO_CHITIN, 0.575, 1),
        21512 => (SPLINTER_STAKATO_CHITIN, 0.746, 1),
        21513 => (NEEDLE_STAKATO_CHITIN, 0.500, 2),
        21514 => (NEEDLE_STAKATO_CHITIN, 0.430, 2),
        21515 => (NEEDLE_STAKATO_CHITIN, 0.520, 2),
        21516 => (NEEDLE_STAKATO_CHITIN, 0.531, 2),
        21517 => (NEEDLE_STAKATO_CHITIN, 0.744, 2),
        _ => return None,
    };
    Some(v)
}

pub struct Q00628HuntGoldenRam;

impl QuestScript for Q00628HuntGoldenRam {
    fn id(&self) -> i32 {
        628
    }
    fn name(&self) -> &'static str {
        "Q00628_HuntGoldenRam"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00628_HuntGoldenRam"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KAHMAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KAHMAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            21508, 21509, 21510, 21511, 21512, 21513, 21514, 21515, 21516, 21517,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[SPLINTER_STAKATO_CHITIN, NEEDLE_STAKATO_CHITIN]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "31554-01.htm"
                } else {
                    "31554-02.htm"
                }
                .to_string(),
            );
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let splinter = ctx.quest_items_count(SPLINTER_STAKATO_CHITIN);
        let needle = ctx.quest_items_count(NEEDLE_STAKATO_CHITIN);
        // The "still gathering splinters" page, reused by several branches.
        let gathering = |c: &QuestCtx| {
            if c.quest_items_count(SPLINTER_STAKATO_CHITIN) >= REQUIRED_ITEM_COUNT {
                "31554-07.html"
            } else {
                "31554-06.html"
            }
        };
        match ctx.cond() {
            1 => Some(gathering(ctx).to_string()),
            2 => {
                if ctx.quest_items_count(GOLDEN_RAM_BADGE_RECRUIT) > 0 {
                    if splinter >= REQUIRED_ITEM_COUNT && needle >= REQUIRED_ITEM_COUNT {
                        ctx.take_items(GOLDEN_RAM_BADGE_RECRUIT, -1);
                        ctx.take_items(SPLINTER_STAKATO_CHITIN, -1);
                        ctx.take_items(NEEDLE_STAKATO_CHITIN, -1);
                        ctx.give_items(GOLDEN_RAM_BADGE_SOLDIER, 1);
                        ctx.set_cond(3, true);
                        Some("31554-10.html".to_string())
                    } else {
                        Some("31554-09.html".to_string())
                    }
                } else {
                    // Lost the recruit badge — fall back to cond 1.
                    ctx.set_cond(1, false);
                    Some(gathering(ctx).to_string())
                }
            }
            3 => {
                if ctx.quest_items_count(GOLDEN_RAM_BADGE_SOLDIER) > 0 {
                    Some("31554-11.html".to_string())
                } else {
                    ctx.set_cond(1, false);
                    Some(gathering(ctx).to_string())
                }
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "accept" => {
                if ctx.is_created() && ctx.player_level() >= MIN_LEVEL {
                    ctx.start_quest();
                    if ctx.quest_items_count(GOLDEN_RAM_BADGE_SOLDIER) > 0 {
                        ctx.set_cond(3, false);
                        Some("31554-05.htm".to_string())
                    } else if ctx.quest_items_count(GOLDEN_RAM_BADGE_RECRUIT) > 0 {
                        ctx.set_cond(2, false);
                        Some("31554-04.htm".to_string())
                    } else {
                        Some("31554-03.htm".to_string())
                    }
                } else {
                    None
                }
            }
            "31554-08.html" => {
                if ctx.quest_items_count(SPLINTER_STAKATO_CHITIN) >= REQUIRED_ITEM_COUNT {
                    ctx.give_items(GOLDEN_RAM_BADGE_RECRUIT, 1);
                    ctx.take_items(SPLINTER_STAKATO_CHITIN, -1);
                    ctx.set_cond(2, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "31554-12.html" | "31554-13.html" => {
                if ctx.is_started() {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "31554-14.html" => {
                if ctx.is_started() {
                    ctx.exit_quest(true, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, -1, 1, npc)` + `!isCond(3)`. Port is
        // killer-only (G11 party deviation).
        if !ctx.has_qs() || ctx.is_cond(3) {
            return;
        }
        let Some((item, chance, count)) = drop_for(ctx.npc_id) else {
            return;
        };
        if i64::from(count) <= i64::from(ctx.cond()) {
            ctx.give_item_randomly(item, 1, REQUIRED_ITEM_COUNT, chance, true);
        }
    }
}
